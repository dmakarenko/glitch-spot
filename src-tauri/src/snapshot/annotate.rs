// ABOUTME: Renders rectangle borders, numbered pill badges, and note captions onto a clone of the
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
const CAPTION_BG: Rgba<u8> = Rgba([22, 22, 22, 255]); // #161616
const CAPTION_BORDER: Rgba<u8> = Rgba([238, 102, 51, 255]); // accent, opaque
const CAPTION_TEXT: Rgba<u8> = Rgba([170, 170, 170, 255]); // #aaaaaa
const CAPTION_MAX_W_CSS: f32 = 240.0;
const CAPTION_OFFSET_CSS: f32 = 6.0;
const CAPTION_PAD_X_CSS: f32 = 8.0;
const CAPTION_PAD_Y_CSS: f32 = 6.0;
const CAPTION_RADIUS_CSS: f32 = 6.0;
const CAPTION_BORDER_W_CSS: f32 = 1.0;
const CAPTION_FONT_PX_CSS: f32 = 13.0;
const CAPTION_LINE_HEIGHT: f32 = 1.4;

#[derive(Debug, thiserror::Error)]
pub enum AnnotateError {
    #[error("image decode/encode: {0}")]
    Image(#[from] ImageError),
    #[error("encode: {0}")]
    Encode(String),
    #[error("bundled font failed to parse")]
    Font,
}

/// Decode the clean PNG, draw rectangle borders + numbered badges + captions,
/// re-encode. Returns both the encoded PNG bytes (for the on-disk annotated
/// image) and the composited `RgbaImage`, so a caller copying to the clipboard
/// can reuse the raw pixels instead of decoding the PNG a second time.
///
/// Encoding goes through [`crate::capture::encode_png`] (fixed `Up` filter) to
/// avoid the default Adaptive filter's per-row 5-filter search, which costs
/// ~1.6s on a 23MP Retina frame.
///
/// `scale_factor` is the snapshot's `frame.scale_factor`. All CSS-pixel constants
/// are multiplied by it so the rendered geometry matches the live overlay's
/// physical-pixel rendering on the captured image.
pub fn render_annotated_png(
    clean_png: &[u8],
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<(Vec<u8>, RgbaImage), AnnotateError> {
    let img = image::load_from_memory_with_format(clean_png, image::ImageFormat::Png)?.to_rgba8();
    render_annotations_onto(img, annotations, scale_factor)
}

/// Draw rectangle borders, numbered badges, and captions onto an already-decoded
/// image, then encode. Used directly by the redaction path (which decodes once,
/// covers the redacted regions, and renders annotations on the result) so the
/// image is not decoded a second time.
pub fn render_annotations_onto(
    mut img: RgbaImage,
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<(Vec<u8>, RgbaImage), AnnotateError> {
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

    for anno in annotations {
        draw_caption(&mut img, anno, scale_factor, &font);
    }

    let png = crate::capture::encode_png(&img).map_err(|e| AnnotateError::Encode(e.to_string()))?;
    Ok((png, img))
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
    // Bands collapse to zero size when r hits half the smaller side; skip those
    // to avoid imageproc's "width must be strictly positive" panic.
    let center_h = h - 2 * r;
    let side_w = w - 2 * r;
    // Center band (full width, inset top/bottom by r).
    if center_h > 0 {
        draw_filled_rect_mut(
            img,
            IpRect::at(x, y + r).of_size(w as u32, center_h as u32),
            color,
        );
    }
    // Top and bottom bands (inset left/right by r).
    if side_w > 0 {
        draw_filled_rect_mut(
            img,
            IpRect::at(x + r, y).of_size(side_w as u32, r as u32),
            color,
        );
        draw_filled_rect_mut(
            img,
            IpRect::at(x + r, y + h - r).of_size(side_w as u32, r as u32),
            color,
        );
    }
    // Four corner circles (right/bottom centers nudged in by 1px to stay in bounds).
    draw_filled_circle_mut(img, (x + r, y + r), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + r), r, color);
    draw_filled_circle_mut(img, (x + r, y + h - r - 1), r, color);
    draw_filled_circle_mut(img, (x + w - r - 1, y + h - r - 1), r, color);
}

/// Draw the annotation's note as a word-wrapped caption box anchored below the
/// rectangle (flipped above on bottom-overflow, clamped fully on-canvas).
/// No-op when the note is blank.
fn draw_caption(img: &mut RgbaImage, anno: &Annotation, scale_factor: f32, font: &FontRef<'_>) {
    let note = anno.note.trim();
    if note.is_empty() {
        return;
    }

    let pad_x = (CAPTION_PAD_X_CSS * scale_factor).round() as i32;
    let pad_y = (CAPTION_PAD_Y_CSS * scale_factor).round() as i32;
    let radius = (CAPTION_RADIUS_CSS * scale_factor).round() as i32;
    let offset = (CAPTION_OFFSET_CSS * scale_factor).round() as i32;
    let border_w = (CAPTION_BORDER_W_CSS * scale_factor).round().max(1.0) as i32;
    let font_px = CAPTION_FONT_PX_CSS * scale_factor;
    let scale = PxScale::from(font_px);
    let line_h = (font_px * CAPTION_LINE_HEIGHT).round() as i32;
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;
    // Cap the caption width to the image so a narrow capture wraps the note to fit
    // rather than overflowing the right edge and clipping the text.
    let max_box_w = ((CAPTION_MAX_W_CSS * scale_factor).round() as i32).min(img_w);
    let text_max_w = (max_box_w - 2 * pad_x).max(1) as f32;

    let lines = wrap_text(note, font, scale, text_max_w);

    let scaled = font.as_scaled(scale);
    let widest = lines
        .iter()
        .map(|l| {
            l.chars()
                .map(|c| scaled.h_advance(scaled.scaled_glyph(c).id))
                .sum::<f32>()
        })
        .fold(0.0_f32, f32::max);

    let box_w = ((widest.ceil() as i32) + 2 * pad_x).min(max_box_w);
    let box_h = lines.len() as i32 * line_h + 2 * pad_y;

    let rect_x = anno.coords.x.round() as i32;
    let rect_y = anno.coords.y.round() as i32;
    let rect_h = anno.coords.h.round() as i32;

    // Below the rect; flip above if placing below overflows the bottom edge.
    let below_y = rect_y + rect_h + offset;
    let mut box_y = if below_y + box_h > img_h {
        rect_y - offset - box_h
    } else {
        below_y
    };
    box_y = box_y.clamp(0, (img_h - box_h).max(0));
    let box_x = rect_x.clamp(0, (img_w - box_w).max(0));

    // Border layer, then inset background fill -> clean 1px rounded border.
    draw_filled_rounded_rect(img, box_x, box_y, box_w, box_h, radius, CAPTION_BORDER);
    draw_filled_rounded_rect(
        img,
        box_x + border_w,
        box_y + border_w,
        box_w - 2 * border_w,
        box_h - 2 * border_w,
        (radius - border_w).max(0),
        CAPTION_BG,
    );

    for (i, line) in lines.iter().enumerate() {
        let tx = box_x + pad_x;
        let ty = box_y + pad_y + i as i32 * line_h;
        draw_text_mut(img, CAPTION_TEXT, tx, ty, scale, font, line);
    }
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

    #[test]
    fn rounded_rect_handles_even_dimensions_without_panicking() {
        let mut img = RgbaImage::from_pixel(40, 40, Rgba([255, 255, 255, 255]));
        // Even width and height with r == w/2 / h/2 would make a band zero-sized.
        draw_filled_rounded_rect(&mut img, 4, 4, 10, 4, 6, Rgba([0, 0, 0, 255]));
        draw_filled_rounded_rect(&mut img, 4, 20, 4, 10, 6, Rgba([0, 0, 0, 255]));
        // Center of the first box should be filled black.
        assert_eq!(img.get_pixel(9, 6), &Rgba([0, 0, 0, 255]));
    }

    use crate::snapshot::Coords;

    fn white_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([255, 255, 255, 255]));
        let mut out = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
            .unwrap();
        out
    }

