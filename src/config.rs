// SPDX-License-Identifier: AGPL-3.0-or-later

//! Persistent, cross-launch application settings, read/written through
//! `cosmic_config` (not to be confused with a saved *project* file — see
//! `components::project_data` for that).

use cosmic::cosmic_config::{self, CosmicConfigEntry, cosmic_config_derive::CosmicConfigEntry};

/// App-wide settings that persist between runs via `cosmic_config`.
#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct Config {
    /// Path to the most recently saved/loaded project file, so Save/Load
    /// default to it instead of the fixed fallback location. `None` until
    /// the first successful save or load, and cleared again if that path
    /// ever fails to load — see `AppModel`'s `FileMenuAction` handlers.
    pub last_project_path: Option<String>,
}
