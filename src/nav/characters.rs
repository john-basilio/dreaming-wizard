//! The "Characters" nav page: a scrollable list of `character_card`s backed
//! by real `Character` data, plus — while one is selected — a
//! `CharacterCardEditor` shown as a centered modal over a dimming shade that
//! blocks (but doesn't dismiss) the list underneath — only the editor's own
//! Close button closes it. This mirrors `nav::canvas`'s `StoryNodeEditor`,
//! but as an overlay rather than a side panel, since characters don't need
//! the canvas's camera framing.

use std::time::{Duration, Instant};

use cosmic::Element;
use cosmic::iced::{Background, Color, Length};
use cosmic::iced::widget::Stack;
use cosmic::iced::widget::scrollable::Viewport;
use cosmic::widget::{self, Column, Id, Row, Space, button, container, icon, mouse_area};
use uuid::Uuid;

use crate::components::{Character, CharacterCardEditor, ConfirmDialog, character_card};
use crate::components::character_card_editor::{EditorEvent, EditorMessage};
use crate::components::confirm_dialog::ConfirmDialogMessage;
use crate::components::overlay::{with_corner_button, with_overlay, toast_box, HoverTooltip, pulse_alpha, is_pulse_active};
use crate::components::unsaved_changes_dialog::{unsaved_changes_dialog, UnsavedChangesMessage};
use crate::fl;

/// Widget `Id` of the character list's `scrollable`, so a Find-triggered
/// scroll animation (see `focus_character`) can target it with
/// `scrollable::snap_to`.
pub fn characters_scroll_id() -> Id {
    Id::new("characters-scroll")
}

/// Alpha of the dimming shade behind the delete-confirmation overlay.
const SHADE_ALPHA: f32 = 0.3;
/// Padding from both edges for the floating "add character" button.
const ADD_BUTTON_PADDING: f32 = 24.0;
/// Vertical gap (beyond `ADD_BUTTON_PADDING`) between the add button and its
/// hover tooltip floating above it.
const ADD_BUTTON_TOOLTIP_OFFSET: f32 = 56.0;
/// How long a Find-triggered scroll-into-view animation takes.
const SCROLL_ANIM_DURATION: Duration = Duration::from_millis(350);
/// Timing of the Find panel's "found it" glow ring (see `focus_character`);
/// same values as `CanvasPage`'s node glow, for a consistent feel.
const CHARACTER_GLOW_FADE_IN: Duration = Duration::from_millis(200);
const CHARACTER_GLOW_VISIBLE: Duration = Duration::from_millis(1500);
const CHARACTER_GLOW_FADE_OUT: Duration = Duration::from_millis(800);

/// An in-flight Find-triggered scroll animation, driven forward by repeated
/// `CharactersMessage::AnimationTick` messages while it is `Some` — mirrors
/// `nav::canvas`'s `CameraAnimation`, but over a single relative Y offset
/// instead of a `Vector`+zoom pair.
struct ScrollAnimation {
    start: Instant,
    start_y: f32,
    target_y: f32,
}

/// Page model for the Characters page.
pub struct CharactersPage {
    pub characters: Vec<Character>,
    /// Some while a character's editor is open; the list becomes visually
    /// dimmed and non-interactive underneath it (see `view`'s shade). Only
    /// the editor's own Close button clears this.
    pub editor: Option<CharacterCardEditor>,
    /// The card currently under the cursor, if any; shows its hover-delete
    /// button (see `components::character_card`).
    hovered: Option<Uuid>,
    /// Some while a delete confirmation is pending for this character —
    /// set either by a card's hover-delete button or the open editor's own
    /// Delete button. The `ConfirmDialog` is built once at request time
    /// (rather than fresh in `view()`) so its `view()` borrow has somewhere
    /// long-lived (`self`) to borrow from.
    pending_delete: Option<(Uuid, ConfirmDialog)>,

    /// True while the open editor's own Close was pressed with unsaved
    /// draft changes — shows `unsaved_changes_dialog` over the editor
    /// instead of closing it immediately (see `EditorEvent::Closed`).
    pending_unsaved_close: bool,

    /// Drives the floating "add character" button's delayed-fade hover
    /// tooltip.
    add_button_tooltip: HoverTooltip,

    /// Last known relative (0.0-1.0) Y scroll offset — kept up to date by
    /// both the list's own `on_scroll` and, while animating, `AnimationTick`
    /// itself, so a Find-triggered scroll animation always has a real
    /// starting point to interpolate from.
    scroll_relative_y: f32,
    /// Some while a Find-triggered scroll-into-view is in flight; see
    /// `focus_character`.
    scroll_anim: Option<ScrollAnimation>,
    /// The relative Y offset `AnimationTick` wants applied to the list this
    /// frame, if any — taken (and turned into the actual `scrollable::
    /// snap_to` `Task`) by `app::mod`, since only it can return a `Task`.
    pending_scroll: Option<f32>,

