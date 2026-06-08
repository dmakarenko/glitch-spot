// ABOUTME: ngrok provider adapter — uses the ngrok agent API to open a tunnel on port 7821.
// ABOUTME: Requires NGROK_AUTHTOKEN in the environment; queries the local agent API for the public URL.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::FutureExt;
use serde::Deserialize;
use tokio::process::{Child, Command};

use crate::share::tunnel::provider::{ProviderError, ProviderKind, TunnelProvider, UrlFuture};

pub struct Ngrok;

#[derive(Debug, Deserialize)]
struct TunnelsResp {
    tunnels: Vec<TunnelEntry>,
}

#[derive(Debug, Deserialize)]
struct TunnelEntry {
    #[serde(default)]
    proto: String,
    #[serde(default)]
    public_url: String,
}

pub fn parse_public_url(body: &str) -> Option<String> {
    let parsed: TunnelsResp = serde_json::from_str(body).ok()?;
    parsed
        .tunnels
        .into_iter()
        .find(|t| t.proto == "https" && !t.public_url.is_empty())
        .map(|t| t.public_url)
}

async fn poll_for_url(deadline: Duration) -> Result<String, String> {
    let start = std::time::Instant::now();
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .map_err(|e| format!("reqwest client init failed: {e}"))?;
    while start.elapsed() < deadline {
        match client.get("http://127.0.0.1:4040/api/tunnels").send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(body) = resp.text().await {
                    if let Some(url) = parse_public_url(&body) {
                        return Ok(url);
                    }
                }
            }
            _ => {}
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    Err("ngrok control API did not expose a public URL in time".to_string())
}

#[async_trait]
impl TunnelProvider for Ngrok {
    async fn spawn(&self, local_port: u16) -> Result<(Child, UrlFuture), ProviderError> {
        let bin = crate::share::tunnel::resolve::resolve_executable_async("ngrok")
            .await
            .ok_or(ProviderError::BinaryNotFound {
                provider: "ngrok",
                binary: "ngrok",
            })?;
        let mut child = Command::new(bin)
            .arg("http")
            .arg(local_port.to_string())
            .arg("--log=stdout")
            .arg("--log-format=json")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ProviderError::BinaryNotFound {
                        provider: "ngrok",
                        binary: "ngrok",
                    }
                } else {
                    ProviderError::Spawn {
                        provider: "ngrok",
                        reason: e.to_string(),
                    }
                }
            })?;

        // Tee stdout (ngrok logs via --log=stdout) and stderr to the tunnel log file.
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    crate::share::tunnel::log::append("ngrok", &line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    crate::share::tunnel::log::append("ngrok", &line);
                }
            });
        }

        let url_future: UrlFuture = async move {
            // Deadline here is intentionally generous (8s); TunnelManager wraps
            // the whole thing in its own DEFAULT_URL_DEADLINE (10s) anyway.
            poll_for_url(Duration::from_secs(8)).await
        }
        .boxed();
        Ok((child, url_future))
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Ngrok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_public_url_from_api_response() {
        let body = r#"{
            "tunnels": [{
                "name": "command_line",
                "public_url": "https://abcd-1234.ngrok-free.app",
                "proto": "https",
                "config": { "addr": "http://localhost:7821" }
            }]
        }"#;
        assert_eq!(
            parse_public_url(body).unwrap(),
            "https://abcd-1234.ngrok-free.app"
        );
    }

    #[test]
    fn returns_none_when_no_https_tunnel() {
        let body = r#"{ "tunnels": [{ "proto": "http", "public_url": "http://abc.ngrok.io" }] }"#;
        assert!(
            parse_public_url(body).is_none(),
            "must require https tunnel"
        );
    }

    #[test]
    fn returns_none_when_empty_tunnels_array() {
        let body = r#"{ "tunnels": [] }"#;
        assert!(parse_public_url(body).is_none());
    }
}
