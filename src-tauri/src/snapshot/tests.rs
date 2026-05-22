// ABOUTME: Snapshot serde round-trip and shape contract tests.

use super::*;

fn sample_snapshot() -> Snapshot {
    Snapshot {
        id: "snap_test123".into(),
        created_at: "2026-05-22T14:32:11Z".into(),
        mode: "frozen".into(),
        annotations: vec![Annotation {
            id: "anno_1".into(),
            shape: "rect".into(),
            coords: Coords {
                x: 280.0,
                y: 620.0,
                w: 560.0,
                h: 120.0,
            },
            note: "make this smaller".into(),
        }],
        context: Context {
            screen: ScreenInfo {
                width: 2560,
                height: 1414,
                scale_factor: 2.0,
                display_index: 0,
            },
            os: OsInfo {
                platform: "macos".into(),
                version: "26.0".into(),
            },
        },
        images: Images {
            clean: "/tmp/glitch-spot-snap_test123.png".into(),
            annotated: None,
        },
    }
}

#[test]
fn round_trips_through_json() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn json_uses_snake_case_keys() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        json.contains("\"created_at\""),
        "expected snake_case created_at, got {json}"
    );
    assert!(json.contains("\"scale_factor\""));
    assert!(json.contains("\"display_index\""));
}

#[test]
fn annotated_serializes_as_null_when_none() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        json.contains("\"annotated\":null"),
        "expected explicit null for annotated, got {json}"
    );
}

#[test]
fn new_id_has_snap_prefix_and_reasonable_length() {
    let id = new_id();
    assert!(id.starts_with("snap_"));
    assert_eq!(id.len(), "snap_".len() + 12);
}

use crate::capture::CapturedFrame;
use crate::snapshot::build::{build_and_write, IncomingAnnotation};

fn fake_frame() -> CapturedFrame {
    // Real 2x2 white PNG produced via the image crate so the bytes pass strict
    // CRC checks when decoded by snapshot::annotate.
    let png = solid_white_png(2, 2);
    CapturedFrame {
        width: 2,
        height: 2,
        scale_factor: 2.0,
        png,
        display_index: 0,
    }
}

#[test]
fn build_scales_annotation_coords_by_scale_factor() {
    let snap = build_and_write(
        &fake_frame(),
        "frozen",
        vec![IncomingAnnotation {
            id: "anno_x".into(),
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            note: "hi".into(),
        }],
        "macos",
        "26.0",
    )
    .unwrap();
    assert_eq!(snap.annotations.len(), 1);
    let a = &snap.annotations[0];
    assert_eq!(a.coords.x, 20.0);
    assert_eq!(a.coords.y, 40.0);
    assert_eq!(a.coords.w, 60.0);
    assert_eq!(a.coords.h, 80.0);
}

#[test]
fn build_writes_png_to_tmp() {
    let snap = build_and_write(&fake_frame(), "live", vec![], "macos", "26.0").unwrap();
    let path = std::path::Path::new(&snap.images.clean);
    assert!(path.starts_with("/tmp/"));
    assert!(path.exists());
    let bytes = std::fs::read(path).unwrap();
    assert!(
        bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "must be a PNG"
    );
    // Cleanup so we don't accumulate /tmp files when re-running tests.
    let _ = std::fs::remove_file(path);
    if let Some(ann) = &snap.images.annotated {
        let _ = std::fs::remove_file(ann);
    }
}

#[test]
fn build_writes_annotated_png_alongside_clean() {
    let snap = build_and_write(&fake_frame(), "frozen", vec![], "macos", "26.0").unwrap();
    let annotated = snap
        .images
        .annotated
        .as_ref()
        .expect("annotated path must be set");
    let path = std::path::Path::new(annotated);
    assert!(path.exists(), "annotated PNG must be written to disk");
    assert!(
        annotated.ends_with("-annotated.png"),
        "annotated path must end with -annotated.png, got {annotated}"
    );
    let bytes = std::fs::read(path).unwrap();
    assert!(
        bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "annotated file must be a PNG"
    );
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(&snap.images.clean);
}

