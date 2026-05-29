// ABOUTME: Pure rectangle algebra for computing the visible portion of each
// ABOUTME: window after subtracting every window in front of it.

use super::WindowBounds;

/// Return the portion of `base` not covered by `sub`. The result is a list of
/// non-overlapping `WindowBounds` whose union equals `base ∖ sub`. When the two
/// rectangles do not intersect, the returned list is `vec![base]`. When `sub`
/// fully covers `base`, the returned list is empty.
///
/// Strips are produced in this order: top, bottom, left, right — but callers
/// must not rely on the order, only on the union semantics.
pub fn subtract_rect(base: WindowBounds, sub: WindowBounds) -> Vec<WindowBounds> {
    let bx1 = base.x;
    let by1 = base.y;
    let bx2 = base.x + base.w as i32;
    let by2 = base.y + base.h as i32;
    let sx1 = sub.x;
    let sy1 = sub.y;
    let sx2 = sub.x + sub.w as i32;
    let sy2 = sub.y + sub.h as i32;

    // Intersection.
    let ix1 = bx1.max(sx1);
    let iy1 = by1.max(sy1);
    let ix2 = bx2.min(sx2);
    let iy2 = by2.min(sy2);

    if ix1 >= ix2 || iy1 >= iy2 {
        // No overlap — base survives intact.
        return vec![base];
    }

    let mut out = Vec::with_capacity(4);

    // Top strip: full base width, from base.y up to intersection.y.
    if iy1 > by1 {
        out.push(WindowBounds {
            x: bx1,
            y: by1,
            w: base.w,
            h: (iy1 - by1) as u32,
        });
    }
    // Bottom strip: full base width, from intersection.y+h down to base.y+h.
    if iy2 < by2 {
        out.push(WindowBounds {
            x: bx1,
            y: iy2,
            w: base.w,
            h: (by2 - iy2) as u32,
        });
    }
    // Left strip: within the intersection's y-band only.
    if ix1 > bx1 {
        out.push(WindowBounds {
            x: bx1,
            y: iy1,
            w: (ix1 - bx1) as u32,
            h: (iy2 - iy1) as u32,
        });
    }
    // Right strip: within the intersection's y-band only.
    if ix2 < bx2 {
        out.push(WindowBounds {
            x: ix2,
            y: iy1,
            w: (bx2 - ix2) as u32,
            h: (iy2 - iy1) as u32,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(x: i32, y: i32, w: u32, h: u32) -> WindowBounds {
        WindowBounds { x, y, w, h }
    }

    #[test]
    fn no_overlap_returns_base_unchanged() {
        let base = b(0, 0, 100, 100);
        let sub = b(200, 200, 50, 50);
        assert_eq!(subtract_rect(base, sub), vec![base]);
    }

    #[test]
    fn sub_fully_covers_base_returns_empty() {
        let base = b(10, 10, 50, 50);
        let sub = b(0, 0, 200, 200);
        assert_eq!(subtract_rect(base, sub), vec![]);
    }

    #[test]
    fn sub_in_the_middle_yields_four_strips() {
        // base = 0,0,100,100 ; sub = 25,25,50,50 → top/bottom/left/right strips
        let base = b(0, 0, 100, 100);
        let sub = b(25, 25, 50, 50);
        let got = subtract_rect(base, sub);
        // Top strip: y=0..25 spans full width
        assert!(got.contains(&b(0, 0, 100, 25)), "missing top strip in {got:?}");
        // Bottom strip: y=75..100 spans full width
        assert!(got.contains(&b(0, 75, 100, 25)), "missing bottom strip in {got:?}");
        // Left strip: within the intersection's y-band (25..75)
        assert!(got.contains(&b(0, 25, 25, 50)), "missing left strip in {got:?}");
        // Right strip: within the intersection's y-band (25..75)
        assert!(got.contains(&b(75, 25, 25, 50)), "missing right strip in {got:?}");
        assert_eq!(got.len(), 4);
    }

    #[test]
    fn sub_clipping_top_left_corner_yields_two_strips() {
        // base = 50,50,100,100 ; sub = 0,0,80,80 → only bottom and right strips
        let base = b(50, 50, 100, 100);
        let sub = b(0, 0, 80, 80);
        let got = subtract_rect(base, sub);
        // intersection = 50,50..80,80 → bottom strip y=80..150 across full width,
        // right strip x=80..150 within y=50..80
        assert!(got.contains(&b(50, 80, 100, 70)), "missing bottom strip in {got:?}");
        assert!(got.contains(&b(80, 50, 70, 30)), "missing right strip in {got:?}");
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn sub_clipping_only_right_edge_yields_one_strip() {
        // base = 0,0,100,100 ; sub = 80,0,100,100 → left strip only
        let base = b(0, 0, 100, 100);
        let sub = b(80, 0, 100, 100);
        let got = subtract_rect(base, sub);
        assert_eq!(got, vec![b(0, 0, 80, 100)]);
    }

    #[test]
    fn sub_clipping_horizontal_band_yields_top_and_bottom() {
        // base = 0,0,100,100 ; sub = -50,30,300,40 → covers full width 30..70
        let base = b(0, 0, 100, 100);
        let sub = b(-50, 30, 300, 40);
        let got = subtract_rect(base, sub);
        assert!(got.contains(&b(0, 0, 100, 30)), "missing top strip in {got:?}");
        assert!(got.contains(&b(0, 70, 100, 30)), "missing bottom strip in {got:?}");
        assert_eq!(got.len(), 2);
    }

    #[test]
    fn zero_width_sub_does_not_change_base() {
        let base = b(0, 0, 100, 100);
        let sub = b(20, 20, 0, 50);
        assert_eq!(subtract_rect(base, sub), vec![base]);
    }
}
