//! The Project Data file contains the core data that is serialized
//! and deserialized by the app.

use std::path::Path;

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::components::StoryNode;

#[derive(Debug, Serialize, Deserialize, Default)]
/// The core struct holding all the JSON-serializable data.
pub struct ProjectFile {
    pub metadata: ProjectData,
    pub canvas: CanvasData,
    // `#[serde(default)]` so project files saved before this field existed
    // (missing `"characters"` entirely) still load instead of failing
    // deserialization outright.
    #[serde(default)]
    pub characters: CharactersData,
}

impl ProjectFile {
    /// Bundles the current canvas state (`nodes`, camera position) and
    /// character list with `metadata` into the on-disk JSON shape. Called
    /// from `app.rs`'s `FileMenuAction::Save` handler; see `ProjectFile`
    /// load/save round-trip there for the counterpart
    /// `serde_json::from_str`/`to_writer_pretty`.
    pub fn new(
        nodes: Vec<StoryNode>,
        last_camera: (f32, f32),
        metadata: ProjectData,
        characters: Vec<Character>,
    ) -> Self {
        Self {
            metadata,
            canvas: CanvasData { last_camera, nodes },
            characters: CharactersData { characters },
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
    /// Project repository URL (e.g. a GitHub link); purely informational.
    /// `#[serde(default)]` so project files saved before this field
    /// existed still load.
    #[serde(default)]
    pub repository: String,
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

/// The characters page's own serializable state: every character entry.
/// Mirrors `CanvasData`'s shape so it can grow the same way (e.g. a
/// remembered scroll position) without changing `ProjectFile`.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CharactersData {
    pub characters: Vec<Character>,
}

/// A single character entry, rendered as a `character_card` in the UI and,
/// when selected, edited through a `CharacterCardEditor`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// Stable identity, independent of `name` — used to target the right
    /// entry in `CharactersPage::characters` from a click or an open editor.
    pub id: Uuid,
    pub name: String,
    /// Either the literal string `"default"` (use the built-in placeholder
    /// avatar icon) or a path to a `.png`/`.jpg`/`.jpeg` image file.
    /// Currently stored exactly as picked (typically absolute, from
    /// `CharacterCardEditor`'s file picker) and used as-is — there's no
    /// import step yet that copies the image into the project's own
    /// directory and rewrites this to a project-relative path, but that's
    /// the intended eventual behavior (see the planned `.fizz` export
    /// format, which moves native storage to a project directory).
    pub avatar: String,
    pub comment: String,
    pub description: String,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            name: String::new(),
            avatar: "default".to_string(),
            comment: String::new(),
            description: String::new(),
        }
    }
}

impl Character {
    /// `None` for the built-in placeholder avatar — `avatar` is `"default"`,
    /// empty, or not a `.png`/`.jpg`/`.jpeg` file (those are the only
    /// formats `character_card` knows how to render) — otherwise
    /// `Some(path)`, directly loadable as-is (see `avatar`'s doc comment).
    pub fn avatar_path(&self) -> Option<&str> {
        resolve_avatar_path(&self.avatar)
    }
}

/// Interprets the `Character::avatar` sentinel/path convention. Shared by
/// `Character::avatar_path` and `CharacterCardEditor`'s live avatar preview,
/// which only has the in-progress draft string (not a full `Character`) to
/// work from.
pub fn resolve_avatar_path(avatar: &str) -> Option<&str> {
    if avatar.is_empty() || avatar == "default" {
        return None;
    }

    let ext = Path::new(avatar).extension()?.to_str()?.to_ascii_lowercase();
    matches!(ext.as_str(), "png" | "jpg" | "jpeg").then_some(avatar)
}