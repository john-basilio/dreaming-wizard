//! Project directory I/O and the New/Load/Save orchestration that drives
//! it. Lives as a child module of `app` (rather than e.g.
//! `components::project_data`) because every operation here reaches into
//! `AppModel`'s canvas/characters/project-metadata state as a unit.
//!
//! A project is a directory (see `components::project_data`'s module doc
//! for the exact layout): `project.toml` (metadata + root node),
//! `canvas.json` (camera + node placement), and one JSON file per node/
//! character under `scripts/`/`characters/`. Loading treats the directory
//! as the source of truth (every parseable `scripts/*.json` is a node);
//! saving syncs it — writing current entries and deleting UUID-named files
//! whose entry no longer exists. Legacy single-`project.json` projects
//! still load and migrate to the directory layout on their next save.

use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use cosmic::iced::Vector;
use cosmic::Task;
use uuid::Uuid;

use crate::components::new_project_dialog::{NewProjectEvent, NewProjectMessage};
use crate::components::project_data::{CanvasLayout, NodeLayout, ScriptFile};
use crate::components::{Character, NewProjectDialog, ProjectData, ProjectFile, SimplePopup, StoryNode};
use crate::fl;
use crate::nav::{CanvasPage, CharactersPage};

use super::chrome::FileMenuAction;
use super::{AppModel, Message};

/// The manifest at a project directory's top level.
const MANIFEST_FILE: &str = "project.toml";
/// The pre-directory-split monolithic project file, accepted on load only.
const LEGACY_FILE: &str = "project.json";
/// Editor state (camera + node placement).
const CANVAS_FILE: &str = "canvas.json";
/// One story node per `<uuid>.json` inside this subdirectory.
const SCRIPTS_DIR: &str = "scripts";
/// One character per `<uuid>.json` inside this subdirectory.
const CHARACTERS_DIR: &str = "characters";

/// Why a project couldn't be loaded from a directory; lets
/// `handle_load_dir_picked` show a more specific `SimplePopup` message than
/// a bare `bool` would allow.
enum ProjectLoadError {
    /// The directory has neither a `project.toml` nor a legacy
    /// `project.json`.
    Missing,
    /// A project file exists but couldn't be read or parsed.
    Invalid,
}

/// Everything a successful load produces, in runtime shape (layout joined
/// into the nodes, avatars resolved to absolute paths).
struct LoadedProject {
    metadata: ProjectData,
    nodes: Vec<StoryNode>,
    characters: Vec<Character>,
    last_camera: (f32, f32),
}

/// Reads and parses (but doesn't apply) a project from `dir` — the new
/// directory layout when `project.toml` exists, the legacy single
/// `project.json` otherwise. See `AppModel::apply_project` for the other
/// half.
fn read_project_dir(dir: &Path) -> Result<LoadedProject, ProjectLoadError> {
    let manifest_path = dir.join(MANIFEST_FILE);
    if manifest_path.is_file() {
        return read_split_project(dir, &manifest_path);
    }

    // Legacy fallback: the whole project in one JSON file.
    let legacy_path = dir.join(LEGACY_FILE);
    if !legacy_path.is_file() {
        return Err(ProjectLoadError::Missing);
    }
    let contents = fs::read_to_string(&legacy_path).map_err(|_| ProjectLoadError::Invalid)?;
    let project: ProjectFile = serde_json::from_str(&contents).map_err(|_| ProjectLoadError::Invalid)?;

    let mut characters = project.characters.characters;
    for character in &mut characters {
        character.resolve_avatar_on_load(dir);
    }

    Ok(LoadedProject {
        metadata: project.metadata,
        nodes: project.canvas.nodes,
        characters,
        last_camera: project.canvas.last_camera,
    })
}

/// The directory-layout half of `read_project_dir`. Strict about parse
/// failures in `scripts/`/`characters/` (a silently skipped file would be
/// deleted as "stale" by the next save — better to refuse the load), but
/// lenient about `canvas.json`: layout is reconstructible, story isn't.
fn read_split_project(dir: &Path, manifest_path: &Path) -> Result<LoadedProject, ProjectLoadError> {
    let manifest = fs::read_to_string(manifest_path).map_err(|_| ProjectLoadError::Invalid)?;
    let metadata: ProjectData = toml::from_str(&manifest).map_err(|_| ProjectLoadError::Invalid)?;

    let layout: CanvasLayout = fs::read_to_string(dir.join(CANVAS_FILE))
        .ok()
        .and_then(|contents| serde_json::from_str(&contents).ok())
        .unwrap_or_default();

    let mut nodes = Vec::new();
    for script in read_entry_files::<ScriptFile>(&dir.join(SCRIPTS_DIR))? {
        let node_layout = layout.nodes.get(&script.id);
        nodes.push(script.into_node(node_layout));
    }

    let mut characters = read_entry_files::<Character>(&dir.join(CHARACTERS_DIR))?;
    for character in &mut characters {
        character.resolve_avatar_on_load(dir);
    }

    Ok(LoadedProject {
        metadata,
        nodes,
        characters,
        last_camera: layout.last_camera,
    })
}

