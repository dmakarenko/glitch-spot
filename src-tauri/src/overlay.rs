// ABOUTME: Overlay window lifecycle: position over active monitor, show, hide.
// ABOUTME: Supports two modes: `live` (transparent over desktop) and `frozen` (with screenshot).

use crate::capture::CapturedFrame;
use tauri::{Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow};

const OVERLAY_LABEL: &str = "overlay";

// CGPoint / CGSize / CGRect with explicit Objective-C encodings so msg_send!
// dispatches struct returns/arguments correctly on both x86_64 and aarch64.
#[cfg(target_os = "macos")]
mod cg {
    use objc2::encode::{Encode, Encoding, RefEncode};

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }
    unsafe impl Encode for CGPoint {
        const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl RefEncode for CGPoint {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGSize {
        pub width: f64,
        pub height: f64,
    }
    unsafe impl Encode for CGSize {
        const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
    }
    unsafe impl RefEncode for CGSize {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGRect {
        pub origin: CGPoint,
        pub size: CGSize,
    }
    unsafe impl Encode for CGRect {
        const ENCODING: Encoding =
            Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
    }
    unsafe impl RefEncode for CGRect {
        const ENCODING_REF: Encoding = Encoding::Pointer(&Self::ENCODING);
    }
}

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

/// Which of the window's screen rectangles to re-assert the frame to after
/// show(). The work-area overlays (live/frozen) want `Visible` (excludes the
/// menu bar and Dock — their capture is cropped to match); window mode wants
/// `Full` so a window overlapping the menu bar can be annotated.
#[derive(Clone, Copy)]
enum ScreenRect {
    Visible,
    Full,
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

/// Re-assert the window's frame after show() to undo macOS constrainFrameRect
/// position pushes. Reads the window's own screen geometry via AppKit
/// (`[NSWindow screen]` → `visibleFrame`/`frame`) and applies it with
/// `[NSWindow setFrame:display:NO]`, queued on the main thread FIFO after
/// show(). This runs after orderFront:/constrainFrameRect have pushed the
/// window, and uses AppKit's own coordinate space — so it is correct for any
/// display arrangement and any mix of scale factors, without manual conversion.
/// Both Tauri's show() and this call use run_on_main_thread, so FIFO ordering
/// is guaranteed.
#[cfg(target_os = "macos")]
fn reassert_screen_frame(window: &WebviewWindow, which: ScreenRect) {
    use cg::CGRect;
    use objc2::msg_send;
    use objc2::runtime::AnyObject;

    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    let ns_window = ns_window as usize;

    let _ = window.run_on_main_thread(move || {
        let ns_window = ns_window as *mut AnyObject;
        if ns_window.is_null() {
            return;
        }
        unsafe {
            // The window stays on its intended screen through the constrain push
            // (which only nudges it down within the same display), so its own
            // screen's geometry is the frame we want.
            let screen: *mut AnyObject = msg_send![ns_window, screen];
            if screen.is_null() {
                return;
            }
            let rect: CGRect = match which {
                ScreenRect::Visible => msg_send![screen, visibleFrame],
                ScreenRect::Full => msg_send![screen, frame],
            };
            let _: () = msg_send![ns_window, setFrame: rect, display: false];
        }
    });
}

#[cfg(not(target_os = "macos"))]
fn reassert_screen_frame(_window: &WebviewWindow, _which: ScreenRect) {}

/// Set a webview window's alpha. The overlay and the picker pool are shown at
/// alpha 0 (ordered-in but invisible) so each repaints the target display's
/// layout before it's revealed at alpha 1 — otherwise `show()` composites the
/// previous display's retained backing buffer for a beat. A hidden (occluded)
/// WKWebView won't repaint, but an ordered-in alpha-0 window renders normally.
#[cfg(target_os = "macos")]
pub fn set_window_alpha(window: &WebviewWindow, alpha: f64) {
    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    // `setAlphaValue:` is AppKit and must run on the main thread; the overlay
    // commands run on the async runtime. run_on_main_thread queues FIFO behind
    // the window's other main-thread ops (set_position/show), so an alpha-0 set
    // before show() is applied before the window is ordered in. The pointer is
    // cast through usize to cross the Send boundary into the closure.
    let ns_window = ns_window as usize;
    let _ = window.run_on_main_thread(move || {
        use objc2::msg_send;
        use objc2::runtime::AnyObject;
        let ns_window = ns_window as *mut AnyObject;
        if !ns_window.is_null() {
            unsafe {
                let _: () = msg_send![ns_window, setAlphaValue: alpha];
            }
        }
    });
}

#[cfg(not(target_os = "macos"))]
pub fn set_window_alpha(_window: &WebviewWindow, _alpha: f64) {}

/// Show the overlay as a transparent annotation layer over the live desktop.
pub fn show_live(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    // Show at alpha 0 so that macOS constrainFrameRect's position push (observed
    // on the primary display) isn't visible. The re-assert below corrects the
    // position before the frontend signals reveal. The frontend's presentationId
    // double-rAF effect calls revealOverlay() (~33ms) to bring alpha to 1.
    set_window_alpha(&window, 0.0);

    // Synchronously reset any prior frozen frame the overlay might still be
    // painting from a previous show. The eval is processed by the webview's
    // main thread before the next paint.
    let _ = window.eval("if (window.__glitchSpotPresent) window.__glitchSpotPresent('live');");

    window.show()?;
    window.set_focus()?;
    // Queue setFrame:display:NO on the main thread FIFO after orderFront: so it
    // runs after constrainFrameRect has pushed the window, undoing the push.
    reassert_screen_frame(&window, ScreenRect::Visible);
    Ok(())
}

/// Show the overlay in frozen mode with no frame yet — the grey "freeze
/// engaged" backdrop. Shown immediately on trigger so the overlay paints before
/// the (slower) screen capture runs; frames arrive afterwards via [`push_frame`].
pub fn show_frozen_pending(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    // Show at alpha 0: prevents the brief wrong-position flash that macOS
    // constrainFrameRect causes on show(), and prevents a stale capture from a
    // prior session blinking in before React resets. The frontend's
    // presentationId double-rAF effect calls revealOverlay() (~33ms) once the
    // grey veil has painted at the correct position.
    set_window_alpha(&window, 0.0);

    let _ = window.eval("if (window.__glitchSpotPresent) window.__glitchSpotPresent('frozen');");

    window.show()?;
    window.set_focus()?;
    // Queue setFrame:display:NO on the main thread FIFO after orderFront: so it
    // runs after constrainFrameRect has pushed the window, undoing the push.
    reassert_screen_frame(&window, ScreenRect::Visible);

    Ok(())
}

/// Push a replacement frame to an already-visible frozen overlay. The frontend
/// decodes off-screen and swaps atomically, so the crisp full-resolution frame
/// supersedes the preview without a flash.
pub fn push_frame(app: &tauri::AppHandle, frame: &CapturedFrame) -> Result<(), OverlayError> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    window.emit("glitch-spot://frame", frame)?;
    Ok(())
}

pub fn hide(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    window.hide()?;
    // Reset the overlay to a blank transparent state *after* hiding. The frozen
    // backdrop is a React `imageUrl` that only clears on an async state update;
    // if we leave the previous capture's image in the DOM, the next show()
    // paints it for a frame before the new frame decodes (most jarring across
    // displays, where the stale image is entirely different content). Resetting
    // while hidden gives React the full idle interval to flush before next show.
    let _ = window.eval("if (window.__glitchSpotPresent) window.__glitchSpotPresent('live');");
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
    /// Menu-bar/notch height in CSS px. Window mode covers the full display, so
    /// the toolbar offsets by this to clear the menu bar (see `menu_bar_inset_css`).
    pub top_inset_css: f32,
    /// First annotation drawn in the picker (drag-to-draw), in overlay-relative
    /// CSS px. When present, the overlay pre-populates it as a committed rect.
    pub initial_rect_css: Option<CssRect>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CssRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowRectPhysical {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

/// Height of the menu bar / notch on a monitor, in CSS pixels — the gap between
/// the monitor's full frame top and its work area top. Window mode covers the
/// full frame, so the toolbar offsets by this much to clear the menu bar. The
/// webview's own `window.screenY` is unreliable for a borderless full-screen
/// overlay (it reports Cocoa bottom-left coords), so the inset is computed here
/// from the monitor geometry instead. `monitor_top`/`work_area_top` are physical
/// pixels; the result is divided by `scale` into the CSS space the overlay draws in.
pub fn menu_bar_inset_css(monitor_top: i32, work_area_top: i32, scale: f32) -> f32 {
    ((work_area_top - monitor_top) as f32 / scale).max(0.0)
}

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

    // Show invisible: the overlay is a single window repositioned across
    // displays, and macOS retains the previous display's last-painted buffer.
    // Showing at alpha 0 lets the (now ordered-in, un-occluded) window repaint
    // this display's window-mode layout while unseen; `reveal` (or the fallback
    // timer in picker_select) fades it to alpha 1 once the frontend signals its
    // frame has painted, so show() can't flash the prior display's stale frame.
    set_window_alpha(&window, 0.0);

    let json = serde_json::to_string(payload)?;
    let js = format!(
        "if (window.__glitchSpotPresent) window.__glitchSpotPresent('window', {});",
        json
    );
    let _ = window.eval(&js);
    window.show()?;
    window.set_focus()?;
    // Re-assert after show(): macOS constrainFrameRect pushes a full-display
    // window down (it covers the menu-bar region). Queue setFrame: on the main
    // thread FIFO after orderFront: so it runs after the push — re-asserting via
    // Tauri set_position here would run on the async runtime *before* the queued
    // orderFront:/constrainFrameRect in release builds, missing the push.
    reassert_screen_frame(&window, ScreenRect::Full);
    Ok(())
}

/// Payload sent to the overlay React tree when presenting an opened image.
/// `image_rect_css` is where the letterboxed image is drawn, in overlay CSS px.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ImageModePayload {
    pub image_rect_css: CssRect,
}

/// Present the overlay over the cursor monitor's work area in `image` mode. The
/// decoded image frame is pushed separately via `push_frame` after this returns
/// (see `present_image`), arriving through the same `glitch-spot://frame` event
/// the frozen/window paths use.
pub fn show_image_mode(
    app: &tauri::AppHandle,
    payload: &ImageModePayload,
) -> Result<(), OverlayError> {
    let window = position_over_active_monitor(app)?;

    // Show invisible, then reveal once the frame has painted — same alpha-0
    // dance as the live/frozen/window presents (avoids compositing a stale
    // backing buffer when the overlay was last shown on another display).
    set_window_alpha(&window, 0.0);

    let json = serde_json::to_string(payload)?;
    let js = format!(
        "if (window.__glitchSpotPresent) window.__glitchSpotPresent('image', {});",
        json
    );
    let _ = window.eval(&js);
    window.show()?;
    window.set_focus()?;
    reassert_screen_frame(&window, ScreenRect::Visible);
    Ok(())
}

/// Fade the overlay in to full opacity after a window-mode present (see
/// `show_window_mode`). Idempotent — the frontend signals this one rAF after its
/// window-mode frame paints, and picker_select schedules a fallback call in case
/// that signal is lost; calling it on an already-opaque overlay is harmless.
pub fn reveal(app: &tauri::AppHandle) -> Result<(), OverlayError> {
    let window = app
        .get_webview_window(OVERLAY_LABEL)
        .ok_or(OverlayError::WindowMissing)?;
    set_window_alpha(&window, 1.0);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_bar_inset_is_work_area_offset_in_css_px() {
        // Retina display (scale 2): full frame top at physical y=0, work area
        // pushed down by a 66px menu bar → 33 CSS px of inset.
        assert_eq!(menu_bar_inset_css(0, 66, 2.0), 33.0);
    }

    #[test]
    fn menu_bar_inset_is_zero_when_work_area_meets_frame_top() {
        // Secondary display with no menu bar: work area starts at the frame top.
        assert_eq!(menu_bar_inset_css(-1080, -1080, 1.0), 0.0);
    }

    #[test]
    fn menu_bar_inset_never_negative() {
        // A work-area top above the frame top would imply a negative inset;
        // clamp to 0 so the toolbar never floats above the viewport.
        assert_eq!(menu_bar_inset_css(100, 50, 1.0), 0.0);
    }
}
