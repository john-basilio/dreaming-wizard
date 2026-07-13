//! Project file I/O (JSON read/write) and the New/Load/Save orchestration
//! that drives it. Lives as a child module of `app` (rather than e.g.
//! `components::project_data`) because every operation here reaches into
//! `AppModel`'s canvas/characters/project-metadata state as a unit.

use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use cosmic::iced::Vector;
use cosmic::Task;

use crate::components::save_project_dialog::{SaveDialogEvent, SaveDialogMessage};
use crate::components::{ProjectData, ProjectFile, SaveProjectDialog, SimplePopup};
use crate::fl;
use crate::nav::{CanvasPage, CharactersPage};

use super::chrome::FileMenuAction;
use super::{AppModel, Message};

/// Why `read_project_file` couldn't produce a `ProjectFile`; lets
/// `handle_load_dir_picked` show a more specific `SimplePopup` message than
/// a bare `bool` would allow.
enum ProjectLoadError {
    /// `path` doesn't exist or couldn't be read.
    Missing,
    /// `path` exists but isn't valid `ProjectFile` JSON.
    Invalid,
}

/// Reads and parses (but doesn't apply) a `ProjectFile` from `path` — see
/// `AppModel::apply_project` for the other half.
fn read_project_file(path: &Path) -> Result<ProjectFile, ProjectLoadError> {
    let contents = std::fs::read_to_string(path).map_err(|_| ProjectLoadError::Missing)?;
    serde_json::from_str(&contents).map_err(|_| ProjectLoadError::Invalid)
}

impl AppModel {
    /// Applies an already-parsed `ProjectFile` onto canvas/metadata/
    /// characters state. Split out from `try_load_project` so
    /// `handle_load_dir_picked` can parse first — to decide whether to show
    /// a `SimplePopup` — and only apply on success, without parsing twice.
    pub(super) fn apply_project(&mut self, project: ProjectFile) {
        self.canvas.nodes = project.canvas.nodes;
        self.canvas.geo_cache.clear();
        self.project_meta = project.metadata;
        self.canvas.offset = Vector::new(project.canvas.last_camera.0, project.canvas.last_camera.1);
        self.characters.characters = project.characters.characters;
        self.characters.editor = None;
    }

    /// Reads and applies a project file from `path`. Returns whether it
    /// succeeded — on failure this leaves existing state untouched. Used
    /// only by the silent auto-load-on-startup path (`auto_load_last_project`);
    /// the interactive Load Project flow uses `read_project_file`/
    /// `apply_project` directly instead, so it can show a `SimplePopup`
    /// explaining *why* on failure.
    fn try_load_project(&mut self, path: &Path) -> bool {
        let Ok(project) = read_project_file(path) else {
            return false;
        };

        self.apply_project(project);
        true
    }

    /// Builds a `ProjectFile` from current canvas/metadata/characters state
    /// and writes it to `path` as JSON, remembering `path` in
    /// `config.last_project_path` on success. Used both when re-saving an
    /// already-open project (straight to the remembered path, no dialog)
    /// and after `SaveProjectDialog` is confirmed for a brand-new project.
    pub(super) fn write_project_file(&mut self, path: &Path) {
        let project = ProjectFile::new(
            self.canvas.nodes.clone(),
            (self.canvas.offset.x, self.canvas.offset.y),
            self.project_meta.clone(),
            self.characters.characters.clone(),
        );

        match File::create(path) {
            Ok(file) => {
                let writer = BufWriter::new(file);

                match serde_json::to_writer_pretty(writer, &project) {
                    Ok(()) => {
                        self.config.last_project_path = Some(path.display().to_string());
                        self.save_config();

                        println!("Saved to {}", path.display());
                    }
                    Err(err) => {
                        eprintln!("failed to serialize project to {}: {err}", path.display());
                    }
                }
            }
            Err(err) => {
                eprintln!("failed to create savefile at {}: {err}", path.display());
            }
        }
    }

    /// Auto-loads the last remembered project, if any. Only attempted when a
    /// path was actually remembered — an absent path means "no prior
    /// session," not "try the fallback location," so a fresh install doesn't
    /// pick up an unrelated leftover file there. Called once from `init`.
    pub(super) fn auto_load_last_project(&mut self) {
        let Some(path) = self.config.last_project_path.clone() else {
            return;
        };
        let path = PathBuf::from(path);

        if self.try_load_project(&path) {
            println!("Loaded from {}", path.display());
        } else {
            // `canvas`/`project_meta`/`characters` are already fresh
            // defaults, so there's nothing to reset beyond forgetting the
            // bad path.
            self.config.last_project_path = None;
            self.save_config();

            eprintln!("Could not load remembered project from {}; starting a new session.", path.display());
        }
    }