/// Parses every `*.json` directly inside `dir` (a missing directory is just
/// an empty list; non-JSON files are ignored). Any JSON file that fails to
/// parse aborts the load — see `read_split_project` on why that's safer
/// than skipping it.
fn read_entry_files<T: serde::de::DeserializeOwned>(dir: &Path) -> Result<Vec<T>, ProjectLoadError> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Ok(Vec::new());
    };

    let mut items = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }

        let contents = fs::read_to_string(&path).map_err(|_| ProjectLoadError::Invalid)?;
        items.push(serde_json::from_str(&contents).map_err(|_| ProjectLoadError::Invalid)?);
    }
    Ok(items)
}

/// Writes `contents` to `path` via a sibling temp file + rename, so a crash
/// mid-write can't leave a half-written file where a good one was.
fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
    }
    fs::rename(&tmp, path)
}

/// Longest encoded name kept in a script/character filename before the
/// short-id suffix; anything longer is simply cut (see `entry_file_stem`).
const MAX_STEM_NAME: usize = 32;

/// A script/character file's stem: the entry's name — whitespace as `_`,
/// path-hostile characters dropped, cut at `MAX_STEM_NAME` — plus the first
/// segment of its UUID for uniqueness. `"Some really long title…"` with
/// uuid `3ac7bfe3-0496-…` becomes `Some_really_long_title…-3ac7bfe3`.
/// Recognizable, but never authoritative: identity is the `id` *inside*
/// the file.
fn entry_file_stem(name: &str, id: Uuid) -> String {
    let encoded: String = name.trim().chars()
        .map(|c| if c.is_whitespace() { '_' } else { c })
        .filter(|c| !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '.') && !c.is_control())
        .take(MAX_STEM_NAME)
        .collect();

    // The first hyphen-delimited segment of the UUID (8 hex chars).
    let short = &id.simple().to_string()[..8];

    if encoded.is_empty() {
        short.to_string()
    } else {
        format!("{encoded}-{short}")
    }
}

/// Whether a file stem looks like one this app wrote — `entry_file_stem`
/// output (`…-3ac7bfe3`, or a bare 8-hex short id for nameless entries) or
/// a full-UUID name from the first directory-format iteration. Only such
/// files are ever deleted by `sync_entry_dir`; anything else in the
/// directory isn't ours to touch.
fn is_managed_stem(stem: &str) -> bool {
    if stem.parse::<Uuid>().is_ok() {
        return true;
    }

    let bytes = stem.as_bytes();
    let short = match bytes.len() {
        8 => bytes,
        len if len > 9 && bytes[len - 9] == b'-' => &bytes[len - 8..],
        _ => return false,
    };
    short.iter().all(u8::is_ascii_hexdigit)
}

/// Syncs a `scripts/`-style directory to `entries` (`(filename, contents)`
/// pairs): writes every current file, then deletes managed JSON files (see
/// `is_managed_stem`) that aren't in the current set — covering both
/// entries deleted this session and files left under an old name after a
/// title/name change.
fn sync_entry_dir(dir: &Path, entries: &[(String, String)]) -> std::io::Result<()> {
    fs::create_dir_all(dir)?;

    for (filename, contents) in entries {
        write_atomic(&dir.join(filename), contents)?;
    }

    for entry in fs::read_dir(dir)?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let is_current = path.file_name().and_then(|name| name.to_str())
            .is_some_and(|name| entries.iter().any(|(filename, _)| filename == name));
        let is_managed = path.file_stem().and_then(|stem| stem.to_str())
            .is_some_and(is_managed_stem);

        if !is_current && is_managed {
            fs::remove_file(&path)?;
        }
    }
    Ok(())
}

