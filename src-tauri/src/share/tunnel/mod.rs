// ABOUTME: TunnelManager state machine: owns the spawned tunnel binary, the public URL,
// ABOUTME: and the share-latest entry point. Provider adapters live in sibling files.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

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
    monitor: Option<JoinHandle<()>>,
}

impl TunnelManager {
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(TunnelState::Idle);
        TunnelManager {
            inner: Mutex::new(Inner { state: TunnelState::Idle, monitor: None }),
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
        self: Arc<Self>,
        provider: Arc<dyn TunnelProvider>,
        local_port: u16,
    ) -> Result<String, ShareError> {
        self.start_tunnel_with_deadline(provider, local_port, DEFAULT_URL_DEADLINE).await
    }

    pub async fn start_tunnel_with_deadline(
        self: Arc<Self>,
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
        let (mut child, url_future) = match spawn_result {
            Ok(pair) => pair,
            Err(e) => {
                let reason = e.to_string();
                self.set_state(TunnelState::Error { reason: reason.clone() }).await;
                return Err(ShareError::TunnelStartFailed(reason));
            }
        };

        let url = match tokio::time::timeout(url_deadline, url_future).await {
            Ok(Ok(u)) => u,
            Ok(Err(reason)) => {
                drop(child); // kill_on_drop → SIGKILL
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
                drop(child);
                self.set_state(TunnelState::Error { reason: reason.clone() }).await;
                return Err(ShareError::TunnelStartFailed(reason));
            }
        };

        self.set_state(TunnelState::Live { url: url.clone() }).await;

        if provider.child_persists_with_tunnel() {
            // Spawn monitor that owns the child for the lifetime of the Live state.
            // If the child exits unexpectedly while we are still Live, transition to Error.
            let self_clone = Arc::clone(&self);
            let monitor = tokio::spawn(async move {
                let _ = child.wait().await;
                let inner = self_clone.inner.lock().await;
                if matches!(inner.state, TunnelState::Live { .. }) {
                    drop(inner);
                    self_clone.set_state(TunnelState::Error {
                        reason: "tunnel disconnected".to_string(),
                    }).await;
                }
            });

            {
                let mut inner = self.inner.lock().await;
                if let Some(prev) = inner.monitor.replace(monitor) {
                    prev.abort();
                }
            }
        } else {
            // Child is a marker that has already exited or is about to. No
            // disconnect-detection path for this provider — the tunnel persists
            // in its own daemon (e.g. tailscaled). Implication: Tailscale Funnel
            // shares stay live across app quits because the manager has no
            // handle to revoke them. Accepted v1.0 trade-off, documented in the
            // seed §2.4.
            drop(child);
        }

        Ok(url)
    }

    pub async fn shutdown(&self) {
        // Set state to Idle under the SAME lock as the monitor take, so any
        // monitor task waiting on the lock sees Idle (not Live) and skips the
        // spurious "tunnel disconnected" transition.
        let monitor_opt = {
            let mut inner = self.inner.lock().await;
            let monitor = inner.monitor.take();
            inner.state = TunnelState::Idle;
            let _ = self.tx.send_replace(TunnelState::Idle);
            monitor
        };
        if let Some(monitor) = monitor_opt {
            monitor.abort();
            let _ = monitor.await;
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
pub mod log;

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
        let mgr = Arc::new(TunnelManager::new());
        assert!(matches!(mgr.current_state(), TunnelState::Idle));
    }

    #[tokio::test]
    async fn share_latest_transitions_to_live() {
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        let url = mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        assert_eq!(url, "https://abc.trycloudflare.com");
        assert!(matches!(mgr.current_state(), TunnelState::Live { .. }));
    }

    #[tokio::test]
    async fn second_start_while_live_returns_same_url() {
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        let url1 = mgr.clone().start_tunnel(provider.clone(), 7821).await.unwrap();
        let url2 = mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        assert_eq!(url1, url2);
    }

    #[tokio::test]
    async fn error_outcome_transitions_to_error() {
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_error("authtoken missing"));
        let result = mgr.clone().start_tunnel(provider, 7821).await;
        assert!(result.is_err());
        assert!(matches!(mgr.current_state(), TunnelState::Error { .. }));
    }

    #[tokio::test]
    async fn shutdown_returns_to_idle() {
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_url("https://abc.trycloudflare.com"));
        mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        mgr.shutdown().await;
        assert!(matches!(mgr.current_state(), TunnelState::Idle));
    }

    #[tokio::test]
    async fn subscribe_receives_transitions() {
        let mgr = Arc::new(TunnelManager::new());
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

        mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        let final_state = handle.await.unwrap();
        assert!(matches!(final_state, TunnelState::Live { .. }));
    }

    #[tokio::test]
    async fn url_future_timeout_yields_error_state() {
        let mgr = Arc::new(TunnelManager::new());
        let outcome = Arc::new(std::sync::Mutex::new(MockOutcome::DelayedUrl {
            url: "https://late.trycloudflare.com".into(),
            delay_ms: 5_000,
        }));
        let provider = Arc::new(MockTunnelProvider {
            kind: crate::share::tunnel::provider::ProviderKind::Cloudflared,
            outcome,
            child_persists: true,
        });
        let result = mgr.clone().start_tunnel_with_deadline(provider, 7821, std::time::Duration::from_millis(50)).await;
        assert!(result.is_err());
        assert!(matches!(mgr.current_state(), TunnelState::Error { .. }));
    }

    #[tokio::test]
    async fn child_exit_transitions_to_error() {
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_short_lived_url("https://x.trycloudflare.com", 100));
        mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        assert!(matches!(mgr.current_state(), TunnelState::Live { .. }));

        // Wait for the mock child (sleep 0.1s) to exit and the monitor to detect it.
        // Poll up to 2s.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while std::time::Instant::now() < deadline {
            if matches!(mgr.current_state(), TunnelState::Error { .. }) { break; }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let final_state = mgr.current_state();
        if !matches!(final_state, TunnelState::Error { .. }) {
            panic!("expected Error state after child exit, got {:?}", final_state);
        }
    }

    #[tokio::test]
    async fn marker_child_exit_stays_live() {
        // Tailscale-style: child is a marker that exits immediately, but the
        // tunnel itself persists in a separate daemon. Manager must NOT
        // transition to Error on child exit.
        let mgr = Arc::new(TunnelManager::new());
        let provider = Arc::new(MockTunnelProvider::with_url_marker_child(
            "https://ts.example.ts.net",
            100,
        ));
        mgr.clone().start_tunnel(provider, 7821).await.unwrap();
        assert!(matches!(mgr.current_state(), TunnelState::Live { .. }));

        // Wait beyond when the mock child would have exited.
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // State must STILL be Live — marker-child exit must not flip to Error.
        let final_state = mgr.current_state();
        assert!(
            matches!(final_state, TunnelState::Live { .. }),
            "marker child exit should not transition to Error, got {:?}",
            final_state,
        );
    }
}
