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
            label: "01".into(),
        }],
        redactions: vec![],
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
            window: None,
            app: None,
            device: None,
        },
        images: Images {
            clean: "/tmp/glitch-spot-snap_test123.png".into(),
            annotated: None,
        },
        agent_prompt: default_agent_prompt(),
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
fn annotation_serializes_with_label() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        json.contains("\"label\":\"01\""),
        "expected label field on annotation, got {json}"
    );
}

#[test]
fn agent_prompt_present_in_json() {
    let s = sample_snapshot();
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        json.contains("\"agent_prompt\""),
        "expected agent_prompt field in serialized JSON, got {json}"
    );
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
fn reveal_target_prefers_annotated_image() {
    let mut s = sample_snapshot();
    s.images.annotated = Some("/tmp/glitch-spot-snap_test123_annotated.png".into());
    assert_eq!(
        s.reveal_target(),
        "/tmp/glitch-spot-snap_test123_annotated.png"
    );
}

#[test]
fn reveal_target_falls_back_to_clean_when_no_annotated() {
    let s = sample_snapshot(); // annotated is None
    assert_eq!(s.reveal_target(), "/tmp/glitch-spot-snap_test123.png");
}

#[test]
fn new_id_is_sortable_timestamp_with_uuid_suffix() {
    let id = new_id();
    // `YYYY-MM-DD_HH-MM-SS_<12-char uuid>` = 19 + 1 + 12 = 32 chars.
    assert_eq!(id.len(), 32, "unexpected id length: {id}");
    let mut parts = id.splitn(3, '_');
    let date = parts.next().expect("date segment");
    let time = parts.next().expect("time segment");
    let uuid = parts.next().expect("uuid segment");
    assert_eq!(date.len(), 10, "date should be YYYY-MM-DD, got {date}");
    assert_eq!(time.len(), 8, "time should be HH-MM-SS, got {time}");
    assert_eq!(uuid.len(), 12, "uuid should be 12 chars, got {uuid}");
    assert!(date.chars().nth(4) == Some('-') && date.chars().nth(7) == Some('-'));
    assert!(time.chars().nth(2) == Some('-') && time.chars().nth(5) == Some('-'));
}

#[test]
fn new_id_at_renders_a_known_timestamp() {
    let t = chrono::DateTime::parse_from_rfc3339("2026-05-27T11:50:52Z")
        .unwrap()
        .with_timezone(&chrono::Utc);
    let id = new_id_at(t);
    assert!(id.starts_with("2026-05-27_11-50-52_"), "got {id}");
}

use crate::capture::CapturedFrame;
use crate::snapshot::build::{build, format_label, IncomingAnnotation};

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
    let temp = tempfile::TempDir::new().unwrap();
    let (built, _) = build(
        &fake_frame(),
        "frozen",
        vec![IncomingAnnotation {
            id: "anno_x".into(),
            x: 10.0,
            y: 20.0,
            w: 30.0,
            h: 40.0,
            note: "hi".into(),
            kind: "annotation".into(),
        }],
        "macos",
        "26.0",
        temp.path(),
        None,
        2.0,
        crate::settings::RedactionStyle::Branded,
    )
    .unwrap();
    assert_eq!(built.snapshot.annotations.len(), 1);
    let a = &built.snapshot.annotations[0];
    assert_eq!(a.coords.x, 20.0);
    assert_eq!(a.coords.y, 40.0);
    assert_eq!(a.coords.w, 60.0);
    assert_eq!(a.coords.h, 80.0);
}

#[test]
fn build_assigns_zero_padded_labels_from_array_position() {
    let temp = tempfile::TempDir::new().unwrap();
    let incoming = (0..11)
        .map(|i| IncomingAnnotation {
            id: format!("anno_{i}"),
            x: 0.0,
            y: 0.0,
            w: 1.0,
            h: 1.0,
            note: String::new(),
            kind: "annotation".into(),
        })
        .collect();
    let (built, _) = build(
        &fake_frame(),
        "frozen",
        incoming,
        "macos",
        "26.0",
        temp.path(),
        None,
        2.0,
        crate::settings::RedactionStyle::Branded,
    )
    .unwrap();
    let labels: Vec<String> = built
        .snapshot
        .annotations
        .iter()
        .map(|a| a.label.clone())
        .collect();
    assert_eq!(
        labels,
        vec!["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11",]
    );
}

