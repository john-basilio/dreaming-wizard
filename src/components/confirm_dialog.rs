//! A small reusable Yes/No confirmation modal (e.g. "delete this node?").
//! Unlike `SimplePopup` (a dismiss-only notice), this offers Cancel/Delete
//! and reports back which one was pressed; the caller decides what
//! "confirmed" means.

use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget::{Column, Row, Space, button, container, text};

use crate::fl;

/// A modal notice: a title, a message, and Cancel/Delete buttons.
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
}

/// Widget-level messages from `ConfirmDialog::view()`.
#[derive(Debug, Clone)]
pub enum ConfirmDialogMessage {
    /// "Delete" was pressed — the caller should proceed.
    Confirm,
    /// "Cancel" was pressed, or the dialog should otherwise be dropped
    /// without acting.
    Cancel,
}

impl ConfirmDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into() }
    }

    pub fn view(&self) -> Element<'_, ConfirmDialogMessage> {
        container(
            Column::new()
                .push(text::title4(self.title.clone()))
                .push(text::body(self.message.clone()))
                .push(
                    Row::new()
                        .push(Space::new().width(Length::Fill))
                        .push(button::standard(fl!("confirm-dialog-cancel")).on_press(ConfirmDialogMessage::Cancel))
                        .push(button::destructive(fl!("confirm-dialog-delete")).on_press(ConfirmDialogMessage::Confirm))
                        .spacing(10),
                )
                .spacing(14)
                .width(Length::Shrink),
        )
        .padding(20)
        .width(Length::Shrink)
        .class(cosmic::theme::Container::Card)
        .into()
    }
}
