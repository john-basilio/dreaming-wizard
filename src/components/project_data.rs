//! The on-disk shapes of a saved project, plus the `Character` data model.
//!
//! A project is a **directory**, not a single file:
//!
//! ```text
//! MyProject/
//!   project.toml       — metadata only (this file *is* `ProjectData`)
//!   canvas.json        — editor state: camera + node positions
//!   scripts/
//!     Node_title-3ac7bf12.json — one `ScriptFile` per story node
//!   characters/
//!     Char_name-90e10a34.json — one `Character` per file
//!   assets/images/     — imported avatar images (see `Character::avatar`)
//! ```
//!
//! Script/character filenames are recognizable but incidental: the encoded
//! name (spaces → `_`, truncated when long) plus the first UUID segment
//! for uniqueness (see `app::project_io::entry_file_stem`). Identity always
//! comes from the `id` *inside* the file — renaming a file changes nothing.
//!
//! The split keeps the manifest human-editable (TOML, diff-friendly) while
//! the machine-written, engine-consumable data stays JSON. Node positions
//! live in `canvas.json` rather than the script files so `scripts/` carries
//! *story* content only. Everything referenced between files uses stable
//! UUIDs, never paths.
//!
//! `ProjectFile` is the **legacy** single-`project.json` shape, kept so
//! projects saved before the split still load (they migrate to the
//! directory layout on their next save — see `app::project_io`).

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Serialize, Deserialize};
use uuid::Uuid;
use crate::components::StoryNode;
use crate::components::story_block::StoryBlock;
use crate::components::story_node::{NodePosition, NodeSize};

/// Legacy single-file project shape (`project.json`), pre-directory-split.
/// Load-only: nothing writes this anymore.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ProjectFile {
    pub metadata: ProjectData,
    pub canvas: CanvasData,
    // `#[serde(default)]` so project files saved before this field existed
    // (missing `"characters"` entirely) still load instead of failing
    // deserialization outright.
    #[serde(default)]
    pub characters: CharactersData,
}

/// The project manifest — exactly what `project.toml` holds. Pure metadata:
/// no nodes, no characters, no editor state.
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
    /// The story's entry point, by node id. `skip_serializing_if` because
    /// TOML has no `null` — an unset root is simply an absent key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_node: Option<Uuid>,
}

/// Legacy `project.json`'s canvas section (see `ProjectFile`).
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CanvasData {
    pub last_camera: (f32, f32), // Last camera X Y
    pub nodes: Vec<StoryNode>,
}

/// Legacy `project.json`'s characters section (see `ProjectFile`).
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CharactersData {
    pub characters: Vec<Character>,
}

/// `canvas.json`: editor-only state — the camera, plus where each node sits
/// on the canvas. Kept out of `scripts/` so those files carry story content
/// only; a script file with no layout entry (e.g. dropped in externally)
/// just spawns at the default position.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct CanvasLayout {
    pub last_camera: (f32, f32),
    /// `BTreeMap` (not `HashMap`) for a stable key order on disk, so
    /// re-saves don't shuffle the file and diffs stay readable.
    #[serde(default)]
    pub nodes: BTreeMap<Uuid, NodeLayout>,
}

/// One node's world-space placement within `CanvasLayout`. Position only:
/// nodes have a fixed in-app size (`NodeSize::default`), so none is stored.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLayout {
    pub position: NodePosition,
}

/// One `scripts/<uuid>.json` file: a story node's *content* — its layout
/// lives in `canvas.json` (see `CanvasLayout`).
#[derive(Debug, Serialize, Deserialize)]
pub struct ScriptFile {
    pub id: Uuid,
    pub title: String,
    #[serde(default)]
    pub blocks: Vec<StoryBlock>,
}

impl ScriptFile {
    pub fn from_node(node: &StoryNode) -> Self {
        Self { id: node.id, title: node.title.clone(), blocks: node.blocks.clone() }
    }

    /// Joins this script with its layout entry (or the default placement
    /// when none exists) back into a runtime `StoryNode`.
    pub fn into_node(self, layout: Option<&NodeLayout>) -> StoryNode {
        let position = layout.map_or_else(|| StoryNode::default().position, |l| l.position.clone());

        StoryNode { id: self.id, position, size: NodeSize::default(), title: self.title, blocks: self.blocks }
    }
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
    ///
    /// **In memory this is always absolute** (directly loadable by every
    /// render site); **on disk it's project-relative** whenever the image
    /// lives inside the project directory (picked images are copied into
    /// `assets/images/` at pick time — see `AppModel::import_avatar`).
    /// `avatar_for_disk`/`resolve_avatar_on_load` are the two directions
    /// of that mapping, applied only at the save/load boundary. A legacy
    /// absolute path outside the project keeps working as-is.
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

    /// The `avatar` value as written to this character's file: relative to
    /// `project_dir` when the image lives inside it, unchanged otherwise
    /// (sentinels and external absolute paths pass through).
    pub fn avatar_for_disk(&self, project_dir: &Path) -> String {
        if resolve_avatar_path(&self.avatar).is_none() {
            return self.avatar.clone();
        }

        Path::new(&self.avatar)
            .strip_prefix(project_dir)
            .map_or_else(|_| self.avatar.clone(), |rel| rel.display().to_string())
    }

    /// Rejoins a project-relative on-disk `avatar` with the project
    /// directory (the inverse of `avatar_for_disk`); sentinels and absolute
    /// paths pass through.
    pub fn resolve_avatar_on_load(&mut self, project_dir: &Path) {
        if resolve_avatar_path(&self.avatar).is_some() && Path::new(&self.avatar).is_relative() {
            self.avatar = project_dir.join(&self.avatar).display().to_string();
        }
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
