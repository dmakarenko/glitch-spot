// ABOUTME: TunnelManager state machine: owns the spawned tunnel binary, the public URL,
// ABOUTME: and the share-latest entry point. Provider adapters live in sibling files.

use std::sync::Arc;
use std::time::Duration;

use tokio::process::Child;
use tokio::sync::{watch, Mutex};

use crate::share::tunnel::provider::TunnelProvider;

#[derive(Debug, Clone, PartialEq)]
pub enum TunnelState {
    Idle,
    Starting,
    Live { url: String },
    Error { reason: String },
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("tunnel start failed: {0}")]
    TunnelStartFailed(String),
    #[error("no snapshot to share")]
    NoSnapshot,
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
}

const DEFAULT_URL_DEADLINE: Duration = Duration::from_secs(10);

pub struct TunnelManager {
    inner: Mutex<Inner>,
    tx: watch::Sender<TunnelState>,
}

struct Inner {
    state: TunnelState,
    child: Option<Child>,
}

impl TunnelManager {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(TunnelState::Idle);
        TunnelManager {
            inner: Mutex::new(Inner { state: TunnelState::Idle, child: None }),
            tx,
        }
    }

    pub fn current_state(&self) -> TunnelState {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<TunnelState> {
        self.tx.subscribe()
    }

    /// Start the tunnel (if Idle/Error) and return its public URL.
    /// Re-uses the existing URL when already Live.
    pub async fn start_tunnel(
        &self,
        provider: Arc<dyn TunnelProvider>,
        local_port: u16,
    ) -> Result<String, ShareError> {
        self.start_tunnel_with_deadline(provider, local_port, DEFAULT_URL_DEADLINE).await
    }

    pub async fn start_tunnel_with_deadline(
        &self,
        provider: Arc<dyn TunnelProvider>,
        local_port: u16,
        url_deadline: Duration,
    ) -> Result<String, ShareError> {
        // Fast path — already Live.
        {
            let inner = self.inner.lock().await;
            if let TunnelState::Live { url } = &inner.state {
                return Ok(url.clone());
            }
        }

        self.set_state(TunnelState::Starting).await;

        let spawn_result = provider.spawn(local_port).await;
        let (child, url_future) = match spawn_result {
            Ok(pair) => pair,
            Err(e) => {
                let reason = e.to_string();
                self.set_state(TunnelState::Error { reason: reason.clone() }).await;
                return Err(ShareError::TunnelStartFailed(reason));
            }
        };

        // Stash the child immediately so a concurrent shutdown can kill it.
        {
            let mut inner = self.inner.lock().await;
            inner.child = Some(child);
        }

        let url = match tokio::time::timeout(url_deadline, url_future).await {
            Ok(Ok(u)) => u,
            Ok(Err(reason)) => {
                self.kill_child().await;
                self.set_state(TunnelState::Error { reason: reason.clone() }).await;
                return Err(ShareError::TunnelStartFailed(reason));
            }
            Err(_elapsed) => {
                let pretty_deadline = if url_deadline < Duration::from_secs(1) {
                    format!("{} ms", url_deadline.as_millis())
                } else {
                    format!("{} s", url_deadline.as_secs())
                };
                let reason = format!(
                    "{} didn't publish a URL within {}",
                    provider.kind().as_str(),
                    pretty_deadline,
                );
                self.kill_child().await;
                self.set_state(TunnelState::Error { reason: reason.clone() }).await;
                return Err(ShareError::TunnelStartFailed(reason));
            }
        };

        self.set_state(TunnelState::Live { url: url.clone() }).await;
        Ok(url)
    }

    pub async fn shutdown(&self) {
        self.kill_child().await;
        self.set_state(TunnelState::Idle).await;
    }

    async fn kill_child(&self) {
        let child_opt = {
            let mut inner = self.inner.lock().await;
            inner.child.take()
        };
        if let Some(mut child) = child_opt {
            // SIGKILL via tokio's start_kill (no graceful SIGTERM path on Unix from
            // tokio::process::Child). Acceptable for our tunnel binaries — the
            // trycloudflare/ngrok sessions expire server-side within seconds anyway.
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        }
    }

    async fn set_state(&self, next: TunnelState) {
        let mut inner = self.inner.lock().await;
        inner.state = next.clone();
        // send_replace always updates the stored value, even with no receivers.
        self.tx.send_replace(next);
    }
}

