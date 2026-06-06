// ABOUTME: Snapshot data model + JSON serialization for the v0.2 save action.
// ABOUTME: Coords are physical pixels relative to the captured work-area image.

use serde::{Deserialize, Serialize};

pub fn default_agent_prompt() -> String {
    "Glitch Spot annotation snapshot. Each entry in 'annotations' is a labelled rectangle (by 'label': '01', '02', …) drawn over the captured screenshot. The 'note' field is the user's feedback for that region. 'images.annotated' is the screenshot with rectangles and badges rendered; 'images.clean' is the original. When addressing feedback, reference annotations by their label. Any entries in 'redactions' mark regions the user intentionally hid; their pixels are destroyed in both images — do not attempt to infer their contents.".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub id: String,
    pub created_at: String,
    pub mode: String,
    pub annotations: Vec<Annotation>,
    #[serde(default)]
    pub redactions: Vec<RedactionRegion>,
    pub context: Context,
    pub images: Images,
    #[serde(default = "default_agent_prompt")]
    pub agent_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Annotation {
    pub id: String,
    pub shape: String,
    pub coords: Coords,
    pub note: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coords {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

/// A user-redacted region, physical pixels relative to the captured image. The
/// pixels are destroyed in the saved images; these coordinates are recorded only
/// so an agent knows a region was intentionally hidden.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RedactionRegion {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowInfo {
    pub app_name: String,
    pub title: String,
    pub bounds: WindowBounds,
    pub window_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub screen: ScreenInfo,
    pub os: OsInfo,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub window: Option<WindowInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScreenInfo {
    pub width: u32,
    pub height: u32,
    pub scale_factor: f32,
    pub display_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OsInfo {
    pub platform: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Images {
    pub clean: String,
    pub annotated: Option<String>,
}

/// Renders a snapshot as a plain-text prompt ready to paste into a coding agent.
/// Includes instructional framing, each annotation with its label, note, and
/// physical-pixel coordinates, window context when available, and absolute
/// file paths for the annotated image, clean image, and snapshot JSON.
pub fn render_as_prompt(snap: &Snapshot) -> String {
    let mut out = String::new();

    out.push_str("The user annotated a screenshot with Glitch Spot. Review the following feedback:\n\n");

    // Context line
    if let Some(w) = &snap.context.window {
        out.push_str(&format!("Window: \"{}\" — \"{}\"\n", w.app_name, w.title));
    }
    out.push_str(&format!(
        "Captured: {} | Mode: {} | {} {} | Screen: {}×{} @{}x\n",
        snap.created_at,
        snap.mode,
        snap.context.os.platform,
        snap.context.os.version,
        snap.context.screen.width,
        snap.context.screen.height,
        snap.context.screen.scale_factor,
    ));

    // Annotations
    out.push('\n');
    let count = snap.annotations.len();
    if count == 0 {
        out.push_str("No annotations.\n");
    } else {
        out.push_str(&format!("Annotations ({count}):\n"));
        for a in &snap.annotations {
            let note = if a.note.trim().is_empty() { "(no note)" } else { a.note.trim() };
            out.push_str(&format!(
                "  [{}] {}\n       coords: x={} y={} w={} h={} (physical px)\n",
                a.label,
                note,
                a.coords.x as i64,
                a.coords.y as i64,
                a.coords.w as i64,
                a.coords.h as i64,
            ));
        }
    }

    // File paths
    out.push('\n');
    out.push_str("Files:\n");
    if let Some(annotated) = &snap.images.annotated {
        out.push_str(&format!("  Annotated image: {annotated}\n"));
    }
    out.push_str(&format!("  Clean screenshot: {}\n", snap.images.clean));
    let json_path = std::path::Path::new(&snap.images.clean)
        .parent()
        .map(|p| p.join(snapshot_json_filename(&snap.id)))
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| snapshot_json_filename(&snap.id));
    out.push_str(&format!("  Snapshot JSON:    {json_path}\n"));

    out.push_str("\nReference annotations by their label (e.g. [01]) when describing changes.");

    out
}

pub mod annotate;
pub mod build;
pub mod persist;
pub mod redact;

pub use build::BuiltSnapshot;

/// Returns a sortable snapshot identifier: `YYYY-MM-DD_HH-MM-SS_<12-char uuid>`.
/// Used as the on-disk folder name, the `id` field of [`Snapshot`], and the
/// `:id` segment of the share-server URL routes.
pub fn new_id() -> String {
    new_id_at(chrono::Utc::now())
}

pub fn new_id_at(now: chrono::DateTime<chrono::Utc>) -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    format!("{}_{}", now.format("%Y-%m-%d_%H-%M-%S"), &uuid[..12])
}

/// On-disk filename for the unannotated PNG inside `{base}/{id}/`.
pub fn clean_png_filename(id: &str) -> String {
    format!("{id}_clean.png")
}

/// On-disk filename for the annotated PNG inside `{base}/{id}/`.
pub fn annotated_png_filename(id: &str) -> String {
    format!("{id}_annotated.png")
}

/// On-disk filename for the serialised snapshot inside `{base}/{id}/`.
pub fn snapshot_json_filename(id: &str) -> String {
    format!("{id}.json")
}

#[cfg(test)]
mod tests;
