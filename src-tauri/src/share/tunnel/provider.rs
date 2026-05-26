// ABOUTME: Per-provider tunnel-binary adapter trait + provider enum + error types.
// ABOUTME: select_provider(ProviderKind) returns a Box<dyn TunnelProvider> ready to spawn.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Cloudflared,
    Ngrok,
    Tailscale,
}

impl Default for ProviderKind {
    fn default() -> Self { ProviderKind::Cloudflared }
}

impl ProviderKind {
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "cloudflared" => Some(ProviderKind::Cloudflared),
            "ngrok"       => Some(ProviderKind::Ngrok),
            "tailscale"   => Some(ProviderKind::Tailscale),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            ProviderKind::Cloudflared => "cloudflared",
            ProviderKind::Ngrok       => "ngrok",
            ProviderKind::Tailscale   => "tailscale",
        }
    }
}

use async_trait::async_trait;
use futures::future::BoxFuture;
use tokio::process::Child;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("{provider} binary not installed (looked for `{binary}` on PATH)")]
    BinaryNotFound { provider: &'static str, binary: &'static str },
    #[error("{provider}: {reason}")]
    Spawn { provider: &'static str, reason: String },
}

/// One-shot URL discovery result. The future is awaited under a deadline by
/// the TunnelManager; per-provider stderr/stdout/control-API watchers feed it.
pub type UrlFuture = BoxFuture<'static, Result<String, String>>;

#[async_trait]
pub trait TunnelProvider: Send + Sync {
    /// Spawn the tunnel binary pointing at local_port. Returns the running
    /// child process plus a future that resolves to the public URL (or an
    /// error reason). The caller is responsible for awaiting the future
    /// under a deadline and for killing the child on shutdown.
    async fn spawn(&self, local_port: u16) -> Result<(Child, UrlFuture), ProviderError>;
    fn kind(&self) -> ProviderKind;
}
