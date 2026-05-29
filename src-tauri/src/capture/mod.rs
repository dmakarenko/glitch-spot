// ABOUTME: Public screen-capture API for Glitch Spot. OS-agnostic surface; impls behind cfg.
// ABOUTME: Currently macOS-only (v0.1); Windows lands in v1.

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
pub fn capture_window(app: &tauri::AppHandle, window_id: u32) -> Result<CapturedFrame, CaptureError> {
    macos::capture_window(app, window_id)
}

#[cfg(not(target_os = "macos"))]
pub fn capture_window(_app: &tauri::AppHandle, _window_id: u32) -> Result<CapturedFrame, CaptureError> {
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
}
