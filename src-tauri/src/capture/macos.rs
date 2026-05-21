// ABOUTME: macOS implementation of screen capture using ScreenCaptureKit.
// ABOUTME: Picks the display under the cursor via Tauri's available_monitors() API.

use image::codecs::png::PngEncoder;
use image::{ImageEncoder, RgbaImage};
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

    let config = SCStreamConfiguration::new()
        .with_width(display.width())
        .with_height(display.height());

    let filter = SCContentFilter::create()
        .with_display(display)
        .with_excluding_windows(&[])
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

    let mut png_bytes: Vec<u8> = Vec::new();
    PngEncoder::new(&mut png_bytes)
        .write_image(
            img.as_raw(),
            pixel_width,
            pixel_height,
            image::ColorType::Rgba8.into(),
        )
        .map_err(|e| CaptureError::Encoding(format!("png encode: {e}")))?;

    Ok(CapturedFrame {
        width: pixel_width,
        height: pixel_height,
        scale_factor,
        png: png_bytes,
        display_index,
    })
}