#[test]
fn format_label_zero_pads_and_expands() {
    assert_eq!(format_label(0), "01");
    assert_eq!(format_label(8), "09");
    assert_eq!(format_label(9), "10");
    assert_eq!(format_label(98), "99");
    assert_eq!(format_label(99), "100");
    assert_eq!(format_label(998), "999");
}

#[test]
fn build_returns_paths_under_base_dir_and_bytes_match_frame() {
    let temp = tempfile::TempDir::new().unwrap();
    let frame = fake_frame();
    let (built, _) = build(
        &frame,
        "live",
        vec![],
        "macos",
        "26.0",
        temp.path(),
        None,
        2.0,
        crate::settings::RedactionStyle::Branded,
    )
    .unwrap();

    // Embedded paths point under `temp/{id}/...`
    let id = &built.snapshot.id;
    let expected_clean = temp.path().join(id).join(format!("{id}_clean.png"));
    let expected_annotated = temp.path().join(id).join(format!("{id}_annotated.png"));
    assert_eq!(
        std::path::Path::new(&built.snapshot.images.clean),
        expected_clean
    );
    assert_eq!(
        built
            .snapshot
            .images
            .annotated
            .as_deref()
            .map(std::path::Path::new),
        Some(expected_annotated.as_path())
    );

    // No files were actually written (build is pure).
    assert!(!expected_clean.exists());
    assert!(!expected_annotated.exists());

    // Returned bytes match the frame's PNG; annotated bytes start with PNG magic.
    assert_eq!(built.clean_bytes, frame.png);
    assert!(
        built.annotated_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "annotated_bytes must be PNG-encoded"
    );
}

use crate::snapshot::annotate::render_annotated_png;
use image::Rgba;

const ACCENT: Rgba<u8> = Rgba([238, 102, 51, 255]);

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
    let (bytes, _) = render_annotated_png(&clean, &[], 2.0).unwrap();
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
fn annotated_png_draws_accent_pixels_on_rect_border() {
    let clean = solid_white_png(128, 128);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 40.0,
            y: 40.0,
            w: 40.0,
            h: 40.0,
        },
        note: String::new(),
        label: "01".into(),
    }];
    let (bytes, _) = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    let top_left = img.get_pixel(40, 40);
    assert_eq!(
        top_left, &ACCENT,
        "top-left of rect must be the accent colour, got {top_left:?}"
    );
    let interior = img.get_pixel(60, 60);
    assert_ne!(
        interior, &ACCENT,
        "rect interior must not be filled, got {interior:?}"
    );
}

#[test]
fn annotated_png_draws_pill_badge_at_rect_top_left() {
    // Rect at (50,50). Pill (26x18 scaled by 2 → 52x36) is centered on top-left
    // corner, so its body spans roughly x in [24, 76] and y in [32, 68]. A point
    // a few pixels above the rect's top edge sits inside the badge.
    let clean = solid_white_png(160, 160);
    let annos = vec![Annotation {
        id: "anno_1".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 50.0,
            y: 50.0,
            w: 60.0,
            h: 60.0,
        },
        note: String::new(),
        label: "01".into(),
    }];
    let (bytes, _) = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // Sample a pixel that should sit inside the badge body (above the rect's top).
    let inside_badge = img.get_pixel(50, 40);
    assert_eq!(
        inside_badge, &ACCENT,
        "expected accent badge pixel above rect's top-left, got {inside_badge:?}"
    );
}

