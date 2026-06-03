// ABOUTME: Renders rectangle borders + numbered pill badges onto a clone of the
// ABOUTME: clean captured PNG to produce the annotated PNG for snapshot.images.annotated.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{ImageError, Rgba, RgbaImage};
use imageproc::drawing::{
    draw_filled_circle_mut, draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut,
};
use imageproc::rect::Rect as IpRect;

use crate::snapshot::Annotation;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
const ACCENT: Rgba<u8> = Rgba([238, 102, 51, 255]);
const BADGE_TEXT: Rgba<u8> = Rgba([255, 255, 255, 255]);
const BORDER_WIDTH_CSS: f32 = 2.0;
// CSS-pixel pill geometry; matches src/overlay/annotate.ts.
const PILL_W_CSS: f32 = 26.0;
const PILL_H_CSS: f32 = 18.0;
const PILL_R_CSS: f32 = 9.0;
const BADGE_FONT_PX_CSS: f32 = 11.0;

// CSS-pixel caption geometry; mirrors src/overlay/annotate.ts.
#[allow(dead_code)]
const CAPTION_BG: Rgba<u8> = Rgba([22, 22, 22, 255]); // #161616
#[allow(dead_code)]
const CAPTION_BORDER: Rgba<u8> = Rgba([238, 102, 51, 255]); // accent, opaque
#[allow(dead_code)]
const CAPTION_TEXT: Rgba<u8> = Rgba([170, 170, 170, 255]); // #aaaaaa
#[allow(dead_code)]
const CAPTION_MAX_W_CSS: f32 = 240.0;
#[allow(dead_code)]
const CAPTION_OFFSET_CSS: f32 = 6.0;
#[allow(dead_code)]
const CAPTION_PAD_X_CSS: f32 = 8.0;
#[allow(dead_code)]
const CAPTION_PAD_Y_CSS: f32 = 6.0;
#[allow(dead_code)]
const CAPTION_RADIUS_CSS: f32 = 6.0;
#[allow(dead_code)]
const CAPTION_BORDER_W_CSS: f32 = 1.0;
#[allow(dead_code)]
const CAPTION_FONT_PX_CSS: f32 = 13.0;
#[allow(dead_code)]
const CAPTION_LINE_HEIGHT: f32 = 1.4;