use crate::snapshot::annotate::render_annotated_png;
use image::Rgba;

fn solid_white_png(width: u32, height: u32) -> Vec<u8> {
    let img: image::RgbaImage =
        image::ImageBuffer::from_pixel(width, height, Rgba([255u8, 255, 255, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

#[test]
fn annotated_png_round_trips_with_no_annotations() {
    let clean = solid_white_png(32, 32);
    let bytes = render_annotated_png(&clean, &[], 2.0).unwrap();
    assert!(
        bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "must be a PNG"
    );
    let decoded = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(decoded.width(), 32);
    assert_eq!(decoded.height(), 32);
    assert_eq!(decoded.get_pixel(0, 0), &Rgba([255, 255, 255, 255]));
}

#[test]
fn annotated_png_draws_red_pixels_on_rect_border() {
    let clean = solid_white_png(64, 64);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 10.0,
            y: 10.0,
            w: 20.0,
            h: 20.0,
        },
        note: String::new(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    let top_left = img.get_pixel(10, 10);
    assert_eq!(
        top_left,
        &Rgba([255, 59, 48, 255]),
        "top-left of rect must be red, got {top_left:?}"
    );
    let interior = img.get_pixel(20, 20);
    assert_ne!(
        interior,
        &Rgba([255, 59, 48, 255]),
        "rect interior must not be filled, got {interior:?}"
    );
}

#[test]
fn annotated_png_renders_note_panel_for_non_empty_notes() {
    let clean = solid_white_png(200, 200);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 20.0,
            y: 20.0,
            w: 60.0,
            h: 30.0,
        },
        note: "hello".into(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Panel area sits at y >= rect.y + rect.h + gap.
    let panel_top_y: i32 = 20 + 30 + 6;
    let mut found_dark = false;
    'outer: for y in panel_top_y..(panel_top_y + 60).min(img.height() as i32) {
        for x in 20..(20 + 140).min(img.width() as i32) {
            let pix = img.get_pixel(x as u32, y as u32);
            if pix.0[0] < 80 && pix.0[1] < 80 && pix.0[2] < 80 {
                found_dark = true;
                break 'outer;
            }
        }
    }
    assert!(
        found_dark,
        "expected dark text pixels in the panel region below the rect"
    );
}

#[test]
fn annotated_png_caps_panel_width_for_long_notes() {
    // Wide rect (500 px) + a very long single-line note. Cap should keep the
    // panel from extending past 480 px from the rect's left edge.
    let clean = solid_white_png(600, 200);
    let long_note: String = "x".repeat(200);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 0.0,
            y: 10.0,
            w: 500.0,
            h: 30.0,
        },
        note: long_note,
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Panel sits at y = 10 + 30 + 6 = 46. Sample a y row clearly inside the panel.
    let panel_inside_y: u32 = 46 + 8 + 5;
    // At x = 485 (past the 480-px cap, still inside the image) the pixel must be
    // pure white — i.e. NOT covered by the panel fill (which writes alpha=245).
    let outside_panel = img.get_pixel(485, panel_inside_y);
    assert_eq!(
        outside_panel,
        &Rgba([255, 255, 255, 255]),
        "panel must not extend past the 480px cap, got {outside_panel:?} at (485, {panel_inside_y})"
    );
    // And the panel itself should exist somewhere left of the cap.
    let inside_panel = img.get_pixel(50, panel_inside_y);
    assert_eq!(
        inside_panel.0[3], 245,
        "expected panel-fill alpha at (50, {panel_inside_y}), got {inside_panel:?}"
    );
}

#[test]
fn annotated_png_skips_panel_for_empty_notes() {
    let clean = solid_white_png(200, 200);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 20.0,
            y: 20.0,
            w: 60.0,
            h: 30.0,
        },
        note: String::new(),
    }];
    let bytes = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Below the rect, in empty space — must still be white.
    let pix = img.get_pixel(40, (20 + 30 + 10) as u32);
    assert_eq!(
        pix,
        &Rgba([255, 255, 255, 255]),
        "empty note ⇒ no panel pixels, got {pix:?}"
    );
}