    /// `Some((character_id, started_at))` while that character's
    /// Find-triggered "found it" glow ring is fading in/holding/fading out;
    /// see `focus_character`.
    glow: Option<(Uuid, Instant)>,
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
            hovered: None,
            pending_delete: None,
            pending_unsaved_close: false,
            add_button_tooltip: HoverTooltip::default(),
            scroll_relative_y: 0.0,
            scroll_anim: None,
            pending_scroll: None,
            glow: None,
        }
    }
}

impl CharactersPage {
    /// A truly empty page — unlike `Default`, which seeds one placeholder
    /// character for the very first run's UI, this is for resetting an
    /// existing session (e.g. starting a new project) to a blank state.
    pub fn empty() -> Self {
        Self {
            characters: Vec::new(),
            editor: None,
            hovered: None,
            pending_delete: None,
            pending_unsaved_close: false,
            add_button_tooltip: HoverTooltip::default(),
            scroll_relative_y: 0.0,
            scroll_anim: None,
            pending_scroll: None,
            glow: None,
        }
    }

    /// Whether the add-button tooltip is mid-fade; used by the app's
    /// subscription the same way as `CanvasPage::is_animating_camera`.
    pub fn is_add_button_tooltip_active(&self) -> bool {
        self.add_button_tooltip.is_active()
    }

    /// Whether a Find-triggered scroll-into-view is in flight; used by the
    /// app's subscription the same way as `is_add_button_tooltip_active`.
    pub fn is_animating_scroll(&self) -> bool {
        self.scroll_anim.is_some()
    }

    /// Whether a Find-triggered glow ring is still fading in/holding/fading
    /// out; used by the app's subscription the same way as
    /// `is_add_button_tooltip_active`.
    pub fn is_glow_active(&self) -> bool {
        self.glow.is_some_and(|(_, started)| {
            is_pulse_active(started, CHARACTER_GLOW_FADE_IN, CHARACTER_GLOW_VISIBLE, CHARACTER_GLOW_FADE_OUT)
        })
    }

    /// Starts scrolling the list to bring character `id` into view — as
    /// close to centered as the list's current scroll range allows, via a
    /// proportional heuristic (`flex_row` wraps by available width rather
    /// than a fixed column count, so there's no exact pixel position to
    /// compute from model state alone) — and starts its "found it" glow
    /// ring. Used by the Find panel; does nothing if `id` isn't a character
    /// on this page (e.g. a stale result after it was deleted).
    pub fn focus_character(&mut self, id: Uuid) {
        let Some(index) = self.characters.iter().position(|c| c.id == id) else { return };
        let total = self.characters.len();
        let target_y = if total <= 1 { 0.0 } else { index as f32 / (total - 1) as f32 };

        self.scroll_anim = Some(ScrollAnimation {
            start: Instant::now(),
            start_y: self.scroll_relative_y,
            target_y,
        });
        self.glow = Some((id, Instant::now()));
    }

    /// Takes whatever relative Y offset `AnimationTick` computed this frame
    /// for an in-flight Find-triggered scroll animation, if any — `app::mod`
    /// turns it into the actual `scrollable::snap_to` `Task`, since only it
    /// can return one (this page's `update` can't).
    pub fn take_pending_scroll(&mut self) -> Option<f32> {
        self.pending_scroll.take()
    }
}

/// Messages emitted by the Characters page.
#[derive(Debug, Clone)]
pub enum CharactersMessage {
    /// A `character_card` in the list was clicked; open its editor.
    CardClicked(Uuid),
    /// The cursor entered/left a `character_card`; see `hovered`.
    CardHovered(Uuid),
    CardUnhovered,
    /// A card's hover-delete button was clicked; see `pending_delete`.
    DeleteRequested(Uuid),
    /// Forwarded from the open `ConfirmDialog`'s own `view()`.
    ConfirmDelete(ConfirmDialogMessage),
    /// The floating "+" button was clicked; adds a new default character.
    AddCharacter,
    /// The cursor entered/left the floating "+" button; see
    /// `add_button_tooltip`.
    AddButtonHoverEnter,
    AddButtonHoverExit,
    /// Drives `add_button_tooltip`'s fade, an in-flight Find-triggered
    /// scroll animation, and a Find "found it" glow ring forward while any
    /// of them are active; see `AppModel::subscription`.
    AnimationTick,
    /// The list's own `scrollable` reported a new scroll position (user
    /// drag, wheel, or a `snap_to` this page itself requested); see
    /// `scroll_relative_y`.
    Scrolled(Viewport),
    /// Forwarded from the open `CharacterCardEditor`'s own `view()`.
    Editor(EditorMessage),
    /// Forwarded from `unsaved_changes_dialog`'s own `view()`, shown when
    /// the editor's Close was pressed with unsaved changes; see
    /// `pending_unsaved_close`. `Save` additionally needs `AppModel` to
    /// trigger the whole-project save, so `app/mod.rs` intercepts that
    /// variant specifically before forwarding here.
    UnsavedClose(UnsavedChangesMessage),
}

