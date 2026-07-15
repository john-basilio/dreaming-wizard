//! A small reusable Yes/No confirmation modal (e.g. "delete this node?").
//! Unlike `SimplePopup` (a dismiss-only notice), this offers Cancel/Delete
//! and reports back which one was pressed; the caller decides what
//! "confirmed" means. An optional "Don't ask again" checkbox lets the
//! caller offer disabling this kind of confirmation entirely (see the
//! per-entity toggles on the Preferences page).

use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget::{Column, Row, Space, button, checkbox, container, text};

use crate::fl;

/// A modal notice: a title, a message, and Cancel/Delete buttons.
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    /// `Some(checked)` renders the "Don't ask again" checkbox; on Confirm
    /// the caller reads it back to know whether to disable this
    /// confirmation kind (the corresponding Preferences toggle).
    pub dont_ask: Option<bool>,
}

/// Widget-level messages from `ConfirmDialog::view()`.
#[derive(Debug, Clone)]
pub enum ConfirmDialogMessage {
    /// "Delete" was pressed — the caller should proceed.
    Confirm,
    /// "Cancel" was pressed, or the dialog should otherwise be dropped
    /// without acting.
    Cancel,
    /// The "Don't ask again" checkbox was toggled; the caller should
    /// write it back into `dont_ask` so the dialog re-renders checked.
    DontAskToggled(bool),
}

impl ConfirmDialog {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into(), dont_ask: None }
    }

    /// Adds the (initially unchecked) "Don't ask again" checkbox.
    pub fn with_dont_ask(mut self) -> Self {
        self.dont_ask = Some(false);
        self
    }

    /// Whether the user asked (via the checkbox) for this confirmation
    /// kind to stop appearing; read on Confirm.
    pub fn dont_ask_again(&self) -> bool {
        self.dont_ask == Some(true)
    }

    pub fn view(&self) -> Element<'_, ConfirmDialogMessage> {
        let mut content = Column::new()
            .push(text::title4(self.title.clone()))
            .push(text::body(self.message.clone()));

        if let Some(checked) = self.dont_ask {
            content = content.push(
                checkbox(checked)
                    .label(fl!("confirm-dont-ask-again"))
                    .on_toggle(ConfirmDialogMessage::DontAskToggled),
            );
        }

        container(
            content
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
