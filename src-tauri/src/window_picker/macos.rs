// ABOUTME: macOS-specific window enumeration via SCShareableContent + filter pipeline.
// ABOUTME: Translates SCWindow logical-point CGRects into PickableWindow physical-px bounds.

use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex};
use core_foundation::base::TCFType;
use core_graphics::window::{
    create_window_list, kCGNullWindowID, kCGWindowListExcludeDesktopElements,
    kCGWindowListOptionOnScreenOnly,
};
use screencapturekit::shareable_content::SCShareableContent;

use super::{
    coords::points_to_physical_bounds,
    filter::{filter_standard_windows, RawWindow},
    visible_regions::compute_visible_regions,
    z_order::sort_by_z_order,
    DisplayBounds, PickableWindow,
};

/// Window ids (`CGWindowID`) of on-screen, non-desktop windows in the window
/// server's true front-to-back order. This is the authoritative z-order;
/// `SCShareableContent`'s window list order is not reliable. `CGWindowListCreate`
/// stores each window id directly as the array's value pointer.
fn on_screen_window_order() -> Vec<u32> {
    let options = kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements;
    let Some(list) = create_window_list(options, kCGNullWindowID) else {
        return Vec::new();
    };
    let array_ref = list.as_concrete_TypeRef();
    let mut ids = Vec::new();
    unsafe {
        let count = CFArrayGetCount(array_ref);
        for i in 0..count {
            let value = CFArrayGetValueAtIndex(array_ref, i);
            ids.push(value as u32);
        }
    }
    ids
}

#[derive(Debug, thiserror::Error)]
pub enum EnumerateError {
    #[error("SCShareableContent: {0}")]
    ShareableContent(String),
}

/// Enumerate pickable windows and the displays they live on.
///
/// Returns the windows in front-to-back order (preserving SCShareableContent's
/// own ordering), filtered to "standard app windows", with bounds normalised
/// to physical pixels in the global screen coordinate space.
pub fn enumerate(
    app: &tauri::AppHandle,
) -> Result<(Vec<PickableWindow>, Vec<DisplayBounds>), EnumerateError> {
    let monitors = app
        .available_monitors()
        .map_err(|e| EnumerateError::ShareableContent(format!("monitors: {e}")))?;

    let displays: Vec<DisplayBounds> = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| DisplayBounds {
            display_index: i,
            x: m.position().x,
            y: m.position().y,
            w: m.size().width,
            h: m.size().height,
            scale_factor: m.scale_factor() as f32,
        })
        .collect();

    let content = SCShareableContent::get()
        .map_err(|e| EnumerateError::ShareableContent(format!("{e:?}")))?;

    // SCDisplay::frame() is in logical points (origin in the global logical
    // arrangement). Pair these with Tauri monitors by index — the existing
    // capture path already assumes same-index ordering (see capture/macos.rs
    // for the TODO note documenting this).
    let display_logical_frames: Vec<(f64, f64, f64, f64)> = content
        .displays()
        .iter()
        .map(|d| {
            let f = d.frame();
            (f.origin.x, f.origin.y, f.size.width, f.size.height)
        })
        .collect();

    let raws: Vec<RawWindow> = content
        .windows()
        .iter()
        .filter_map(|w| {
            let frame = w.frame();
            let app_obj = w.owning_application();
            let app_name = app_obj
                .as_ref()
                .map(|a| a.application_name())
                .unwrap_or_default();
            let bundle_id = app_obj
                .as_ref()
                .map(|a| a.bundle_identifier())
                .filter(|s| !s.is_empty());

            // Find the display this window primarily lives on by largest
            // intersection area — matching macOS NSWindow.screen convention.
            let display_index = primary_display_index(
                frame.origin.x,
                frame.origin.y,
                frame.size.width,
                frame.size.height,
                &display_logical_frames,
            );

            let display = displays.get(display_index)?;
            let (lx, ly, _, _) = display_logical_frames
                .get(display_index)
                .copied()
                .unwrap_or((0.0, 0.0, 0.0, 0.0));
            let bounds = points_to_physical_bounds(
                (
                    frame.origin.x as f32,
                    frame.origin.y as f32,
                    frame.size.width as f32,
                    frame.size.height as f32,
                ),
                (lx.round() as i32, ly.round() as i32), // logical origin (points)
                (display.x, display.y),                 // physical origin (px) from Tauri
                display.scale_factor,
            );

            Some(RawWindow {
                window_id: w.window_id(),
                app_name,
                bundle_id,
                title: w.title().unwrap_or_default(),
                bounds,
                display_index,
                layer: w.window_layer(),
                is_on_screen: w.is_on_screen(),
            })
        })
        .collect();

    // SCShareableContent's window order is not reliable front-to-back z-order,
    // so reorder by the window server's true z-order before computing regions.
    // Both hit-testing and visible-region clipping depend on this order.
    let mut filtered = sort_by_z_order(filter_standard_windows(&raws), &on_screen_window_order());

    // Compute visible regions over the front-to-back list, then drop windows
    // whose visible region is empty (fully occluded by windows in front).
    let bounds: Vec<_> = filtered.iter().map(|w| w.bounds).collect();
    let regions = compute_visible_regions(&bounds);
    for (w, r) in filtered.iter_mut().zip(regions.into_iter()) {
        w.visible_regions = r;
    }
    filtered.retain(|w| !w.visible_regions.is_empty());

    Ok((filtered, displays))
}

