//! A modal editor for a single `Character`'s card: avatar, name, comment,
//! and description. `nav::characters` opens one of these — centered over a
//! dimming shade above the character list — when a `character_card` is
//! clicked, mirroring `StoryNodeEditor`'s role for `StoryNode`s on the
//! canvas.
//!
//! Every field is buffered in a local draft and only written back to the
//! caller when "Save" is pressed (or dropped entirely if "Delete" is
//! pressed instead, subject to the caller's own delete confirmation) —
//! there's no live/automatic write-through as the user types.

use std::path::{Path, PathBuf};

use cosmic::iced::{
    Alignment, Length, ContentFit,
    alignment::{Horizontal, Vertical},
    advanced::text::{Wrapping, Ellipsize, EllipsizeHeightLimit},
};
use cosmic::widget::{
    Column, Row,
    button, container, icon, image, mouse_area,
    text::{title4, heading},
    text_editor, text_input,
};
use cosmic::Element;
use uuid::Uuid;

use crate::components::Character;
use crate::components::project_data::resolve_avatar_path;
use crate::fl;

const AVATAR_SIZE: f32 = 110.0;
/// Width budget for the "Editing <name>" header label — fixed rather than
/// `Length::Fill` so its exact size doesn't depend on content;
/// `.ellipsize(...)` in `view()` truncates whatever doesn't fit, based on
/// actual rendered width rather than a guessed character count.
const HEADER_TITLE_WIDTH: f32 = 220.0;

/// A modal panel for editing a single `Character`. Holds its own "draft"
/// copy of each field, seeded from the `Character` it targets when opened.
pub struct CharacterCardEditor {
    /// Which `Character` (by id) this editor session is for.
    pub character_id: Uuid,
    pub draft_name: String,
    /// Same string convention as `Character::avatar` (the `"default"`
    /// sentinel, or a path).
    pub draft_avatar: String,
    pub draft_comment: String,
    /// No plain `String` counterpart, unlike the other drafts — `text_editor`
    /// is stateful, so this holds its `Content` directly; `.text()` is
    /// pulled out only when reporting an `EditorEvent::Saved`.
    description_content: text_editor::Content,
    /// What the "Editing <name>" header shows — only refreshed on Save, so
    /// the header doesn't reflow/jitter on every keystroke while typing.
    saved_name: String,
    /// Last-saved copies of the remaining fields, kept purely so `is_dirty`
    /// can tell whether there's anything to warn about on close/app-exit —
    /// `saved_name` alone (used for the header) isn't enough since the
    /// other fields can change without the name changing.
    saved_avatar: String,
    saved_comment: String,
    saved_description: String,
}

/// Widget-level messages from the editor's own `view()`.
#[derive(Debug, Clone)]
pub enum EditorMessage {
    /// The name `text_input` changed (fires on every keystroke).
    NameChanged(String),
    /// The comment `text_input` changed (fires on every keystroke).
    CommentChanged(String),
    /// An edit/cursor action on the description `text_editor`.
    DescriptionAction(text_editor::Action),
    /// The avatar "+" button was pressed. `app.rs` is what actually opens
    /// the system file picker (only the top-level `update` can return an
    /// async `Task`); this just gives the button somewhere to send its
    /// click, and is a no-op here.
    ChangeAvatar,
    /// The system file picker opened by `ChangeAvatar` finished — `Some`
    /// with the chosen image path, or `None` if it was cancelled/failed.
    AvatarPicked(Option<PathBuf>),
    /// The "Save" button was pressed.
    Save,
    /// The "Delete" button was pressed.
    Delete,
    /// The "Close" button was pressed.
    Close,
}

/// What `CharacterCardEditor::update` reports back to its caller
/// (`CharactersPage`) after handling an `EditorMessage`, so it can write
/// the change through to the actual `Character` (or drop the editor).
pub enum EditorEvent {
    /// Nothing for the caller to do (e.g. a draft-only keystroke).
    None,
    /// "Save" was pressed; the caller should persist every field to the
    /// character.
    Saved {
        name: String,
        avatar: String,
        comment: String,
        description: String,
    },
    /// "Delete" was pressed; the caller should confirm and, if accepted,
    /// remove the character (the editor stays open until then).
    DeleteRequested,
    /// The editor was closed and should be dropped.
    Closed,
}

impl CharacterCardEditor {
    pub fn new(character: &Character) -> Self {
        Self {
            character_id: character.id,
            draft_name: character.name.clone(),
            draft_avatar: character.avatar.clone(),
            draft_comment: character.comment.clone(),
            description_content: text_editor::Content::with_text(&character.description),
            saved_name: character.name.clone(),
            saved_avatar: character.avatar.clone(),
            saved_comment: character.comment.clone(),
            saved_description: character.description.clone(),
        }
    }

    /// Whether any draft field differs from what's actually saved — used to
    /// decide whether closing (or exiting the app) should warn first.
    pub fn is_dirty(&self) -> bool {
        self.draft_name != self.saved_name
            || self.draft_avatar != self.saved_avatar
            || self.draft_comment != self.saved_comment
            || self.description_content.text() != self.saved_description
    }

