// ABOUTME: Tailscale Funnel provider adapter — uses `tailscale funnel` to expose port 7821.
// ABOUTME: Parses the HTTPS URL from `tailscale funnel status --json` after the funnel starts.

use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use futures::future::FutureExt;
use serde::Deserialize;
use tokio::process::{Child, Command};

use crate::share::tunnel::provider::{ProviderError, ProviderKind, TunnelProvider, UrlFuture};

pub struct Tailscale;

#[derive(Debug, Deserialize)]
struct StatusRoot {
    #[serde(rename = "Self")]
    self_node: Option<SelfNode>,
}

#[derive(Debug, Deserialize)]
struct SelfNode {
    #[serde(rename = "DNSName")]
    dns_name: String,
}

pub fn parse_funnel_url(status_json: &str) -> Option<String> {
    let root: StatusRoot = serde_json::from_str(status_json).ok()?;
    let s = root.self_node?;
    let trimmed = s.dns_name.trim_end_matches('.');
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("https://{trimmed}"))
    }
}

async fn fetch_tailscale_status() -> Result<String, String> {
    let bin = crate::share::tunnel::resolve::resolve_executable_async("tailscale")
        .await
        .ok_or_else(|| "tailscale binary not found".to_string())?;
    let out = Command::new(bin)
        .arg("status")
        .arg("--json")
        .output()
        .await
        .map_err(|e| format!("tailscale status failed to run: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[async_trait]
impl TunnelProvider for Tailscale {
    async fn spawn(&self, local_port: u16) -> Result<(Child, UrlFuture), ProviderError> {
        // `tailscale funnel --bg` returns immediately after enabling the funnel.
        // We treat this child as a no-op marker; the actual public surface is
        // managed by the tailscaled daemon already running on the system.
        let bin = crate::share::tunnel::resolve::resolve_executable_async("tailscale")
            .await
            .ok_or(ProviderError::BinaryNotFound {
                provider: "tailscale",
                binary: "tailscale",
            })?;
        let mut child = Command::new(bin)
            .arg("funnel")
            .arg("--bg")
            .arg(local_port.to_string())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ProviderError::BinaryNotFound {
                        provider: "tailscale",
                        binary: "tailscale",
                    }
                } else {
                    ProviderError::Spawn {
                        provider: "tailscale",
                        reason: e.to_string(),
                    }
                }
            })?;

        // Tee stdout and stderr to the tunnel log file for post-mortem debugging.
        if let Some(stdout) = child.stdout.take() {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stdout).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    crate::share::tunnel::log::append("tailscale", &line);
                }
            });
        }
        if let Some(stderr) = child.stderr.take() {
            tokio::spawn(async move {
                use tokio::io::{AsyncBufReadExt, BufReader};
                let mut reader = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = reader.next_line().await {
                    crate::share::tunnel::log::append("tailscale", &line);
                }
            });
        }

        let url_future: UrlFuture = async move {
            // Allow tailscaled a couple of seconds to register the funnel.
            tokio::time::sleep(Duration::from_millis(500)).await;
            let status = fetch_tailscale_status().await?;
            parse_funnel_url(&status).ok_or_else(|| "tailscale status: no Self.DNSName".to_string())
        }
        .boxed();

        Ok((child, url_future))
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Tailscale
    }

    /// `tailscale funnel --bg` is a control-plane invocation that exits
    /// immediately after handing off to the tailscaled daemon. The child
    /// exit is not a disconnect signal — the tunnel itself persists in the
    /// daemon.
    fn child_persists_with_tunnel(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_STATUS: &str = r#"{
        "Self": {
            "DNSName": "macbook.example.ts.net.",
            "TailscaleIPs": ["100.100.100.100"]
        }
    }"#;

    #[test]
    fn parses_https_url_from_status_dnsname() {
        let url = parse_funnel_url(SAMPLE_STATUS).unwrap();
        assert_eq!(url, "https://macbook.example.ts.net");
    }

    #[test]
    fn returns_none_when_self_missing() {
        assert!(parse_funnel_url(r#"{ "Peer": {} }"#).is_none());
    }
}