impl CharactersPage {
    pub fn view(&self) -> Element<'_, CharactersMessage> {
        // `flex_row` lays cards out left-to-right, wrapping to a new row
        // (top-to-bottom) once one no longer fits — a grid read in the
        // usual top-left-to-bottom-right order, unlike a plain `Column`
        // which stacked one card per row regardless of available width.
        let cards = self.characters.iter()
            .map(|character| {
                let is_hovered = self.hovered == Some(character.id);
                let glow_alpha = match self.glow {
                    Some((id, started)) if id == character.id => {
                        pulse_alpha(started, CHARACTER_GLOW_FADE_IN, CHARACTER_GLOW_VISIBLE, CHARACTER_GLOW_FADE_OUT)
                    }
                    _ => 0.0,
                };

                character_card(
                    character.name.clone(),
                    character.avatar_path().map(std::path::Path::new),
                    is_hovered,
                    glow_alpha,
                    CharactersMessage::CardClicked(character.id),
                    CharactersMessage::CardHovered(character.id),
                    CharactersMessage::CardUnhovered,
                    CharactersMessage::DeleteRequested(character.id),
                )
            })
            .collect();

        let content = widget::flex_row(cards)
            .width(Length::Fill)
            .padding(12)
            .spacing(12);

        let list: Element<'_, CharactersMessage> = widget::scrollable(content)
            .id(characters_scroll_id())
            .on_scroll(CharactersMessage::Scrolled)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        let add_button = button::icon(icon::from_name("list-add-symbolic"))
            .large()
            .icon_size(25)
            .class(cosmic::theme::Button::Suggested)
            .on_press(CharactersMessage::AddCharacter);

        // No built-in `.tooltip(...)`: that shows instantly on hover, but
        // this one should only fade in after a pause (see `HoverTooltip`) —
        // so hover tracking/the tooltip label are both handled by hand
        // instead.
        let add_button: Element<'_, CharactersMessage> = mouse_area(add_button)
            .on_enter(CharactersMessage::AddButtonHoverEnter)
            .on_exit(CharactersMessage::AddButtonHoverExit)
            .into();

        let tooltip = toast_box(fl!("tooltip-add-character"), self.add_button_tooltip.alpha());

        let list = with_corner_button(list, add_button, tooltip, ADD_BUTTON_PADDING, ADD_BUTTON_TOOLTIP_OFFSET);

        let content = match &self.editor {
            Some(editor) => {
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

            None => list,
        };

        let content = match &self.pending_delete {
            Some((_, dialog)) => with_overlay(content, dialog.view().map(CharactersMessage::ConfirmDelete), SHADE_ALPHA),
            None => content,
        };

        if !self.pending_unsaved_close {
            return content;
        }

        with_overlay(content, unsaved_changes_dialog().map(CharactersMessage::UnsavedClose), SHADE_ALPHA)
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
                // The dimming shade already blocks clicks from reaching a
                // stale hover-delete button underneath, but clearing this
                // too keeps the card from visibly looking "hovered" behind
                // the shade for the rest of the editing session.
                self.hovered = None;
                Some(id)
            }

            CharactersMessage::CardHovered(id) => {
                self.hovered = Some(id);
                None
            }

            CharactersMessage::CardUnhovered => {
                self.hovered = None;
                None
            }

            CharactersMessage::DeleteRequested(id) => {
                self.request_delete(id);
                None
            }

            CharactersMessage::ConfirmDelete(ConfirmDialogMessage::Cancel) => {
                self.pending_delete = None;
                None
            }

            CharactersMessage::ConfirmDelete(ConfirmDialogMessage::Confirm) => {
                if let Some((id, _)) = self.pending_delete.take() {
                    self.characters.retain(|c| c.id != id);
                    self.hovered = None;

                    if self.editor.as_ref().is_some_and(|e| e.character_id == id) {
                        self.close_editor();
                    }
                }
                None
            }

            CharactersMessage::AddCharacter => {
                self.characters.push(Character { name: fl!("character-default-name"), ..Character::default() });
                None
            }

            CharactersMessage::AddButtonHoverEnter => {
                self.add_button_tooltip.enter();
                None
            }

