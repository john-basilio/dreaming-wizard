// SPDX-License-Identifier: AGPL-3.0-or-later

//! Persistent, cross-launch application settings, read/written through
//! `cosmic_config` (not to be confused with a saved *project* file — see
//! `components::project_data` for that).

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// App-wide settings that persist between runs via `cosmic_config`.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Path to the most recently saved/loaded project file. `None` until
    /// the first successful save or load, and cleared again if that path
    /// ever fails to load. Used to auto-load on the next startup (see
    /// `AppModel::init`) — Save/Load themselves always prompt via the file
    /// picker now, rather than defaulting to this path silently.
    pub last_project_path: Option<String>,

    /// Starting directory for the Save file picker (see `AppModel`'s
    /// `FileMenuAction::Save` handler). Hardcoded to a mock project folder
    /// for now, as a single source of truth other components can read
    /// instead of each hardcoding their own copy of the path — a stand-in
    /// until the app grows real per-project directory support (see the
    /// planned `.fizz` export/import format, which moves native storage to
    /// a project directory rather than one JSON file).
    pub project_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            last_project_path: None,
            project_dir: "/home/inuxiuz/Documents/new_project".to_string(),
        }
    }
}
