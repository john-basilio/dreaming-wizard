// SPDX-License-Identifier: AGPL-3.0-or-later

//! Persistent, cross-launch application settings, read/written through
//! `cosmic_config` (not to be confused with a saved *project* file — see
//! `components::project_data` for that). Most fields are edited on the
//! Preferences page (`nav::settings`); each field falls back to its
//! `Default` when missing on disk, so adding fields is backward-compatible
//! without a version bump.

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// App-wide settings that persist between runs via `cosmic_config`.
#[derive(Debug, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Path to the most recently saved/loaded project *directory* (a
    /// pre-split value pointing at its `project.json` file is tolerated —
    /// see `AppModel::project_dir`). `None` until the first successful
    /// save or load, and cleared again if that path ever fails to load.
    /// Used to:
    /// - auto-load on the next startup (see `AppModel::init`), unless
    ///   `reopen_last_project` is off
    /// - target `FileMenuAction::Save`/autosave writes
    ///
    /// Load itself always prompts via the folder picker regardless.
    pub last_project_path: Option<String>,

    /// Periodically re-save the open project while it has unsaved changes
    /// (only once it has a path on disk — autosave never opens the
    /// name/location dialog). Off by default so nothing writes to disk
    /// without being asked to.
    pub autosave: bool,
    /// Minutes between autosave checks while `autosave` is on.
    pub autosave_interval_minutes: u32,

    /// Auto-load `last_project_path` on startup; off means every launch
    /// starts at the New Project dialog instead (the app never runs
    /// without a project).
    pub reopen_last_project: bool,

    /// Scroll-zoom speed on the canvas, in percent (10 = the 0.10 factor
    /// the canvas always used). Stored as an integer because
    /// `cosmic_config`'s derive wants `Eq`.
    pub zoom_sensitivity_percent: u32,

    /// How many lines a prose block shows in the node editor before
    /// ellipsizing, while not being edited.
    pub preview_lines: u32,

    /// Whether deleting each kind of thing asks for confirmation first.
    /// Blocks default to no dialog (they always deleted instantly);
    /// nodes/characters default to asking (they always did).
    pub confirm_delete_nodes: bool,
    pub confirm_delete_characters: bool,
    pub confirm_delete_blocks: bool,

    /// UI language override as a language identifier (e.g. `"en"`);
    /// `None` follows the system locale. Applied at startup in `main` and
    /// live when changed on the Preferences page.
    pub language: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            last_project_path: None,
            autosave: false,
            autosave_interval_minutes: 5,
            reopen_last_project: true,
            zoom_sensitivity_percent: 10,
            preview_lines: 6,
            confirm_delete_nodes: true,
            confirm_delete_characters: true,
            confirm_delete_blocks: false,
            language: None,
        }
    }
}

impl Config {
    /// `zoom_sensitivity_percent` as the multiplier the canvas actually
    /// uses.
    pub fn zoom_sensitivity(&self) -> f32 {
        self.zoom_sensitivity_percent as f32 / 100.0
    }
}