impl AppModel {
    /// The open project's directory, from `config.last_project_path`.
    /// Tolerates the pre-split config value, which pointed at the
    /// `project.json` *file* rather than the directory holding it.
    pub(super) fn project_dir(&self) -> Option<PathBuf> {
        let path = PathBuf::from(self.config.last_project_path.as_ref()?);
        if path.file_name().and_then(|name| name.to_str()) == Some(LEGACY_FILE) {
            return path.parent().map(Path::to_path_buf);
        }
        Some(path)
    }

    /// Applies an already-parsed project onto canvas/metadata/characters
    /// state. Split out from `try_load_project` so `handle_load_dir_picked`
    /// can parse first — to decide whether to show a `SimplePopup` — and
    /// only apply on success, without parsing twice.
    fn apply_project(&mut self, project: LoadedProject) {
        // A manifest root that doesn't match any loaded node (edited
        // externally, or the script file was removed) is treated as unset.
        let root = project.metadata.root_node
            .filter(|id| project.nodes.iter().any(|n| n.id == *id));

        self.canvas.nodes = project.nodes;
        self.canvas.root_node = root;
        self.canvas.geo_cache.clear();
        self.project_meta = project.metadata;
        self.canvas.offset = Vector::new(project.last_camera.0, project.last_camera.1);
        self.characters.characters = project.characters;
        self.characters.editor = None;

        // Loading resolved the "no project open" state, if it was pending.
        self.new_project_dialog = None;

        // Freshly loaded from disk — by definition nothing's unsaved yet.
        self.dirty = false;
    }

    /// Reads and applies a project from directory `dir`. Returns whether it
    /// succeeded — on failure this leaves existing state untouched. Used
    /// only by the silent auto-load-on-startup path (`auto_load_last_project`);
    /// the interactive Load flow uses `read_project_dir`/`apply_project`
    /// directly instead, so it can show a `SimplePopup` explaining *why* on
    /// failure.
    fn try_load_project(&mut self, dir: &Path) -> bool {
        let Ok(project) = read_project_dir(dir) else {
            return false;
        };

        self.apply_project(project);
        true
    }

    /// Writes the whole project to directory `dir` (creating it and its
    /// subdirectories as needed), remembering `dir` in
    /// `config.last_project_path` on success.
    pub(super) fn write_project_dir(&mut self, dir: &Path) {
        match self.write_project_dir_inner(dir) {
            Ok(()) => {
                self.config.last_project_path = Some(dir.display().to_string());
                self.save_config();

                println!("Saved to {}", dir.display());
            }
            Err(err) => {
                eprintln!("failed to save project to {}: {err}", dir.display());
                self.popup = Some(SimplePopup::new(fl!("popup-save-error-title"), fl!("popup-save-dir-failed-message")));
            }
        }
    }

    fn write_project_dir_inner(&mut self, dir: &Path) -> std::io::Result<()> {
        fs::create_dir_all(dir)?;

        // Manifest: metadata + the canvas's current root node.
        let mut metadata = self.project_meta.clone();
        metadata.root_node = self.canvas.root_node;
        let manifest = toml::to_string_pretty(&metadata)
            .map_err(|err| std::io::Error::other(err.to_string()))?;
        write_atomic(&dir.join(MANIFEST_FILE), &manifest)?;

        // Editor state: camera + per-node placement.
        let layout = CanvasLayout {
            last_camera: (self.canvas.offset.x, self.canvas.offset.y),
            nodes: self.canvas.nodes.iter()
                .map(|node| (node.id, NodeLayout { position: node.position.clone() }))
                .collect::<BTreeMap<_, _>>(),
        };
        write_atomic(&dir.join(CANVAS_FILE), &serde_json::to_string_pretty(&layout)?)?;

        // Story content and cast, one file per entry.
        let scripts: Vec<(String, String)> = self.canvas.nodes.iter()
            .map(|node| Ok((
                format!("{}.json", entry_file_stem(&node.title, node.id)),
                serde_json::to_string_pretty(&ScriptFile::from_node(node))?,
            )))
            .collect::<std::io::Result<_>>()?;
        sync_entry_dir(&dir.join(SCRIPTS_DIR), &scripts)?;

        let characters: Vec<(String, String)> = self.characters.characters.iter()
            .map(|character| {
                let mut on_disk = character.clone();
                on_disk.avatar = character.avatar_for_disk(dir);
                Ok((
                    format!("{}.json", entry_file_stem(&character.name, character.id)),
                    serde_json::to_string_pretty(&on_disk)?,
                ))
            })
            .collect::<std::io::Result<_>>()?;
        sync_entry_dir(&dir.join(CHARACTERS_DIR), &characters)?;

        // Pre-make the asset tree too, so the project's shape is complete
        // from the first save (avatar import also creates it on demand).
        fs::create_dir_all(dir.join("assets").join("images"))?;

        // The new layout is fully written — a leftover legacy monolith
        // would only shadow it confusingly, so park it as a backup.
        let legacy = dir.join(LEGACY_FILE);
        if legacy.is_file() {
            let _ = fs::rename(&legacy, dir.join(format!("{LEGACY_FILE}.bak")));
        }

        Ok(())
    }

