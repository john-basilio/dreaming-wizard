use cosmic::iced::{Length, Alignment};
use cosmic::widget::{
    Column, 
    Row, 
    text::{title4, heading}, 
    button, 
    text_input};
use cosmic::Element;
use uuid::Uuid;

use crate::fl;

/// A side panel for editing a single `StoryNode`'s title.
///
/// Holds its own "draft" copy of the title rather than writing directly into
/// the `StoryNode` being edited.
pub struct StoryNodeEditor {
    pub node_id: Uuid,
    pub draft_title: String,
}

#[derive(Debug, Clone)]
pub enum EditorMessage {
    TitleChanged(String),
    TitleSubmitted(String),
    Close,
}

pub enum EditorEvent {
    None,
    TitleCommitted(String),
    Closed,
}

impl StoryNodeEditor {
    pub fn new(node_id: Uuid, title: impl Into<String>) -> Self {
        Self { node_id, draft_title: title.into() }
    }

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
                .push(title4(fl!("editor-label")  + " " + self.draft_title.as_str()).width(Length::Fill))
                .push(button::text(fl!("editor-close")).on_press(EditorMessage::Close))
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
            .width(Length::Fixed(580.0))
            .height(Length::Fill)
            .into()
    }
}