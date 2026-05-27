// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod capture;
mod hotkey;
pub mod mcp;
mod overlay;
pub mod snapshot;
mod spike_capture;
mod tray;
pub mod settings;
pub mod share;

pub use capture::CapturedFrame;

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;
use tauri::Manager;
use tauri_plugin_clipboard_manager::ClipboardExt;

use snapshot::build::IncomingAnnotation;

/// Process-wide cache of the most recently captured frame.
///
/// Currently only populated by the frozen-overlay path; the live-overlay path
/// doesn't capture at trigger time. When the save/persist slice lands, both
/// paths will capture (live mode at save time, frozen mode at trigger time)
/// and this slot becomes the staging area for the snapshot artifact.
pub struct AppState {
    pub latest_frame: Mutex<Option<capture::CapturedFrame>>,
    pub snapshot_store: Arc<mcp::store::SnapshotStore>,
    pub latest_snapshot_at: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
    pub tunnel: Arc<share::tunnel::TunnelManager>,
    pub settings: Arc<settings::Settings>,
    pub config_dir: std::path::PathBuf,
    /// Set to true by `mcp::server::serve` once the listener is bound, cleared
    /// if/when serve returns. Tray reads this to decide healthy vs unhealthy —
    /// avoids the TCP-connect false positive when another process happens to
    /// own the configured port.
    pub mcp_bound: Arc<AtomicBool>,
}

