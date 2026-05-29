// ABOUTME: Public surface for the per-window picker mode. Cross-platform types;
// ABOUTME: macOS implementation lives in window_picker::macos behind a cfg gate.

use serde::{Deserialize, Serialize};

pub mod filter;
pub mod hit_test;
pub mod coords;
pub mod visible_regions;

#[cfg(target_os = "macos")]
pub mod macos;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PickableWindow {
    pub window_id: u32,
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,
    pub display_index: usize,
    pub layer: i32,
    pub visible_regions: Vec<WindowBounds>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DisplayBounds {
    pub display_index: usize,
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub scale_factor: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pickable_window_round_trips_through_json() {
        let w = PickableWindow {
            window_id: 42,
            app_name: "Safari".into(),
            title: "Reader".into(),
            bounds: WindowBounds { x: 10, y: 20, w: 800, h: 600 },
            display_index: 1,
            layer: 0,
            visible_regions: vec![
                WindowBounds { x: 10, y: 20, w: 200, h: 600 },
                WindowBounds { x: 400, y: 20, w: 410, h: 600 },
            ],
        };
        let json = serde_json::to_string(&w).unwrap();
        let back: PickableWindow = serde_json::from_str(&json).unwrap();
        assert_eq!(w, back);
    }
}