impl Default for TunnelManager {
    fn default() -> Self { Self::new() }
}

pub mod provider;
pub mod cloudflared;
pub mod ngrok;
pub mod tailscale;

// `mock` is used by both the inline unit tests in this module AND by the
// integration test in `tests/share_e2e.rs`. Integration tests cannot see
// `#[cfg(test)]` items, so it has to be compiled unconditionally.
pub mod mock;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::share::tunnel::mock::{MockOutcome, MockTunnelProvider};
    use std::sync::Arc;

    #[tokio::test]
    async fn starts_idle() {
        let mgr = TunnelManager::new();
        assert!(matches!(mgr.current_state(), TunnelState::Idle));
    }

    #[tokio::test]
    async fn share_latest_transitions_to_live() {
        let mgr = TunnelManager::new();
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        let url = mgr.start_tunnel(provider, 7821).await.unwrap();
        assert_eq!(url, "https://abc.trycloudflare.com");
        assert!(matches!(mgr.current_state(), TunnelState::Live { .. }));
    }

    #[tokio::test]
    async fn second_start_while_live_returns_same_url() {
        let mgr = TunnelManager::new();
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        let url1 = mgr.start_tunnel(provider.clone(), 7821).await.unwrap();
        let url2 = mgr.start_tunnel(provider, 7821).await.unwrap();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn error_outcome_transitions_to_error() {
        let mgr = TunnelManager::new();
        let provider = Arc::new(MockTunnelProvider::with_error("authtoken missing"));
        let result = mgr.start_tunnel(provider, 7821).await;
        assert!(result.is_err());
        assert!(matches!(mgr.current_state(), TunnelState::Error { .. }));
    }

    #[tokio::test]
    async fn shutdown_returns_to_idle() {
        let mgr = TunnelManager::new();
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        mgr.start_tunnel(provider, 7821).await.unwrap();
        mgr.shutdown().await;
        assert!(matches!(mgr.current_state(), TunnelState::Idle));
    }

    #[tokio::test]
    async fn subscribe_receives_transitions() {
        let mgr = TunnelManager::new();
        let mut rx = mgr.subscribe();
        let provider = Arc::new(MockTunnelProvider::with_url("https://x.trycloudflare.com"));

        let handle = tokio::spawn(async move {
            // Drain transitions until we hit Live, then return.
            loop {
                rx.changed().await.unwrap();
                let state = rx.borrow().clone();
                if matches!(state, TunnelState::Live { .. }) {
                    return state;
                }
            }
        });

        mgr.start_tunnel(provider, 7821).await.unwrap();
        let final_state = handle.await.unwrap();
        assert!(matches!(final_state, TunnelState::Live { .. }));
    }

    #[tokio::test]
    async fn url_future_timeout_yields_error_state() {
        let mgr = TunnelManager::new();
        let outcome = Arc::new(std::sync::Mutex::new(MockOutcome::DelayedUrl {
            url: "https://late.trycloudflare.com".into(),
            delay_ms: 5_000,
        }));
        let provider = Arc::new(MockTunnelProvider {
            kind: crate::share::tunnel::provider::ProviderKind::Cloudflared,
            outcome,
        });
        let result = mgr.start_tunnel_with_deadline(provider, 7821, std::time::Duration::from_millis(50)).await;
        assert!(result.is_err());
        assert!(matches!(mgr.current_state(), TunnelState::Error { .. }));
    }
}