    /// Handles `Message::HeaderFile` (New/Load/Save).
    pub(super) fn handle_file_menu(&mut self, action: FileMenuAction) -> Task<cosmic::Action<Message>> {
        match action {
            // Resets every piece of session state a saved project would
            // otherwise carry. `CharactersPage::empty()` (rather than
            // `Default`, which seeds a placeholder character for the very
            // first run's UI) is what makes this a truly blank project.
            FileMenuAction::New => {
                self.canvas = CanvasPage::default();
                self.characters = CharactersPage::empty();
                self.project_meta = ProjectData::default();

                // Drop any overlay left over from the replaced project
                // (e.g. a Save dialog mid-fill for the old name).
                self.popup = None;
                self.save_dialog = None;

                // Forget the remembered path so the next Save prompts for a
                // new location instead of overwriting the discarded project.
                self.config.last_project_path = None;
                self.save_config();

                return self.update_title();
            }

            // Always prompts via the system folder picker (xdg-portal) —
            // the actual load work happens once it resolves, in
            // `handle_load_dir_picked`.
            FileMenuAction::Load => {
                return cosmic::task::future(async {
                    // Picks a *directory*, not the JSON file directly — see
                    // `handle_load_dir_picked` for the `project.json`-on-
                    // top-level check. No file filter: folder pickers don't
                    // filter by extension.
                    let dialog = cosmic::dialog::file_chooser::open::Dialog::new().title(fl!("dialog-load-title"));

                    let path = dialog.open_folder().await.ok().and_then(|response| response.url().to_file_path().ok());

                    cosmic::Action::App(Message::LoadDirPicked(path))
                });
            }

            FileMenuAction::Save => {
                let now = jiff::Timestamp::now().to_string();

                if self.project_meta.created_at.is_empty() {
                    self.project_meta.created_at = now.clone();
                }
                self.project_meta.updated_at = now;
                self.project_meta.app_version = env!("CARGO_PKG_VERSION").to_string();

                // No UI yet to set the author, so default it until project
                // settings exist. The name, by contrast, is set
                // interactively below (either already set from a prior
                // save/load, or `SaveProjectDialog` collects it).
                if self.project_meta.author.is_empty() {
                    self.project_meta.author = fl!("project-author-fallback");
                }

                if let Some(path) = self.config.last_project_path.clone() {
                    // Already have a project open — re-save straight to it,
                    // no dialog; just a brief "Saved" toast.
                    self.write_project_file(&PathBuf::from(path));
                    self.show_saved_toast();
                } else {
                    // First save of a brand-new project: open the name/
                    // location dialog instead of saving immediately.
                    if self.project_meta.name.is_empty() {
                        self.project_meta.name = fl!("project-name-fallback");
                    }
                    self.save_dialog = Some(SaveProjectDialog::new(self.project_meta.name.clone()));
                }
            }
        }
        Task::none()
    }

    /// Handles `Message::LoadDirPicked`. "Parse the directory" just means
    /// requiring a `project.json` directly inside it (not a subdirectory).
    /// A failure here doesn't touch existing state at all — it only shows a
    /// `SimplePopup` explaining why and leaves the current session exactly
    /// as it was; only a successful parse replaces it.
    pub(super) fn handle_load_dir_picked(&mut self, dir: Option<PathBuf>) {
        let Some(dir) = dir else {
            // Dialog was cancelled or failed to open; nothing to do.
            return;
        };

        let path = dir.join("project.json");

        match read_project_file(&path) {
            Ok(project) => {
                self.apply_project(project);

                self.config.last_project_path = Some(path.display().to_string());
                self.save_config();

                println!("Loaded from {}", path.display());
            }
            Err(ProjectLoadError::Missing) => {
                self.popup = Some(SimplePopup::new(fl!("popup-load-error-title"), fl!("popup-missing-project-message")));
            }
            Err(ProjectLoadError::Invalid) => {
                self.popup = Some(SimplePopup::new(fl!("popup-load-error-title"), fl!("popup-invalid-project-message")));
            }
        }
    }

    /// Handles `Message::SaveDialog`. `Browse` is intercepted here (rather
    /// than in `SaveProjectDialog::update`) because only the top-level
    /// `update` can return a `Task` — the system folder picker is async, and
    /// its result comes back around as `SaveDialogMessage::BaseDirPicked`,
    /// which *does* flow through the normal forwarding below.
    pub(super) fn handle_save_dialog(&mut self, msg: SaveDialogMessage) -> Task<cosmic::Action<Message>> {
        if matches!(msg, SaveDialogMessage::Browse) {
            return cosmic::task::future(async {
                let dialog = cosmic::dialog::file_chooser::open::Dialog::new().title(fl!("dialog-save-title"));

                let dir = dialog.open_folder().await.ok().and_then(|response| response.url().to_file_path().ok());

                cosmic::Action::App(Message::SaveDialog(SaveDialogMessage::BaseDirPicked(dir)))
            });
        }

        let Some(dialog) = &mut self.save_dialog else {
            return Task::none();
        };

        match dialog.update(msg) {
            SaveDialogEvent::None => {}
            SaveDialogEvent::Cancelled => {
                self.save_dialog = None;
            }
            SaveDialogEvent::Confirmed(path) => {
                self.save_dialog = None;

                match std::fs::create_dir_all(&path) {
                    Ok(()) => {
                        // `path`'s last component is exactly the trimmed
                        // name the dialog validated against (see
                        // `SaveProjectDialog::target_path`).
                        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                            self.project_meta.name = name.to_string();
                        }
                        self.write_project_file(&path.join("project.json"));
                        self.show_saved_toast();
                    }
                    Err(err) => {
                        eprintln!("failed to create project folder at {}: {err}", path.display());
                        self.popup = Some(SimplePopup::new(fl!("popup-save-error-title"), fl!("popup-save-dir-failed-message")));
                    }
                }
            }
        }

        Task::none()
    }
}
