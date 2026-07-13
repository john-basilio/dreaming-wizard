//! Reusable, page-agnostic building blocks: the `StoryNode` data model and
//! its on-canvas editor, the `Character` data model with its `character_card`
//! view helper and `CharacterCardEditor`, the generic `SimplePopup` modal,
//! the `SaveProjectDialog` used for a brand-new project's first save, the
//! `overlay` dimming-shade/toast helpers, small shared helpers, and the
//! project file shape used for JSON save/load. `nav::canvas`,
//! `nav::characters`, and `app` are the main consumers of these.

pub mod story_node;
pub use story_node::{StoryNode, NodePosition};

pub mod story_node_editor;
pub use story_node_editor::StoryNodeEditor;

pub mod dev_helper_fn;
pub use dev_helper_fn::{display_title};

pub mod project_data;
pub use project_data::{ProjectFile, ProjectData, Character};

pub mod character_card;
pub use character_card::character_card;

pub mod character_card_editor;
pub use character_card_editor::CharacterCardEditor;

pub mod simple_popup;
pub use simple_popup::SimplePopup;

pub mod save_project_dialog;
pub use save_project_dialog::SaveProjectDialog;

pub mod overlay;