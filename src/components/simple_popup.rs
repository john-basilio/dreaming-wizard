//! A small, reusable modal notice: a title, a message, and a close ("X")
//! button. For anything needing its own fields or interactive widgets, see
//! `CharacterCardEditor` instead — this is deliberately minimal, meant for
//! one-off errors/confirmations. Its first use is `app.rs`'s Load Project
//! failure notice (missing/invalid `project.json`). It only overlays the
//! current page's content area — the nav bar and header menus stay visible
//! and usable while it's open.

use cosmic::Element;
use cosmic::iced::{Alignment, Length};
use cosmic::widget::{Column, Row, button, container, icon, text};

use crate::fl;

/// A modal notice: a title, a message, and a close button.
pub struct SimplePopup {
    pub title: String,
    pub message: String,
}

/// Widget-level messages from `SimplePopup::view()`.
#[derive(Debug, Clone)]
pub enum PopupMessage {
    /// The close ("X") button was pressed — the only way to dismiss.
    Close,
}

impl SimplePopup {
    pub fn new(title: impl Into<String>, message: impl Into<String>) -> Self {
        Self { title: title.into(), message: message.into() }
    }

    pub fn view(&self) -> Element<'_, PopupMessage> {
        let header = Row::new()
            .push(text::title4(self.title.clone()).width(Length::Fill))
            .push(
                button::icon(icon::from_name("window-close-symbolic"))
                    .extra_small()
                    .tooltip(fl!("popup-close-tooltip"))
                    .on_press(PopupMessage::Close),
            )
            .spacing(10)
            .align_y(Alignment::Center);

        container(
            Column::new()
                .push(header)
                .push(text::body(self.message.clone()))
                .spacing(12)
                .width(Length::Shrink),
        )
        .padding(20)
        .width(Length::Shrink)
        .class(cosmic::theme::Container::Card)
        .into()
    }
}
