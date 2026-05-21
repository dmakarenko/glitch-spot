// ABOUTME: Overlay window lifecycle: position over active monitor, show, hide.
// ABOUTME: Reads cursor position to pick the monitor, then sizes the "overlay" window to it.

use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const OVERLAY_LABEL: &str = "overlay";

#[derive(Debug, thiserror::Error)]
pub enum OverlayError {
    #[error("overlay window not found")]
    WindowMissing,
    #[error("no monitor found at cursor")]
    NoMonitor,
    #[error("tauri error: {0}")]
    Tauri(#[from] tauri::Error),
}

pub fn show_over_active_monitor(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window: WebviewWindow = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;

    let cursor = app.cursor_position()?;
    let monitor = app
        .available_monitors()?
        .into_iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            let x_min = pos.x as f64;
            let y_min = pos.y as f64;
            let x_max = x_min + size.width as f64;
            let y_max = y_min + size.height as f64;
            cursor.x >= x_min && cursor.x < x_max && cursor.y >= y_min && cursor.y < y_max
        })
        .ok_or(OverlayError::NoMonitor)?;

    window.set_position(PhysicalPosition::new(
        monitor.position().x,
        monitor.position().y,
    ))?;
    window.set_size(PhysicalSize::new(
        monitor.size().width,
        monitor.size().height,
    ))?;
    window.show()?;
    window.set_focus()?;
    Ok(())
}

pub fn hide(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    window.hide()?;
    Ok(())
}
