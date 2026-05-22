// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod capture;
mod hotkey;
mod overlay;
mod spike_capture;

use std::sync::Mutex;
use tauri::Manager;

/// Process-wide cache of the most recently captured frame.
///
/// The overlay webview pulls from here on mount so it can render the screenshot
/// even when it isn't yet listening for the `glitch-spot://frame` event on the
/// very first hotkey press (the listener registers asynchronously after the
/// webview finishes loading; events emitted before then are dropped).
pub struct AppState {
    pub latest_frame: Mutex<Option<capture::CapturedFrame>>,
}

#[tauri::command]
async fn capture_active_monitor(app: tauri::AppHandle) -> Result<capture::CapturedFrame, String> {
    capture::capture_active_monitor(&app).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_with_capture(app: tauri::AppHandle) -> Result<(), String> {
    trigger_overlay(&app).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    overlay::hide(&app).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_latest_frame(state: tauri::State<'_, AppState>) -> Option<capture::CapturedFrame> {
    // On subsequent hotkey presses we overwrite the slot, so this only returns
    // a stale frame if the overlay mounts without a preceding capture — which
    // shouldn't happen in v0.1 but is acceptable if it does.
    state.latest_frame.lock().ok().and_then(|g| g.clone())
}

pub(crate) async fn trigger_overlay(app: &tauri::AppHandle) -> anyhow::Result<()> {
    let frame = capture::capture_active_monitor(app)?;
    // Cache before showing so that the overlay's mount-time pull always finds
    // the frame, regardless of how fast the webview finishes loading.
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut slot) = state.latest_frame.lock() {
            *slot = Some(frame.clone());
        }
    }
    overlay::show_over_active_monitor(app, &frame)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(AppState {
            latest_frame: Mutex::new(None),
        })
        .setup(|app| {
            hotkey::register(app.handle())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            capture_active_monitor,
            open_overlay_with_capture,
            hide_overlay,
            get_latest_frame,
            spike_capture::spike_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
