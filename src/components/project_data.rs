//! The Project Data file contains the core data that is serialized
//! and deserialized by the app.

use serde::{Serialize, Deserialize};
use crate::components::StoryNode;

#[derive(Debug, Serialize, Deserialize, Default)]
/// The core struct holding the all the JSON serializable data.
pub struct ProjectFile {
    pub metadata: ProjectData,
    pub canvas: CanvasData,
}

impl ProjectFile {
    /// Bundles the current canvas state (`nodes`, camera position) with
    /// `metadata` into the on-disk JSON shape. Called from `app.rs`'s
    /// `FileMenuAction::Save` handler; see `ProjectFile` load/save round-trip
    /// there for the counterpart `serde_json::from_str`/`to_writer_pretty`.
    pub fn new(
        nodes: Vec<StoryNode>,
        last_camera: (f32, f32),
        metadata: ProjectData,
    ) -> Self {
        Self {
            metadata,
            canvas: CanvasData { last_camera, nodes },
        }
    }
}

/// User/project-facing metadata, separate from the canvas contents so it
/// can be shown/edited (project name, author, etc.) without touching node
/// data.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectData {
    pub name: String,
    pub author: String,
    pub comment: String,
    pub app_version: String, // App version that last saved the file
    pub created_at: String, // ISO 8601 timestamp
    pub updated_at: String, // Updates every save.
}

/// The canvas's own serializable state: every node plus the camera position
/// it was last viewed at, so re-loading a project restores the view too.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CanvasData {
    pub last_camera: (f32, f32), // Last camera X Y
    pub nodes: Vec<StoryNode>,
}

// pub struct CharactersData {

// }