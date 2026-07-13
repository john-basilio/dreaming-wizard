//! The Find popup (Ctrl+F / the Action menu's Find item): searches nodes or
//! characters by name and lets the user jump to a match. Unlike the
//! editors/dialogs, this is a non-blocking corner panel (see
//! `components::overlay::with_corner_panel`) — `nav::canvas`/
//! `nav::characters` stay fully interactive (pan/drag/scroll) while it's
//! open. This only holds the panel's own UI state and `view()`; like
//! `SimplePopup`, there's no `update()` here — resolving a result needs
//! both pages' data plus the pan/scroll+glow effects, so `app::mod`'s
//! `update()` handles every `FindMessage` directly.

use cosmic::Element;
use cosmic::iced::{Alignment, Background, Border, Color, Length, mouse};
use cosmic::widget::{self, Column, Id, Row, button, container, icon, mouse_area, scrollable, text, text_input};
use uuid::Uuid;

use crate::fl;

const PANEL_WIDTH: f32 = 320.0;
const RESULTS_HEIGHT: f32 = 160.0;

/// Widget `Id` of the query `text_input`, so `app::mod` can focus it the
/// moment the panel opens.
pub fn query_input_id() -> Id {
    Id::new("find-query-input")
}

/// Which kind of thing the Find panel is currently searching.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindTarget {
    Node,
    Character,
}

impl FindTarget {
    const ALL: [FindTarget; 2] = [FindTarget::Node, FindTarget::Character];

    fn label(self) -> String {
        match self {
            FindTarget::Node => fl!("find-target-node"),
            FindTarget::Character => fl!("find-target-character"),
        }
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }

    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(FindTarget::Node)
    }
}

/// One matching result row, already resolved to a display name; built by
/// `app::mod` from whichever page's data matches the current `FindTarget`.
pub struct FindResult {
    pub id: Uuid,
    pub label: String,
}

/// Panel state: the query, which target it's searching, and which result
/// row is currently highlighted (by keyboard arrows or mouse hover).
pub struct FindPanel {
    pub query: String,
    pub target: FindTarget,
    pub highlighted: usize,
}

/// Widget-level messages from `FindPanel::view()`.
#[derive(Debug, Clone)]
pub enum FindMessage {
    QueryChanged(String),
    TargetChanged(usize),
    ResultHovered(usize),
    ResultClicked(usize),
    /// Enter pressed in the query field — act on whichever result is
    /// currently highlighted (see `highlighted`).
    Confirm,
    Close,
}

impl FindPanel {
    pub fn new(target: FindTarget) -> Self {
        Self { query: String::new(), target, highlighted: 0 }
    }

    pub fn view(&self, results: &[FindResult]) -> Element<'_, FindMessage> {
        let targets: Vec<String> = FindTarget::ALL.iter().map(|t| t.label()).collect();

        let header = Row::new()
            .push(text::body(fl!("find-label")))
            .push(
                text_input(fl!("find-placeholder"), &self.query)
                    .id(query_input_id())
                    .on_input(FindMessage::QueryChanged)
                    .on_submit(|_| FindMessage::Confirm)
                    // `Fill` rather than a fixed width: the dropdown's
                    // rendered width varies with which target's selected
                    // ("Character" is wider than "Node") — letting the
                    // query field absorb that instead of a fixed width
                    // keeps the close button from ever being pushed out
                    // past the panel's own fixed width.
                    .width(Length::Fill),
            )
            .push(widget::dropdown(targets, Some(self.target.index()), FindMessage::TargetChanged))
            .push(
                button::icon(icon::from_name("window-close-symbolic"))
                    .extra_small()
                    .tooltip(fl!("find-close-tooltip"))
                    .on_press(FindMessage::Close),
            )
            .spacing(8)
            .align_y(Alignment::Center);

        let rows: Vec<Element<'_, FindMessage>> = results.iter().enumerate().map(|(index, result)| {
            let highlighted = index == self.highlighted;

            let row = container(text::body(result.label.clone()))
                .padding([6, 10])
                .width(Length::Fill)
                .style(move |_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: highlighted.then(|| Background::Color(Color::from_rgba(1.0, 1.0, 1.0, 0.08))),
                    border: Border { radius: 6.0.into(), ..Default::default() },
                    ..Default::default()
                });

            mouse_area(row)
                .on_press(FindMessage::ResultClicked(index))
                .on_enter(FindMessage::ResultHovered(index))
                .interaction(mouse::Interaction::Pointer)
                .into()
        }).collect();

        let results_list = scrollable(Column::with_children(rows).spacing(4))
            .height(Length::Fixed(RESULTS_HEIGHT));

        container(
            Column::new()
                .push(header)
                .push(results_list)
                .spacing(10)
                .width(Length::Fixed(PANEL_WIDTH)),
        )
        .padding(16)
        .class(cosmic::theme::Container::Card)
        .into()
    }
}
