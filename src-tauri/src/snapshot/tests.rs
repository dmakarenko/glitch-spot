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
    // A minimal valid PNG: 1x1 transparent pixel.
    let png: Vec<u8> = vec![
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
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
}