#[tauri::command]
async fn capture_active_monitor(app: tauri::AppHandle) -> Result<capture::CapturedFrame, String> {
    capture::capture_active_monitor(&app).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_live(app: tauri::AppHandle) -> Result<(), String> {
    trigger_overlay_live(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_frozen(app: tauri::AppHandle) -> Result<(), String> {
    trigger_overlay_frozen(&app)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    overlay::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_latest_frame(state: tauri::State<'_, AppState>) -> Option<capture::CapturedFrame> {
    state.latest_frame.lock().ok().and_then(|g| g.clone())
}

#[tauri::command]
async fn save_snapshot(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    mode: String,
    annotations: Vec<IncomingAnnotation>,
) -> Result<snapshot::Snapshot, String> {
    let frame = match mode.as_str() {
        "frozen" => state
            .latest_frame
            .lock()
            .ok()
            .and_then(|g| g.clone())
            .ok_or_else(|| "no cached frame for frozen mode".to_string())?,
        "live" => {
            // Hide the overlay BEFORE capturing so the saved screenshot doesn't
            // include our own annotation rectangles + checker pattern. A brief
            // sleep gives macOS time to repaint without the overlay before the
            // screen-capture sample fires.
            overlay::hide(&app).map_err(|e| e.to_string())?;
            std::thread::sleep(std::time::Duration::from_millis(120));
            capture::capture_active_monitor(&app).map_err(|e| e.to_string())?
        }
        other => return Err(format!("unknown mode: {other}")),
    };

    let os_platform: &str = tauri_plugin_os::platform();
    let os_version: String = tauri_plugin_os::version().to_string();

    let base = snapshot::persist::snapshots_dir().map_err(|e| e.to_string())?;
    let built = snapshot::build::build(
        &frame,
        &mode,
        annotations,
        os_platform,
        &os_version,
        &base,
        None,
    )
    .map_err(|e| e.to_string())?;
    snapshot::persist::write(&base, &built).map_err(|e| e.to_string())?;
    let snap = built.snapshot;

    // Notify any waiting MCP caller.
    state.snapshot_store.publish(snap.clone());

    if let Ok(mut ts) = state.latest_snapshot_at.lock() {
        *ts = Some(chrono::Utc::now());
    }

    let json = serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())?;
    app.clipboard()
        .write_text(json)
        .map_err(|e| format!("clipboard: {e}"))?;

    overlay::hide(&app).map_err(|e| e.to_string())?;
    Ok(snap)
}

/// Shares the latest snapshot: starts the tunnel, generates a token, writes it to disk,
/// copies the URL to the clipboard, and fires a notification. Called by both the Tauri
/// command and the tray menu handler.
pub(crate) async fn do_share_latest(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<String, String> {
    let snap = state
        .snapshot_store
        .get_latest()
        .ok_or_else(|| "no snapshot to share".to_string())?;
    let snap_id = snap.id.clone();

    let provider: Arc<dyn share::tunnel::provider::TunnelProvider> =
        match state.settings.current_provider() {
            settings::ProviderKind::Cloudflared => Arc::new(share::tunnel::cloudflared::Cloudflared),
            settings::ProviderKind::Ngrok       => Arc::new(share::tunnel::ngrok::Ngrok),
            settings::ProviderKind::Tailscale   => Arc::new(share::tunnel::tailscale::Tailscale),
        };

    let tunnel_url = Arc::clone(&state.tunnel).start_tunnel(provider, 7821).await
        .map_err(|e| e.to_string())?;

    let token = share::token::generate();
    let snap_dir = snapshot::persist::snapshots_dir().map_err(|e| e.to_string())?.join(&snap_id);
    share::token::write_for(&snap_dir, &token).map_err(|e| e.to_string())?;

    let share_url = format!("{tunnel_url}/share/{snap_id}/{token}/");
    app.clipboard().write_text(share_url.clone()).map_err(|e| e.to_string())?;

    use tauri_plugin_notification::NotificationExt;
    let _ = app.notification()
        .builder()
        .title("Glitch Spot")
        .body("Share URL copied to clipboard")
        .show();

    Ok(share_url)
}

/// Sets the tunnel provider: shuts down any running tunnel and persists the new choice.
/// Called by both the Tauri command and the tray menu handler.
pub(crate) async fn do_set_tunnel_provider(
    state: &AppState,
    kind: String,
) -> Result<(), String> {
    let parsed = settings::ProviderKind::from_name(&kind)
        .ok_or_else(|| format!("unknown provider: {kind}"))?;
    state.tunnel.shutdown().await;
    state.settings.save_provider(&state.config_dir, parsed).map_err(|e| e.to_string())
}

#[tauri::command]
async fn share_latest(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    do_share_latest(&app, &state).await
}

#[tauri::command]
fn current_tunnel_state(state: tauri::State<'_, AppState>) -> String {
    match state.tunnel.current_state() {
        share::tunnel::TunnelState::Idle => "idle".into(),
        share::tunnel::TunnelState::Starting => "starting…".into(),
        share::tunnel::TunnelState::Live { url } => url,
        share::tunnel::TunnelState::Error { reason } => format!("error — {reason}"),
    }
}

#[tauri::command]
async fn set_tunnel_provider(
    state: tauri::State<'_, AppState>,
    kind: String,
) -> Result<(), String> {
    do_set_tunnel_provider(&state, kind).await
}

pub(crate) async fn trigger_overlay_live(app: &tauri::AppHandle) -> anyhow::Result<()> {
    overlay::show_live(app)?;
    Ok(())
}

pub(crate) async fn trigger_overlay_frozen(app: &tauri::AppHandle) -> anyhow::Result<()> {
    // Clear any stale frame BEFORE capturing the new one. Without this, a fast
    // hotkey press right after switching monitors would let the overlay's
    // mount-time pull return the *previous* monitor's screenshot for a moment.
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.latest_frame.lock() {
            *slot = None;
        }
    }
    let frame = capture::capture_active_monitor(app)?;
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.latest_frame.lock() {
            *slot = Some(frame.clone());
        }
    }
    overlay::show_frozen(app, &frame)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn app_state_latest_snapshot_at_starts_none() {
        let temp = tempfile::TempDir::new().unwrap();
        let state = AppState {
            latest_frame: Mutex::new(None),
            snapshot_store: Arc::new(mcp::store::SnapshotStore::new()),
            latest_snapshot_at: Mutex::new(None),
            tunnel: Arc::new(share::tunnel::TunnelManager::new()),
            settings: Arc::new(
                settings::Settings::load_from(temp.path()).unwrap(),
            ),
            config_dir: temp.path().to_path_buf(),
            mcp_bound: Arc::new(AtomicBool::new(false)),
        };
        assert!(state.latest_snapshot_at.lock().unwrap().is_none());
    }

    #[test]
    fn share_router_is_reachable_from_lib() {
        let temp = tempfile::TempDir::new().unwrap();
        let _router = crate::share::router::build_share_router(temp.path().to_path_buf());
        // Smoke test: just confirm construction doesn't panic.
    }
}

pub fn run() {
    let config_dir = std::path::PathBuf::from(
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string()),
    )
    .join(".glitch-spot");

    let settings = match settings::Settings::load_from(&config_dir) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("[glitch-spot] settings load failed: {e}; using defaults");
            let temp = std::env::temp_dir().join("glitch-spot-fallback");
            let _ = std::fs::create_dir_all(&temp);
            Arc::new(
                settings::Settings::load_from(&temp)
                    .expect("default settings construction must succeed"),
            )
        }
    };

    let snapshot_store = Arc::new(mcp::store::SnapshotStore::new());
    let store_for_mcp = snapshot_store.clone();
    let mcp_port = settings.mcp_port();
    let mcp_bound = Arc::new(AtomicBool::new(false));
    let mcp_bound_for_serve = mcp_bound.clone();

    tauri::async_runtime::spawn(async move {
        mcp::server::serve(store_for_mcp, mcp_port, mcp_bound_for_serve).await;
    });

    let share_base = snapshot::persist::snapshots_dir().ok();
    if let Some(base) = share_base.clone() {
        tauri::async_runtime::spawn(async move {
            share::router::serve_share(base).await;
        });
    } else {
        eprintln!("[glitch-spot] share server NOT started — could not resolve snapshots dir");
    }

    // Sweep snapshots older than 24h. Synchronous: a directory scan over a
    // handful of small dirs is sub-millisecond. Failures never block startup.
    match snapshot::persist::snapshots_dir() {
        Ok(base) => {
            if let Err(e) = snapshot::persist::cleanup(&base, snapshot::persist::MAX_AGE) {
                eprintln!("[glitch-spot] cleanup failed: {e}");
            }
        }
        Err(e) => eprintln!("[glitch-spot] could not resolve snapshots dir: {e}"),
    }

    let tunnel = Arc::new(share::tunnel::TunnelManager::new());

    let tunnel_for_exit = tunnel.clone();
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .plugin(tauri_plugin_notification::init())
        .manage(AppState {
            latest_frame: Mutex::new(None),
            snapshot_store,
            latest_snapshot_at: Mutex::new(None),
            tunnel: tunnel.clone(),
            settings: settings.clone(),
            config_dir: config_dir.clone(),
            mcp_bound: mcp_bound.clone(),
        })
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Auto-enable login item on first launch (if not already registered).
            {
                use tauri_plugin_autostart::ManagerExt;
                let manager = app.autolaunch();
                if !manager.is_enabled().unwrap_or(false) {
                    let _ = manager.enable();
                }
            }

            hotkey::register(app.handle())?;
            let _tray = tray::setup_tray(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture_active_monitor,
            open_overlay_live,
            open_overlay_frozen,
            hide_overlay,
            get_latest_frame,
            save_snapshot,
            spike_capture::spike_capture,
            share_latest,
            current_tunnel_state,
            set_tunnel_provider,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |_app, event| {
        if let tauri::RunEvent::ExitRequested { .. } = &event {
            let tunnel = tunnel_for_exit.clone();
            tauri::async_runtime::block_on(async move {
                tunnel.shutdown().await;
            });
        }
    });
}
