// ABOUTME: Coordinate conversions for window picker — logical points to physical pixels.
// ABOUTME: All hit-testing operates on physical pixels to match Tauri's cursor_position units.

use super::WindowBounds;

/// Convert a SCWindow CGRect (logical points, origin top-left of the global
/// display arrangement) into physical-pixel bounds. The display the window
/// primarily lives on supplies the scale_factor. The display's *own* physical
/// origin is supplied so we can offset the rect into global physical space.
///
/// All inputs and outputs are i32/f32 — overflow guards aren't critical given
/// realistic display geometry.
pub fn points_to_physical_bounds(
    rect_points: (f32, f32, f32, f32),       // (x, y, w, h) in logical points
    display_logical_origin: (i32, i32),      // logical-point origin of the display
    display_physical_origin: (i32, i32),     // physical-pixel origin of the same display
    scale_factor: f32,
) -> WindowBounds {
    let (rx, ry, rw, rh) = rect_points;
    let (dlx, dly) = display_logical_origin;
    let (dpx, dpy) = display_physical_origin;
    // 1. translate rect's logical origin into display-local logical coords
    let local_x = rx - dlx as f32;
    let local_y = ry - dly as f32;
    // 2. scale to physical pixels
    let phys_local_x = local_x * scale_factor;
    let phys_local_y = local_y * scale_factor;
    let phys_w = rw * scale_factor;
    let phys_h = rh * scale_factor;
    // 3. translate back into global physical coords using display physical origin
    WindowBounds {
        x: (dpx as f32 + phys_local_x).round() as i32,
        y: (dpy as f32 + phys_local_y).round() as i32,
        w: phys_w.round().max(0.0) as u32,
        h: phys_h.round().max(0.0) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_when_scale_is_1_and_origins_match() {
        let out = points_to_physical_bounds(
            (10.0, 20.0, 800.0, 600.0),
            (0, 0),
            (0, 0),
            1.0,
        );
        assert_eq!(out, WindowBounds { x: 10, y: 20, w: 800, h: 600 });
    }

    #[test]
    fn scales_dimensions_by_factor() {
        let out = points_to_physical_bounds(
            (0.0, 0.0, 800.0, 600.0),
            (0, 0),
            (0, 0),
            2.0,
        );
        assert_eq!(out, WindowBounds { x: 0, y: 0, w: 1600, h: 1200 });
    }

    #[test]
    fn offsets_into_secondary_display_physical_origin() {
        // Secondary display sits to the right of primary in logical space at
        // x=1440pt, primary is 1440×900 @ 2x → 2880px wide.
        let out = points_to_physical_bounds(
            (1500.0, 50.0, 200.0, 100.0),   // window 60pt into the secondary display
            (1440, 0),                       // logical origin of secondary
            (2880, 0),                       // physical origin of secondary
            2.0,
        );
        // local_x = 1500 - 1440 = 60pt → 120px; global_x = 2880 + 120 = 3000
        assert_eq!(out, WindowBounds { x: 3000, y: 100, w: 400, h: 200 });
    }
}