    /// Auto-loads the last remembered project, if any. Only attempted when a
    /// path was actually remembered — an absent path means "no prior
    /// session," not "try the fallback location," so a fresh install doesn't
    /// pick up an unrelated leftover file there. Called once from `init`;
    /// returns whether a project is now open.
    pub(super) fn auto_load_last_project(&mut self) -> bool {
        let Some(dir) = self.project_dir() else {
            return false;
        };

        if self.try_load_project(&dir) {
            // Normalize a pre-split config value (which pointed at the
            // project.json file) to the directory itself.
            self.config.last_project_path = Some(dir.display().to_string());
            self.save_config();

            println!("Loaded from {}", dir.display());
            true
        } else {
            // `canvas`/`project_meta`/`characters` are already fresh
            // defaults, so there's nothing to reset beyond forgetting the
            // bad path.
            self.config.last_project_path = None;
            self.save_config();

            eprintln!("Could not load remembered project from {}; starting without one.", dir.display());
            false
        }
    }

    /// Opens the New Project dialog. `can_cancel` is false only for the
    /// startup variant, where there's no current project to fall back to.
    pub(super) fn open_new_project_dialog(&mut self, can_cancel: bool) {
        self.new_project_dialog = Some(NewProjectDialog::new(can_cancel));
    }

    /// The async system folder picker used by both File → Load and the New
    /// Project dialog's "Open existing…"; resolves to
    /// `Message::LoadDirPicked`.
    fn pick_project_dir() -> Task<cosmic::Action<Message>> {
        cosmic::task::future(async {
            // Picks a *directory*, not a file — see `handle_load_dir_picked`
            // for the manifest-on-top-level check. No file filter: folder
            // pickers don't filter by extension.
            let dialog = cosmic::dialog::file_chooser::open::Dialog::new().title(fl!("dialog-load-title"));

            let path = dialog.open_folder().await.ok().and_then(|response| response.url().to_file_path().ok());

            cosmic::Action::App(Message::LoadDirPicked(path))
        })
    }

    /// Handles `Message::HeaderFile` (New/Load/Save).
    pub(super) fn handle_file_menu(&mut self, action: FileMenuAction) -> Task<cosmic::Action<Message>> {
        match action {
            // New always goes through the dialog now — a project doesn't
            // exist until it has a directory. Cancellable, since there's a
            // current project to keep.
            FileMenuAction::New => {
                self.open_new_project_dialog(true);
                Task::none()
            }

            FileMenuAction::Load => Self::pick_project_dir(),

            FileMenuAction::Save => self.save_project(),
        }
    }

    /// Persists the *entire* project to disk. This is the *only* way
    /// anything reaches disk: the editors just write straight through to
    /// the in-memory node/character as the user types (see
    /// `StoryNodeEditor`/`CharacterCardEditor`), but never save on their
    /// own — only the File menu's Save/Ctrl+S (or autosave) does that.
    pub(super) fn save_project(&mut self) -> Task<cosmic::Action<Message>> {
        // Since the New Project dialog fronts every project, a missing path
        // should be unreachable — but if it somehow happens, resolve it the
        // same way: no project directory means nothing to save into.
        let Some(dir) = self.project_dir() else {
            self.open_new_project_dialog(true);
            return Task::none();
        };

        let now = jiff::Timestamp::now().to_string();
        if self.project_meta.created_at.is_empty() {
            self.project_meta.created_at = now.clone();
        }
        self.project_meta.updated_at = now;
        self.project_meta.app_version = env!("CARGO_PKG_VERSION").to_string();

        self.write_project_dir(&dir);
        if self.popup.is_none() {
            self.show_saved_toast();
            self.dirty = false;
        }

        Task::none()
    }

