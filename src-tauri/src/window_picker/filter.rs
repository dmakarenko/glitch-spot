// ABOUTME: Pure filter that selects "standard app windows" for the picker.
// ABOUTME: No platform dependencies; takes the raw list and returns the pickable subset.

use super::{PickableWindow, WindowBounds};

/// Minimum width and height (in physical px) for a window to be considered
/// "standard". Below this we treat the entry as chrome/shadow/transient.
pub const MIN_DIMENSION_PX: u32 = 80;

/// Owning-application names that are always excluded regardless of size/layer.
/// Bundle ID is the primary check (`com.glitchspot.app`); these are belt-and-braces
/// for system processes that don't surface a stable bundle id.
const EXCLUDED_APP_NAMES: &[&str] = &[
    "Dock",
    "WindowServer",
    "SystemUIServer",
    "Control Center",
];

/// Source-of-truth bundle id for *our own* app — excluded from the pickable set.
pub const SELF_BUNDLE_ID: &str = "com.glitchspot.app";

/// Inputs to the filter. Decoupled from the macOS SCWindow type so this is
/// trivially testable on any platform.
#[derive(Debug, Clone, PartialEq)]
pub struct RawWindow {
    pub window_id: u32,
    pub app_name: String,
    pub bundle_id: Option<String>,
    pub title: String,
    pub bounds: WindowBounds,
    pub display_index: usize,
    pub layer: i32,
    pub is_on_screen: bool,
}

pub fn filter_standard_windows(raws: &[RawWindow]) -> Vec<PickableWindow> {
    raws.iter()
        .filter(|r| r.is_on_screen)
        .filter(|r| r.layer == 0)
        .filter(|r| r.bundle_id.as_deref() != Some(SELF_BUNDLE_ID))
        .filter(|r| !EXCLUDED_APP_NAMES.contains(&r.app_name.as_str()))
        .filter(|r| r.bounds.w >= MIN_DIMENSION_PX && r.bounds.h >= MIN_DIMENSION_PX)
        .map(|r| PickableWindow {
            window_id: r.window_id,
            app_name: r.app_name.clone(),
            title: r.title.clone(),
            bounds: r.bounds,
            display_index: r.display_index,
            layer: r.layer,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(window_id: u32) -> RawWindow {
        RawWindow {
            window_id,
            app_name: "Safari".into(),
            bundle_id: Some("com.apple.Safari".into()),
            title: "Reader".into(),
            bounds: WindowBounds { x: 0, y: 0, w: 800, h: 600 },
            display_index: 0,
            layer: 0,
            is_on_screen: true,
        }
    }

    #[test]
    fn standard_safari_window_is_included() {
        let out = filter_standard_windows(&[raw(1)]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].window_id, 1);
    }

    #[test]
    fn off_screen_windows_are_excluded() {
        let mut r = raw(1);
        r.is_on_screen = false;
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn non_zero_layer_windows_are_excluded() {
        let mut r = raw(1);
        r.layer = 1000;
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty(), "menu/panel layers must be excluded");
    }

    #[test]
    fn own_bundle_id_is_excluded() {
        let mut r = raw(1);
        r.bundle_id = Some(SELF_BUNDLE_ID.into());
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn system_app_names_are_excluded() {
        for name in &["Dock", "WindowServer", "SystemUIServer", "Control Center"] {
            let mut r = raw(1);
            r.app_name = (*name).into();
            r.bundle_id = None; // some don't have a stable bundle id
            let out = filter_standard_windows(&[r]);
            assert!(out.is_empty(), "{name} should be excluded");
        }
    }

    #[test]
    fn windows_smaller_than_min_are_excluded() {
        let mut r = raw(1);
        r.bounds = WindowBounds { x: 0, y: 0, w: 50, h: 50 };
        let out = filter_standard_windows(&[r]);
        assert!(out.is_empty());
    }

    #[test]
    fn preserves_input_order() {
        let mut a = raw(1);
        a.title = "A".into();
        let mut b = raw(2);
        b.title = "B".into();
        let mut c = raw(3);
        c.title = "C".into();
        let out = filter_standard_windows(&[a, b, c]);
        let titles: Vec<&str> = out.iter().map(|w| w.title.as_str()).collect();
        assert_eq!(titles, vec!["A", "B", "C"]);
    }
}
