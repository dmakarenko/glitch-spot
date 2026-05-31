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
    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),
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

/// Payload sent to the overlay React tree when transitioning into window mode.
/// Coordinates are mixed-unit by design: backend keeps physical-px for capture,
/// frontend renders in CSS-px.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowModePayload {
    pub app_name: String,
    pub title: String,
    pub window_id: u32,
    /// Window rect in CSS pixels relative to the overlay's top-left.
    pub window_rect_css: CssRect,
    /// Same window rect in physical pixels; used by the backend at save time.
    pub window_rect_physical: WindowRectPhysical,
    pub display_scale_factor: f32,
    /// Picker-click position in overlay-relative CSS px. Present when the user
    /// held the mouse button while selecting the window (drag intent), so the
    /// overlay can seed a create gesture at that position.
    pub initial_click_css: Option<CssPoint>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CssRect { pub x: f32, pub y: f32, pub w: f32, pub h: f32 }

#[derive(Debug, Clone, serde::Serialize)]
pub struct CssPoint { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowRectPhysical { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }

pub fn show_window_mode(
    app: &tauri::AppHandle,
    display_index: usize,
    payload: &WindowModePayload,
) -> Result<(), OverlayError> {
    let window: WebviewWindow = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    let monitors = app.available_monitors()?;
    let monitor = monitors.get(display_index).ok_or(OverlayError::NoMonitor)?;

    // Cover the *full* physical display (not just work area) so a window that
    // overlaps the menu bar can be annotated without cropping.
    let pos = monitor.position();
    let size = monitor.size();
    window.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    window.set_size(PhysicalSize::new(size.width, size.height))?;

    let json = serde_json::to_string(payload)?;
    let js = format!(
        "if (window.__glitchSpotPresent) window.__glitchSpotPresent('window', {});",
        json
    );
    let _ = window.eval(&js);
    window.show()?;
    window.set_focus()?;
    // Re-assert the frame after show()/set_focus(): this overlay covers the
    // full physical display (top edge at the menu bar), so macOS
    // constrainFrameRect pushes it a title-bar height down on the initial show,
    // before the borderless window is realized. The work-area overlays
    // (show_live/show_frozen) start below the menu bar and aren't constrained.
    window.set_position(PhysicalPosition::new(pos.x, pos.y))?;
    window.set_size(PhysicalSize::new(size.width, size.height))?;
    Ok(())
}
