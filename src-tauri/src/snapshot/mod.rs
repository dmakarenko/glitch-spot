// ABOUTME: Snapshot data model + JSON serialization for the v0.2 save action.
// ABOUTME: Coords are physical pixels relative to the captured work-area image.

use serde::{Deserialize, Serialize};

pub fn default_agent_prompt() -> String {
    "Glitch Spot annotation snapshot. Each entry in 'annotations' is a labelled rectangle (by 'label': '01', '02', …) drawn over the captured screenshot. The 'note' field is the user's feedback for that region. 'images.annotated' is the screenshot with rectangles and badges rendered; 'images.clean' is the original. When addressing feedback, reference annotations by their label.".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub id: String,
    pub created_at: String,
    pub mode: String,
    pub annotations: Vec<Annotation>,
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

pub mod annotate;
pub mod build;
pub mod persist;

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