#[test]
fn annotated_png_draws_caption_below_rect_when_note_present() {
    // When an annotation has a non-blank note, a caption box is rendered just
    // below the rectangle. The pixel directly under the rect must not be white.
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
        note: "caption text".into(),
        label: "01".into(),
    }];
    let (bytes, _) = render_annotated_png(&clean, &annos, 2.0).unwrap();
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    // rect bottom at scale 2 = (20+30)*2 = 100, offset 6*2=12, so caption top ~112.
    // Sample at y=120 which should be inside the caption box.
    let caption_px = img.get_pixel(40, 120);
    assert!(
        caption_px[0] < 250,
        "expected a caption pixel below the rect, got {caption_px:?}"
    );
}

#[test]
fn build_crop_origin_translates_annotation_coords_into_window_local() {
    use crate::capture::CapturedFrame;
    use crate::snapshot::build::{build, IncomingAnnotation};
    use tempfile::TempDir;

    let frame = CapturedFrame {
        width: 800,
        height: 600,
        scale_factor: 2.0,
        png: solid_white_png(1, 1),
        display_index: 0,
    };
    let annotations = vec![IncomingAnnotation {
        id: "r1".into(),
        x: 100.0, // CSS px on the overlay
        y: 50.0,
        w: 80.0,
        h: 40.0,
        note: "n".into(),
        kind: "annotation".into(),
    }];

    let temp = TempDir::new().unwrap();
    // crop_origin in physical pixels: (60, 30)
    let (built, _) = build(
        &frame,
        "window",
        annotations,
        "macos",
        "26.0",
        temp.path(),
        Some((60.0, 30.0)),
        2.0,
        crate::settings::RedactionStyle::Branded,
    )
    .expect("build with crop_origin should succeed");
    let a = &built.snapshot.annotations[0];
    // x*scale - crop_x = 100*2 - 60 = 140
    // y*scale - crop_y = 50*2  - 30 = 70
    assert_eq!(a.coords.x, 140.0);
    assert_eq!(a.coords.y, 70.0);
    assert_eq!(a.coords.w, 160.0);
    assert_eq!(a.coords.h, 80.0);
}

