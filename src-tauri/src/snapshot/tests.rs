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
