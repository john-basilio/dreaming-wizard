use cosmic::iced::{Length, Alignment, advanced::text::{Wrapping, Ellipsize, EllipsizeHeightLimit}};
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

/// A side panel for editing a single `StoryNode`'s title.
///
/// Holds its own "draft" copy of the title rather than writing directly into
/// the `StoryNode` being edited — only written back on "Save".
pub struct StoryNodeEditor {
    /// Which `StoryNode` (by id) this editor session is for.
    pub node_id: Uuid,
    /// Unsaved title text; only written back to the node on submit.
    pub draft_title: String,
    /// What the "Editing <title>" header shows — only refreshed on Save, so
    /// the header doesn't reflow/jitter on every keystroke while typing.
    saved_title: String,
}

/// Widget-level messages from the editor's own `view()`.
#[derive(Debug, Clone)]
pub enum EditorMessage {
    /// The title `text_input` changed (fires on every keystroke).
    TitleChanged(String),
    /// The "Save" button was pressed, or the title `text_input` was
    /// submitted (Enter key) — both commit the draft title the same way.
    Save,
    /// The "Delete" button was pressed.
    Delete,
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
    /// "Save" was pressed (or the title submitted); the caller should
    /// persist it to the node.
    TitleCommitted(String),
    /// "Delete" was pressed; the caller should confirm and, if accepted,
    /// remove the node (the editor stays open until then).
    DeleteRequested,
    /// The editor was closed and should be dropped.
    Closed,
}

impl StoryNodeEditor {
    pub fn new(node_id: Uuid, title: impl Into<String>) -> Self {
        let title = title.into();
        Self { node_id, saved_title: title.clone(), draft_title: title }
    }

    /// Whether the draft title differs from what's actually saved — used to
    /// decide whether closing (or exiting the app) should warn first.
    pub fn is_dirty(&self) -> bool {
        self.draft_title != self.saved_title
    }

    /// Applies an `EditorMessage` to local draft state and reports back
    /// what, if anything, the caller needs to do about it.
    pub fn update(&mut self, message: EditorMessage) -> EditorEvent {
        match message {
            EditorMessage::TitleChanged(value) => {
                self.draft_title = value;
                EditorEvent::None
            }
            EditorMessage::Save => {
                self.saved_title = self.draft_title.clone();
                EditorEvent::TitleCommitted(self.draft_title.clone())
            }
            EditorMessage::Delete => EditorEvent::DeleteRequested,
            EditorMessage::Close => EditorEvent::Closed,
        }
    }

    pub fn view(&self) -> Element<'_, EditorMessage> {

        Column::new()
            // The editor label takes up all remaining space until the close
            // button, ellipsizing only if the title is too long even for
            // that; save/delete moved to their own row below, left-aligned,
            // so they no longer compete with the title for room here.
            .push(
                Row::new()
                        .push(
                            title4(format!("{} {}", fl!("editor-label"), self.saved_title))
                                .width(Length::Fill)
                                .wrapping(Wrapping::None)
                                .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
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
                        .push(
                            button::text(fl!("editor-save"))
                                .class(crate::components::save_button_class())
                                .on_press(EditorMessage::Save)
                        )
                        .push(
                            button::text(fl!("editor-delete"))
                                .class(cosmic::theme::Button::Destructive)
                                .on_press(EditorMessage::Delete)
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
                    .on_submit(|_| EditorMessage::Save),)
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
