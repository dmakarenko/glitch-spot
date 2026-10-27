// ABOUTME: The snapshot JSON that a paired iPhone seals (the Electron shape plus device context).
// ABOUTME: Converts it into a Desktop BuiltSnapshot with a Desktop-made id and re-applied redactions.

use std::path::Path;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::RgbaImage;
use serde::Deserialize;

use crate::settings::RedactionStyle;
use crate::snapshot::build::{format_label, BuiltSnapshot};
use crate::snapshot::redact::{self, Region};
use crate::snapshot::{
    annotated_png_filename, clean_png_filename, default_agent_prompt, new_id, Annotation, AppInfo,
    Context, Coords, DeviceInfo, Images, OsInfo, RedactionRegion, ScreenInfo, Snapshot,
};

pub const VERSION: u32 = 1;
/// Largest width or height of a received image, checked before the pixels are decoded.
pub const MAX_IMAGE_SIDE: u32 = 4096;
/// The pixelated redaction block grows with the scale; a cap keeps its arithmetic in range.
const MAX_SCALE_FACTOR: f32 = 10.0;

/// One upload from a Sender: the snapshot plus the id that dedups a retried upload.
#[derive(Debug, Deserialize)]
pub struct Delivery {
    pub v: u32,
    pub delivery_id: uuid::Uuid,
    pub snapshot: WireSnapshot,
}

/// The sender's `id`, `mode`, `shape`, and `label` are not read: the Desktop app sets them.
#[derive(Debug, Deserialize)]
pub struct WireSnapshot {
    pub created_at: String,
    pub annotations: Vec<WireAnnotation>,
    #[serde(default)]
    pub redactions: Vec<RedactionRegion>,
    pub context: WireContext,
    pub images: WireImages,
}

#[derive(Debug, Deserialize)]
pub struct WireAnnotation {
    pub id: String,
    pub coords: Coords,
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct WireContext {
    pub screen: WireScreen,
    pub os: OsInfo,
    pub app: AppInfo,
    pub device: DeviceInfo,
}

#[derive(Debug, Deserialize)]
pub struct WireScreen {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
}

#[derive(Debug, Deserialize)]
pub struct WireImages {
    pub clean_base64: String,
    pub annotated_base64: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("delivery json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown delivery version {0}")]
    UnknownVersion(u32),
    #[error("image base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("image: {0}")]
    Image(#[from] image::ImageError),
    #[error("encode png: {0}")]
    Encode(#[from] crate::capture::CaptureError),
    #[error("scale_factor is not in (0, {MAX_SCALE_FACTOR}]")]
    BadScale,
    #[error("the clean and annotated images differ in size")]
    ImageSizeMismatch,
    #[error("a redaction is not a non-empty rectangle inside the image")]
    BadRedaction,
}

impl Delivery {
    pub fn from_slice(bytes: &[u8]) -> Result<Self, WireError> {
        let delivery: Delivery = serde_json::from_slice(bytes)?;
        if delivery.v != VERSION {
            return Err(WireError::UnknownVersion(delivery.v));
        }
        Ok(delivery)
    }

    /// Builds the Desktop snapshot under `{base_dir}/{new id}/`, ready for `persist::write`.
    pub fn into_built(
        self,
        base_dir: &Path,
        redaction_style: RedactionStyle,
    ) -> Result<BuiltSnapshot, WireError> {
        let wire = self.snapshot;
        let scale = wire.context.screen.scale_factor;
        if !(scale > 0.0 && scale <= MAX_SCALE_FACTOR) {
            return Err(WireError::BadScale);
        }

        let mut clean = decode_png(&wire.images.clean_base64)?;
        let mut annotated = decode_png(&wire.images.annotated_base64)?;
        if clean.dimensions() != annotated.dimensions() {
            return Err(WireError::ImageSizeMismatch);
        }
        let (width, height) = (clean.width() as f32, clean.height() as f32);
        let inside = |r: &RedactionRegion| {
            r.x >= 0.0
                && r.y >= 0.0
                && r.w > 0.0
                && r.h > 0.0
                && r.x + r.w <= width
                && r.y + r.h <= height
        };
        if !wire.redactions.iter().all(inside) {
            return Err(WireError::BadRedaction);
        }
        for r in &wire.redactions {
            let region = Region {
                x: r.x.round() as i32,
                y: r.y.round() as i32,
                w: r.w.round() as i32,
                h: r.h.round() as i32,
            };
            redact::apply_redaction(&mut clean, region, redaction_style, scale);
            redact::apply_redaction(&mut annotated, region, redaction_style, scale);
        }

        let id = new_id();
        let dir = base_dir.join(&id);
        let annotations = wire
            .annotations
            .into_iter()
            .enumerate()
            .map(|(idx, a)| Annotation {
                id: a.id,
                shape: "rect".into(),
                coords: a.coords,
                note: a.note,
                label: format_label(idx),
            })
            .collect();

        let snapshot = Snapshot {
            images: Images {
                clean: dir
                    .join(clean_png_filename(&id))
                    .to_string_lossy()
                    .into_owned(),
                annotated: Some(
                    dir.join(annotated_png_filename(&id))
                        .to_string_lossy()
                        .into_owned(),
                ),
            },
            id,
            created_at: wire.created_at,
            mode: "frozen".into(),
            annotations,
            redactions: wire.redactions,
            context: Context {
                screen: ScreenInfo {
                    width: wire.context.screen.width,
                    height: wire.context.screen.height,
                    scale_factor: scale,
                    display_index: 0,
                },
                os: OsInfo {
                    platform: "iOS".into(),
                    version: wire.context.os.version,
                },
                window: None,
                app: Some(wire.context.app),
                device: Some(wire.context.device),
            },
            agent_prompt: default_agent_prompt(),
        };
        Ok(BuiltSnapshot {
            snapshot,
            clean_bytes: crate::capture::encode_png(&clean)?,
            annotated_bytes: crate::capture::encode_png(&annotated)?,
        })
    }
}

fn decode_png(encoded: &str) -> Result<RgbaImage, WireError> {
    let bytes = STANDARD.decode(encoded)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_SIDE);
    limits.max_image_height = Some(MAX_IMAGE_SIDE);
    let mut reader =
        image::ImageReader::with_format(std::io::Cursor::new(bytes), image::ImageFormat::Png);
    reader.limits(limits);
    Ok(reader.decode()?.to_rgba8())
}
