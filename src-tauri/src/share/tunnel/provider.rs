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
