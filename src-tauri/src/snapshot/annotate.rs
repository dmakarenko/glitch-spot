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