    /// Applies an `EditorMessage` to local draft state and reports back
    /// what, if anything, the caller needs to do about it.
    pub fn update(&mut self, message: EditorMessage) -> EditorEvent {
        match message {
            EditorMessage::NameChanged(value) => {
                self.draft_name = value;
                EditorEvent::None
            }
            EditorMessage::CommentChanged(value) => {
                self.draft_comment = value;
                EditorEvent::None
            }
            EditorMessage::DescriptionAction(action) => {
                self.description_content.perform(action);
                EditorEvent::None
            }
            // The actual dialog is dispatched by `app.rs`; nothing to do here.
            EditorMessage::ChangeAvatar => EditorEvent::None,
            EditorMessage::AvatarPicked(path) => {
                // Draft-only, same as the other fields — the path is stored
                // as given for now (no relative-to-project-file resolution
                // or import-into-the-project-folder copy step yet — see
                // `Character::avatar`'s doc comment).
                if let Some(path) = path {
                    self.draft_avatar = path.to_string_lossy().into_owned();
                }
                EditorEvent::None
            }
            EditorMessage::Save => {
                self.saved_name = self.draft_name.clone();
                self.saved_avatar = self.draft_avatar.clone();
                self.saved_comment = self.draft_comment.clone();
                self.saved_description = self.description_content.text();
                EditorEvent::Saved {
                    name: self.draft_name.clone(),
                    avatar: self.draft_avatar.clone(),
                    comment: self.draft_comment.clone(),
                    description: self.saved_description.clone(),
                }
            }
            EditorMessage::Delete => EditorEvent::DeleteRequested,
            EditorMessage::Close => EditorEvent::Closed,
        }
    }

    pub fn view(&self) -> Element<'_, EditorMessage> {
        use cosmic::iced::widget::{Stack, pin};

        let avatar_content: Element<'_, EditorMessage> = match resolve_avatar_path(&self.draft_avatar) {
            Some(path) => image(Path::new(path))
                .width(Length::Fixed(AVATAR_SIZE))
                .height(Length::Fixed(AVATAR_SIZE))
                .content_fit(ContentFit::Cover)
                .into(),
            None => icon::from_name("avatar-default-symbolic").icon().size(56).into(),
        };

        let avatar = container(avatar_content)
            .width(Length::Fixed(AVATAR_SIZE))
            .height(Length::Fixed(AVATAR_SIZE))
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .class(cosmic::theme::Container::Card);

        // The "+" badge sits pinned over the avatar's bottom-right corner
        // rather than laid out beside it, matching the mockup.
        let change_avatar_button = button::icon(icon::from_name("list-add-symbolic"))
            .extra_small()
            .class(cosmic::theme::Button::Suggested)
            .on_press(EditorMessage::ChangeAvatar);

        let avatar_stack = Stack::new()
            .push(avatar)
            .push(pin(change_avatar_button).x(AVATAR_SIZE - 28.0).y(AVATAR_SIZE - 28.0))
            .width(Length::Fixed(AVATAR_SIZE))
            .height(Length::Fixed(AVATAR_SIZE));

        let fields = Column::new()
            .push(
                Row::new()
                    .push(heading(fl!("editor-name-label")))
                    .push(
                        text_input(fl!("editor-name-placeholder"), self.draft_name.as_str())
                            .on_input(EditorMessage::NameChanged),
                    )
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(
                Row::new()
                    .push(heading(fl!("editor-comment-label")))
                    .push(
                        text_input(fl!("editor-comment-placeholder"), self.draft_comment.as_str())
                            .on_input(EditorMessage::CommentChanged),
                    )
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(
                Row::new()
                    .push(button::text(fl!("editor-save")).class(crate::components::save_button_class()).on_press(EditorMessage::Save))
                    .push(button::text(fl!("editor-delete")).class(cosmic::theme::Button::Destructive).on_press(EditorMessage::Delete))
                    .spacing(10),
            )
            .spacing(12)
            .width(Length::Fill);

        let content = Column::new()
            // The editor label and the close button — the only thing within
            // the editor's own bounds that closes it. (`nav::characters`'s
            // dimming shade behind the editor is what closes it on an
            // outside click.)
            .push(
                Row::new()
                    .push(
                        title4(format!("{} {}", fl!("editor-label"), self.saved_name))
                            .width(Length::Fixed(HEADER_TITLE_WIDTH))
                            .wrapping(Wrapping::None)
                            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
                    )
                    .push(cosmic::widget::Space::new().width(Length::Fill))
                    .push(button::text(fl!("editor-close")).on_press(EditorMessage::Close))
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(
                Row::new()
                    .push(avatar_stack)
                    .push(fields)
                    .spacing(20)
                    .align_y(Alignment::Start),
            )
            .push(heading(fl!("editor-description-label")))
            .push(
                text_editor(&self.description_content)
                    .placeholder(fl!("editor-description-placeholder"))
                    .on_action(EditorMessage::DescriptionAction)
                    .height(Length::Fill),
            )
            .spacing(12)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill);

        // Wraps the editor body in a plain `mouse_area` with no `on_press`
        // set. `iced`'s `MouseArea` still unconditionally captures left
        // clicks within its bounds (see `mouse_area::update`), so this just
        // swallows clicks that land on bare editor space (labels, padding,
        // the avatar/card background) instead of letting them fall through
        // the `Stack` in `nav::characters` to the dimming shade underneath,
        // which would otherwise close the editor on a click that visibly
        // landed *on* it. Clicks that land on the inputs, description
        // editor, or any button are captured by those widgets first and
        // never reach this wrapper at all.
        mouse_area(content).into()
    }
}
