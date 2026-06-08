// ABOUTME: Destructive cover rendering for redaction regions on a captured frame.
// ABOUTME: Solid fill, averaged pixelation, and a source-independent branded glitch cover.

use image::{Rgba, RgbaImage};

use crate::settings::RedactionStyle;

const COVER_DARK: Rgba<u8> = Rgba([12, 12, 12, 255]); // #0c0c0c — DESIGN.md background
/// Warm dark grey for the solid cover — distinctly not pure black so the cover
/// reads as a deliberate block rather than a hole, while staying dark and on-tone.
const SOLID_COVER: Rgba<u8> = Rgba([36, 31, 27, 255]); // #241f1b
const ACCENT: Rgba<u8> = Rgba([238, 102, 51, 255]); // #ee6633
const ACCENT_DIM: Rgba<u8> = Rgba([120, 52, 26, 255]); // darkened accent for mosaic contrast
/// Pixelation/branded block size in CSS px. Coarse so a text glyph spans <1 block.
const BLOCK_CSS: f32 = 12.0;

/// A region to cover, in physical pixels on the captured image. May extend
/// outside the image; all writes are clamped.
#[derive(Debug, Clone, Copy)]
pub struct Region {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Clamp the region to the image bounds, returning `(x0, y0, x1, y1)` as a
/// half-open pixel range. Returns `None` if the clamped region is empty.
fn clamp(region: Region, img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let iw = img.width() as i32;
    let ih = img.height() as i32;
    let x0 = region.x.max(0).min(iw);
    let y0 = region.y.max(0).min(ih);
    let x1 = (region.x + region.w).max(0).min(iw);
    let y1 = (region.y + region.h).max(0).min(ih);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    Some((x0 as u32, y0 as u32, x1 as u32, y1 as u32))
}

/// Cover `region` of `img` in place using `style`. Destructive — the original
/// pixels under the region are overwritten and cannot be recovered (except that
/// `Pixelated` averages them; see [`RedactionStyle`] docs).
pub fn apply_redaction(
    img: &mut RgbaImage,
    region: Region,
    style: RedactionStyle,
    scale_factor: f32,
) {
    let Some(bounds) = clamp(region, img) else {
        return;
    };
    match style {
        RedactionStyle::Solid => solid_fill(img, bounds),
        RedactionStyle::Pixelated => pixelate(img, bounds, scale_factor),
        RedactionStyle::Branded => branded(img, bounds, scale_factor),
    }
}

fn block_px(scale_factor: f32) -> u32 {
    (BLOCK_CSS * scale_factor).round().max(2.0) as u32
}

fn solid_fill(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32)) {
    for y in y0..y1 {
        for x in x0..x1 {
            img.put_pixel(x, y, SOLID_COVER);
        }
    }
}

fn pixelate(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32), scale_factor: f32) {
    let block = block_px(scale_factor);
    let mut by = y0;
    while by < y1 {
        let by_end = (by + block).min(y1);
        let mut bx = x0;
        while bx < x1 {
            let bx_end = (bx + block).min(x1);
            let (mut r, mut g, mut b, mut a, mut n) = (0u64, 0u64, 0u64, 0u64, 0u64);
            for y in by..by_end {
                for x in bx..bx_end {
                    let p = img.get_pixel(x, y).0;
                    r += p[0] as u64;
                    g += p[1] as u64;
                    b += p[2] as u64;
                    a += p[3] as u64;
                    n += 1;
                }
            }
            let avg = Rgba([(r / n) as u8, (g / n) as u8, (b / n) as u8, (a / n) as u8]);
            for y in by..by_end {
                for x in bx..bx_end {
                    img.put_pixel(x, y, avg);
                }
            }
            bx = bx_end;
        }
        by = by_end;
    }
}

