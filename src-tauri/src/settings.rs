// ABOUTME: Read/write ~/.glitch-spot/config.toml — provider selection lives here.
// ABOUTME: Defaults to ProviderKind::Cloudflared when the file or [tunnel].provider is absent.

use std::io;
use std::path::Path;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use std::time::Duration;

use serde::{Deserialize, Serialize};

pub use crate::share::tunnel::provider::ProviderKind;

const CONFIG_FILE: &str = "config.toml";
const DEFAULT_MCP_PORT: u16 = 7820;
const DEFAULT_TRAY_ANIMATED: bool = true;
const DEFAULT_TRAY_COLOURED: bool = true;

/// How long to retain snapshots before auto-deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SnapshotRetention {
    #[default]
    Hours24,
    Days7,
    Days30,
    /// Never auto-delete — cleanup is skipped entirely.
    Never,
}

impl SnapshotRetention {
    /// Returns the max age for cleanup, or `None` if auto-deletion is disabled.
    pub fn max_age(self) -> Option<Duration> {
        match self {
            Self::Hours24 => Some(Duration::from_secs(24 * 60 * 60)),
            Self::Days7   => Some(Duration::from_secs(7 * 24 * 60 * 60)),
            Self::Days30  => Some(Duration::from_secs(30 * 24 * 60 * 60)),
            Self::Never   => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hours24 => "24h",
            Self::Days7   => "7d",
            Self::Days30  => "30d",
            Self::Never   => "never",
        }
    }

    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "24h"   => Some(Self::Hours24),
            "7d"    => Some(Self::Days7),
            "30d"   => Some(Self::Days30),
            "never" => Some(Self::Never),
            _       => None,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    tunnel: TunnelSection,
    #[serde(default)]
    mcp: McpSection,
    #[serde(default)]
    tray: TraySection,
    #[serde(default)]
    snapshot: SnapshotSection,
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

#[derive(Debug, Serialize, Deserialize, Default)]
struct TraySection {
    #[serde(default)]
    animated: Option<bool>,
    #[serde(default)]
    coloured: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
struct SnapshotSection {
    #[serde(default)]
    retention: Option<String>,
}

pub struct Settings {
    provider: RwLock<ProviderKind>,
    mcp_port: u16,
    /// Tray icon flags. `Arc<AtomicBool>` mirrors the `mcp_bound` pattern —
    /// the animation loop reads these every tick (~10Hz) and the menu handler
    /// flips them, so an atomic is cheaper than a lock and avoids any chance
    /// of the menu click stalling the animation thread.
    tray_animated: Arc<AtomicBool>,
    tray_coloured: Arc<AtomicBool>,
    snapshot_retention: RwLock<SnapshotRetention>,
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

        let tray_animated = parsed
            .get("tray")
            .and_then(|t| t.get("animated"))
            .and_then(|v| v.as_bool())
            .unwrap_or(DEFAULT_TRAY_ANIMATED);

        let tray_coloured = parsed
            .get("tray")
            .and_then(|t| t.get("coloured"))
            .and_then(|v| v.as_bool())
            .unwrap_or(DEFAULT_TRAY_COLOURED);

        let snapshot_retention = parsed
            .get("snapshot")
            .and_then(|s| s.get("retention"))
            .and_then(|v| v.as_str())
            .and_then(SnapshotRetention::from_name)
            .unwrap_or_default();

        Ok(Settings {
            provider: RwLock::new(provider),
            mcp_port,
            tray_animated: Arc::new(AtomicBool::new(tray_animated)),
            tray_coloured: Arc::new(AtomicBool::new(tray_coloured)),
            snapshot_retention: RwLock::new(snapshot_retention),
        })
    }

    pub fn current_provider(&self) -> ProviderKind {
        *self.provider.read().expect("settings lock poisoned")
    }

    pub fn mcp_port(&self) -> u16 {
        self.mcp_port
    }

    /// `(animated, coloured)`. Cheap atomic load — safe to call every tick.
    pub fn current_tray(&self) -> (bool, bool) {
        (
            self.tray_animated.load(Ordering::Relaxed),
            self.tray_coloured.load(Ordering::Relaxed),
        )
    }

    /// Hand the animation loop its own `Arc` to the flag so it can read each
    /// tick without going through `&Settings`.
    pub fn tray_animated_flag(&self) -> Arc<AtomicBool> { Arc::clone(&self.tray_animated) }
    pub fn tray_coloured_flag(&self) -> Arc<AtomicBool> { Arc::clone(&self.tray_coloured) }

    /// Update the in-memory provider AND persist the new value to disk.
    pub fn save_provider(&self, dir: &Path, kind: ProviderKind) -> io::Result<()> {
        {
            let mut w = self.provider.write().expect("settings lock poisoned");
            *w = kind;
        }
        self.write_config_file(dir)
    }

    /// Update the in-memory tray flags AND persist them to disk.
    pub fn save_tray_appearance(
        &self,
        dir: &Path,
        animated: bool,
        coloured: bool,
    ) -> io::Result<()> {
        self.tray_animated.store(animated, Ordering::Relaxed);
        self.tray_coloured.store(coloured, Ordering::Relaxed);
        self.write_config_file(dir)
    }

