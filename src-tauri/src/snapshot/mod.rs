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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Coords {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub screen: ScreenInfo,
    pub os: OsInfo,
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

pub fn new_id() -> String {
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    format!("snap_{}", &uuid[..12])
}

#[cfg(test)]
mod tests;
