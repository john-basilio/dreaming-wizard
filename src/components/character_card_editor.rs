//! A modal editor for a single `Character`'s card: avatar, name, comment,
//! and description. `nav::characters` opens one of these — centered over a
//! dimming shade above the character list — when a `character_card` is
//! clicked, mirroring `StoryNodeEditor`'s role for `StoryNode`s on the
//! canvas.
//!
//! Every field writes straight through to the caller (`CharactersPage`) as
//! it's typed — there's no separate draft/Save step, and no data lost by
//! just closing the editor. None of that reaches disk on its own though;
//! only the File menu's Save/Ctrl+S does that (see `AppModel::save_project`).

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

/// A modal panel for editing a single `Character`, seeded from the
/// `Character` it targets when opened. Every field here *is* the live
/// value — edits apply immediately, there's no separate draft.
pub struct CharacterCardEditor {
    /// Which `Character` (by id) this editor session is for.
    pub character_id: Uuid,
    pub name: String,
    /// Same string convention as `Character::avatar` (the `"default"`
    /// sentinel, or a path).
    pub avatar: String,
    pub comment: String,
    description_content: text_editor::Content,
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
    /// The "Delete" button was pressed.
    Delete,
    /// The "Close" button was pressed.
    Close,
}

/// What `CharacterCardEditor::update` reports back to its caller
/// (`CharactersPage`) after handling an `EditorMessage`, so it can write
/// the change through to the actual `Character` (or drop the editor).
pub enum EditorEvent {
    /// Nothing for the caller to do.
    None,
    /// A field changed and should be written straight through to the
    /// character — fired on every keystroke/action, not just on some
    /// separate "Save".
    NameChanged(String),
    CommentChanged(String),
    AvatarChanged(String),
    DescriptionChanged(String),
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
            name: character.name.clone(),
            avatar: character.avatar.clone(),
            comment: character.comment.clone(),
            description_content: text_editor::Content::with_text(&character.description),
        }
    }

    /// Applies an `EditorMessage`, updating this editor's own copy of the
    /// field so `view()` reflects it, and reports back what the caller
    /// should write through to the actual character.
    pub fn update(&mut self, message: EditorMessage) -> EditorEvent {
        match message {
            EditorMessage::NameChanged(value) => {
                self.name = value.clone();
                EditorEvent::NameChanged(value)
            }
            EditorMessage::CommentChanged(value) => {
                self.comment = value.clone();
                EditorEvent::CommentChanged(value)
            }
            EditorMessage::DescriptionAction(action) => {
                self.description_content.perform(action);
                EditorEvent::DescriptionChanged(self.description_content.text())
            }
            // The actual dialog is dispatched by `app.rs`; nothing to do here.
            EditorMessage::ChangeAvatar => EditorEvent::None,
            EditorMessage::AvatarPicked(path) => {
                // As given for now — no relative-to-project-file resolution
                // or import-into-the-project-folder copy step yet (see
                // `Character::avatar`'s doc comment).
                match path {
                    Some(path) => {
                        self.avatar = path.to_string_lossy().into_owned();
                        EditorEvent::AvatarChanged(self.avatar.clone())
                    }
                    None => EditorEvent::None,
                }
            }
            EditorMessage::Delete => EditorEvent::DeleteRequested,
            EditorMessage::Close => EditorEvent::Closed,
        }
    }

    pub fn view(&self) -> Element<'_, EditorMessage> {
        use cosmic::iced::widget::{Stack, pin};

        let avatar_content: Element<'_, EditorMessage> = match resolve_avatar_path(&self.avatar) {
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
                        text_input(fl!("editor-name-placeholder"), self.name.as_str())
                            .on_input(EditorMessage::NameChanged),
                    )
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .push(
                Row::new()
                    .push(heading(fl!("editor-comment-label")))
                    .push(
                        text_input(fl!("editor-comment-placeholder"), self.comment.as_str())
                            .on_input(EditorMessage::CommentChanged),
                    )
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .spacing(12)
            .width(Length::Fill);

        let content = Column::new()
            // The editor label, Delete, and Close — the only things within
            // the editor's own bounds that act on it. (`nav::characters`'s
            // dimming shade behind the editor blocks outside clicks without
            // closing it.) The label takes up all remaining space until
            // Delete/Close, ellipsizing only if it's still too long.
            .push(
                Row::new()
                    .push(
                        title4(format!("{} {}", fl!("editor-label"), self.name))
                            .width(Length::Fill)
                            .wrapping(Wrapping::None)
                            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
                    )
                    .push(button::text(fl!("editor-delete")).class(cosmic::theme::Button::Destructive).on_press(EditorMessage::Delete))
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