    /// Handles `Message::LoadDirPicked` (from File → Load or the New
    /// Project dialog's "Open existing…"). A failure here doesn't touch
    /// existing state at all — it only shows a `SimplePopup` explaining why
    /// and leaves the current session exactly as it was; only a successful
    /// parse replaces it (and closes the New Project dialog, if open).
    pub(super) fn handle_load_dir_picked(&mut self, dir: Option<PathBuf>) {
        let Some(dir) = dir else {
            // Dialog was cancelled or failed to open; nothing to do.
            return;
        };

        match read_project_dir(&dir) {
            Ok(project) => {
                self.apply_project(project);

                self.config.last_project_path = Some(dir.display().to_string());
                self.save_config();

                println!("Loaded from {}", dir.display());
            }
            Err(ProjectLoadError::Missing) => {
                self.popup = Some(SimplePopup::new(fl!("popup-load-error-title"), fl!("popup-missing-project-message")));
            }
            Err(ProjectLoadError::Invalid) => {
                self.popup = Some(SimplePopup::new(fl!("popup-load-error-title"), fl!("popup-invalid-project-message")));
            }
        }
    }

    /// Handles `Message::NewProject`. `Browse`/`OpenExisting` are
    /// intercepted here (rather than in `NewProjectDialog::update`) because
    /// only the top-level `update` can return a `Task` — the system folder
    /// pickers are async; their results come back around as
    /// `NewProjectMessage::BaseDirPicked` / `Message::LoadDirPicked`.
    pub(super) fn handle_new_project_dialog(&mut self, msg: NewProjectMessage) -> Task<cosmic::Action<Message>> {
        if matches!(msg, NewProjectMessage::Browse) {
            return cosmic::task::future(async {
                let dialog = cosmic::dialog::file_chooser::open::Dialog::new().title(fl!("dialog-save-title"));

                let dir = dialog.open_folder().await.ok().and_then(|response| response.url().to_file_path().ok());

                cosmic::Action::App(Message::NewProject(NewProjectMessage::BaseDirPicked(dir)))
            });
        }

        let Some(dialog) = &mut self.new_project_dialog else {
            return Task::none();
        };

        match dialog.update(msg) {
            NewProjectEvent::None => {}
            NewProjectEvent::OpenExisting => return Self::pick_project_dir(),
            NewProjectEvent::Cancelled => {
                self.new_project_dialog = None;
            }
            NewProjectEvent::Confirmed { path, author, comment } => {
                // Create the directory while the dialog can still show an
                // inline error; only a created directory dismisses it.
                match fs::create_dir_all(&path) {
                    Ok(()) => {
                        self.new_project_dialog = None;
                        return self.create_project(&path, author, comment);
                    }
                    Err(err) => {
                        eprintln!("failed to create project folder at {}: {err}", path.display());
                        dialog.error = Some(fl!("popup-save-dir-failed-message"));
                    }
                }
            }
        }

        Task::none()
    }

    /// Creates a brand-new project at `dir`: resets every piece of session
    /// state a project carries, seeds the metadata collected by the dialog,
    /// and writes the initial (empty) project files immediately — from this
    /// moment the project exists on disk and plain Save/autosave just work.
    fn create_project(&mut self, dir: &Path, author: String, comment: String) -> Task<cosmic::Action<Message>> {
        // `CharactersPage::empty()` (rather than `Default`, which seeds a
        // placeholder character for the very first run's UI) is what makes
        // this a truly blank project.
        self.canvas = CanvasPage::default();
        self.characters = CharactersPage::empty();

        // Drop any overlay left over from the replaced project.
        self.popup = None;
        self.find_panel = None;

        let now = jiff::Timestamp::now().to_string();
        self.project_meta = ProjectData {
            // The directory's last component is exactly the trimmed name
            // the dialog validated against (see `NewProjectDialog::target_path`).
            name: dir.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_string(),
            author,
            comment,
            repository: String::new(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            created_at: now.clone(),
            updated_at: now,
            root_node: None,
        };

        // Point the remembered path at the new project *before* writing —
        // if the initial write fails, a later Save must still target this
        // directory, never the previous project's.
        self.config.last_project_path = Some(dir.display().to_string());
        self.save_config();

        self.write_project_dir(dir);
        if self.popup.is_none() {
            self.show_saved_toast();
            self.dirty = false;
        }

        // The reset canvas/characters pages need their Preferences mirrors
        // re-seeded (a fresh `CanvasPage::default()` doesn't know them).
        self.sync_pref_pages();

        self.update_title()
    }
}
