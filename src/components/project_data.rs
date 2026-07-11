//! The Project Data file contains the core data that is serialized
//! and deserialized by the app.

use serde::{Serialize, Deserialize};
use crate::components::StoryNode;

#[derive(Debug, Serialize, Deserialize,)]
/// The core struct holding the all the saveable data.
/// Serialize this to JSON instead.
pub struct ProjectData {
    pub canvas: CanvasData,
}
impl ProjectData {
    pub fn new(
        nodes: Vec<StoryNode>
    ) -> Self {
        Self { 
            canvas: CanvasData { nodes },
        }
    }
}

#[derive(Debug, Serialize, Deserialize,)]
pub struct CanvasData {
    pub nodes: Vec<StoryNode>,
}