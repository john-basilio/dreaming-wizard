//! The modal "New Project" dialog: name, location (via a "Browse" folder
//! picker), and the core metadata a fresh project starts with (author,
//! comment). Since a project now always lives in a directory from the
//! moment it exists, this is shown in two situations:
//!
//! - **At startup**, whenever no project could be (or should be) reopened —
//!   the app never runs project-less, so this blocks until a project is
//!   created or an existing one is opened (`can_cancel: false`, no Cancel
//!   button; the "Open existing…" button covers the load path).
//! - **From File → New / Ctrl+N**, replacing the old silent session reset —
//!   here Cancel is available and keeps the current project untouched.

use std::path::PathBuf;

use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{Column, Row, Space, button, container, text, text_input};

use crate::fl;

/// Width of the name/location/author/comment inputs.
const INPUT_WIDTH: f32 = 320.0;
/// Width of the field labels to their left.
const LABEL_WIDTH: f32 = 70.0;

/// Draft state for a brand-new project, shown as a modal until confirmed,
/// cancelled (when allowed), or replaced by opening an existing project.
pub struct NewProjectDialog {
    pub draft_name: String,
    /// The parent folder chosen via "Browse"; the project's own directory is
    /// always `base_dir` joined with `draft_name`. `None` until Browse
    /// resolves once.
    pub base_dir: Option<PathBuf>,
    pub author: String,
    pub comment: String,
    /// Set after a failed `Confirm` (e.g. the target already exists and
    /// isn't empty); cleared as soon as a relevant field changes again.
    pub error: Option<String>,
    /// Whether the dialog can be dismissed without resolving to a project.
    /// `false` for the startup variant (the app has nothing to fall back
    /// to), which simply renders no Cancel button.
    pub can_cancel: bool,
}

/// Widget-level messages from the dialog's own `view()`.
#[derive(Debug, Clone)]
pub enum NewProjectMessage {
    NameChanged(String),
    AuthorChanged(String),
    CommentChanged(String),
    /// The "Browse" button was pressed. `app.rs` is what actually opens the
    /// system folder picker (only the top-level `update` can return an
    /// async `Task`); this just gives the button somewhere to send its
    /// click, and is a no-op here.
    Browse,
    /// The system folder picker opened by `Browse` finished — `Some` with
    /// the chosen parent folder, or `None` if it was cancelled/failed.
    BaseDirPicked(Option<PathBuf>),
    /// The "Open existing…" button was pressed; `app.rs` runs the same
    /// folder picker as File → Load (a successful load closes this dialog).
    OpenExisting,
    /// The "Create Project" button was pressed.
    Confirm,
    /// The "Cancel" button was pressed (only rendered while `can_cancel`).
    Cancel,
}

/// What `NewProjectDialog::update` reports back to `app.rs` after handling
/// a `NewProjectMessage`.
pub enum NewProjectEvent {
    /// Nothing for the caller to do yet — keep the dialog open.
    None,
    /// Validation passed. `path` is the project's own directory (not yet
    /// created) — `app.rs` creates it, resets the session, writes the
    /// initial project files inside it, and drops the dialog.
    Confirmed {
        path: PathBuf,
        author: String,
        comment: String,
    },
    /// The user wants to open an existing project instead — `app.rs` runs
    /// the Load folder picker; the dialog stays until that succeeds.
    OpenExisting,
    /// Cancelled; drop the dialog, keeping the current project as-is.
    Cancelled,
}

impl NewProjectDialog {
    pub fn new(can_cancel: bool) -> Self {
        Self {
            draft_name: String::new(),
            base_dir: None,
            author: String::new(),
            comment: String::new(),
            error: None,
            can_cancel,
        }
    }

    /// The project directory as currently configured — `base_dir` joined
    /// with the trimmed name — or `None` until both are set.
    fn target_path(&self) -> Option<PathBuf> {
        let base = self.base_dir.as_ref()?;
        let name = self.draft_name.trim();
        (!name.is_empty()).then(|| base.join(name))
    }

