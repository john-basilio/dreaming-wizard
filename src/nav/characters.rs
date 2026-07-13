//! The "Characters" nav page: a scrollable list of `character_card`s backed
//! by real `Character` data, plus — while one is selected — a
//! `CharacterCardEditor` shown as a centered modal over a dimming shade that
//! blocks (but doesn't dismiss) the list underneath — only the editor's own
//! Close button closes it. This mirrors `nav::canvas`'s `StoryNodeEditor`,
//! but as an overlay rather than a side panel, since characters don't need
//! the canvas's camera framing.

use cosmic::Element;
use cosmic::iced::{Background, Color, Length};
use cosmic::iced::widget::Stack;
use cosmic::widget::{self, Column, Row, Space, container, mouse_area};
use uuid::Uuid;

use crate::components::{Character, CharacterCardEditor, character_card};
use crate::components::character_card_editor::{EditorEvent, EditorMessage};
use crate::fl;

/// Page model for the Characters page.
pub struct CharactersPage {
    pub characters: Vec<Character>,
    /// Some while a character's editor is open; the list becomes visually
    /// dimmed and non-interactive underneath it (see `view`'s shade). Only
    /// the editor's own Close button clears this.
    pub editor: Option<CharacterCardEditor>,
}

impl Default for CharactersPage {
    fn default() -> Self {
        Self {
            // One placeholder entry so the page has something to look at
            // (and click) before there's any UI to add more characters.
            characters: vec![Character {
                name: fl!("character-default-name"),
                ..Character::default()
            }],
            editor: None,
        }
    }
}

/// Messages emitted by the Characters page.
#[derive(Debug, Clone)]
pub enum CharactersMessage {
    /// A `character_card` in the list was clicked; open its editor.
    CardClicked(Uuid),
    /// Forwarded from the open `CharacterCardEditor`'s own `view()`.
    Editor(EditorMessage),
}

impl CharactersPage {
    pub fn view(&self) -> Element<'_, CharactersMessage> {
        let content = self.characters.iter()
            .fold(Column::new(), |column, character| {
                column.push(character_card(
                    character.name.clone(),
                    character.avatar_path().map(std::path::Path::new),
                    CharactersMessage::CardClicked(character.id),
                ))
            })
            .width(Length::Fill)
            .padding(12)
            .spacing(12);

        let list: Element<'_, CharactersMessage> = widget::scrollable(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let Some(editor) = &self.editor else {
            return list;
        };

        // Dims and blocks interaction with the list behind the editor. No
        // `on_press`: this only exists to swallow clicks (`mouse_area`
        // unconditionally captures left-clicks within its bounds — see
        // `CharacterCardEditor::view`'s own wrapper for the same trick) so
        // they can't reach the list underneath; the editor is only closed
        // via its own Close button.
        let shade: Element<'_, CharactersMessage> = mouse_area(
            container(Space::new().width(Length::Fill).height(Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(|_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    background: Some(Background::Color(Color::from_rgba8(0, 0, 0, 0.6))),
                    ..Default::default()
                }),
        )
        .into();

        // 80% width / 90% height, centered: a Column/Row pair of
        // `FillPortion` spacers around the editor gives a fraction of
        // whatever space the Stack actually has, without needing to know
        // real pixel bounds up front.
        let editor_panel = Column::new()
            .push(Space::new().width(Length::Fill).height(Length::FillPortion(1)))
            .push(
                Row::new()
                    .push(Space::new().width(Length::FillPortion(1)).height(Length::Fill))
                    .push(
                        container(editor.view().map(CharactersMessage::Editor))
                            .width(Length::FillPortion(8))
                            .height(Length::Fill)
                            .class(cosmic::theme::Container::Card),
                    )
                    .push(Space::new().width(Length::FillPortion(1)).height(Length::Fill))
                    .height(Length::FillPortion(18)),
            )
            .push(Space::new().width(Length::Fill).height(Length::FillPortion(1)))
            .width(Length::Fill)
            .height(Length::Fill);

        Stack::new()
            .push(list)
            .push(shade)
            .push(editor_panel)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    /// Applies a `CharactersMessage`. Returns `Some(id)` when a card was
    /// clicked and its editor opened, mirroring `CanvasPage::update`'s
    /// return so `app.rs` can close the nav bar the same way it does for
    /// canvas node edits.
    pub fn update(&mut self, message: CharactersMessage) -> Option<Uuid> {
        match message {
            CharactersMessage::CardClicked(id) => {
                let character = self.characters.iter().find(|c| c.id == id)?;
                self.editor = Some(CharacterCardEditor::new(character));
                Some(id)
            }

            CharactersMessage::Editor(message) => {
                let editor = self.editor.as_mut()?;
                let event = editor.update(message);
                let character_id = editor.character_id;

                match event {
                    EditorEvent::None => {}
                    EditorEvent::NameChanged(name) => {
                        if let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id) {
                            character.name = name;
                        }
                    }
                    EditorEvent::CommentChanged(comment) => {
                        if let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id) {
                            character.comment = comment;
                        }
                    }
                    EditorEvent::DescriptionChanged(description) => {
                        if let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id) {
                            character.description = description;
                        }
                    }
                    EditorEvent::AvatarChanged(avatar) => {
                        if let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id) {
                            character.avatar = avatar;
                        }
                    }
                    EditorEvent::Closed => {
                        self.editor = None;
                    }
                }
                None
            }
        }
    }
}
