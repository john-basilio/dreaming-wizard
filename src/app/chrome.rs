//! The header bar (File/Canvas/Help menus + title) and nav-bar chrome, plus
//! the `MenuAction` family they're built from. Pure presentation — nothing
//! here reaches into `AppModel` directly; each builder takes exactly what it
//! needs as a parameter, so `mod.rs`'s trait methods are thin delegations.

use std::collections::HashMap;

use cosmic::{
    prelude::*,
    Element,
    widget::{
        self,
        menu::{self, key_bind::{KeyBind, Modifier}, ItemWidth},
        nav_bar, text,
    },
    iced::{
        Length,
        keyboard::Key,
        advanced::text::{Wrapping, Ellipsize, EllipsizeHeightLimit},
    },
};

use crate::fl;
use super::Message;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileMenuAction {
    New,
    Load,
    Save,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasMenuAction {
    AddNode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelpMenuAction {
    About,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MenuAction {
    File(FileMenuAction),
    Canvas(CanvasMenuAction),
    Help(HelpMenuAction),
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::File(intent) => Message::HeaderFile(*intent),
            MenuAction::Canvas(intent) => Message::HeaderCanvas(*intent),
            MenuAction::Help(intent) => Message::HeaderHelp(*intent),
        }
    }
}

/// The app's global keyboard shortcuts, built once in `AppModel::init`.
pub(super) fn default_key_binds() -> HashMap<menu::KeyBind, MenuAction> {
    let mut key_binds = HashMap::new();

    key_binds.insert(
        KeyBind { modifiers: vec![Modifier::Ctrl], key: Key::Character("n".into()) },
        MenuAction::File(FileMenuAction::New),
    );
    key_binds.insert(
        KeyBind { modifiers: vec![Modifier::Ctrl], key: Key::Character("s".into()) },
        MenuAction::File(FileMenuAction::Save),
    );
    key_binds.insert(
        KeyBind { modifiers: vec![Modifier::Ctrl], key: Key::Character("o".into()) }, // "o" for open/load
        MenuAction::File(FileMenuAction::Load),
    );
    key_binds.insert(
        KeyBind { modifiers: vec![Modifier::Ctrl], key: Key::Character("a".into()) },
        MenuAction::Canvas(CanvasMenuAction::AddNode),
    );

    key_binds
}

/// Builds the File/Canvas/Help header menus.
pub(super) fn header_start(key_binds: &HashMap<menu::KeyBind, MenuAction>) -> Vec<Element<'_, Message>> {
    let file_menu = menu::bar(vec![menu::Tree::with_children(
        menu::root(fl!("hs_file")).apply(Element::from),
        menu::items(
            key_binds,
            vec![
                menu::Item::Button(fl!("item_new"), None, MenuAction::File(FileMenuAction::New)),
                menu::Item::Button(fl!("item_load"), None, MenuAction::File(FileMenuAction::Load)),
                menu::Item::Button(fl!("item_save"), None, MenuAction::File(FileMenuAction::Save)),
            ],
        ),
    )])
    // "Load Project"/"New Project" are the longest labels here, plus their
    // "Ctrl + _" hints — `menu::bar`'s default width (150) is too tight.
    .item_width(ItemWidth::Uniform(190));

    let canvas_menu = menu::bar(vec![menu::Tree::with_children(
        menu::root(fl!("hs_canvas")).apply(Element::from),
        menu::items(
            key_binds,
            vec![menu::Item::Button(
                fl!("item_add_node"),
                None,
                MenuAction::Canvas(CanvasMenuAction::AddNode),
            )],
        ),
    )])
    // "New Story Node" is the longest label in any of our menus.
    .item_width(ItemWidth::Uniform(200));

    let help_menu = menu::bar(vec![menu::Tree::with_children(
        menu::root(fl!("hs_help")).apply(Element::from),
        menu::items(
            key_binds,
            vec![menu::Item::Button(fl!("item_about"), None, MenuAction::Help(HelpMenuAction::About))],
        ),
    )])
    // "About" has no keybind hint, so it only needs to fit the label.
    .item_width(ItemWidth::Uniform(160));

    vec![file_menu.into(), canvas_menu.into(), help_menu.into()]
}

/// Builds the header's centered title (`project_name`, or a fallback if the
/// project hasn't been named yet).
pub(super) fn header_center(project_name: &str) -> Vec<Element<'static, Message>> {
    let display_name = if project_name.is_empty() {
        fl!("project-title-fallback")
    } else {
        project_name.to_string()
    };

    let title = text::heading(format!("{} {}", fl!("project_title_prefix"), display_name))
        .width(Length::Fill)
        .center()
        .wrapping(Wrapping::None)
        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));

    let title = widget::container(title).width(Length::Fill).padding([0, 64]);

    vec![title.into()]
}

/// Builds the nav sidebar, sized to hug its widest item instead of always
/// taking the default fixed width.
pub(super) fn build_nav_bar(nav_model: &nav_bar::Model) -> Element<'_, cosmic::Action<Message>> {
    let theme = cosmic::theme::active();
    let space_xxs = theme.cosmic().space_xxs();
    let space_s = theme.cosmic().space_s();
    let space_l = theme.cosmic().space_l();

    // Ideally this would match the width of the nav's shortest item, but
    // that requires measuring shaped text against a live renderer, which
    // isn't available while just building the widget tree. `space_l` gives
    // comfortable, deliberate breathing room instead.
    let nav = cosmic::widget::segmented_button::vertical(nav_model)
        .on_activate(|id| cosmic::Action::Cosmic(cosmic::app::Action::NavBar(id)))
        .button_height(32)
        .button_padding([space_l, space_xxs, space_l, space_xxs])
        .button_spacing(space_xxs)
        .spacing(space_xxs)
        .style(cosmic::theme::SegmentedButton::NavBar)
        .width(Length::Shrink)
        .apply(widget::container)
        .padding(space_s)
        .apply(widget::scrollable)
        .class(cosmic::style::iced::Scrollable::Minimal)
        .height(Length::Fill)
        .apply(widget::container)
        .width(Length::Shrink)
        .height(Length::Fill)
        .class(cosmic::theme::Container::custom(nav_bar::nav_bar_style));

    Element::from(nav)
}
