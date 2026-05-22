// ABOUTME: Assembles a Snapshot from incoming annotation data plus the latest captured frame.
// ABOUTME: Writes the clean PNG to /tmp for v0.2; persistence to ~/.glitch-spot is v0.3 work.

use std::path::PathBuf;

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

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("annotate: {0}")]
    Annotate(#[from] AnnotateError),
    #[error("no captured frame available to save")]
    NoFrame,
}

/// Build a Snapshot, write the clean PNG to /tmp, and return the snapshot.
pub fn build_and_write(
    frame: &CapturedFrame,
    mode: &str,
    annotations: Vec<IncomingAnnotation>,
    os_platform: &str,
    os_version: &str,
) -> Result<Snapshot, BuildError> {
    let id = new_id();
    let clean_path = PathBuf::from(format!("/tmp/glitch-spot-{}.png", id));
    std::fs::write(&clean_path, &frame.png)?;

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

    let annotated_path = PathBuf::from(format!("/tmp/glitch-spot-{}-annotated.png", id));
    let annotated_bytes = render_annotated_png(&frame.png, &annotations, frame.scale_factor)?;
    std::fs::write(&annotated_path, &annotated_bytes)?;

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
    Ok(snapshot)
}
