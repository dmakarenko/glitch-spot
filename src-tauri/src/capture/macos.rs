// ABOUTME: macOS implementation of screen capture using ScreenCaptureKit.
// ABOUTME: Picks the display under the cursor via Tauri's available_monitors() API.

use image::RgbaImage;
use screencapturekit::screenshot_manager::{CGImageExt, SCScreenshotManager};
use screencapturekit::shareable_content::SCShareableContent;
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;

use super::{CaptureError, CapturedFrame};

pub(super) fn capture_active_monitor(
    app: &tauri::AppHandle,
) -> Result<CapturedFrame, CaptureError> {
    let cursor = app
        .cursor_position()
        .map_err(|e| CaptureError::Platform(format!("cursor position: {e}")))?;

    // Enumerate Tauri monitors in physical-pixel coordinates to find which one
    // the cursor is on. SCDisplay.frame() is in logical points, so we avoid
    // mixing units by doing all containment checks in physical pixels here.
    //
    // TODO(v0.1-followup): Tauri and SCShareableContent enumerate displays via
    // CGGetActiveDisplayList; same-index ordering is assumed but not formally
    // verified. A future hardening pass could match by SCDisplay::display_id()
    // against a platform-native display identifier.
    let monitors = app
        .available_monitors()
        .map_err(|e| CaptureError::Platform(format!("available monitors: {e}")))?;

    let display_index = monitors
        .iter()
        .position(|m| {
            let pos = m.position();
            let size = m.size();
            let x_min = pos.x as f64;
            let y_min = pos.y as f64;
            let x_max = x_min + size.width as f64;
            let y_max = y_min + size.height as f64;
            cursor.x >= x_min && cursor.x < x_max && cursor.y >= y_min && cursor.y < y_max
        })
        .unwrap_or_else(|| {
            eprintln!(
                "glitch-spot: cursor ({}, {}) not inside any monitor rect; falling back to display 0",
                cursor.x, cursor.y
            );
            0
        });

    let content = SCShareableContent::get()
        .map_err(|e| CaptureError::Platform(format!("shareable content: {e:?}")))?;

    let displays = content.displays();
    let display = displays.get(display_index).ok_or(CaptureError::NoDisplay)?;

    // Capture at full physical-pixel monitor resolution; we'll crop to the work
    // area (excluding macOS menu bar) after the capture completes. Doing the
    // crop in `image` rather than via SCStreamConfiguration::source_rect keeps
    // this independent of the crate's optional features.
    let monitor = &monitors[display_index];
    let monitor_size = monitor.size();
    let config = SCStreamConfiguration::new()
        .with_width(monitor_size.width)
        .with_height(monitor_size.height);

    // Exclude our own windows (the overlay) from the grab. The frozen flow shows
    // the grey overlay *before* capturing so the user gets instant feedback;
    // without this exclusion that overlay would appear in the screenshot.
    let me = std::process::id() as i32;
    let windows = content.windows();
    let own_windows: Vec<&_> = windows
        .iter()
        .filter(|w| {
            w.owning_application()
                .map(|a| a.process_id() == me)
                .unwrap_or(false)
        })
        .collect();
    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&own_windows)
        .build();

    let image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| CaptureError::Platform(format!("capture: {e:?}")))?;

    // Use the actual buffer dimensions — on Retina the pixel buffer can be
    // larger than the logical display size reported by SCDisplay.
    let pixel_width = image.width() as u32;
    let pixel_height = image.height() as u32;

    let scale_factor = pixel_width as f32 / display.width() as f32;

    let rgba = image
        .rgba_data()
        .map_err(|e| CaptureError::Encoding(format!("rgba_data: {e:?}")))?;

    let img = RgbaImage::from_raw(pixel_width, pixel_height, rgba)
        .ok_or_else(|| CaptureError::Encoding("rgba buffer size mismatch".into()))?;

    // Crop to the monitor's work area (skips the menu bar at the top, and any
    // dock area at the bottom if visible).
    let work_area = monitor.work_area();
    let work_pos = work_area.position;
    let work_size = work_area.size;
    let crop_x = work_pos.x.saturating_sub(monitor.position().x).max(0) as u32;
    let crop_y = work_pos.y.saturating_sub(monitor.position().y).max(0) as u32;
    let crop_w = work_size.width.min(pixel_width.saturating_sub(crop_x));
    let crop_h = work_size.height.min(pixel_height.saturating_sub(crop_y));

    let cropped = image::imageops::crop_imm(&img, crop_x, crop_y, crop_w, crop_h).to_image();

    Ok(CapturedFrame {
        width: crop_w,
        height: crop_h,
        scale_factor,
        png: super::encode_png(&cropped)?,
        display_index,
    })
}

pub(super) fn capture_window(
    app: &tauri::AppHandle,
    window_id: u32,
) -> Result<CapturedFrame, CaptureError> {
    let content = SCShareableContent::get()
        .map_err(|e| CaptureError::Platform(format!("shareable content: {e:?}")))?;

    let window = content
        .windows()
        .iter()
        .find(|w| w.window_id() == window_id)
        .cloned()
        .ok_or_else(|| CaptureError::Platform(format!("window {window_id} no longer available")))?;

    let frame = window.frame();
    let logical_w = frame.size.width as f32;
    let logical_h = frame.size.height as f32;

    // Determine the window's owning display so we know the scale factor.
    let monitors = app
        .available_monitors()
        .map_err(|e| CaptureError::Platform(format!("monitors: {e}")))?;

    // Determine the window's owning display by comparing its logical-point
    // origin against each SCDisplay's logical frame (origins/sizes in points,
    // matching the units of SCWindow.frame). Then look up the matching Tauri
    // monitor's scale_factor by same-index ordering (same convention used in
    // window_picker/macos.rs and capture_active_monitor's TODO note).
    let display_index = content
        .displays()
        .iter()
        .position(|d| {
            let f = d.frame();
            frame.origin.x >= f.origin.x
                && frame.origin.x < f.origin.x + f.size.width
                && frame.origin.y >= f.origin.y
                && frame.origin.y < f.origin.y + f.size.height
        })
        .unwrap_or(0);

    let scale_factor = monitors
        .get(display_index)
        .map(|m| m.scale_factor() as f32)
        .unwrap_or(1.0);

    let physical_w = (logical_w * scale_factor).round() as u32;
    let physical_h = (logical_h * scale_factor).round() as u32;

    let config = SCStreamConfiguration::new()
        .with_width(physical_w)
        .with_height(physical_h);

    let filter = SCContentFilter::create().with_window(&window).build();

    let image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| CaptureError::Platform(format!("capture: {e:?}")))?;

    let pixel_width = image.width() as u32;
    let pixel_height = image.height() as u32;
    let rgba = image
        .rgba_data()
        .map_err(|e| CaptureError::Encoding(format!("rgba_data: {e:?}")))?;

    let img = RgbaImage::from_raw(pixel_width, pixel_height, rgba)
        .ok_or_else(|| CaptureError::Encoding("rgba buffer size mismatch".into()))?;

    Ok(CapturedFrame {
        width: pixel_width,
        height: pixel_height,
        scale_factor,
        png: super::encode_png(&img)?,
        display_index: 0, // set by the caller from picker_session
    })
}
