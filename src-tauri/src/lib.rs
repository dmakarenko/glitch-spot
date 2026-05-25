// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod capture;
mod hotkey;
pub mod mcp;
mod overlay;
pub mod snapshot;
mod spike_capture;
mod tray;

use std::sync::{Arc, Mutex};
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
        let state = AppState {
            latest_frame: Mutex::new(None),
            snapshot_store: Arc::new(mcp::store::SnapshotStore::new()),
            latest_snapshot_at: Mutex::new(None),
        };
        assert!(state.latest_snapshot_at.lock().unwrap().is_none());
    }
}

pub fn run() {
    let snapshot_store = Arc::new(mcp::store::SnapshotStore::new());
    let store_for_mcp = snapshot_store.clone();

    tauri::async_runtime::spawn(async move {
        mcp::server::serve(store_for_mcp).await;
    });

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

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None::<Vec<&str>>,
        ))
        .manage(AppState {
            latest_frame: Mutex::new(None),
            snapshot_store,
            latest_snapshot_at: Mutex::new(None),
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