    fn anno_with_note(note: &str) -> Annotation {
        Annotation {
            id: "a1".into(),
            shape: "rect".into(),
            coords: Coords {
                x: 40.0,
                y: 40.0,
                w: 120.0,
                h: 80.0,
            },
            note: note.into(),
            label: "01".into(),
        }
    }

    #[test]
    fn caption_draws_dark_pixels_below_the_rect_when_note_present() {
        let clean = white_png(400, 400);
        let (out, _) = render_annotated_png(&clean, &[anno_with_note("hello world")], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (400, 400), "dimensions must not change");
        // The caption box sits just below the rect (rect bottom = 40+80 = 120,
        // gap 6 -> box top ~126). Sample a pixel inside the box; it must be dark
        // (#161616 fill or accent border), not the white background.
        let px = img.get_pixel(50, 135);
        assert!(
            px[0] < 250,
            "expected a non-white caption pixel, got {px:?}"
        );
    }

    #[test]
    fn no_caption_when_note_is_blank() {
        let clean = white_png(400, 400);
        let (out, _) = render_annotated_png(&clean, &[anno_with_note("   ")], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        // Same sample point as above must stay white -- no caption box drawn.
        assert_eq!(img.get_pixel(50, 135), &Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn caption_flips_above_rect_near_bottom_edge() {
        let clean = white_png(200, 200);
        // Rect low on the canvas: below_y = 150 + 40 + 6 = 196; a one-line caption
        // box (~30px tall) would overflow 200, so the caption flips ABOVE the rect.
        let anno = Annotation {
            id: "a1".into(),
            shape: "rect".into(),
            coords: Coords {
                x: 40.0,
                y: 150.0,
                w: 120.0,
                h: 40.0,
            },
            note: "near the bottom".into(),
            label: "01".into(),
        };
        let (out, _) = render_annotated_png(&clean, &[anno], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();
        // box_h = round(13*1.4) + 2*round(6) = 18 + 12 = 30; flipped box_y = 150 - 6 - 30
        // = 114; box spans y[114,144), so (50,140) lands inside the flipped box.
        let px = img.get_pixel(50, 140);
        assert!(
            px[0] < 250,
            "expected flipped caption pixel above the rect, got {px:?}"
        );
    }

    #[test]
    fn caption_does_not_clip_against_a_narrow_image() {
        // A small rect near the top so the caption renders below it without flipping.
        // The note is long enough that, uncapped, the caption box would be wider than
        // this 130px image and get clipped at the right edge.
        let anno = Annotation {
            id: "a1".into(),
            shape: "rect".into(),
            coords: Coords {
                x: 10.0,
                y: 8.0,
                w: 40.0,
                h: 16.0,
            },
            note: "alpha bravo charlie delta echo foxtrot golf hotel india juliet".into(),
            label: "01".into(),
        };
        let w = 130u32;
        let (out, _) = render_annotated_png(&white_png(w, 300), &[anno], 1.0).unwrap();
        let img = image::load_from_memory(&out).unwrap().to_rgba8();

        // At the image's right edge, mid-caption, the box must stay on-canvas: the
        // pixel is either background (white) or the box's own border (accent, r >> b),
        // never the interior fill (#161616, r≈b) that a right-clipped box leaves behind.
        let px = img.get_pixel(w - 1, 55);
        let is_white = px[0] > 250 && px[1] > 250 && px[2] > 250;
        let is_border = px[0] as i32 > px[2] as i32 + 40;
        assert!(
            is_white || is_border,
            "caption reached the image's right edge as interior fill (clipped), got {px:?}"
        );
    }

    #[test]
    fn returned_image_matches_the_encoded_png() {
        // The clipboard path reuses the returned RgbaImage instead of decoding
        // the PNG again, so the two must be pixel-identical.
        let clean = white_png(120, 90);
        let (png, img) =
            render_annotated_png(&clean, &[anno_with_note("hello world")], 2.0).unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), decoded.dimensions());
        assert_eq!(
            img.as_raw(),
            decoded.as_raw(),
            "RGBA must equal the decoded PNG"
        );
    }
}