/// Returns the index of the display the window primarily lives on, measured by
/// the largest intersection area. Falls back to `0` for off-screen windows.
fn primary_display_index(
    win_x: f64,
    win_y: f64,
    win_w: f64,
    win_h: f64,
    display_frames: &[(f64, f64, f64, f64)],
) -> usize {
    display_frames
        .iter()
        .enumerate()
        .filter_map(|(i, (lx, ly, lw, lh))| {
            let ix = win_x.max(*lx);
            let iy = win_y.max(*ly);
            let iw = (win_x + win_w).min(lx + lw) - ix;
            let ih = (win_y + win_h).min(ly + lh) - iy;
            if iw > 0.0 && ih > 0.0 {
                Some((i, iw * ih))
            } else {
                None
            }
        })
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(i, _)| i)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    type Frame = (f64, f64, f64, f64);

    #[test]
    fn window_mostly_on_right_display_picks_right() {
        // Two side-by-side 2560×1414 displays.
        let displays: Vec<Frame> = vec![(0.0, 0.0, 2560.0, 1414.0), (2560.0, 0.0, 2560.0, 1414.0)];
        // Origin on left (x=2400), but 960×600 window → 160px on left, 800px on right.
        assert_eq!(
            primary_display_index(2400.0, 200.0, 960.0, 600.0, &displays),
            1
        );
    }

    #[test]
    fn window_fully_on_left_display_picks_left() {
        let displays: Vec<Frame> = vec![(0.0, 0.0, 2560.0, 1414.0), (2560.0, 0.0, 2560.0, 1414.0)];
        assert_eq!(
            primary_display_index(100.0, 100.0, 800.0, 600.0, &displays),
            0
        );
    }

    #[test]
    fn off_screen_window_falls_back_to_zero() {
        let displays: Vec<Frame> = vec![(0.0, 0.0, 2560.0, 1414.0)];
        assert_eq!(
            primary_display_index(-200.0, -200.0, 100.0, 100.0, &displays),
            0
        );
    }

    #[test]
    fn window_with_origin_on_minor_side_picks_major_side() {
        // Window origin on left display but 90% on right.
        let displays: Vec<Frame> = vec![(0.0, 0.0, 1920.0, 1080.0), (1920.0, 0.0, 1920.0, 1080.0)];
        // Origin at x=1870 → 50px on left, 750px on right.
        assert_eq!(
            primary_display_index(1870.0, 100.0, 800.0, 600.0, &displays),
            1
        );
    }
}
