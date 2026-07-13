//! A reusable Save/Discard/Cancel warning shown whenever an editor (or the
//! whole app) would otherwise close with unsaved draft changes. Unlike
//! `ConfirmDialog`, there's no per-instance title/message to hold — the
//! wording is the same everywhere it's used — so this is a plain `view()`
//! function rather than a struct.

use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget::{Column, Row, Space, button, container, text};

use crate::fl;

/// Widget-level messages from `unsaved_changes_dialog()`.
#[derive(Debug, Clone)]
pub enum UnsavedChangesMessage {
    /// Save the draft (the same full-project save the File menu's Save
    /// triggers), then proceed with closing.
    Save,
    /// Throw the draft away and proceed with closing.
    Discard,
    /// Stay open; don't close after all.
    Cancel,
}

pub fn unsaved_changes_dialog() -> Element<'static, UnsavedChangesMessage> {
    container(
        Column::new()
            .push(text::title4(fl!("unsaved-changes-title")))
            .push(text::body(fl!("unsaved-changes-message")))
            .push(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(button::standard(fl!("unsaved-changes-cancel")).on_press(UnsavedChangesMessage::Cancel))
                    .push(button::destructive(fl!("unsaved-changes-discard")).on_press(UnsavedChangesMessage::Discard))
                    .push(
                        button::text(fl!("unsaved-changes-save"))
                            .class(crate::components::save_button_class())
                            .on_press(UnsavedChangesMessage::Save),
                    )
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
