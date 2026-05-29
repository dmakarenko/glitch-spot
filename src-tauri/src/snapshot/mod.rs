// ABOUTME: Snapshot data model + JSON serialization for the v0.2 save action.
// ABOUTME: Coords are physical pixels relative to the captured work-area image.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Snapshot {
    pub id: String,
    pub created_at: String,
    pub mode: String,
    pub annotations: Vec<Annotation>,
    pub context: Context,
    pub images: Images,
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