            CharactersMessage::AddButtonHoverExit => {
                self.add_button_tooltip.exit();
                None
            }

            CharactersMessage::AnimationTick => {
                self.add_button_tooltip.tick();

                if let Some(anim) = &self.scroll_anim {
                    let elapsed = anim.start.elapsed().as_secs_f32();
                    let duration = SCROLL_ANIM_DURATION.as_secs_f32();
                    let t = if duration > 0.0 { (elapsed / duration).clamp(0.0, 1.0) } else { 1.0 };
                    let y = cosmic::anim::slerp(anim.start_y, anim.target_y, t);

                    self.scroll_relative_y = y;
                    self.pending_scroll = Some(y);

                    if t >= 1.0 {
                        self.scroll_anim = None;
                    }
                }

                if let Some((_, started)) = self.glow
                    && !is_pulse_active(started, CHARACTER_GLOW_FADE_IN, CHARACTER_GLOW_VISIBLE, CHARACTER_GLOW_FADE_OUT)
                {
                    self.glow = None;
                }

                None
            }

            CharactersMessage::Scrolled(viewport) => {
                // Ignore live-scroll reports while our own animation is
                // driving the position (see `ScrollAnimation`) — otherwise
                // this would immediately stomp `scroll_relative_y` with
                // whatever the widget's still-catching-up real position
                // was the instant before our `snap_to` for this same frame
                // takes effect.
                if self.scroll_anim.is_none() {
                    self.scroll_relative_y = viewport.relative_offset().y;
                }
                None
            }

            CharactersMessage::Editor(message) => {
                let editor = self.editor.as_mut()?;
                let event = editor.update(message);
                let character_id = editor.character_id;

                match event {
                    EditorEvent::None => {}
                    EditorEvent::Saved { name, avatar, comment, description } => {
                        if let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id) {
                            character.name = name;
                            character.avatar = avatar;
                            character.comment = comment;
                            character.description = description;
                        }
                    }
                    EditorEvent::DeleteRequested => {
                        self.request_delete(character_id);
                    }
                    EditorEvent::Closed => {
                        // Warn instead of dropping the draft outright; the
                        // editor stays open until the warning is resolved
                        // (see `pending_unsaved_close` and
                        // `CharactersMessage::UnsavedClose`).
                        if editor.is_dirty() {
                            self.pending_unsaved_close = true;
                        } else {
                            self.close_editor();
                        }
                    }
                }
                None
            }

            CharactersMessage::UnsavedClose(UnsavedChangesMessage::Cancel) => {
                self.pending_unsaved_close = false;
                None
            }

            CharactersMessage::UnsavedClose(UnsavedChangesMessage::Discard) => {
                self.pending_unsaved_close = false;
                self.close_editor();
                None
            }

            CharactersMessage::UnsavedClose(UnsavedChangesMessage::Save) => {
                // The whole-project write itself is handled by `app/mod.rs`
                // (it intercepts this variant before forwarding here) —
                // this only commits the draft into the character and closes.
                self.pending_unsaved_close = false;
                self.commit_editor();
                self.close_editor();
                None
            }
        }
    }

    /// Drops the open editor — the shared second half of every path that
    /// ends an edit session (a clean Close, Discard from the
    /// unsaved-changes warning, a Save-and-close, or the edited character
    /// being deleted out from under it).
    fn close_editor(&mut self) {
        self.editor = None;
    }

    /// Writes the open editor's draft fields into its character, without
    /// closing the editor. Used both by a normal Save-and-close (paired
    /// with `close_editor` right after) and by `AppModel::on_app_exit`'s
    /// unsaved-changes warning, which commits every dirty editor but only
    /// actually exits if that doesn't itself need a dialog (see
    /// `app::project_io::save_project`).
    pub fn commit_editor(&mut self) {
        let Some(editor) = &mut self.editor else { return };
        let character_id = editor.character_id;

        if let EditorEvent::Saved { name, avatar, comment, description } = editor.update(EditorMessage::Save)
            && let Some(character) = self.characters.iter_mut().find(|c| c.id == character_id)
        {
            character.name = name;
            character.avatar = avatar;
            character.comment = comment;
            character.description = description;
        }
    }

    /// Builds the delete-confirmation dialog for character `id` (using its
    /// current name) and stores it as `pending_delete`; shared by a card's
    /// hover-delete button and the open editor's own Delete button.
    fn request_delete(&mut self, id: Uuid) {
        let name = self.characters.iter().find(|c| c.id == id).map(|c| c.name.as_str()).unwrap_or_default();
        let dialog = ConfirmDialog::new(
            fl!("confirm-delete-character-title"),
            fl!("confirm-delete-character-message", name = name),
        );

        self.pending_delete = Some((id, dialog));
    }
}
