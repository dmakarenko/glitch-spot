// ABOUTME: Pure assembly of a Snapshot from an incoming frame + annotations.
// ABOUTME: Filesystem writes live in crate::snapshot::persist, not here.

use std::path::Path;

use chrono::Utc;

use crate::capture::CapturedFrame;
use crate::snapshot::annotate::{render_annotated_png, AnnotateError};
use crate::snapshot::{new_id, Annotation, Context, Coords, Images, OsInfo, ScreenInfo, Snapshot};

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
}

/// Assemble a `BuiltSnapshot` from the captured frame + incoming annotations.
/// Pure function — no filesystem writes. The embedded `images.{clean,annotated}`
/// paths are computed as `{base_dir}/{id}/clean.png` and `…/annotated.png`,
/// matching the layout that `persist::write` will materialise.
pub fn build(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
    base_dir: &Path,
) -> Result<BuiltSnapshot, BuildError> {
    let id = new_id();
    let dir = base_dir.join(&id);
    let clean_path = dir.join("clean.png");
    let annotated_path = dir.join("annotated.png");

    let scale = frame.scale_factor;
    let annotations: Vec<Annotation> = annotations
        .into_iter()
        .map(|a| Annotation {
            id: a.id,
            shape: "rect".into(),
            coords: Coords {
                x: a.x * scale,
                y: a.y * scale,
                w: a.w * scale,
                h: a.h * scale,
            },
            note: a.note,
        })
        .collect();

    let annotated_bytes = render_annotated_png(&frame.png, &annotations, frame.scale_factor)?;

    let snapshot = Snapshot {
        id,
        created_at: Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        mode: mode.to_string(),
        annotations,
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
        },
        images: Images {
            clean: clean_path.to_string_lossy().into_owned(),
            annotated: Some(annotated_path.to_string_lossy().into_owned()),
        },
    };
    Ok(BuiltSnapshot {
        snapshot,
        clean_bytes: frame.png.clone(),
        annotated_bytes,
    })
}