    pub fn current_retention(&self) -> SnapshotRetention {
        *self.snapshot_retention.read().expect("settings lock poisoned")
    }

    /// Update the in-memory retention AND persist it to disk.
    pub fn save_retention(&self, dir: &Path, retention: SnapshotRetention) -> io::Result<()> {
        {
            let mut w = self.snapshot_retention.write().expect("settings lock poisoned");
            *w = retention;
        }
        self.write_config_file(dir)
    }

    /// Snapshot all in-memory settings into the on-disk config. All `save_*`
    /// methods go through here so that saving one section never silently drops
    /// the others.
    fn write_config_file(&self, dir: &Path) -> io::Result<()> {
        let (animated, coloured) = self.current_tray();
        let cfg = ConfigFile {
            tunnel: TunnelSection {
                provider: Some(self.current_provider().as_str().to_string()),
            },
            mcp: McpSection { port: Some(self.mcp_port) },
            tray: TraySection {
                animated: Some(animated),
                coloured: Some(coloured),
            },
            snapshot: SnapshotSection {
                retention: Some(self.current_retention().as_str().to_string()),
            },
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
    use std::time::Duration;

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

    #[test]
    fn missing_tray_section_yields_animated_coloured_defaults() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_tray(), (true, true));
    }

    #[test]
    fn explicit_tray_section_is_parsed() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[tray]\nanimated = false\ncoloured = false\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_tray(), (false, false));
    }

    #[test]
    fn save_tray_appearance_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_tray_appearance(temp.path(), false, true).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_tray(), (false, true));
    }

    #[test]
    fn save_provider_after_tray_change_preserves_tray_section() {
        // Regression: writing one section must not silently drop another.
        // Both save_provider and save_tray_appearance share the same
        // write_config_file path; this locks that in.
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_tray_appearance(temp.path(), false, false).unwrap();
        settings.save_provider(temp.path(), ProviderKind::Ngrok).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_tray(), (false, false), "tray section preserved");
        assert_eq!(reloaded.current_provider(), ProviderKind::Ngrok, "provider applied");
    }

    #[test]
    fn save_tray_appearance_after_provider_change_preserves_provider() {
        // Mirror of the above in the opposite direction.
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_provider(temp.path(), ProviderKind::Tailscale).unwrap();
        settings.save_tray_appearance(temp.path(), true, false).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_provider(), ProviderKind::Tailscale, "provider preserved");
        assert_eq!(reloaded.current_tray(), (true, false), "tray section applied");
    }

    #[test]
    fn missing_snapshot_section_yields_default_retention() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_retention(), SnapshotRetention::Hours24);
    }

    #[test]
    fn snapshot_retention_max_age_hours24() {
        assert_eq!(SnapshotRetention::Hours24.max_age(), Some(Duration::from_secs(24 * 60 * 60)));
    }

    #[test]
    fn snapshot_retention_max_age_days7() {
        assert_eq!(SnapshotRetention::Days7.max_age(), Some(Duration::from_secs(7 * 24 * 60 * 60)));
    }

    #[test]
    fn snapshot_retention_max_age_days30() {
        assert_eq!(SnapshotRetention::Days30.max_age(), Some(Duration::from_secs(30 * 24 * 60 * 60)));
    }

    #[test]
    fn snapshot_retention_max_age_never() {
        assert_eq!(SnapshotRetention::Never.max_age(), None);
    }

    #[test]
    fn explicit_snapshot_retention_is_parsed() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[snapshot]\nretention = \"7d\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_retention(), SnapshotRetention::Days7);
    }

    #[test]
    fn unknown_retention_falls_back_to_default() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            temp.path().join("config.toml"),
            b"[snapshot]\nretention = \"bogus\"\n",
        )
        .unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        assert_eq!(settings.current_retention(), SnapshotRetention::Hours24);
    }

    #[test]
    fn save_retention_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_retention(temp.path(), SnapshotRetention::Days30).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_retention(), SnapshotRetention::Days30);
    }

    #[test]
    fn save_retention_never_roundtrip() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_retention(temp.path(), SnapshotRetention::Never).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_retention(), SnapshotRetention::Never);
    }

    #[test]
    fn save_retention_preserves_other_sections() {
        let temp = tempfile::TempDir::new().unwrap();
        let settings = Settings::load_from(temp.path()).unwrap();
        settings.save_provider(temp.path(), ProviderKind::Ngrok).unwrap();
        settings.save_tray_appearance(temp.path(), false, false).unwrap();
        settings.save_retention(temp.path(), SnapshotRetention::Days7).unwrap();

        let reloaded = Settings::load_from(temp.path()).unwrap();
        assert_eq!(reloaded.current_provider(), ProviderKind::Ngrok, "provider preserved");
        assert_eq!(reloaded.current_tray(), (false, false), "tray preserved");
        assert_eq!(reloaded.current_retention(), SnapshotRetention::Days7, "retention applied");
    }
}
