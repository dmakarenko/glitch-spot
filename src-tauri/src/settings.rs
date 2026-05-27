// ABOUTME: Read/write ~/.glitch-spot/config.toml — provider selection lives here.
// ABOUTME: Defaults to ProviderKind::Cloudflared when the file or [tunnel].provider is absent.

use std::io;
use std::path::Path;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};

pub use crate::share::tunnel::provider::ProviderKind;

const CONFIG_FILE: &str = "config.toml";
const DEFAULT_MCP_PORT: u16 = 7820;

#[derive(Debug, Serialize, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    tunnel: TunnelSection,
    #[serde(default)]
    mcp: McpSection,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct TunnelSection {
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct McpSection {
    #[serde(default)]
    port: Option<u16>,
}

pub struct Settings {
    provider: RwLock<ProviderKind>,
    mcp_port: u16,
}

impl Settings {
    /// Load settings from `{dir}/config.toml`. Missing or malformed files are
    /// tolerated: each setting is extracted independently from `toml::Value`,
    /// so a bad value in one section doesn't silently reset unrelated sections.
    pub fn load_from(dir: &Path) -> io::Result<Self> {
        let path = dir.join(CONFIG_FILE);
        let parsed: toml::Value = match std::fs::read_to_string(&path) {
            Ok(text) => match text.parse::<toml::Value>() {
                Ok(v) => v,
                Err(e) => {
                    eprintln!(
                        "[glitch-spot settings] failed to parse {}: {e}; using defaults",
                        path.display()
                    );
                    toml::Value::Table(toml::map::Map::new())
                }
            },
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                toml::Value::Table(toml::map::Map::new())
            }
            Err(e) => return Err(e),
        };

        let provider = parsed
            .get("tunnel")
            .and_then(|t| t.get("provider"))
            .and_then(|v| v.as_str())
            .and_then(ProviderKind::from_name)
            .unwrap_or_default();

        let mcp_port = parsed
            .get("mcp")
            .and_then(|m| m.get("port"))
            .and_then(|v| v.as_integer())
            .and_then(|n| u16::try_from(n).ok())
            .filter(|p| *p != 0)
            .unwrap_or(DEFAULT_MCP_PORT);

        Ok(Settings { provider: RwLock::new(provider), mcp_port })
    }

    pub fn current_provider(&self) -> ProviderKind {
        *self.provider.read().expect("settings lock poisoned")
    }

    pub fn mcp_port(&self) -> u16 {
        self.mcp_port
    }

    /// Update the in-memory provider AND persist the new value to disk.
    pub fn save_provider(&self, dir: &Path, kind: ProviderKind) -> io::Result<()> {
        {
            let mut w = self.provider.write().expect("settings lock poisoned");
            *w = kind;
        }
        let cfg = ConfigFile {
            tunnel: TunnelSection { provider: Some(kind.as_str().to_string()) },
            mcp: McpSection { port: Some(self.mcp_port) },
        };
        let text = toml::to_string_pretty(&cfg).map_err(io::Error::other)?;
        std::fs::create_dir_all(dir)?;
        std::fs::write(dir.join(CONFIG_FILE), text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_yields_default_provider() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Cloudflared);
    }

    #[test]
    fn malformed_toml_yields_default_provider() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("config.toml"), b"this is { not :: valid !! toml").unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Cloudflared);
    }

    #[test]
    fn explicit_ngrok_is_parsed() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("config.toml"), b"[tunnel]\nprovider = \"ngrok\"\n").unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Ngrok);
    }

    #[test]
    fn unknown_provider_falls_back_to_default() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("config.toml"), b"[tunnel]\nprovider = \"localtunnel\"\n").unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Cloudflared);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_provider(temp.path(), ProviderKind::Tailscale).unwrap();

        let settings2 = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings2.current_provider(), ProviderKind::Tailscale);
    }

    #[test]
    fn missing_mcp_section_yields_default_port() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.mcp_port(), 7820);
    }

    #[test]
    fn explicit_mcp_port_is_parsed() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[mcp]\nport = 9090\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.mcp_port(), 9090);
    }

    #[test]
    fn malformed_mcp_section_falls_back_to_default_port() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[mcp]\nport = \"not-a-number\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.mcp_port(), 7820);
    }

    #[test]
    fn malformed_mcp_port_preserves_unrelated_tunnel_provider() {
        // Regression for Codex P2: a typo in [mcp].port must not silently reset
        // [tunnel].provider to the default.
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[tunnel]\nprovider = \"ngrok\"\n\n[mcp]\nport = \"not-a-number\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Ngrok);
        assert_eq!(settings.mcp_port(), 7820);
    }

    #[test]
    fn malformed_tunnel_provider_preserves_unrelated_mcp_port() {
        // Same idea in the opposite direction: a wrong type on tunnel.provider
        // must not blow away a valid mcp.port.
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[tunnel]\nprovider = 42\n\n[mcp]\nport = 9090\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_provider(), ProviderKind::Cloudflared);
        assert_eq!(settings.mcp_port(), 9090);
    }

    #[test]
    fn port_zero_falls_back_to_default() {
        // Regression for Codex P3: port=0 makes the OS pick an ephemeral port
        // the tray and clients can't predict. Reject and use the default.
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("config.toml"), b"[mcp]\nport = 0\n").unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.mcp_port(), 7820);
    }

    #[test]
    fn out_of_range_port_falls_back_to_default() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("config.toml"), b"[mcp]\nport = 70000\n").unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.mcp_port(), 7820);
    }
}
