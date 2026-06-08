// ABOUTME: Public screen-capture API for Glitch Spot. OS-agnostic surface; impls behind cfg.
// ABOUTME: Currently macOS-only (v0.1); Windows lands in v1.

use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ImageEncoder, RgbaImage};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "macos")]
mod macos;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapturedFrame {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    /// PNG-encoded bytes.
    #[serde(with = "serde_bytes")]
    pub png: Vec<u8>,
    /// Index of the captured monitor in the OS's display list.
    pub display_index: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("no display found")]
    NoDisplay,
    #[error("platform error: {0}")]
    Platform(String),
    #[error("encoding error: {0}")]
    Encoding(String),
}

/// Encode an RGBA image to PNG. A single fixed `Up` filter avoids the default
/// Adaptive filter's per-row 5-filter search (~1.6s on a 23MP Retina frame)
/// while still compressing screen content well enough to keep the IPC payload
/// small.
pub(crate) fn encode_png(img: &RgbaImage) -> Result<Vec<u8>, CaptureError> {
    let mut png_bytes: Vec<u8> = Vec::new();
    PngEncoder::new_with_quality(&mut png_bytes, CompressionType::Fast, FilterType::Up)
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            image::ColorType::Rgba8.into(),
        )
        .map_err(|e| CaptureError::Encoding(format!("png encode: {e}")))?;
    Ok(png_bytes)
}

#[cfg(target_os = "macos")]
pub fn capture_active_monitor(app: &tauri::AppHandle) -> Result<CapturedFrame, CaptureError> {
    macos::capture_active_monitor(app)
}

#[cfg(not(target_os = "macos"))]
pub fn capture_active_monitor(_app: &tauri::AppHandle) -> Result<CapturedFrame, CaptureError> {
    Err(CaptureError::Platform(
        "only macOS is supported in v0.1".into(),
    ))
}

#[cfg(target_os = "macos")]
pub fn capture_window(
    app: &tauri::AppHandle,
    window_id: u32,
) -> Result<CapturedFrame, CaptureError> {
    macos::capture_window(app, window_id)
}

#[cfg(not(target_os = "macos"))]
pub fn capture_window(
    _app: &tauri::AppHandle,
    _window_id: u32,
) -> Result<CapturedFrame, CaptureError> {
    Err(CaptureError::Platform("only macOS is supported".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn captured_frame_round_trips_through_json() {
        let frame = CapturedFrame {
            width: 1920,
            height: 1080,
            scale_factor: 2.0,
            png: vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A],
            display_index: 0,
        };
        let json = serde_json::to_string(&frame).unwrap();
        let back: CapturedFrame = serde_json::from_str(&json).unwrap();
        assert_eq!(frame, back);
    }

    #[test]
    fn encode_png_produces_a_decodable_png_of_the_same_dimensions() {
        let img = RgbaImage::from_fn(640, 400, |x, y| {
            image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255])
        });
        let png = encode_png(&img).unwrap();
        let decoded = image::load_from_memory(&png).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (640, 400));
    }
}
