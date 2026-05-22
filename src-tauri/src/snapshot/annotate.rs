// ABOUTME: Renders rectangle borders + note text labels onto a clone of the captured
// ABOUTME: clean PNG to produce the v0.2 annotated PNG for snapshot.images.annotated.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use image::{ImageError, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect_mut, draw_hollow_rect_mut, draw_text_mut};
use imageproc::rect::Rect as IpRect;

use crate::snapshot::Annotation;

const FONT_BYTES: &[u8] = include_bytes!("../../assets/DejaVuSans.ttf");
const BORDER_COLOR: Rgba<u8> = Rgba([255, 59, 48, 255]);
const TEXT_COLOR: Rgba<u8> = Rgba([17, 17, 17, 255]);
const PANEL_BG: Rgba<u8> = Rgba([255, 255, 255, 245]);
const BORDER_WIDTH: i32 = 3;
const PANEL_PADDING: i32 = 8;
const PANEL_GAP: i32 = 6;
const BASE_FONT_PX: f32 = 24.0;

#[derive(Debug, thiserror::Error)]
pub enum AnnotateError {
    #[error("image decode/encode: {0}")]
    Image(#[from] ImageError),
    #[error("bundled font failed to parse")]
    Font,
}

/// Decode the clean PNG, draw rectangle borders + note panels, re-encode.
///
/// `scale_factor` is the snapshot's `frame.scale_factor`. We use it to keep the
/// rendered text roughly the same physical size across 1x and Retina displays:
/// the base size is calibrated for 2x, so at 1x we draw smaller.
pub fn render_annotated_png(
    clean_png: &[u8],
    annotations: &[Annotation],
    scale_factor: f32,
) -> Result<Vec<u8>, AnnotateError> {
    let mut img = image::load_from_memory_with_format(clean_png, image::ImageFormat::Png)?
        .to_rgba8();
    let font = FontRef::try_from_slice(FONT_BYTES).map_err(|_| AnnotateError::Font)?;
    let font_px = BASE_FONT_PX * (scale_factor / 2.0).max(0.5);
    let scale = PxScale::from(font_px);

    for anno in annotations {
        draw_rect_border(&mut img, anno);
        if !anno.note.is_empty() {
            draw_note_panel(&mut img, anno, &font, scale);
        }
    }

    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img).write_to(
        &mut std::io::Cursor::new(&mut out),
        image::ImageFormat::Png,
    )?;
    Ok(out)
}

fn draw_rect_border(img: &mut RgbaImage, anno: &Annotation) {
    let x = anno.coords.x.round() as i32;
    let y = anno.coords.y.round() as i32;
    let w = anno.coords.w.round() as i32;
    let h = anno.coords.h.round() as i32;
    if w <= 0 || h <= 0 {
        return;
    }
    // Stack BORDER_WIDTH concentric hollow rects to thicken the line without
    // drawing a fill over the underlying image.
    for i in 0..BORDER_WIDTH {
        let rx = x + i;
        let ry = y + i;
        let rw = (w - 2 * i).max(1) as u32;
        let rh = (h - 2 * i).max(1) as u32;
        let rect = IpRect::at(rx, ry).of_size(rw, rh);
        draw_hollow_rect_mut(img, rect, BORDER_COLOR);
    }
}

fn draw_note_panel(img: &mut RgbaImage, anno: &Annotation, font: &FontRef<'_>, scale: PxScale) {
    let lines: Vec<&str> = anno.note.lines().collect();
    if lines.is_empty() {
        return;
    }

    let scaled = font.as_scaled(scale);
    let line_h = (scaled.height() + scaled.line_gap()).round() as i32;
    let max_line_w = lines
        .iter()
        .map(|l| text_width(font, scale, l))
        .fold(0_i32, |acc, w| acc.max(w));

    let rect_x = anno.coords.x.round() as i32;
    let rect_y = anno.coords.y.round() as i32;
    let rect_h = anno.coords.h.round() as i32;
    let img_w = img.width() as i32;
    let img_h = img.height() as i32;

    let panel_w = (max_line_w + 2 * PANEL_PADDING).max(120);
    let panel_h = (line_h * lines.len() as i32) + 2 * PANEL_PADDING;

    let mut panel_x = rect_x;
    let mut panel_y = rect_y + rect_h + PANEL_GAP;
    if panel_y + panel_h > img_h {
        panel_y = (rect_y - panel_h - PANEL_GAP).max(0);
    }
    if panel_x + panel_w > img_w {
        panel_x = (img_w - panel_w).max(0);
    }
    if panel_x < 0 {
        panel_x = 0;
    }

    let pw = panel_w.min(img_w - panel_x).max(0) as u32;
    let ph = panel_h.min(img_h - panel_y).max(0) as u32;
    if pw > 0 && ph > 0 {
        let rect = IpRect::at(panel_x, panel_y).of_size(pw, ph);
        draw_filled_rect_mut(img, rect, PANEL_BG);
    }

    let text_x = panel_x + PANEL_PADDING;
    let mut text_y = panel_y + PANEL_PADDING;
    for line in lines {
        draw_text_mut(img, TEXT_COLOR, text_x, text_y, scale, font, line);
        text_y += line_h;
    }
}

fn text_width(font: &FontRef<'_>, scale: PxScale, s: &str) -> i32 {
    let scaled = font.as_scaled(scale);
    let mut w = 0.0_f32;
    for c in s.chars() {
        let g = scaled.scaled_glyph(c);
        w += scaled.h_advance(g.id);
    }
    w.ceil() as i32
}
