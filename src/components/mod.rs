//! Reusable, page-agnostic building blocks: the `StoryNode` data model and
//! its on-canvas editor, small shared helpers, and the project file shape
//! used for JSON save/load. `nav::canvas` is the main consumer of these.

pub mod story_node;
pub use story_node::{StoryNode, NodePosition};

pub mod story_node_editor;
pub use story_node_editor::StoryNodeEditor;

pub mod dev_helper_fn;
pub use dev_helper_fn::{display_title};

pub mod project_data;
pub use project_data::{ProjectFile, ProjectData};