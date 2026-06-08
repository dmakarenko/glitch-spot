// ABOUTME: Opening an image file for annotation: fit math, decode-to-frame, session.
// ABOUTME: Pure helpers (fit/decode) are unit-tested; orchestration lives in lib.rs.

/// Result of fitting an image into the overlay work area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FitResult {
    /// Where to draw the image, in overlay CSS px: (x, y, w, h), centered.
    pub css_rect: (f32, f32, f32, f32),
    /// Native image px per overlay CSS px. Downscale-only, so always >= 1.0.
    pub coord_scale: f32,
    /// Native-px offset of the image's top-left within its own pixel space,
    /// derived so `a.x * coord_scale - crop_origin.0` lands in image pixels.
    pub crop_origin: (f32, f32),
}

/// Fit an image into the work area, downscale-only: images larger than the work
/// area shrink to fit (preserving aspect); images at or below it show at native
/// pixel size, centered. `coord_scale` is native px per CSS px and is clamped
/// to `>= 1.0` so annotation strokes never thin below native resolution.
pub fn fit_image_rect(work_css: (f32, f32), image: (u32, u32)) -> FitResult {
    let (ww, wh) = work_css;
    let (iw, ih) = (image.0 as f32, image.1 as f32);
    let fit = (ww / iw).min(wh / ih);
    let css_per_img = fit.min(1.0);
    let w = iw * css_per_img;
    let h = ih * css_per_img;
    let x = ((ww - w) / 2.0).max(0.0);
    let y = ((wh - h) / 2.0).max(0.0);
    let coord_scale = 1.0 / css_per_img;
    FitResult {
        css_rect: (x, y, w, h),
        coord_scale,
        crop_origin: (x * coord_scale, y * coord_scale),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) {
        assert!((a - b).abs() < 0.01, "expected {b}, got {a}");
    }

    #[test]
    fn large_image_downscales_to_fit_and_centers() {
        // Work 1000x800, image 4000x2000 (wider aspect) → width-constrained.
        let r = fit_image_rect((1000.0, 800.0), (4000, 2000));
        approx(r.css_rect.2, 1000.0); // w fills width
        approx(r.css_rect.3, 500.0); // h = 2000 * 0.25
        approx(r.css_rect.0, 0.0); // centered x
        approx(r.css_rect.1, 150.0); // centered y = (800-500)/2
        approx(r.coord_scale, 4.0); // 4000/1000
        approx(r.crop_origin.0, 0.0);
        approx(r.crop_origin.1, 600.0); // 150 * 4
    }

    #[test]
    fn small_image_shows_native_size_centered() {
        // Work 1000x800, image 200x100 → no upscale.
        let r = fit_image_rect((1000.0, 800.0), (200, 100));
        approx(r.css_rect.2, 200.0);
        approx(r.css_rect.3, 100.0);
        approx(r.css_rect.0, 400.0); // (1000-200)/2
        approx(r.css_rect.1, 350.0); // (800-100)/2
        approx(r.coord_scale, 1.0); // clamped, no upscale
        approx(r.crop_origin.0, 400.0);
        approx(r.crop_origin.1, 350.0);
    }

    #[test]
    fn tall_image_is_height_constrained() {
        // Work 1000x800, image 1000x4000 → height-constrained.
        let r = fit_image_rect((1000.0, 800.0), (1000, 4000));
        approx(r.css_rect.3, 800.0); // h fills height
        approx(r.css_rect.2, 200.0); // w = 1000 * 0.2
        approx(r.coord_scale, 5.0); // 4000/800
    }

    #[test]
    fn coord_scale_is_never_below_one() {
        let r = fit_image_rect((4000.0, 4000.0), (50, 50));
        assert!(r.coord_scale >= 1.0, "got {}", r.coord_scale);
    }

    #[test]
    fn annotation_at_image_top_left_maps_to_origin() {
        // The invariant build() relies on: a click at the rect's top-left maps to
        // image px (0, 0) via a*coord_scale - crop_origin.
        let r = fit_image_rect((1000.0, 800.0), (4000, 2000));
        let (x, y, _, _) = r.css_rect;
        approx(x * r.coord_scale - r.crop_origin.0, 0.0);
        approx(y * r.coord_scale - r.crop_origin.1, 0.0);
    }
}
