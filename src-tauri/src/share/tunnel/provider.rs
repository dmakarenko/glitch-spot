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

    /// Human-facing name for menus. Distinct from `as_str()`, which is the
    /// kebab/lowercase key used on disk and in tray menu ids.
    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderKind::Cloudflared => "Cloudflare",
            ProviderKind::Ngrok       => "ngrok",
            ProviderKind::Tailscale   => "Tailscale",
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

    /// Whether early exit of the spawned child indicates a real tunnel disconnect.
    /// True for providers where the child IS the tunnel process (cloudflared, ngrok).
    /// False for providers where the child is a control-plane invocation that hands
    /// off to a separately-running daemon (tailscale funnel --bg).
    fn child_persists_with_tunnel(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_names_are_human_facing() {
        assert_eq!(ProviderKind::Cloudflared.display_name(), "Cloudflare");
        assert_eq!(ProviderKind::Ngrok.display_name(), "ngrok");
        assert_eq!(ProviderKind::Tailscale.display_name(), "Tailscale");
    }

    #[test]
    fn as_str_keys_stay_stable_for_persistence_and_ids() {
        // as_str() is the on-disk config value and the tray menu id suffix.
        // Changing it would break round-tripping and provider selection, so
        // it must stay distinct from the human-facing display_name().
        for kind in [ProviderKind::Cloudflared, ProviderKind::Ngrok, ProviderKind::Tailscale] {
            assert_eq!(ProviderKind::from_name(kind.as_str()), Some(kind));
        }
        assert_eq!(ProviderKind::Cloudflared.as_str(), "cloudflared");
        assert_eq!(ProviderKind::Ngrok.as_str(), "ngrok");
        assert_eq!(ProviderKind::Tailscale.as_str(), "tailscale");
    }
}
