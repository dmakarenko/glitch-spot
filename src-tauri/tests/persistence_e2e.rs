// ABOUTME: End-to-end integration test for the v0.3 snapshot persistence chain.
// ABOUTME: Walks build → persist::write → cleanup against a real filesystem.

use std::path::Path;
use std::time::{Duration, SystemTime};

use filetime::{set_file_mtime, FileTime};
use tempfile::TempDir;

use glitch_spot::snapshot::build::{build, IncomingAnnotation};
use glitch_spot::snapshot::{persist, Snapshot};
use glitch_spot::CapturedFrame;

fn solid_white_png(w: u32, h: u32) -> Vec<u8> {
    let img: image::RgbaImage =
        image::ImageBuffer::from_pixel(w, h, image::Rgba([255u8, 255, 255, 255]));
    let mut out = Vec::new();
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)
        .unwrap();
    out
}

fn fake_frame() -> CapturedFrame {
    CapturedFrame {
        width: 2,
        height: 2,
        scale_factor: 2.0,
        png: solid_white_png(2, 2),
        display_index: 0,
    }
}

#[test]
fn build_then_persist_writes_files_at_paths_in_snapshot_images() {
    let temp = TempDir::new().unwrap();
    let frame = fake_frame();
    let annotations = vec![IncomingAnnotation {
        id: "anno_1".into(),
        x: 0.0,
        y: 0.0,
        w: 1.0,
        h: 1.0,
        note: "e2e".into(),
        kind: "annotation".into(),
    }];

    let (built, _) = build(&frame, "frozen", annotations, "macos", "26.0", temp.path(), None, glitch_spot::settings::RedactionStyle::Branded).unwrap();
    persist::write(temp.path(), &built).unwrap();

    // The paths embedded in the snapshot must actually point to the files on disk.
    let clean = Path::new(&built.snapshot.images.clean);
    let annotated = Path::new(built.snapshot.images.annotated.as_deref().unwrap());
    assert!(
        clean.is_file(),
        "clean path from snapshot.images.clean must exist: {}",
        clean.display()
    );
    assert!(
        annotated.is_file(),
        "annotated path from snapshot.images.annotated must exist: {}",
        annotated.display()
    );

    // Bytes round-trip.
    let clean_bytes = std::fs::read(clean).unwrap();
    assert_eq!(clean_bytes, frame.png, "clean.png on disk must equal frame.png");
    let annotated_bytes = std::fs::read(annotated).unwrap();
    assert!(
        annotated_bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]),
        "annotated.png must be PNG-encoded"
    );

    // The serialised snapshot round-trips back to an equal Snapshot.
    let id = &built.snapshot.id;
    let json_path = temp
        .path()
        .join(id)
        .join(glitch_spot::snapshot::snapshot_json_filename(id));
    let parsed: Snapshot = serde_json::from_slice(&std::fs::read(&json_path).unwrap()).unwrap();
    assert_eq!(parsed, built.snapshot);
}

#[test]
fn cleanup_removes_a_persisted_snapshot_dir_after_24h() {
    let temp = TempDir::new().unwrap();
    let frame = fake_frame();
    let (built, _) = build(&frame, "frozen", vec![], "macos", "26.0", temp.path(), None, glitch_spot::settings::RedactionStyle::Branded).unwrap();
    persist::write(temp.path(), &built).unwrap();

    let snap_dir = temp.path().join(&built.snapshot.id);
    assert!(snap_dir.is_dir());

    // Backdate the dir mtime by 25h.
    let past = SystemTime::now() - Duration::from_secs(25 * 60 * 60);
    set_file_mtime(&snap_dir, FileTime::from_system_time(past)).unwrap();

    let removed = persist::cleanup(temp.path(), Duration::from_secs(24 * 60 * 60)).unwrap();
    assert_eq!(removed, 1);
    assert!(!snap_dir.exists());
}
