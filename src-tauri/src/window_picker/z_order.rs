// ABOUTME: Reorders the pickable window list to match the window server's true
// ABOUTME: front-to-back z-order, since SCShareableContent's order is unreliable.

use std::collections::HashMap;

use super::PickableWindow;

/// Reorder `windows` to follow `front_to_back_ids` (the window server's true
/// front-to-back z-order, keyed by `CGWindowID` == `PickableWindow.window_id`).
///
/// Windows whose id appears in `front_to_back_ids` are placed in that order.
/// Windows absent from the ranking (e.g. on another Space, or filtered out of
/// the on-screen list) are appended to the back, preserving their original
/// relative order. The sort is stable.
pub fn sort_by_z_order(
    windows: Vec<PickableWindow>,
    front_to_back_ids: &[u32],
) -> Vec<PickableWindow> {
    let rank: HashMap<u32, usize> = front_to_back_ids
        .iter()
        .enumerate()
        .map(|(i, id)| (*id, i))
        .collect();
    let mut out = windows;
    out.sort_by_key(|w| rank.get(&w.window_id).copied().unwrap_or(usize::MAX));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::window_picker::WindowBounds;

    fn pw(id: u32) -> PickableWindow {
        PickableWindow {
            window_id: id,
            app_name: format!("App{id}"),
            title: format!("Window {id}"),
            bounds: WindowBounds {
                x: 0,
                y: 0,
                w: 100,
                h: 100,
            },
            display_index: 0,
            layer: 0,
            visible_regions: vec![],
        }
    }

    fn ids(ws: &[PickableWindow]) -> Vec<u32> {
        ws.iter().map(|w| w.window_id).collect()
    }

    #[test]
    fn reorders_present_windows_to_match_ranking() {
        let ws = vec![pw(1), pw(2), pw(3)];
        let out = sort_by_z_order(ws, &[3, 1, 2]);
        assert_eq!(ids(&out), vec![3, 1, 2]);
    }

    #[test]
    fn windows_absent_from_ranking_go_to_the_back() {
        // id 1 is not in the ranking → it sorts behind the ranked id 2.
        let ws = vec![pw(1), pw(2)];
        let out = sort_by_z_order(ws, &[2]);
        assert_eq!(ids(&out), vec![2, 1]);
    }

    #[test]
    fn multiple_absent_windows_preserve_original_order() {
        // 1 and 2 are absent; 3 is ranked front. Absent ones keep input order.
        let ws = vec![pw(1), pw(2), pw(3)];
        let out = sort_by_z_order(ws, &[3]);
        assert_eq!(ids(&out), vec![3, 1, 2]);
    }

    #[test]
    fn ranking_ids_not_in_windows_are_ignored() {
        let ws = vec![pw(1)];
        let out = sort_by_z_order(ws, &[5, 1, 9]);
        assert_eq!(ids(&out), vec![1]);
    }

    #[test]
    fn empty_ranking_preserves_input_order() {
        let ws = vec![pw(1), pw(2), pw(3)];
        let out = sort_by_z_order(ws, &[]);
        assert_eq!(ids(&out), vec![1, 2, 3]);
    }

    #[test]
    fn empty_windows_yields_empty() {
        let out = sort_by_z_order(vec![], &[1, 2, 3]);
        assert!(out.is_empty());
    }
}
