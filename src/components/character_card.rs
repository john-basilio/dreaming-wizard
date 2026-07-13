//! A visual card representing a single character: a portrait (or a
//! placeholder avatar) and a name underneath. `nav::characters` pushes one
//! of these per character onto its scrollable list; clicking one sends
//! `on_select` so the caller can open a `CharacterCardEditor` for it.
//!
//! `avatar_path` takes an already-resolved path to a png/jpg/jpeg file (see
//! `Character::avatar_path`, which interprets the `Character::avatar`
//! sentinel/path convention); `None` falls back to the placeholder icon
//! shown in the mockup.
//!
//! While `is_hovered` (driven by the caller's own hover-tracking state, via
//! `on_hover_enter`/`on_hover_exit`), a red delete button is pinned over the
//! card's top-right corner — a real `Button`, not a click-swallowing
//! `mouse_area`, so it only intercepts clicks within its own small bounds
//! and everywhere else on the card still reaches `on_select` normally.

use std::path::Path;

use cosmic::{
    Element,
    iced::{ContentFit, Length, alignment::{Horizontal, Vertical}, mouse, widget::{Stack, pin}},
    widget::{Column, button, container, icon, image, mouse_area, text},
};

use crate::fl;

/// Footprint of a card, so a list/grid of them lines up evenly regardless
/// of name length.
const CARD_WIDTH: f32 = 140.0;
const AVATAR_SIZE: f32 = 96.0;

/// Renders a clickable character card. `avatar_path` is a resolved path to
/// a png/jpg/jpeg image file; `None` falls back to a generic placeholder
/// person icon. `on_select` fires on click; `on_hover_enter`/`on_hover_exit`
/// fire as the mouse enters/leaves the card (the caller stores which card,
/// if any, is hovered and passes it back in as `is_hovered`); `on_delete`
/// fires when the hover-only delete button is clicked. `glow_alpha` (`0.0`
/// = no ring at all) draws the Find panel's "found it" highlight around the
/// card — see `nav::characters::CharactersPage::focus_character`.
#[allow(clippy::too_many_arguments)]
pub fn character_card<'a, Message: Clone + 'static>(
    name: impl Into<String>,
    avatar_path: Option<&Path>,
    is_hovered: bool,
    glow_alpha: f32,
    on_select: Message,
    on_hover_enter: Message,
    on_hover_exit: Message,
    on_delete: Message,
) -> Element<'a, Message> {
    let avatar_content: Element<'a, Message> = match avatar_path {
        Some(path) => image(path)
            .width(Length::Fixed(AVATAR_SIZE))
            .height(Length::Fixed(AVATAR_SIZE))
            .content_fit(ContentFit::Cover)
            .border_radius(12.0)
            .into(),
        None => icon::from_name("avatar-default-symbolic").icon().size(48).into(),
    };

    let avatar = container(avatar_content)
        .width(Length::Fixed(AVATAR_SIZE))
        .height(Length::Fixed(AVATAR_SIZE))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .class(cosmic::theme::Container::Card);

    let card = container(
        Column::new()
            .push(avatar)
            .push(text::body(name.into()))
            .align_x(Horizontal::Center)
            .spacing(12)
    )
        .width(Length::Fixed(CARD_WIDTH))
        .padding(12)
        .align_x(Horizontal::Center)
        .class(cosmic::theme::Container::Card);

    let card: Element<'a, Message> = mouse_area(card)
        .on_press(on_select)
        .on_enter(on_hover_enter)
        .on_exit(on_hover_exit)
        .interaction(mouse::Interaction::Pointer)
        .into();

    let content: Element<'a, Message> = if is_hovered {
        let delete_button = button::icon(icon::from_name("edit-delete-symbolic"))
            .extra_small()
            .class(cosmic::theme::Button::Destructive)
            .tooltip(fl!("tooltip-delete"))
            .on_press(on_delete);

        Stack::new()
            .push(card)
            .push(pin(delete_button).x(CARD_WIDTH - 28.0).y(0.0))
            .width(Length::Fixed(CARD_WIDTH))
            .height(Length::Shrink)
            .into()
    } else {
        card
    };

    if glow_alpha <= 0.0 {
        return content;
    }

    // A few pixels of padding beyond the card's own edge, so the ring reads
    // as a highlight *around* it — matching the node glow on the canvas,
    // which is inflated past the node's bounds for the same reason — rather
    // than sitting flush against the card's own (already-rounded) edge,
    // where it barely reads at all.
    container(content)
        .padding(4)
        .style(move |_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
            border: cosmic::iced::Border {
                width: 3.0,
                color: cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, glow_alpha),
                radius: 14.0.into(),
            },
            ..Default::default()
        })
        .into()
}
