use cosmic::iced::{Length, Alignment};
use cosmic::widget::{
    Column, 
    Row, 
    text::{title4, heading}, 
    button, 
    text_input,
};
use cosmic::Element;
use uuid::Uuid;
use crate::fl;
use super::display_title;

/// A side panel for editing a single `StoryNode`'s title.
///
/// Holds its own "draft" copy of the title rather than writing directly into
/// the `StoryNode` being edited.
pub struct StoryNodeEditor {
    /// Which `StoryNode` (by id) this editor session is for.
    pub node_id: Uuid,
    /// Unsaved title text; only written back to the node on submit.
    pub draft_title: String,
}

/// Widget-level messages from the editor's own `view()`.
#[derive(Debug, Clone)]
pub enum EditorMessage {
    /// The title `text_input` changed (fires on every keystroke).
    TitleChanged(String),
    /// The title `text_input` was submitted (Enter key).
    TitleSubmitted(String),
    /// The "Close" button was pressed.
    Close,
}

/// What `StoryNodeEditor::update` reports back to its caller (`CanvasPage`)
/// after handling an `EditorMessage`, so the canvas can react — e.g. write
/// the committed title into the actual `StoryNode`, or tear down the editor
/// and animate the camera back on `Closed`.
pub enum EditorEvent {
    /// Nothing for the caller to do (e.g. a draft title keystroke).
    None,
    /// The title was submitted; the caller should persist it to the node.
    TitleCommitted(String),
    /// The editor was closed and should be dropped.
    Closed,
}

impl StoryNodeEditor {
    pub fn new(node_id: Uuid, title: impl Into<String>) -> Self {
        Self { node_id, draft_title: title.into() }
    }

    /// Applies an `EditorMessage` to local draft state and reports back
    /// what, if anything, the caller needs to do about it.
    pub fn update(&mut self, message: EditorMessage) -> EditorEvent {
        match message {
            EditorMessage::TitleChanged(value) => {
                self.draft_title = value;
                EditorEvent::None
            }
            EditorMessage::TitleSubmitted(value) => EditorEvent::TitleCommitted(value),
            EditorMessage::Close => EditorEvent::Closed,
        }
    }

    pub fn view(&self) -> Element<'_, EditorMessage> {

        Column::new()
            // The editor label and the close button
            // TODO: Decide whether to implement some widgets in this row in the future
            .push(
                Row::new()
                        .push(
                            title4(format!("{} {}", fl!("editor-label"), display_title(&self.draft_title, 30)))
                                .width(Length::Fill)
                                .wrapping(cosmic::iced::widget::text::Wrapping::None)
                        )
                        .push(
                            button::text(fl!("editor-close"))
                                .on_press(EditorMessage::Close)
                        )
                        .spacing(10)
                        .align_y(Alignment::Center)
            )
            .push(
                Row::new()
                .push(heading(fl!("editor-title-label")),)
                .push(
                    text_input(fl!("editor-title-placeholder"), self.draft_title.as_str())
                    .on_input(EditorMessage::TitleChanged)
                    .on_submit(EditorMessage::TitleSubmitted),)
                .spacing(10)
                .align_y(Alignment::Center)
                
            )
            // TODO: Add push() for the rest of the StoryNode widgets.
            .spacing(12)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}