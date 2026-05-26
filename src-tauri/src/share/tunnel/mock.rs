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
}

pub enum MockOutcome {
    ImmediateUrl(String),
    ImmediateError(String),
    DelayedUrl { url: String, delay_ms: u64 },
}

impl MockTunnelProvider {
    pub fn with_url(url: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::Cloudflared,
            outcome: Arc::new(Mutex::new(MockOutcome::ImmediateUrl(url.into()))),
        }
    }

    pub fn with_error(reason: impl Into<String>) -> Self {
        Self {
            kind: ProviderKind::Cloudflared,
            outcome: Arc::new(Mutex::new(MockOutcome::ImmediateError(reason.into()))),
        }
    }
}

#[async_trait]
impl TunnelProvider for MockTunnelProvider {
    async fn spawn(&self, _local_port: u16) -> Result<(Child, UrlFuture), ProviderError> {
        // Spawn a real but trivial child so kill() and wait() behave realistically.
        let child = Command::new("sleep")
            .arg("3600")
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| ProviderError::Spawn {
                provider: "mock",
                reason: e.to_string(),
            })?;

        let outcome = self.outcome.lock().unwrap();
        let url_future: UrlFuture = match &*outcome {
            MockOutcome::ImmediateUrl(u) => {
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
}
