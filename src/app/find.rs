//! Find panel orchestration (Ctrl+F / the Action menu's Find item):
//! building the current search results from both pages' data, and
//! resolving a selection into the page-specific pan/scroll+glow effect
//! (`CanvasPage::focus_node`/`CharactersPage::focus_character`). The panel
//! itself only holds its own UI state (query/target/highlighted row) and
//! `view()` — see `components::find_panel`.

use cosmic::Element;
use cosmic::Task;

use crate::components::find_panel::{FindMessage, FindResult, FindTarget};
use crate::components::overlay::with_corner_panel;

use super::{AppModel, Message, Page};

/// Padding from both edges for the floating Find panel.
const FIND_PANEL_PADDING: f32 = 24.0;

impl AppModel {
    /// Layers the Find panel (if open) in the top-right corner of `content`.
    /// Unlike every overlay in `overlays.rs`, this is non-blocking — the
    /// page underneath stays fully interactive (pan/drag/scroll) while it's
    /// open (see `with_corner_panel`).
    pub(super) fn apply_find_panel<'a>(&'a self, content: Element<'a, Message>) -> Element<'a, Message> {
        let Some(panel) = &self.find_panel else { return content };
        let results = self.find_results();

        with_corner_panel(content, panel.view(&results).map(Message::Find), FIND_PANEL_PADDING)
    }

    /// Builds the current match list for the open Find panel's query/
    /// target — empty if the panel isn't open. Shared by `apply_find_panel`
    /// (to render it) and `find_select`/the arrow-key handling in
    /// `Message::Key` (to resolve a row index back to an id), so both
    /// always agree on what "result 2" actually is.
    pub(super) fn find_results(&self) -> Vec<FindResult> {
        let Some(panel) = &self.find_panel else { return Vec::new() };
        let query = panel.query.to_lowercase();

        match panel.target {
            FindTarget::Node => self.canvas.nodes.iter()
                .filter(|node| {
                    query.is_empty()
                        || node.title.to_lowercase().contains(&query)
                        // Content counts too: a node whose blocks (prose,
                        // choice labels, directives) mention the query is a
                        // match, still labeled by its title.
                        || node.blocks.iter().any(|block| block.matches_query(&query))
                })
                .map(|node| FindResult { id: node.id, label: node.title.clone() })
                .collect(),
            FindTarget::Character => self.characters.characters.iter()
                .filter(|character| query.is_empty() || character.name.to_lowercase().contains(&query))
                .map(|character| FindResult { id: character.id, label: character.name.clone() })
                .collect(),
        }
    }

    /// Acts on Find result `index` — a click, or Enter/Confirm on whichever
    /// row is currently highlighted: switches to the relevant page (if not
    /// already there) and starts its pan/scroll+glow effect. The panel
    /// itself stays open afterward — only opening an editor closes it (see
    /// the `Message::Canvas`/`Message::Characters` arms in `update`).
    pub(super) fn find_select(&mut self, index: usize) {
        let Some(target) = self.find_panel.as_ref().map(|panel| panel.target) else { return };
        let results = self.find_results();
        let Some(result) = results.get(index) else { return };
        let id = result.id;

        match target {
            FindTarget::Node => {
                self.activate_page(Page::Canvas);
                self.canvas.focus_node(id);
            }
            FindTarget::Character => {
                self.activate_page(Page::Characters);
                self.characters.focus_character(id);
            }
        }
    }

    /// Handles every `Message::Find` variant — like `SimplePopup`, the
    /// panel itself has no `update()` of its own, since resolving a result
    /// needs cross-page state it doesn't have.
    pub(super) fn handle_find(&mut self, message: FindMessage) -> Task<cosmic::Action<Message>> {
        match message {
            FindMessage::QueryChanged(query) => {
                if let Some(panel) = &mut self.find_panel {
                    panel.query = query;
                    panel.highlighted = 0;
                }
            }
            FindMessage::TargetChanged(index) => {
                if let Some(panel) = &mut self.find_panel {
                    panel.target = FindTarget::from_index(index);
                    panel.highlighted = 0;
                }
            }
            FindMessage::ResultHovered(index) => {
                if let Some(panel) = &mut self.find_panel {
                    panel.highlighted = index;
                }
            }
            FindMessage::ResultClicked(index) => self.find_select(index),
            FindMessage::Confirm => {
                let index = self.find_panel.as_ref().map_or(0, |panel| panel.highlighted);
                self.find_select(index);
            }
            FindMessage::Close => {
                self.find_panel = None;
            }
        }
        Task::none()
    }
}
