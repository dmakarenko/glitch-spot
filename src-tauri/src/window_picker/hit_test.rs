// ABOUTME: Pure cursor-position → window_id hit test for the picker mode.
// ABOUTME: Iterates the front-to-back ordered window list and returns the first contains-match.

use super::{PickableWindow, WindowBounds};

/// Returns the `window_id` of the topmost window containing the cursor, or
/// `None` if the cursor is over no pickable window. Input list is assumed to
/// be in **front-to-back** order; the first match wins.
///
/// Coordinates are physical pixels in global screen space.
pub fn hit_test(windows: &[PickableWindow], cursor_x: i32, cursor_y: i32) -> Option<u32> {
    windows.iter().find(|w| contains(&w.bounds, cursor_x, cursor_y)).map(|w| w.window_id)
}

fn contains(b: &WindowBounds, x: i32, y: i32) -> bool {
    x >= b.x && x < b.x + b.w as i32 && y >= b.y && y < b.y + b.h as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pw(id: u32, x: i32, y: i32, w: u32, h: u32) -> PickableWindow {
        PickableWindow {
            window_id: id,
            app_name: format!("App{id}"),
            title: format!("Window {id}"),
            bounds: WindowBounds { x, y, w, h },
            display_index: 0,
            layer: 0,
        }
    }

    #[test]
    fn cursor_inside_single_window_returns_its_id() {
        let ws = vec![pw(1, 100, 100, 400, 300)];
        assert_eq!(hit_test(&ws, 200, 200), Some(1));
    }

    #[test]
    fn cursor_outside_all_windows_returns_none() {
        let ws = vec![pw(1, 100, 100, 400, 300)];
        assert_eq!(hit_test(&ws, 50, 50), None);
        assert_eq!(hit_test(&ws, 600, 600), None);
    }

    #[test]
    fn cursor_in_overlap_returns_front_window() {
        // Input order is front-to-back: id 2 is in front of id 1.
        let ws = vec![pw(2, 150, 150, 300, 300), pw(1, 100, 100, 400, 400)];
        assert_eq!(hit_test(&ws, 200, 200), Some(2));
    }

    #[test]
    fn cursor_on_left_edge_is_inside() {
        let ws = vec![pw(1, 100, 100, 50, 50)];
        assert_eq!(hit_test(&ws, 100, 120), Some(1));
    }

    #[test]
    fn cursor_on_right_edge_is_outside_half_open_interval() {
        let ws = vec![pw(1, 100, 100, 50, 50)];
        // bounds.x + w = 150 → 150 is NOT inside (half-open).
        assert_eq!(hit_test(&ws, 150, 120), None);
    }

    #[test]
    fn empty_window_list_returns_none() {
        assert_eq!(hit_test(&[], 0, 0), None);
    }
}
