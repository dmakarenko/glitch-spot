// ABOUTME: Overlay window lifecycle: position over active monitor, show, hide.
// ABOUTME: Supports two modes: `live` (transparent over desktop) and `frozen` (with screenshot).

use crate::capture::CapturedFrame;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

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

fn position_over_active_monitor(app: &tauri::AppHandle) -> Result<WebviewWindow, OverlayError> {
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

    // Position over the monitor's *work area* — i.e., excluding the macOS menu
    // bar (and dock if present). For the frozen mode the capture is already
    // cropped to this same area, so the screenshot inside the overlay aligns
    // 1:1 with what's behind it.
    let work_area = monitor.work_area();
    let work_pos = work_area.position;
    let work_size = work_area.size;

    window.set_position(PhysicalPosition::new(work_pos.x, work_pos.y))?;
    window.set_size(PhysicalSize::new(work_size.width, work_size.height))?;

    Ok(window)
}

/// Show the overlay as a transparent annotation layer over the live desktop.
pub fn show_live(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    // Synchronously reset any prior frozen frame the overlay might still be
    // painting from a previous show. The eval is processed by the webview's
    // main thread before the next paint.
    let _ = window.eval("if (window.__glitchSpotPresent) window.__glitchSpotPresent('live');");

    window.show()?;
    window.set_focus()?;
    Ok(())
}

/// Show the overlay with a frozen screenshot as backdrop.
pub fn show_frozen(app: &tauri::AppHandle, frame: &CapturedFrame) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    let _ = window.eval("if (window.__glitchSpotPresent) window.__glitchSpotPresent('frozen');");

    window.show()?;
    window.set_focus()?;
    window.emit("glitch-spot://frame", frame)?;
    Ok(())
}

pub fn hide(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    window.hide()?;
    Ok(())
}
