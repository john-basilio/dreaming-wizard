//! A visual card representing a single character: a portrait (or a
//! placeholder avatar) and a name underneath. `nav::characters` pushes one
//! of these per character onto its scrollable list; clicking one sends
//! `on_select` so the caller can open a `CharacterCardEditor` for it.
//!
//! `avatar_path` takes an already-resolved path to a png/jpg/jpeg file (see
//! `Character::avatar_path`, which interprets the `Character::avatar`
//! sentinel/path convention); `None` falls back to the placeholder icon
//! shown in the mockup.

use std::path::Path;

use cosmic::{
    Element,
    iced::{ContentFit, Length, alignment::{Horizontal, Vertical}, mouse},
    widget::{Column, container, icon, image, mouse_area, text},
};

/// Footprint of a card, so a list/grid of them lines up evenly regardless
/// of name length.
const CARD_WIDTH: f32 = 140.0;
const AVATAR_SIZE: f32 = 96.0;

/// Renders a clickable character card. `avatar_path` is a resolved path to
/// a png/jpg/jpeg image file; `None` falls back to a generic placeholder
/// person icon. `on_select` fires on click.
pub fn character_card<'a, Message: Clone + 'a>(
    name: impl Into<String>,
    avatar_path: Option<&Path>,
    on_select: Message,
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

    mouse_area(card)
        .on_press(on_select)
        .interaction(mouse::Interaction::Pointer)
        .into()
}
