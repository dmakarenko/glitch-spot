// ABOUTME: Pure assembly of a Snapshot from an incoming frame + annotations.
// ABOUTME: Filesystem writes live in crate::snapshot::persist, not here.

use std::path::Path;

use chrono::Utc;
use image::RgbaImage;

use crate::capture::CapturedFrame;
use crate::settings::RedactionStyle;
use crate::snapshot::annotate::{render_annotated_png, render_annotations_onto, AnnotateError};
use crate::snapshot::redact::{self, Region};
use crate::snapshot::{
    annotated_png_filename, clean_png_filename, default_agent_prompt, new_id, Annotation, Context,
    Coords, Images, OsInfo, RedactionRegion, ScreenInfo, Snapshot,
};

fn default_incoming_kind() -> String {
    "annotation".to_string()
}

/// One annotation as received from the React side. Coordinates are CSS pixels
/// relative to the overlay's content area; we multiply by `scale_factor` to
/// get physical pixels for the on-disk image.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct IncomingAnnotation {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub note: String,
    #[serde(default = "default_incoming_kind")]
    pub kind: String,
}

/// The output of a pure snapshot build: a `Snapshot` plus the PNG bytes whose
/// paths are recorded in `snapshot.images.{clean,annotated}`. Written to disk
/// by `crate::snapshot::persist::write`.
#[derive(Debug)]
pub struct BuiltSnapshot {
    pub snapshot: Snapshot,
    pub clean_bytes: Vec<u8>,
    pub annotated_bytes: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("annotate: {0}")]
    Annotate(#[from] AnnotateError),
    #[error("decode clean png: {0}")]
    Decode(#[from] image::ImageError),
    #[error("encode redacted png: {0}")]
    Encode(#[from] crate::capture::CaptureError),
    #[error("unknown annotation kind: {0}")]
    UnknownKind(String),
}

/// Derives a region's visible label from its array index. Zero-pads to two
/// characters minimum, expands further for 100+ regions, never truncates.
/// Mirrors the TypeScript `formatLabel` in `src/overlay/annotations/label.ts`.
pub fn format_label(idx: usize) -> String {
    let n = idx + 1;
    if n < 10 {
        format!("0{n}")
    } else {
        n.to_string()
    }
}

/// Assemble a `BuiltSnapshot` from the captured frame + incoming annotations.
/// Pure function — no filesystem writes. The embedded `images.{clean,annotated}`
/// paths are computed as `{base_dir}/{id}/clean.png` and `…/annotated.png`,
/// matching the layout that `persist::write` will materialise.
///
/// Also returns the composited annotated `RgbaImage` so a caller can copy it to
/// the clipboard without decoding `annotated_bytes` again.
///
/// When redactions are present, both `clean_bytes` and `annotated_bytes` derive
/// from the same redacted base image so covered pixels never survive in either.
/// With no redactions, `clean_bytes` is `frame.png.clone()` (fast path — no
/// second encode).
pub fn build(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
    base_dir: &Path,
    crop_origin: Option<(f32, f32)>,
    coord_scale: f32,
    redaction_style: RedactionStyle,
) -> Result<(BuiltSnapshot, RgbaImage), BuildError> {
    let id = new_id();
    let dir = base_dir.join(&id);
    let clean_path = dir.join(clean_png_filename(&id));
    let annotated_path = dir.join(annotated_png_filename(&id));

    let scale = coord_scale;
    let (crop_x, crop_y) = crop_origin.unwrap_or((0.0, 0.0));

    // Fail closed: an unrecognized kind (contract drift / frontend typo) must
    // error rather than silently routing a redaction into a visible annotation,
    // which would leave sensitive pixels uncovered in the clean image.
    for a in &annotations {
        if a.kind != "annotation" && a.kind != "redaction" {
            return Err(BuildError::UnknownKind(a.kind.clone()));
        }
    }

    // Split redactions out before labelling — redactions are not annotations and
    // must not consume label numbers.
    let (redaction_inc, annotation_inc): (Vec<IncomingAnnotation>, Vec<IncomingAnnotation>) =
        annotations.into_iter().partition(|a| a.kind == "redaction");

    let annotations: Vec<Annotation> = annotation_inc
        .into_iter()
        .enumerate()
        .map(|(idx, a)| Annotation {
            id: a.id,
            shape: "rect".into(),
            coords: Coords {
                x: a.x * scale - crop_x,
                y: a.y * scale - crop_y,
                w: a.w * scale,
                h: a.h * scale,
            },
            note: a.note,
            label: format_label(idx),
        })
        .collect();

    let redaction_regions: Vec<RedactionRegion> = redaction_inc
        .iter()
        .map(|a| RedactionRegion {
            x: a.x * scale - crop_x,
            y: a.y * scale - crop_y,
            w: a.w * scale,
            h: a.h * scale,
        })
        .collect();

    // The security core: both clean_bytes and annotated_image derive from the
    // SAME base. With no redactions, keep the fast path (clean is the raw frame,
    // no second encode). With redactions, decode -> cover -> that redacted image
    // becomes the clean PNG, and annotations render on a copy of it.
    let (clean_bytes, annotated_bytes, annotated_image) = if redaction_regions.is_empty() {
        let (annotated_bytes, annotated_image) =
            render_annotated_png(&frame.png, &annotations, scale)?;
        (frame.png.clone(), annotated_bytes, annotated_image)
    } else {
        let mut base =
            image::load_from_memory_with_format(&frame.png, image::ImageFormat::Png)?.to_rgba8();
        for r in &redaction_regions {
            redact::apply_redaction(
                &mut base,
                Region {
                    x: r.x.round() as i32,
                    y: r.y.round() as i32,
                    w: r.w.round() as i32,
                    h: r.h.round() as i32,
                },
                redaction_style,
                scale,
            );
        }
        let clean_bytes = crate::capture::encode_png(&base)?;
        let (annotated_bytes, annotated_image) =
            render_annotations_onto(base, &annotations, scale)?;
        (clean_bytes, annotated_bytes, annotated_image)
    };

    let snapshot = Snapshot {
        id,
        created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        mode: mode.to_string(),
        annotations,
        redactions: redaction_regions,
        context: Context {
            screen: ScreenInfo {
                width: frame.width,
                height: frame.height,
                scale_factor: frame.scale_factor,
                display_index: frame.display_index,
            },
            os: OsInfo {
                platform: os_platform.to_string(),
                version: os_version.to_string(),
            },
            window: None,
            app: None,
            device: None,
        },
        images: Images {
            clean: clean_path.to_string_lossy().into_owned(),
            annotated: Some(annotated_path.to_string_lossy().into_owned()),
        },
        agent_prompt: default_agent_prompt(),
    };
    Ok((
        BuiltSnapshot {
            snapshot,
            clean_bytes,
            annotated_bytes,
        },
        annotated_image,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capture::CapturedFrame;
    use crate::settings::RedactionStyle;
    use image::{Rgba, RgbaImage};

    fn red_frame_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([255, 0, 0, 255]));
        crate::capture::encode_png(&img).unwrap()
    }

    fn frame(w: u32, h: u32) -> CapturedFrame {
        CapturedFrame {
            width: w,
            height: h,
            scale_factor: 1.0,
            png: red_frame_png(w, h),
            display_index: 0,
        }
    }

    fn incoming(id: &str, kind: &str, x: f32, y: f32, w: f32, h: f32) -> IncomingAnnotation {
        IncomingAnnotation {
            id: id.into(),
            x,
            y,
            w,
            h,
            note: String::new(),
            kind: kind.into(),
        }
    }

    #[test]
    fn no_redactions_uses_fast_path_clean_equals_frame() {
        let f = frame(40, 40);
        let base = std::path::PathBuf::from("/tmp/gs-test-fast");
        let (built, _img) = build(
            &f,
            "frozen",
            vec![incoming("a", "annotation", 4.0, 4.0, 10.0, 10.0)],
            "macos",
            "14",
            &base,
            None,
            1.0,
            RedactionStyle::Branded,
        )
        .unwrap();
        assert_eq!(
            built.clean_bytes, f.png,
            "fast path must clone the original frame"
        );
        assert!(built.snapshot.redactions.is_empty());
        assert_eq!(built.snapshot.annotations.len(), 1);
    }

    #[test]
    fn redaction_destroys_pixels_in_clean_image() {
        let f = frame(40, 40);
        let base = std::path::PathBuf::from("/tmp/gs-test-redact");
        let (built, _img) = build(
            &f,
            "frozen",
            vec![incoming("r", "redaction", 5.0, 5.0, 20.0, 20.0)],
            "macos",
            "14",
            &base,
            None,
            1.0,
            RedactionStyle::Solid,
        )
        .unwrap();
        assert_ne!(
            built.clean_bytes, f.png,
            "clean image must be redacted, not the raw frame"
        );
        let clean = image::load_from_memory(&built.clean_bytes)
            .unwrap()
            .to_rgba8();
        assert_ne!(
            clean.get_pixel(10, 10),
            &Rgba([255, 0, 0, 255]),
            "region pixels covered"
        );
        assert_eq!(
            clean.get_pixel(38, 38),
            &Rgba([255, 0, 0, 255]),
            "outside untouched"
        );
        assert_eq!(built.snapshot.redactions.len(), 1);
        assert!(built.snapshot.annotations.is_empty());
        // The annotated image must ALSO be covered — it is built on the same
        // redacted base, not the raw frame. Guards against a future change that
        // renders annotations onto an un-redacted image.
        let annotated = image::load_from_memory(&built.annotated_bytes)
            .unwrap()
            .to_rgba8();
        assert_ne!(
            annotated.get_pixel(10, 10),
            &Rgba([255, 0, 0, 255]),
            "annotated region also covered"
        );
    }

    #[test]
    fn redactions_do_not_consume_annotation_labels() {
        let f = frame(60, 60);
        let base = std::path::PathBuf::from("/tmp/gs-test-labels");
        let (built, _img) = build(
            &f,
            "frozen",
            vec![
                incoming("a1", "annotation", 2.0, 2.0, 8.0, 8.0),
                incoming("r1", "redaction", 12.0, 2.0, 8.0, 8.0),
                incoming("a2", "annotation", 22.0, 2.0, 8.0, 8.0),
            ],
            "macos",
            "14",
            &base,
            None,
            1.0,
            RedactionStyle::Branded,
        )
        .unwrap();
        let labels: Vec<&str> = built
            .snapshot
            .annotations
            .iter()
            .map(|a| a.label.as_str())
            .collect();
        assert_eq!(labels, vec!["01", "02"]);
    }

    #[test]
    fn unknown_kind_fails_closed() {
        let f = frame(40, 40);
        let base = std::path::PathBuf::from("/tmp/gs-test-unknown");
        let err = build(
            &f,
            "frozen",
            vec![incoming("x", "redacton", 5.0, 5.0, 10.0, 10.0)],
            "macos",
            "14",
            &base,
            None,
            1.0,
            RedactionStyle::Solid,
        )
        .unwrap_err();
        assert!(
            matches!(err, BuildError::UnknownKind(_)),
            "expected UnknownKind, got: {err:?}"
        );
    }

    #[test]
    fn coord_scale_overrides_frame_scale_for_mapping() {
        // Frame metadata scale is 1.0 (an opened image), but coord_scale=2.0
        // maps an annotation at CSS (10,10,5,5) to image px (20,20,10,10).
        let f = frame(100, 100);
        let base = std::path::PathBuf::from("/tmp/gs-test-coordscale");
        let (built, _img) = build(
            &f,
            "image",
            vec![incoming("a", "annotation", 10.0, 10.0, 5.0, 5.0)],
            "macos",
            "14",
            &base,
            None,
            2.0,
            RedactionStyle::Branded,
        )
        .unwrap();
        let c = &built.snapshot.annotations[0].coords;
        assert_eq!((c.x, c.y, c.w, c.h), (20.0, 20.0, 10.0, 10.0));
    }
}
