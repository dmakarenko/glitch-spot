// ABOUTME: One-off spike for ScreenCaptureKit integration.
// ABOUTME: Captures the first available display once and writes /tmp/glitch-spot-spike.png.

use screencapturekit::screenshot_manager::SCScreenshotManager;
use screencapturekit::shareable_content::SCShareableContent;
use screencapturekit::stream::configuration::SCStreamConfiguration;
use screencapturekit::stream::content_filter::SCContentFilter;
use std::path::PathBuf;

#[tauri::command]
pub fn spike_capture() -> Result<String, String> {
    let content = SCShareableContent::get().map_err(|e| format!("shareable content: {e:?}"))?;
    let display = content
        .displays()
        .into_iter()
        .next()
        .ok_or_else(|| "no display found".to_string())?;

    let config = SCStreamConfiguration::new()
        .with_width(display.width())
        .with_height(display.height());

    let filter = SCContentFilter::create()
        .with_display(&display)
        .with_excluding_windows(&[])
        .build();

    let image = SCScreenshotManager::capture_image(&filter, &config)
        .map_err(|e| format!("capture: {e:?}"))?;

    let out = PathBuf::from("/tmp/glitch-spot-spike.png");
    image.save_png(&out).map_err(|e| format!("save: {e:?}"))?;

    Ok(out.display().to_string())
}
