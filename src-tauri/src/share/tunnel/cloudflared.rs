// ABOUTME: Cloudflare Tunnel (cloudflared) provider adapter.
// ABOUTME: Spawns `cloudflared tunnel --url http://127.0.0.1:7821` and parses the public URL from stdout.

use std::process::Stdio;

use async_trait::async_trait;
use futures::future::FutureExt;
use regex::Regex;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

use crate::share::tunnel::provider::{ProviderError, ProviderKind, TunnelProvider, UrlFuture};

pub struct Cloudflared;

pub fn extract_trycloudflare_url(text: &str) -> Option<String> {
    let re = Regex::new(r"https://[a-z0-9-]+\.trycloudflare\.com").unwrap();
    re.find(text).map(|m| m.as_str().to_string())
}

#[async_trait]
impl TunnelProvider for Cloudflared {
    async fn spawn(&self, local_port: u16) -> Result<(Child, UrlFuture), ProviderError> {
        let url = format!("http://127.0.0.1:{local_port}");
        let bin = crate::share::tunnel::resolve::resolve_executable_async("cloudflared")
            .await
            .ok_or(ProviderError::BinaryNotFound {
                provider: "cloudflared",
                binary: "cloudflared",
            })?;
        let mut child = Command::new(bin)
            .arg("tunnel")
            .arg("--url")
            .arg(&url)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    ProviderError::BinaryNotFound {
                        provider: "cloudflared",
                        binary: "cloudflared",
                    }
                } else {
                    ProviderError::Spawn {
                        provider: "cloudflared",
                        reason: e.to_string(),
                    }
                }
            })?;

        let stderr = child.stderr.take().expect("stderr piped");
        let (tx, rx) = oneshot::channel::<Result<String, String>>();

        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            let mut tx_opt = Some(tx);
            while let Ok(Some(line)) = reader.next_line().await {
                crate::share::tunnel::log::append("cloudflared", &line);
                if let Some(url) = extract_trycloudflare_url(&line) {
                    if let Some(tx) = tx_opt.take() {
                        let _ = tx.send(Ok(url));
                    }
                }
            }
            if let Some(tx) = tx_opt.take() {
                let _ = tx.send(Err("cloudflared exited before publishing a URL".to_string()));
            }
        });

        let url_future: UrlFuture = async move {
            match rx.await {
                Ok(res) => res,
                Err(_) => Err("cloudflared URL channel closed unexpectedly".to_string()),
            }
        }
        .boxed();

        Ok((child, url_future))
    }

    fn kind(&self) -> ProviderKind {
        ProviderKind::Cloudflared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_url_from_typical_stderr_line() {
        let line = "2026-05-26T12:34:56Z INF +-------------------------------------------------------------+\n2026-05-26T12:34:56Z INF |  Your quick Tunnel has been created! Visit it at (it may take a few seconds to be reachable): |\n2026-05-26T12:34:56Z INF |  https://random-words-foo-bar-123.trycloudflare.com  |";
        let url = extract_trycloudflare_url(line).unwrap();
        assert_eq!(url, "https://random-words-foo-bar-123.trycloudflare.com");
    }

    #[test]
    fn extract_url_returns_none_when_absent() {
        assert!(extract_trycloudflare_url("INF Starting Cloudflare Tunnel...").is_none());
    }

    #[test]
    fn extract_url_ignores_non_trycloudflare_https() {
        assert!(extract_trycloudflare_url("see https://cloudflare.com/docs").is_none());
    }
}
