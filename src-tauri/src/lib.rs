// ABOUTME: Entry point for the Glitch Spot Tauri 2 application library.
// ABOUTME: Builds and runs the Tauri app with registered Tauri commands.
mod capture;
mod overlay;
mod spike_capture;

#[tauri::command]
async fn capture_active_monitor(app: tauri::AppHandle) -> Result<capture::CapturedFrame, String> {
    capture::capture_active_monitor(&app).map_err(|e| e.to_string())
}

#[tauri::command]
async fn open_overlay_with_capture(app: tauri::AppHandle) -> Result<(), String> {
    let frame = capture::capture_active_monitor(&app).map_err(|e| e.to_string())?;
    overlay::show_over_active_monitor(&app, &frame).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn hide_overlay(app: tauri::AppHandle) -> Result<(), String> {
    overlay::hide(&app).map_err(|e| e.to_string())
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            capture_active_monitor,
            open_overlay_with_capture,
            hide_overlay,
            spike_capture::spike_capture,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