    /// Applies a `NewProjectMessage` to local draft state and reports back
    /// what, if anything, the caller needs to do about it.
    pub fn update(&mut self, message: NewProjectMessage) -> NewProjectEvent {
        match message {
            NewProjectMessage::NameChanged(value) => {
                self.draft_name = value;
                self.error = None;
                NewProjectEvent::None
            }
            NewProjectMessage::AuthorChanged(value) => {
                self.author = value;
                NewProjectEvent::None
            }
            NewProjectMessage::CommentChanged(value) => {
                self.comment = value;
                NewProjectEvent::None
            }
            // The actual picker is dispatched by `app.rs`; nothing to do here.
            NewProjectMessage::Browse => NewProjectEvent::None,
            NewProjectMessage::BaseDirPicked(dir) => {
                if dir.is_some() {
                    self.base_dir = dir;
                    self.error = None;
                }
                NewProjectEvent::None
            }
            NewProjectMessage::OpenExisting => NewProjectEvent::OpenExisting,
            NewProjectMessage::Confirm => {
                let Some(path) = self.target_path() else {
                    self.error = Some(fl!("new-project-error-incomplete"));
                    return NewProjectEvent::None;
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
                    self.error = Some(fl!("new-project-error-not-empty"));
                    NewProjectEvent::None
                } else {
                    NewProjectEvent::Confirmed {
                        path,
                        author: self.author.trim().to_string(),
                        comment: self.comment.trim().to_string(),
                    }
                }
            }
            NewProjectMessage::Cancel => NewProjectEvent::Cancelled,
        }
    }

    /// A labelled `text_input` row in the dialog's fixed two-column layout.
    fn field_row<'a>(
        label: String,
        placeholder: String,
        value: &'a str,
        on_input: fn(String) -> NewProjectMessage,
    ) -> Element<'a, NewProjectMessage> {
        Row::new()
            .push(text::body(label).width(Length::Fixed(LABEL_WIDTH)))
            .push(
                text_input(placeholder, value)
                    .on_input(on_input)
                    .width(Length::Fixed(INPUT_WIDTH)),
            )
            .spacing(10)
            .align_y(Alignment::Center)
            .into()
    }

    pub fn view(&self) -> Element<'_, NewProjectMessage> {
        let path_display = match &self.base_dir {
            Some(base) => base.join(self.draft_name.trim()).display().to_string(),
            None => String::new(),
        };

        let error_color: cosmic::iced::Color =
            cosmic::theme::active().cosmic().destructive_text_color().into();

        let mut buttons = Row::new()
            .push(button::text(fl!("new-project-open-existing")).on_press(NewProjectMessage::OpenExisting))
            .push(Space::new().width(Length::Fill))
            .spacing(10);
        if self.can_cancel {
            buttons = buttons.push(button::standard(fl!("new-project-cancel")).on_press(NewProjectMessage::Cancel));
        }
        buttons = buttons.push(button::suggested(fl!("new-project-create")).on_press(NewProjectMessage::Confirm));

        let content = Column::new()
            .push(text::title4(fl!("new-project-title")))
            .push(Self::field_row(
                fl!("new-project-name-label"),
                fl!("new-project-name-placeholder"),
                self.draft_name.as_str(),
                NewProjectMessage::NameChanged,
            ))
            .push(
                Row::new()
                    .push(text::body(fl!("new-project-path-label")).width(Length::Fixed(LABEL_WIDTH)))
                    .push(
                        // No `on_input`: this field only ever reflects
                        // `base_dir`/`draft_name`, set via "Browse" — typing
                        // directly into it is a no-op (see `text_input`'s
                        // read-only-without-`on_input` behavior).
                        text_input(fl!("new-project-path-placeholder"), path_display)
                            .width(Length::Fixed(INPUT_WIDTH)),
                    )
                    .push(button::standard(fl!("new-project-browse")).on_press(NewProjectMessage::Browse))
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(Self::field_row(
                fl!("new-project-author-label"),
                fl!("new-project-author-placeholder"),
                self.author.as_str(),
                NewProjectMessage::AuthorChanged,
            ))
            .push(Self::field_row(
                fl!("new-project-comment-label"),
                fl!("new-project-comment-placeholder"),
                self.comment.as_str(),
                NewProjectMessage::CommentChanged,
            ))
            .push_maybe(self.error.as_ref().map(|err| {
                text::body(err.clone()).class(cosmic::theme::Text::Color(error_color))
            }))
            .push(buttons)
            .spacing(14)
            .width(Length::Shrink);

        container(content)
            .padding(20)
            .width(Length::Shrink)
            .class(cosmic::theme::Container::Card)
            .into()
    }
}
