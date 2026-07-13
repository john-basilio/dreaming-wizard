//! A modal dialog for the first save of a brand-new project: a Name field
//! and a "Save as" location, picked via a "Browse" folder dialog and joined
//! with the name, with inline validation before confirming. `app.rs` opens
//! this from `FileMenuAction::Save` when no project is open yet (see
//! `Config::last_project_path`); once a project is open, saving is silent
//! (see `app.rs`'s "Saved" toast instead, via `widget::toaster`).

use std::path::PathBuf;

use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{Column, Row, Space, button, container, text, text_input};

use crate::fl;

/// Draft state for a brand-new project's save location, shown as a modal
/// until confirmed or cancelled.
pub struct SaveProjectDialog {
    pub draft_name: String,
    /// The parent folder chosen via "Browse"; the actual save location is
    /// always `base_dir` joined with `draft_name`. `None` until Browse
    /// resolves once.
    pub base_dir: Option<PathBuf>,
    /// Set after a failed `Confirm` (e.g. the target already exists and
    /// isn't empty); cleared as soon as either field changes again.
    pub error: Option<String>,
}

/// Widget-level messages from the dialog's own `view()`.
#[derive(Debug, Clone)]
pub enum SaveDialogMessage {
    /// The name `text_input` changed (fires on every keystroke).
    NameChanged(String),
    /// The "Browse" button was pressed. `app.rs` is what actually opens the
    /// system folder picker (only the top-level `update` can return an
    /// async `Task`); this just gives the button somewhere to send its
    /// click, and is a no-op here.
    Browse,
    /// The system folder picker opened by `Browse` finished — `Some` with
    /// the chosen parent folder, or `None` if it was cancelled/failed.
    BaseDirPicked(Option<PathBuf>),
    /// The "Save Project" button was pressed.
    Confirm,
    /// The "Cancel" button was pressed.
    Cancel,
}

/// What `SaveProjectDialog::update` reports back to `app.rs` after handling
/// a `SaveDialogMessage`.
pub enum SaveDialogEvent {
    /// Nothing for the caller to do yet — keep the dialog open.
    None,
    /// Validation passed. `path` is the project's own directory (not yet
    /// created) — `app.rs` creates it, writes `project.json` inside it, and
    /// drops the dialog.
    Confirmed(PathBuf),
    /// Cancelled; drop the dialog without saving anything.
    Cancelled,
}

impl SaveProjectDialog {
    pub fn new(default_name: impl Into<String>) -> Self {
        Self {
            draft_name: default_name.into(),
            base_dir: None,
            error: None,
        }
    }

    /// The save location as currently configured — `base_dir` joined with
    /// the trimmed name — or `None` until both are set.
    fn target_path(&self) -> Option<PathBuf> {
        let base = self.base_dir.as_ref()?;
        let name = self.draft_name.trim();
        (!name.is_empty()).then(|| base.join(name))
    }

    /// Applies a `SaveDialogMessage` to local draft state and reports back
    /// what, if anything, the caller needs to do about it.
    pub fn update(&mut self, message: SaveDialogMessage) -> SaveDialogEvent {
        match message {
            SaveDialogMessage::NameChanged(value) => {
                self.draft_name = value;
                self.error = None;
                SaveDialogEvent::None
            }
            // The actual picker is dispatched by `app.rs`; nothing to do here.
            SaveDialogMessage::Browse => SaveDialogEvent::None,
            SaveDialogMessage::BaseDirPicked(dir) => {
                if dir.is_some() {
                    self.base_dir = dir;
                    self.error = None;
                }
                SaveDialogEvent::None
            }
            SaveDialogMessage::Confirm => {
                let Some(path) = self.target_path() else {
                    self.error = Some(fl!("save-dialog-error-incomplete"));
                    return SaveDialogEvent::None;
                };

                // A pre-existing, non-empty target is almost certainly the
                // wrong place for a brand-new project (or would mix its
                // files in with something unrelated) — reject rather than
                // risk overwriting. A path that doesn't exist yet, or
                // exists but is empty, is fine; `app.rs` creates it.
                let is_occupied = std::fs::read_dir(&path)
                    .map(|mut entries| entries.next().is_some())
                    .unwrap_or(false);

                if is_occupied {
                    self.error = Some(fl!("save-dialog-error-not-empty"));
                    SaveDialogEvent::None
                } else {
                    SaveDialogEvent::Confirmed(path)
                }
            }
            SaveDialogMessage::Cancel => SaveDialogEvent::Cancelled,
        }
    }

    pub fn view(&self) -> Element<'_, SaveDialogMessage> {
        let path_display = match &self.base_dir {
            Some(base) => base.join(self.draft_name.trim()).display().to_string(),
            None => String::new(),
        };

        let error_color: cosmic::iced::Color =
            cosmic::theme::active().cosmic().destructive_text_color().into();

        let content = Column::new()
            .push(text::title4(fl!("save-dialog-title")))
            .push(
                Row::new()
                    .push(text::body(fl!("save-dialog-name-label")).width(Length::Fixed(70.0)))
                    .push(
                        text_input(fl!("save-dialog-name-placeholder"), self.draft_name.as_str())
                            .on_input(SaveDialogMessage::NameChanged)
                            .width(Length::Fixed(320.0)),
                    )
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(
                Row::new()
                    .push(text::body(fl!("save-dialog-path-label")).width(Length::Fixed(70.0)))
                    .push(
                        // No `on_input`: this field only ever reflects
                        // `base_dir`/`draft_name`, set via "Browse" — typing
                        // directly into it is a no-op (see `text_input`'s
                        // read-only-without-`on_input` behavior).
                        text_input(fl!("save-dialog-path-placeholder"), path_display)
                            .width(Length::Fixed(320.0)),
                    )
                    .push(button::standard(fl!("save-dialog-browse")).on_press(SaveDialogMessage::Browse))
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push_maybe(self.error.as_ref().map(|err| {
                text::body(err.clone()).class(cosmic::theme::Text::Color(error_color))
            }))
            .push(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(button::standard(fl!("save-dialog-cancel")).on_press(SaveDialogMessage::Cancel))
                    .push(button::suggested(fl!("save-dialog-confirm")).on_press(SaveDialogMessage::Confirm))
                    .spacing(10),
            )
            .spacing(14)
            .width(Length::Shrink);

        container(content)
            .padding(20)
            .width(Length::Shrink)
            .class(cosmic::theme::Container::Card)
            .into()
    }
}