#[test]
fn context_window_round_trips() {
    let s = Snapshot {
        id: "snap_test_w".into(),
        created_at: "2026-05-27T10:00:00Z".into(),
        mode: "window".into(),
        annotations: vec![],
        redactions: vec![],
        context: Context {
            screen: ScreenInfo {
                width: 1440,
                height: 900,
                scale_factor: 2.0,
                display_index: 0,
            },
            os: OsInfo {
                platform: "macos".into(),
                version: "26.0".into(),
            },
            window: Some(WindowInfo {
                app_name: "Safari".into(),
                title: "Reader — example.com".into(),
                bounds: WindowBounds {
                    x: 120,
                    y: 88,
                    w: 1440,
                    h: 900,
                },
                window_id: 12345,
            }),
            app: None,
            device: None,
        },
        images: Images {
            clean: "x".into(),
            annotated: None,
        },
        agent_prompt: default_agent_prompt(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

#[test]
fn render_as_prompt_contains_annotation_labels_and_notes() {
    let mut s = sample_snapshot();
    s.annotations.push(Annotation {
        id: "anno_2".into(),
        shape: "rect".into(),
        coords: Coords {
            x: 10.0,
            y: 20.0,
            w: 100.0,
            h: 50.0,
        },
        note: "increase contrast here".into(),
        label: "02".into(),
    });
    let prompt = render_as_prompt(&s);
    assert!(
        prompt.contains("[01]"),
        "should include label [01], got:\n{prompt}"
    );
    assert!(
        prompt.contains("make this smaller"),
        "should include first note, got:\n{prompt}"
    );
    assert!(
        prompt.contains("[02]"),
        "should include label [02], got:\n{prompt}"
    );
    assert!(
        prompt.contains("increase contrast here"),
        "should include second note, got:\n{prompt}"
    );
}

#[test]
fn render_as_prompt_contains_coords() {
    let s = sample_snapshot();
    let prompt = render_as_prompt(&s);
    // coords for the sample annotation: x=280 y=620 w=560 h=120
    assert!(
        prompt.contains("280"),
        "should include x coord, got:\n{prompt}"
    );
    assert!(
        prompt.contains("620"),
        "should include y coord, got:\n{prompt}"
    );
}

#[test]
fn render_as_prompt_contains_file_paths() {
    let s = sample_snapshot();
    let prompt = render_as_prompt(&s);
    assert!(
        prompt.contains("/tmp/glitch-spot-snap_test123.png"),
        "should include clean image path, got:\n{prompt}"
    );
    // JSON path derived: parent of clean + {id}.json
    assert!(
        prompt.contains("snap_test123.json"),
        "should include JSON file path, got:\n{prompt}"
    );
}

#[test]
fn render_as_prompt_includes_window_context_when_present() {
    let mut s = sample_snapshot();
    s.context.window = Some(WindowInfo {
        app_name: "Xcode".into(),
        title: "ContentView.swift".into(),
        bounds: WindowBounds {
            x: 0,
            y: 0,
            w: 1440,
            h: 900,
        },
        window_id: 42,
    });
    let prompt = render_as_prompt(&s);
    assert!(
        prompt.contains("Xcode"),
        "should include app name, got:\n{prompt}"
    );
    assert!(
        prompt.contains("ContentView.swift"),
        "should include window title, got:\n{prompt}"
    );
}

#[test]
fn render_as_prompt_no_window_skips_window_line() {
    let s = sample_snapshot(); // window: None
    let prompt = render_as_prompt(&s);
    assert!(
        !prompt.contains("Window:"),
        "should not have Window: line, got:\n{prompt}"
    );
}

#[test]
fn render_as_prompt_is_instructional() {
    let s = sample_snapshot();
    let prompt = render_as_prompt(&s);
    // Should contain some instruction-to-agent phrasing
    assert!(
        prompt.to_lowercase().contains("review") || prompt.to_lowercase().contains("feedback"),
        "prompt should have instructional tone, got:\n{prompt}"
    );
}

#[test]
fn context_window_omitted_when_none() {
    let mut s = sample_snapshot();
    s.context.window = None;
    let json = serde_json::to_string(&s).unwrap();
    assert!(
        !json.contains("\"window\""),
        "expected `window` key omitted, got: {json}"
    );
}

#[test]
fn snapshot_without_redactions_field_deserializes_to_empty() {
    // Older snapshots have no `redactions` key; serde(default) must fill it.
    let json = r#"{
        "id": "x", "created_at": "t", "mode": "frozen",
        "annotations": [],
        "context": {
            "screen": {"width": 1, "height": 1, "scale_factor": 1.0, "display_index": 0},
            "os": {"platform": "macos", "version": "14"}
        },
        "images": {"clean": "c", "annotated": "a"}
    }"#;
    let snap: crate::snapshot::Snapshot = serde_json::from_str(json).unwrap();
    assert!(snap.redactions.is_empty());
}

#[test]
fn context_app_and_device_round_trip() {
    let mut s = sample_snapshot();
    let mut metadata = serde_json::Map::new();
    metadata.insert("screen".into(), "Checkout".into());
    s.context.app = Some(AppInfo {
        name: "Ondera".into(),
        version: "2.4.0".into(),
        build: "512".into(),
        metadata: Some(metadata),
    });
    s.context.device = Some(DeviceInfo {
        model: "iPhone17,1".into(),
    });
    let json = serde_json::to_string(&s).unwrap();
    let back: Snapshot = serde_json::from_str(&json).unwrap();
    assert_eq!(s, back);
}

mod ios_contract {
    use image::{Rgba, RgbaImage};
    use serde_json::{json, Value};

    use crate::settings::RedactionStyle;
    use crate::snapshot::wire::{Delivery, WireError};
    use crate::snapshot::Snapshot;

    const OLD_SNAPSHOT: &str =
        include_str!("../../../tests/fixtures/ios-contract/snapshot-v0.10.2.json");
    const WIRE: &[u8] = include_bytes!("../../../tests/fixtures/ios-contract/wire-snapshot.json");

    fn png_base64(img: &RgbaImage) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(crate::capture::encode_png(img).unwrap())
    }

    fn delivery_json(image: &RgbaImage) -> Value {
        json!({
            "v": 1,
            "delivery_id": "6F1C2B8E-4D3A-4E5F-9A0B-1C2D3E4F5A6B",
            "snapshot": {
                "id": "anything",
                "created_at": "2026-10-27T16:30:00Z",
                "mode": "window",
                "annotations": [
                    {"id": "a1", "shape": "rect", "coords": {"x": 1, "y": 2, "w": 3, "h": 4}, "note": "first", "label": "07"},
                    {"id": "a2", "shape": "rect", "coords": {"x": 5, "y": 6, "w": 7, "h": 8}, "note": "second", "label": "07"}
                ],
                "redactions": [{"x": 16, "y": 16, "w": 16, "h": 16}],
                "context": {
                    "screen": {"width": 64, "height": 64, "scale_factor": 1},
                    "os": {"platform": "macos", "version": "18.6"},
                    "app": {"name": "Ondera", "version": "2.4.0", "build": "512"},
                    "device": {"model": "iPhone17,1"}
                },
                "images": {"clean_base64": png_base64(image), "annotated_base64": png_base64(image)}
            }
        })
    }

    fn gray_with_white_dot() -> RgbaImage {
        let mut img = RgbaImage::from_pixel(64, 64, Rgba([90, 90, 90, 255]));
        img.put_pixel(20, 20, Rgba([255, 255, 255, 255]));
        img
    }

    fn parse(value: &Value) -> Result<Delivery, WireError> {
        Delivery::from_slice(&serde_json::to_vec(value).unwrap())
    }

    #[test]
    fn old_snapshot_json_without_app_block_loads() {
        let original: Value = serde_json::from_str(OLD_SNAPSHOT).unwrap();
        assert!(original["context"].get("app").is_none());

        let snap: Snapshot = serde_json::from_str(OLD_SNAPSHOT).unwrap();
        assert_eq!(snap.context.app, None);
        assert_eq!(snap.context.device, None);
        assert_eq!(serde_json::to_value(&snap).unwrap(), original);
    }

    #[test]
    fn wire_id_never_reaches_a_path() {
        let raw: Value = serde_json::from_slice(WIRE).unwrap();
        assert_eq!(raw["snapshot"]["id"], "../../etc");

        let delivery = Delivery::from_slice(WIRE).unwrap();
        let base = tempfile::TempDir::new().unwrap();
        let built = delivery
            .into_built(base.path(), RedactionStyle::default())
            .unwrap();
        let id = &built.snapshot.id;

        assert_ne!(id, "../../etc");
        assert!(
            regex::Regex::new(r"^\d{4}-\d{2}-\d{2}_\d{2}-\d{2}-\d{2}_[0-9a-f]{12}$")
                .unwrap()
                .is_match(id),
            "{id}"
        );
        let dir = base.path().join(id);
        for path in [
            built.snapshot.images.clean.as_str(),
            built.snapshot.images.annotated.as_deref().unwrap(),
        ] {
            assert!(std::path::Path::new(path).starts_with(&dir), "{path}");
            assert!(!path.contains(".."), "{path}");
        }
    }

    #[test]
    fn converts_wire_fields_to_desktop_fields() {
        let built = parse(&delivery_json(&gray_with_white_dot()))
            .unwrap()
            .into_built(std::path::Path::new("/base"), RedactionStyle::default())
            .unwrap();
        let s = built.snapshot;

        assert_eq!(s.mode, "frozen");
        assert_eq!(s.created_at, "2026-10-27T16:30:00Z");
        assert_eq!(s.context.os.platform, "iOS");
        assert_eq!(s.context.os.version, "18.6");
        assert_eq!(s.context.screen.display_index, 0);
        assert_eq!(s.context.screen.width, 64);
        assert_eq!(s.context.window, None);
        assert_eq!(s.context.app.as_ref().unwrap().build, "512");
        assert_eq!(s.context.device.as_ref().unwrap().model, "iPhone17,1");
        let labels: Vec<&str> = s.annotations.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["01", "02"]);
        assert_eq!(s.annotations[1].note, "second");
        assert_eq!(s.annotations[1].coords.h, 8.0);
        assert_eq!(s.redactions.len(), 1);
    }

    #[test]
    fn reapplies_redactions_to_both_images() {
        let built = parse(&delivery_json(&gray_with_white_dot()))
            .unwrap()
            .into_built(std::path::Path::new("/base"), RedactionStyle::default())
            .unwrap();

        for bytes in [&built.clean_bytes, &built.annotated_bytes] {
            let img = image::load_from_memory(bytes).unwrap().to_rgba8();
            assert_ne!(*img.get_pixel(20, 20), Rgba([255, 255, 255, 255]));
            assert_eq!(*img.get_pixel(2, 2), Rgba([90, 90, 90, 255]));
        }
    }

    #[test]
    fn rejects_delivery_id_that_is_not_a_uuid() {
        let mut value = delivery_json(&gray_with_white_dot());
        value["delivery_id"] = "../../etc".into();
        assert!(matches!(parse(&value), Err(WireError::Json(_))));
    }

    #[test]
    fn rejects_unknown_payload_version() {
        let mut value = delivery_json(&gray_with_white_dot());
        value["v"] = 2.into();
        assert!(matches!(parse(&value), Err(WireError::UnknownVersion(2))));
    }

    fn build(value: &Value) -> Result<crate::snapshot::BuiltSnapshot, WireError> {
        parse(value)
            .unwrap()
            .into_built(std::path::Path::new("/base"), RedactionStyle::default())
    }

    #[test]
    fn rejects_redactions_outside_the_image() {
        let bad = [
            json!({"x": 1, "y": 1, "w": 3e9, "h": 4}),
            json!({"x": -1, "y": 1, "w": 4, "h": 4}),
            json!({"x": 60, "y": 1, "w": 5, "h": 4}),
            json!({"x": 1, "y": 1, "w": 0, "h": 4}),
            json!({"x": 1, "y": 1, "w": 4, "h": 1e39}),
        ];
        for region in bad {
            let mut value = delivery_json(&gray_with_white_dot());
            value["snapshot"]["redactions"] = json!([region.clone()]);
            assert!(
                matches!(build(&value), Err(WireError::BadRedaction)),
                "{region}"
            );
        }
    }

    #[test]
    fn rejects_scale_factor_out_of_range() {
        for scale in [json!(0), json!(-2), json!(1e39), json!(11)] {
            let mut value = delivery_json(&gray_with_white_dot());
            value["snapshot"]["context"]["screen"]["scale_factor"] = scale.clone();
            assert!(matches!(build(&value), Err(WireError::BadScale)), "{scale}");
        }
    }

    #[test]
    fn rejects_images_over_the_side_limit() {
        let side = crate::snapshot::wire::MAX_IMAGE_SIDE + 1;
        let wide = RgbaImage::from_pixel(side, 1, Rgba([0, 0, 0, 255]));
        let mut value = delivery_json(&gray_with_white_dot());
        value["snapshot"]["redactions"] = json!([]);
        value["snapshot"]["images"]["clean_base64"] = png_base64(&wide).into();
        assert!(matches!(build(&value), Err(WireError::Image(_))));
    }

    #[test]
    fn rejects_images_of_different_sizes() {
        let mut value = delivery_json(&gray_with_white_dot());
        let other = RgbaImage::from_pixel(32, 64, Rgba([0, 0, 0, 255]));
        value["snapshot"]["images"]["annotated_base64"] = png_base64(&other).into();
        assert!(matches!(build(&value), Err(WireError::ImageSizeMismatch)));
    }

    #[test]
    fn rejects_images_that_are_not_png() {
        let mut value = delivery_json(&gray_with_white_dot());
        value["snapshot"]["images"]["clean_base64"] = "bm90IGEgcG5n".into();
        let result = parse(&value)
            .unwrap()
            .into_built(std::path::Path::new("/base"), RedactionStyle::default());
        assert!(matches!(result, Err(WireError::Image(_))));
    }
}
