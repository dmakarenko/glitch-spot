// ABOUTME: TunnelProvider stub that returns a sleep-based child process
// ABOUTME: and a configurable URL future for driving TunnelManager state tests.

use async_trait::async_trait;
use futures::future::FutureExt;
use std::sync::{Arc, Mutex};
use tokio::process::{Child, Command};

use crate::share::tunnel::provider::{ProviderError, ProviderKind, TunnelProvider, UrlFuture};

pub struct MockTunnelProvider {
    pub kind: ProviderKind,
    pub outcome: Arc<Mutex<MockOutcome>>,
    /// Mirrors `TunnelProvider::child_persists_with_tunnel`. True for the
    /// cloudflared/ngrok-style mocks (default); false for the Tailscale-style
    /// marker-child mock built via `with_url_marker_child`.
    pub child_persists: bool,
}

pub enum MockOutcome {
    ImmediateUrl(String),
    ImmediateError(String),
    DelayedUrl { url: String, delay_ms: u64 },
    /// Spawns a child that exits after `child_lives_ms` milliseconds.
    /// The URL future resolves immediately so TunnelManager reaches Live;
    /// then the child's exit triggers the disconnect-detection path.
    ShortLivedUrl { url: String, child_lives_ms: u64 },
}

impl MockTunnelProvider {
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::Cloudflared,
            outcome: Arc::new(Mutex::new(MockOutcome::ImmediateUrl(url.into()))),
            child_persists: true,
        }
    }

    pub fn with_error(reason: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::Cloudflared,
            outcome: Arc::new(Mutex::new(MockOutcome::ImmediateError(reason.into()))),
            child_persists: true,
        }
    }

    pub fn with_short_lived_url(url: impl Into<String>, child_lives_ms: u64) -> Self {
        Self {
            kind: ProviderKind::Cloudflared,
            outcome: Arc::new(Mutex::new(MockOutcome::ShortLivedUrl {
                url: url.into(),
                child_lives_ms,
            })),
            child_persists: true,
        }
    }

    /// Marker-child mock: child exits after `child_lives_ms`, but the provider
    /// reports `child_persists_with_tunnel = false`, so TunnelManager must NOT
    /// transition to Error on child exit. Models the Tailscale Funnel adapter.
    pub fn with_url_marker_child(url: impl Into<String>, child_lives_ms: u64) -> Self {
        Self {
            kind: ProviderKind::Tailscale,
            outcome: Arc::new(Mutex::new(MockOutcome::ShortLivedUrl {
                url: url.into(),
                child_lives_ms,
            })),
            child_persists: false,
        }
    }
}

#[async_trait]
impl TunnelProvider for MockTunnelProvider {
    async fn spawn(&self, _local_port: u16) -> Result<(Child, UrlFuture), ProviderError> {
        let outcome = self.outcome.lock().unwrap();

        // Spawn a real but trivial child so kill() and wait() behave realistically.
        let child = match &*outcome {
            MockOutcome::ShortLivedUrl { child_lives_ms, .. } => {
                let secs = format!("{:.3}", *child_lives_ms as f64 / 1000.0);
                Command::new("sleep")
                    .arg(secs)
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| ProviderError::Spawn { provider: "mock", reason: e.to_string() })?
            }
            _ => {
                Command::new("sleep")
                    .arg("3600")
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| ProviderError::Spawn { provider: "mock", reason: e.to_string() })?
            }
        };

        let url_future: UrlFuture = match &*outcome {
            MockOutcome::ImmediateUrl(u) | MockOutcome::ShortLivedUrl { url: u, .. } => {
                let u = u.clone();
                async move { Ok(u) }.boxed()
            }
            MockOutcome::ImmediateError(e) => {
                let e = e.clone();
                async move { Err(e) }.boxed()
            }
            MockOutcome::DelayedUrl { url, delay_ms } => {
                let url = url.clone();
                let delay = *delay_ms;
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
                    Ok(url)
                }
                .boxed()
            }
        };
        Ok((child, url_future))
    }

    fn kind(&self) -> ProviderKind {
        self.kind
    }

    fn child_persists_with_tunnel(&self) -> bool {
        self.child_persists
    }
}