#[derive(Debug, thiserror::Error)]
pub enum AnnotateError {
    #[error("image decode/encode: {0}")]
    Image(#[from] ImageError),
    #[error("bundled font failed to parse")]
    Font,
}

/// Decode the clean PNG, draw rectangle borders + numbered badges, re-encode.
///
/// `scale_factor` is the snapshot's `frame.scale_factor`. All CSS-pixel constants
/// are multiplied by it so the rendered geometry matches the live overlay's
/// physical-pixel rendering on the captured image.
pub fn render_annotated_png(
    clean_png: &[u8],
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<Vec<u8>, AnnotateError> {
    let mut img =
        image::load_from_memory_with_format(clean_png, image::ImageFormat::Png)?.to_rgba8();
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|_| AnnotateError::Font)?;

    let border_width = (BORDER_WIDTH_CSS * scale_factor).round().max(1.0) as i32;
    let pill_w = (PILL_W_CSS * scale_factor).round() as i32;
    let pill_h = (PILL_H_CSS * scale_factor).round() as i32;
    let pill_r = (PILL_R_CSS * scale_factor).round() as i32;
    let badge_scale = PxScale::from(BADGE_FONT_PX_CSS * scale_factor);

    for anno in annotations {
        draw_rect_border(&mut img, anno, border_width);
        draw_pill_badge(&mut img, anno, pill_w, pill_h, pill_r, &font, badge_scale);
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

fn draw_rect_border(img: &mut RgbaImage, anno: &Annotation, border_width: i32) {
    let x = anno.coords.x.round() as i32;
    let y = anno.coords.y.round() as i32;
    let w = anno.coords.w.round() as i32;
    let h = anno.coords.h.round() as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    // Stack concentric hollow rects to thicken the line without filling the rect.
    for i in 0..border_width {
        let rx = x + i;
        let ry = y + i;
        let rw = (w - 2 * i).max(1) as u32;
        let rh = (h - 2 * i).max(1) as u32;
        let rect = IpRect::at(rx, ry).of_size(rw, rh);
        draw_hollow_rect_mut(img, rect, ACCENT);
    }
}

fn draw_pill_badge(
    img: &mut RgbaImage,
    anno: &Annotation,
    pill_w: i32,
    pill_h: i32,
    pill_r: i32,
    font: &FontRef<'_>,
    scale: PxScale,
) {
    let rect_x = anno.coords.x.round() as i32;
    let rect_y = anno.coords.y.round() as i32;
    // Pill straddles the rect's top-left corner (half-outside on both axes).
    let pill_x = rect_x - pill_w / 2;
    let pill_y = rect_y - pill_h / 2;
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    // Skip if the pill would fall entirely off-canvas (imageproc handles partial
    // overflow on its own but a fully off-canvas draw is a no-op anyway).
    if pill_x + pill_w <= 0 || pill_y + pill_h <= 0 || pill_x >= img_w || pill_y >= img_h {
        return;
    }

    // Center rectangle between the two end caps (only when the pill is wider than
    // its height; for a perfectly circular pill there is no center band).
    let center_w = (pill_w - 2 * pill_r).max(0);
    if center_w > 0 {
        let center_rect =
            IpRect::at(pill_x + pill_r, pill_y).of_size(center_w as u32, pill_h as u32);
        draw_filled_rect_mut(img, center_rect, ACCENT);
    }

    let cap_y = pill_y + pill_h / 2;
    draw_filled_circle_mut(img, (pill_x + pill_r, cap_y), pill_r, ACCENT);
    draw_filled_circle_mut(img, (pill_x + pill_w - pill_r, cap_y), pill_r, ACCENT);

    // Center the label text in the pill.
    let scaled = font.as_scaled(scale);
    let text_w: f32 = anno
        .label
        .chars()
        .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
        .sum();
    let text_h = scaled.ascent() - scaled.descent();
    let text_x = (pill_x as f32 + (pill_w as f32 - text_w) / 2.0).round() as i32;
    let text_y = (pill_y as f32 + (pill_h as f32 - text_h) / 2.0).round() as i32;
    draw_text_mut(img, BADGE_TEXT, text_x, text_y, scale, font, &anno.label);
}

/// Greedy word-wrap `text` into lines no wider than `max_w` px under `font`/`scale`.
/// Explicit newlines split paragraphs; a single word wider than `max_w` is
/// hard-broken character-by-character so no line ever overflows.
#[allow(dead_code)]
fn wrap_text(text: &str, font: &FontRef<'_>, scale: PxScale, max_w: f32) -> Vec<String> {
    let scaled = font.as_scaled(scale);
    let char_w = |c: char| scaled.h_advance(scaled.scaled_glyph(c).id);
    let str_w = |s: &str| s.chars().map(char_w).sum::<f32>();

    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split(' ').filter(|w| !w.is_empty()) {
            if str_w(word) > max_w {
                // Word alone overflows: flush the current line, then hard-break it.
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let mut chunk = String::new();
                for ch in word.chars() {
                    if !chunk.is_empty() && str_w(&chunk) + char_w(ch) > max_w {
                        lines.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(ch);
                }
                line = chunk;
                continue;
            }
            let candidate = if line.is_empty() {
                word.to_string()
            } else {
                format!("{line} {word}")
            };
            if str_w(&candidate) > max_w && !line.is_empty() {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines
}

/// Fill a rounded rectangle by compositing axis-aligned bands and four corner
/// circles. `r` is clamped to half the smaller side. All pixels written opaque.
#[allow(dead_code)]
fn draw_filled_rounded_rect(
    img: &mut RgbaImage,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    r: i32,
    color: Rgba<u8>,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let r = r.min(w / 2).min(h / 2).max(0);
    if r == 0 {
        draw_filled_rect_mut(img, IpRect::at(x, y).of_size(w as u32, h as u32), color);
        return;
    }
    // Center band (full width, inset top/bottom by r).
    draw_filled_rect_mut(
        img,
        IpRect::at(x, y + r).of_size(w as u32, (h - 2 * r) as u32),
        color,
    );
    // Top and bottom bands (inset left/right by r).
    draw_filled_rect_mut(
        img,
        IpRect::at(x + r, y).of_size((w - 2 * r) as u32, r as u32),
        color,
    );
    draw_filled_rect_mut(
        img,
        IpRect::at(x + r, y + h - r).of_size((w - 2 * r) as u32, r as u32),
        color,
    );
    // Four corner circles (right/bottom centers nudged in by 1px to stay in bounds).
    draw_filled_circle_mut(img, (x + r, y + r), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + r), r, color);
    draw_filled_circle_mut(img, (x + r, y + h - r - 1), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + h - r - 1), r, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_font() -> FontRef<'static> {
        FontRef::try_from_slice(FONT_BYTES).unwrap()
    }

    fn line_width(line: &str, font: &FontRef<'_>, scale: PxScale) -> f32 {
        let scaled = font.as_scaled(scale);
        line.chars()
            .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
            .sum()
    }

    #[test]
    fn wraps_long_text_so_every_line_fits_max_width() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let max_w = 120.0;
        let text = "the quick brown fox jumps over the lazy dog repeatedly";
        let lines = wrap_text(text, &font, scale, max_w);
        assert!(lines.len() > 1, "expected multiple lines, got {lines:?}");
        for line in &lines {
            assert!(
                line_width(line, &font, scale) <= max_w,
                "line {line:?} exceeds max width {max_w}"
            );
        }
    }

    #[test]
    fn preserves_explicit_newlines() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let lines = wrap_text("first\nsecond", &font, scale, 1000.0);
        assert_eq!(lines, vec!["first".to_string(), "second".to_string()]);
    }

    #[test]
    fn hard_breaks_a_word_longer_than_max_width() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        let max_w = 40.0;
        let lines = wrap_text("supercalifragilistic", &font, scale, max_w);
        assert!(lines.len() > 1, "expected the long word to hard-break");
        for line in &lines {
            assert!(line_width(line, &font, scale) <= max_w);
        }
    }

    #[test]
    fn empty_text_yields_a_single_empty_line() {
        let font = test_font();
        let scale = PxScale::from(13.0);
        assert_eq!(wrap_text("", &font, scale, 100.0), vec!["".to_string()]);
    }
}
