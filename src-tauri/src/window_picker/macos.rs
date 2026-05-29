// ABOUTME: macOS-specific window enumeration via SCShareableContent + filter pipeline.
// ABOUTME: Translates SCWindow logical-point CGRects into PickableWindow physical-px bounds.

use screencapturekit::shareable_content::SCShareableContent;

use super::{
    coords::points_to_physical_bounds,
    filter::{filter_standard_windows, RawWindow},
    visible_regions::compute_visible_regions,
    DisplayBounds, PickableWindow,
};

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

            // Find the display this window primarily lives on by checking its
            // logical-point origin against each SCDisplay's logical frame.
            let display_index = display_logical_frames
                .iter()
                .position(|(lx, ly, lw, lh)| {
                    frame.origin.x >= *lx
                        && frame.origin.x < lx + lw
                        && frame.origin.y >= *ly
                        && frame.origin.y < ly + lh
                })
                .unwrap_or(0);

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
                (display.x, display.y),                  // physical origin (px) from Tauri
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

    let mut filtered = filter_standard_windows(&raws);

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

// The macOS enumerator has no unit-testable surface beyond what's already
// covered by window_picker::filter and window_picker::coords. End-to-end
// verification is the manual smoke checklist (Task 25).