fn branded(img: &mut RgbaImage, (x0, y0, x1, y1): (u32, u32, u32, u32), scale_factor: f32) {
    let block = block_px(scale_factor);
    let mut by = y0;
    let mut row = 0u32;
    while by < y1 {
        let by_end = (by + block).min(y1);
        let mut bx = x0;
        let mut col = 0u32;
        while bx < x1 {
            let bx_end = (bx + block).min(x1);
            // Deterministic palette pick from the block's grid position only —
            // never reads a source pixel, so the cover leaks zero content.
            let color = match (row.wrapping_add(col.wrapping_mul(3))) % 4 {
                0 => ACCENT,
                1 => ACCENT_DIM,
                _ => COVER_DARK,
            };
            for y in by..by_end {
                for x in bx..bx_end {
                    img.put_pixel(x, y, color);
                }
            }
            bx = bx_end;
            col += 1;
        }
        by = by_end;
        row += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid_red(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]))
    }

    #[test]
    fn solid_fill_covers_region_and_leaves_outside_untouched() {
        let mut img = solid_red(20, 20);
        apply_redaction(
            &mut img,
            Region {
                x: 5,
                y: 5,
                w: 8,
                h: 8,
            },
            RedactionStyle::Solid,
            1.0,
        );
        assert_ne!(
            img.get_pixel(8, 8),
            &Rgba([255, 0, 0, 255]),
            "region must be covered"
        );
        assert_eq!(
            img.get_pixel(0, 0),
            &Rgba([255, 0, 0, 255]),
            "outside must be untouched"
        );
        assert_eq!(img.get_pixel(19, 19), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn solid_fill_is_warm_dark_grey_not_black() {
        // The solid cover must read as a deliberate dark-grey block, not a pure
        // black hole, and lean warm (R > B).
        let mut img = solid_red(10, 10);
        apply_redaction(
            &mut img,
            Region {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            RedactionStyle::Solid,
            1.0,
        );
        let p = img.get_pixel(5, 5).0;
        assert_ne!([p[0], p[1], p[2]], [0, 0, 0], "must not be pure black");
        assert!(p[0] < 80 && p[1] < 80 && p[2] < 80, "must stay dark: {p:?}");
        assert!(p[0] > p[2], "must lean warm (R > B): {p:?}");
        assert_eq!(p[3], 255, "must be fully opaque");
    }

    #[test]
    fn pixelate_changes_region_and_preserves_outside() {
        // Left half red, right half blue. Redact only an inner rect that straddles
        // the colour boundary so the block average differs from both source colours.
        let mut img = RgbaImage::from_fn(20, 20, |x, _| {
            if x < 10 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        apply_redaction(
            &mut img,
            Region {
                x: 4,
                y: 4,
                w: 12,
                h: 12,
            },
            RedactionStyle::Pixelated,
            1.0,
        );
        let p = img.get_pixel(8, 8);
        assert_ne!(p, &Rgba([255, 0, 0, 255]));
        assert_ne!(p, &Rgba([0, 0, 255, 255]));
        // Outside the redacted rect stays the original source colour.
        assert_eq!(
            img.get_pixel(0, 0),
            &Rgba([255, 0, 0, 255]),
            "outside untouched (red side)"
        );
        assert_eq!(
            img.get_pixel(19, 19),
            &Rgba([0, 0, 255, 255]),
            "outside untouched (blue side)"
        );
    }

    #[test]
    fn branded_is_independent_of_source_pixels() {
        let mut a = RgbaImage::from_pixel(20, 20, Rgba([255, 0, 0, 255]));
        let mut b = RgbaImage::from_pixel(20, 20, Rgba([0, 255, 0, 255]));
        let region = Region {
            x: 2,
            y: 2,
            w: 16,
            h: 16,
        };
        apply_redaction(&mut a, region, RedactionStyle::Branded, 1.0);
        apply_redaction(&mut b, region, RedactionStyle::Branded, 1.0);
        for y in 2..18u32 {
            for x in 2..18u32 {
                assert_eq!(
                    a.get_pixel(x, y),
                    b.get_pixel(x, y),
                    "branded cover must not depend on source"
                );
            }
        }
    }

    #[test]
    fn region_clamped_to_image_bounds_does_not_panic() {
        let mut img = solid_red(10, 10);
        apply_redaction(
            &mut img,
            Region {
                x: -4,
                y: -4,
                w: 40,
                h: 40,
            },
            RedactionStyle::Solid,
            1.0,
        );
        assert_ne!(img.get_pixel(0, 0), &Rgba([255, 0, 0, 255]));
    }

    #[test]
    fn pixelate_block_size_scales_with_scale_factor() {
        // At scale 2.0 the block is 24px, so a single 24x24 region is one block:
        // the whole region becomes one flat averaged colour.
        let mut img = RgbaImage::from_fn(24, 24, |x, _| {
            if x < 12 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 255])
            }
        });
        apply_redaction(
            &mut img,
            Region {
                x: 0,
                y: 0,
                w: 24,
                h: 24,
            },
            RedactionStyle::Pixelated,
            2.0,
        );
        let corner = *img.get_pixel(0, 0);
        let opposite = *img.get_pixel(23, 23);
        assert_eq!(
            corner, opposite,
            "one 24px block at scale 2.0 → uniform fill"
        );
    }
}
