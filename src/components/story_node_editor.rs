use std::collections::HashMap;

use cosmic::iced::{
    Alignment, Background, Border, Color, ContentFit, Length, mouse,
    alignment::{Horizontal, Vertical},
    advanced::text::{Wrapping, Ellipsize, EllipsizeHeightLimit},
    widget::{Stack, pin},
};
use cosmic::widget::{
    self, Column, Row, Space,
    text::{title4, caption, body},
    button, container, icon, image, mouse_area, text_editor, text_input,
};
use cosmic::Element;
use uuid::Uuid;

use crate::components::{Character, ConfirmDialog, StoryNode, display_title};
use crate::components::confirm_dialog::ConfirmDialogMessage;
use crate::components::overlay::with_overlay;
use crate::components::story_block::{BlockContent, BlockKind, ChoiceOption, StoryBlock};
use crate::fl;

/// Alpha of the dimming shade behind the block delete-confirmation
/// overlay; same value as the canvas/characters pages use.
const SHADE_ALPHA: f32 = 0.3;

/// Widest a block bubble gets, messaging-app style — prose stays a
/// readable column instead of stretching across the whole panel.
const BUBBLE_MAX_WIDTH: f32 = 420.0;
/// Fixed width of a *dialogue* block's gutter (avatar + speaker dropdown
/// need stable room); right-side gutters (narrator avatar/kind labels)
/// shrink to their content so the drag pill hugs them.
const GUTTER_WIDTH: f32 = 110.0;
/// Avatars are large rounded squares per the design mock, not small
/// circles.
const AVATAR_SIZE: f32 = 72.0;
const AVATAR_RADIUS: f32 = 16.0;
/// How far a preview line in the drag ghost may run before truncating.
const GHOST_PREVIEW_CHARS: usize = 24;
/// How far (vertically) a press must travel before it stops being a
/// click-to-edit and becomes a drag-to-reorder. Same idea as the canvas's
/// click threshold; `mouse_area`'s built-in `on_drag` fires at >1px,
/// which ordinary click jitter would trip constantly.
const DRAG_START_THRESHOLD: f32 = 6.0;
/// Fixed footprint of the drag pill, whether visible (row hovered) or a
/// placeholder — so rows don't shift sideways as the pill appears.
const PILL_WIDTH: f32 = 16.0;
const PILL_HEIGHT: f32 = 40.0;
/// Width of a choice option's inline target selector (the speaker
/// selector instead fills its gutter).
const TARGET_SELECTOR_WIDTH: f32 = 140.0;

/// Which inline dropdown a given toggle press refers to. The cosmic
/// `dropdown` widget's overlay menu never opened inside this editor's
/// nested mouse-tracking wrappers, so selection uses these hand-rolled
/// *inline* dropdowns instead: pressing the "▼ <selected>" face expands
/// the entry list right below it (matching the design mock), no overlay
/// involved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropdownId {
    /// A dialogue block's speaker selector, by block id.
    Speaker(Uuid),
    /// A choice option's target selector.
    ChoiceTarget { block: Uuid, option: Uuid },
}

/// An in-flight drag-to-reorder gesture, started by pressing a block row's
/// drag pill and finished by releasing (drop) or leaving the list (cancel).
struct DragState {
    /// The block being dragged.
    block: Uuid,
    /// The cursor's Y within the block-list area, tracked by the list
    /// wrapper's `on_move` — where the floating ghost is pinned. `None`
    /// until the first move arrives (no ghost yet).
    cursor_y: Option<f32>,
    /// The row index the cursor last hovered, i.e. where the block will
    /// land on drop; `None` until some row is crossed.
    over_index: Option<usize>,
}

/// A side panel for editing a single `StoryNode`: its title plus the
/// ordered list of content blocks (see `components::story_block`), laid
/// out like a messaging thread — character dialogue on the left, narrator
/// and everything else on the right. Prose blocks read as plain text and
/// switch into a Save/Cancel edit mode when clicked; blocks reorder by
/// dragging their side pill; a block deletes via the hover-only "✕" over
/// its bubble. Structural edits (add/remove/reorder, choice/directive
/// fields) write straight through to the node; prose and the title only
/// commit on their explicit Save. None of it reaches disk on its own
/// though; only the File menu's Save/Ctrl+S does that (see
/// `AppModel::save_project`).
pub struct StoryNodeEditor {
    /// Which `StoryNode` (by id) this editor session is for.
    pub node_id: Uuid,
    /// The node's current (committed) title.
    pub title: String,
    /// `Some(draft)` while the header is in title-editing mode — the draft
    /// only replaces `title` (and reaches the node) on Save; Cancel just
    /// drops it.
    title_draft: Option<String>,
    /// The live block list; every committed mutation is reported back
    /// whole via `EditorEvent::BlocksChanged` for the caller to write
    /// through.
    blocks: Vec<StoryBlock>,
    /// `text_editor` widget state for each prose block (narration/
    /// dialogue/note bodies), keyed by block id — kept beside `blocks`
    /// rather than inside them because `text_editor::Content` isn't
    /// `Clone`/serializable. While a block is being edited this holds the
    /// *draft*; it only reaches `blocks` via `commit_prose`.
    prose_contents: HashMap<Uuid, text_editor::Content>,
    /// The block row currently under the cursor, if any — shows that
    /// bubble's hover-only delete button.
    hovered_block: Option<Uuid>,
    /// The prose block currently in edit mode (expanded `text_editor` +
    /// Save/Cancel), if any. Entering another block's edit mode commits
    /// this one — Cancel is the only path that discards a draft.
    editing_block: Option<Uuid>,
    /// The inline dropdown currently expanded, if any; see `DropdownId`.
    open_dropdown: Option<DropdownId>,
    /// A press on a read-mode prose bubble that hasn't decided yet whether
    /// it's a click (edit on release) or a drag (promoted once the cursor
    /// travels `DRAG_START_THRESHOLD` vertically): `(block, first tracked
    /// cursor Y)`.
    pending_drag: Option<(Uuid, Option<f32>)>,
    /// Some while a drag-to-reorder is in flight; see `DragState`.
    drag: Option<DragState>,
    /// Preferences mirror: whether removing a block asks first — set by
    /// `CanvasPage` at construction and on preference changes.
    pub confirm_delete_blocks: bool,
    /// Some while a block delete confirmation is pending for this block.
    pending_delete: Option<(Uuid, ConfirmDialog)>,
    /// Set when the delete dialog's "Don't ask again" was accepted —
    /// polled by `CanvasPage::take_confirm_disables`.
    confirm_disable: bool,
}

/// Widget-level messages from the editor's own `view()`.
#[derive(Debug, Clone)]
pub enum EditorMessage {
    /// The header's "Edit" button was pressed; enter title-editing mode.
    TitleEdit,
    /// The title `text_input` changed while in title-editing mode.
    TitleDraftChanged(String),
    /// "Save" (or Enter in the input) was pressed; commit the draft.
    TitleSave,
    /// "Cancel" was pressed; drop the draft.
    TitleCancel,
    /// One of the add-block toolbar buttons was pressed; appends a fresh
    /// block.
    AddBlock(BlockKind),
    /// A bubble's hover-only "✕" button was pressed; removes the block —
    /// after a confirmation dialog when the Preferences toggle is on.
    RemoveBlock(Uuid),
    /// Forwarded from the block delete-confirmation dialog's own `view()`.
    ConfirmDelete(ConfirmDialogMessage),
    /// The cursor entered/left a block row (while not dragging); see
    /// `hovered_block`.
    BlockHoverEnter(Uuid),
    BlockHoverExit,
    /// A read-mode prose bubble was pressed — could become a click (edit)
    /// or a drag; see `pending_drag`.
    BlockPressed(Uuid),
    /// The press on that bubble was released without turning into a drag;
    /// enter its edit mode.
    BlockReleased(Uuid),
    /// An edit/cursor action on the editing prose block's `text_editor` —
    /// mutates only the draft, not the node.
    ProseAction(Uuid, text_editor::Action),
    /// The editing bubble's "Save"/"Cancel" was pressed.
    ProseSave(Uuid),
    ProseCancel(Uuid),
    /// A block row's drag pill was pressed; start dragging it immediately
    /// (the pill is a dedicated handle — no click meaning to preserve).
    DragStart(Uuid),
    /// The cursor moved to `y` within the list area while dragging (moves
    /// the ghost) or while a press is pending (checks the drag threshold).
    DragMoved(f32),
    /// The cursor crossed row `index` while dragging; see
    /// `DragState::over_index`.
    DragOverRow(usize),
    /// The button was released over the list; drop the block at
    /// `over_index`.
    DragDrop,
    /// The cursor left the list mid-drag; abandon the gesture.
    DragCancel,
    /// An inline dropdown's "▼" face was pressed; expand it (or collapse
    /// it if it was the open one).
    DropdownToggled(DropdownId),
    /// A dialogue block's speaker selector picked a character (`None` =
    /// the explicit "(None)" entry).
    SpeakerPicked(Uuid, Option<Uuid>),
    /// A directive block's command/argument `text_input` changed.
    DirectiveCommandChanged(Uuid, String),
    DirectiveArgumentChanged(Uuid, String),
    /// A choice option's label `text_input` changed.
    ChoiceLabelChanged { block: Uuid, option: Uuid, label: String },
    /// A choice option's target selector picked a node (`None` = the
    /// explicit "(None)" entry).
    ChoiceTargetPicked { block: Uuid, option: Uuid, target: Option<Uuid> },
    /// A choice block's "Add option" button was pressed.
    AddChoiceOption(Uuid),
    /// A choice option's remove button was pressed.
    RemoveChoiceOption { block: Uuid, option: Uuid },
    /// The "Close" button was pressed. (No Delete here — deleting a node
    /// is the canvas hover-delete button's job.)
    Close,
}

/// What `StoryNodeEditor::update` reports back to its caller (`CanvasPage`)
/// after handling an `EditorMessage`, so the canvas can react — e.g. write
/// the change into the actual `StoryNode`, or tear down the editor and
/// animate the camera back on `Closed`.
pub enum EditorEvent {
    /// Nothing for the caller to do (hover/drag bookkeeping, or a draft
    /// edit that hasn't been committed).
    None,
    /// A title edit was *saved* and should be written through to the node —
    /// unlike structural edits, the title doesn't stream per keystroke
    /// (see `title_draft`).
    TitleChanged(String),
    /// The block list changed in some way (committed text, structure, or
    /// order); the caller should write the whole list through to the node.
    BlocksChanged(Vec<StoryBlock>),
    /// The editor was closed and should be dropped. Carries a final block
    /// write-through when closing auto-committed an in-flight prose draft
    /// (see the `Close` arm), so the draft isn't silently lost.
    Closed(Option<Vec<StoryBlock>>),
}

impl StoryNodeEditor {
    pub fn new(node: &StoryNode) -> Self {
        let prose_contents = node.blocks.iter()
            .filter_map(|block| {
                block.prose().map(|text| (block.id, text_editor::Content::with_text(text)))
            })
            .collect();

        Self {
            node_id: node.id,
            title: node.title.clone(),
            title_draft: None,
            blocks: node.blocks.clone(),
            prose_contents,
            hovered_block: None,
            editing_block: None,
            open_dropdown: None,
            pending_drag: None,
            drag: None,
            confirm_delete_blocks: false,
            pending_delete: None,
            confirm_disable: false,
        }
    }

    /// Takes whether the block delete dialog's "Don't ask again" was
    /// accepted since last polled; see `CanvasPage::take_confirm_disables`.
    pub fn take_confirm_disable(&mut self) -> bool {
        std::mem::take(&mut self.confirm_disable)
    }

    /// Applies an `EditorMessage`, updating this editor's own live copy so
    /// `view()` reflects it, and reports back what the caller should write
    /// through to the actual node.
    pub fn update(&mut self, message: EditorMessage) -> EditorEvent {
        match message {
            EditorMessage::TitleEdit => {
                self.title_draft = Some(self.title.clone());
                EditorEvent::None
            }

            EditorMessage::TitleDraftChanged(value) => {
                if let Some(draft) = &mut self.title_draft {
                    *draft = value;
                }
                EditorEvent::None
            }

            EditorMessage::TitleSave => {
                match self.title_draft.take() {
                    Some(draft) => {
                        self.title = draft.clone();
                        EditorEvent::TitleChanged(draft)
                    }
                    None => EditorEvent::None,
                }
            }

            EditorMessage::TitleCancel => {
                self.title_draft = None;
                EditorEvent::None
            }

            EditorMessage::AddBlock(kind) => {
                let block = StoryBlock::new(kind);
                if let Some(text) = block.prose() {
                    self.prose_contents.insert(block.id, text_editor::Content::with_text(text));
                }
                self.blocks.push(block);
                self.blocks_changed()
            }

            EditorMessage::RemoveBlock(id) => {
                if self.confirm_delete_blocks {
                    self.pending_delete = Some((
                        id,
                        ConfirmDialog::new(
                            fl!("confirm-delete-block-title"),
                            fl!("confirm-delete-block-message"),
                        )
                        .with_dont_ask(),
                    ));
                    return EditorEvent::None;
                }
                self.remove_block(id)
            }

            EditorMessage::ConfirmDelete(ConfirmDialogMessage::Cancel) => {
                self.pending_delete = None;
                EditorEvent::None
            }

            EditorMessage::ConfirmDelete(ConfirmDialogMessage::DontAskToggled(checked)) => {
                if let Some((_, dialog)) = &mut self.pending_delete {
                    dialog.dont_ask = Some(checked);
                }
                EditorEvent::None
            }

            EditorMessage::ConfirmDelete(ConfirmDialogMessage::Confirm) => {
                match self.pending_delete.take() {
                    Some((id, dialog)) => {
                        // "Don't ask again": stop confirming block deletes
                        // (`CanvasPage` polls `take_confirm_disable` to
                        // persist it).
                        if dialog.dont_ask_again() {
                            self.confirm_delete_blocks = false;
                            self.confirm_disable = true;
                        }
                        self.remove_block(id)
                    }
                    None => EditorEvent::None,
                }
            }

            EditorMessage::BlockHoverEnter(id) => {
                self.hovered_block = Some(id);
                EditorEvent::None
            }

            EditorMessage::BlockHoverExit => {
                self.hovered_block = None;
                EditorEvent::None
            }

            EditorMessage::BlockPressed(id) => {
                self.pending_drag = Some((id, None));
                EditorEvent::None
            }

            EditorMessage::BlockReleased(id) => {
                let was_pending = matches!(self.pending_drag.take(), Some((block, _)) if block == id);
                if was_pending && self.drag.is_none() {
                    return self.begin_edit(id);
                }
                EditorEvent::None
            }

            EditorMessage::ProseAction(id, action) => {
                if let Some(content) = self.prose_contents.get_mut(&id) {
                    content.perform(action);
                }
                EditorEvent::None
            }

            EditorMessage::ProseSave(id) => {
                if self.editing_block == Some(id) {
                    self.editing_block = None;
                }
                self.commit_prose(id)
            }

            EditorMessage::ProseCancel(id) => {
                if self.editing_block == Some(id) {
                    self.editing_block = None;
                }
                // Rebuild the widget state from the committed text,
                // dropping the draft.
                if let Some(block) = self.blocks.iter().find(|block| block.id == id)
                    && let Some(text) = block.prose()
                {
                    self.prose_contents.insert(id, text_editor::Content::with_text(text));
                }
                EditorEvent::None
            }

            EditorMessage::DragStart(id) => {
                self.pending_drag = None;
                self.hovered_block = None;
                self.drag = Some(DragState { block: id, cursor_y: None, over_index: None });
                EditorEvent::None
            }

            EditorMessage::DragMoved(y) => {
                if let Some(drag) = &mut self.drag {
                    drag.cursor_y = Some(y);
                    return EditorEvent::None;
                }

                // A press is pending: the first move anchors the origin;
                // traveling past the threshold promotes it to a real drag
                // (a release before that stays a click — see
                // `BlockReleased`).
                if let Some((block, origin)) = &mut self.pending_drag {
                    match origin {
                        None => *origin = Some(y),
                        Some(origin) if (y - *origin).abs() > DRAG_START_THRESHOLD => {
                            let block = *block;
                            self.pending_drag = None;
                            self.hovered_block = None;
                            self.drag = Some(DragState {
                                block,
                                cursor_y: Some(y),
                                over_index: None,
                            });
                        }
                        Some(_) => {}
                    }
                }
                EditorEvent::None
            }

            EditorMessage::DragOverRow(index) => {
                if let Some(drag) = &mut self.drag {
                    drag.over_index = Some(index);
                }
                EditorEvent::None
            }

            EditorMessage::DragCancel => {
                self.pending_drag = None;
                self.drag = None;
                EditorEvent::None
            }

            EditorMessage::DragDrop => {
                self.pending_drag = None;
                let Some(drag) = self.drag.take() else {
                    return EditorEvent::None;
                };
                let Some(from) = self.blocks.iter().position(|block| block.id == drag.block) else {
                    return EditorEvent::None;
                };
                let Some(to) = drag.over_index else {
                    return EditorEvent::None;
                };
                if from == to || to >= self.blocks.len() {
                    return EditorEvent::None;
                }

                let block = self.blocks.remove(from);
                self.blocks.insert(to, block);
                self.blocks_changed()
            }

            EditorMessage::DropdownToggled(id) => {
                self.open_dropdown = if self.open_dropdown == Some(id) { None } else { Some(id) };
                EditorEvent::None
            }

            EditorMessage::SpeakerPicked(id, new_speaker) => {
                self.open_dropdown = None;
                if let Some(block) = self.blocks.iter_mut().find(|block| block.id == id)
                    && let BlockContent::Dialogue { speaker, .. } = &mut block.content
                {
                    *speaker = new_speaker;
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::DirectiveCommandChanged(id, value) => {
                if let Some(block) = self.blocks.iter_mut().find(|block| block.id == id)
                    && let BlockContent::Directive { command, .. } = &mut block.content
                {
                    *command = value;
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::DirectiveArgumentChanged(id, value) => {
                if let Some(block) = self.blocks.iter_mut().find(|block| block.id == id)
                    && let BlockContent::Directive { argument, .. } = &mut block.content
                {
                    *argument = value;
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::ChoiceLabelChanged { block, option, label } => {
                if let Some(option) = self.find_choice_option(block, option) {
                    option.label = label;
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::ChoiceTargetPicked { block, option, target } => {
                self.open_dropdown = None;
                if let Some(option) = self.find_choice_option(block, option) {
                    option.target = target;
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::AddChoiceOption(id) => {
                if let Some(block) = self.blocks.iter_mut().find(|block| block.id == id)
                    && let BlockContent::Choice { options } = &mut block.content
                {
                    options.push(ChoiceOption::default());
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::RemoveChoiceOption { block, option } => {
                if self.open_dropdown == Some(DropdownId::ChoiceTarget { block, option }) {
                    self.open_dropdown = None;
                }
                if let Some(found) = self.blocks.iter_mut().find(|b| b.id == block)
                    && let BlockContent::Choice { options } = &mut found.content
                {
                    options.retain(|o| o.id != option);
                    return self.blocks_changed();
                }
                EditorEvent::None
            }

            EditorMessage::Close => {
                // Auto-commit any in-flight prose draft instead of
                // silently dropping it; the event carries the result since
                // only one event can be returned.
                let pending = self.editing_block.take().and_then(|id| {
                    match self.commit_prose(id) {
                        EditorEvent::BlocksChanged(blocks) => Some(blocks),
                        _ => None,
                    }
                });
                EditorEvent::Closed(pending)
            }
        }
    }

    fn blocks_changed(&self) -> EditorEvent {
        EditorEvent::BlocksChanged(self.blocks.clone())
    }

    /// Actually removes block `id` and clears any state pointing at it —
    /// the shared second half of a confirmed dialog and of a
    /// confirmation-free delete.
    fn remove_block(&mut self, id: Uuid) -> EditorEvent {
        self.blocks.retain(|block| block.id != id);
        self.prose_contents.remove(&id);
        if self.editing_block == Some(id) {
            self.editing_block = None;
        }
        if self.hovered_block == Some(id) {
            self.hovered_block = None;
        }
        if matches!(
            self.open_dropdown,
            Some(DropdownId::Speaker(block) | DropdownId::ChoiceTarget { block, .. }) if block == id
        ) {
            self.open_dropdown = None;
        }
        self.blocks_changed()
    }

    /// Enters `id`'s prose edit mode. Switching from another in-flight
    /// edit commits it — Cancel is the only path that discards a draft.
    fn begin_edit(&mut self, id: Uuid) -> EditorEvent {
        if self.editing_block == Some(id) {
            return EditorEvent::None;
        }
        let event = match self.editing_block.take() {
            Some(previous) => self.commit_prose(previous),
            None => EditorEvent::None,
        };
        self.editing_block = Some(id);
        event
    }

    /// Writes the draft in `prose_contents[id]` into its block, reporting
    /// `BlocksChanged` — or `None` when there's nothing to commit or the
    /// draft matches the committed text.
    fn commit_prose(&mut self, id: Uuid) -> EditorEvent {
        let Some(content) = self.prose_contents.get(&id) else {
            return EditorEvent::None;
        };
        let text = content.text();

        let Some(block) = self.blocks.iter_mut().find(|block| block.id == id) else {
            return EditorEvent::None;
        };
        if block.prose() == Some(text.as_str()) {
            return EditorEvent::None;
        }
        block.set_prose(text);
        self.blocks_changed()
    }

    fn find_choice_option(&mut self, block: Uuid, option: Uuid) -> Option<&mut ChoiceOption> {
        let block = self.blocks.iter_mut().find(|b| b.id == block)?;
        match &mut block.content {
            BlockContent::Choice { options } => options.iter_mut().find(|o| o.id == option),
            _ => None,
        }
    }

    /// `characters` (the Characters tab's cast, for dialogue avatars and
    /// speaker dropdowns) and `nodes` (every node on the canvas, for choice
    /// target dropdowns) are only read to build owned widget data —
    /// dangling references simply don't resolve to a selection.
    /// `preview_lines` is the Preferences value for how many lines a
    /// read-mode prose bubble shows before ellipsizing.
    pub fn view<'a>(
        &'a self,
        characters: &[Character],
        nodes: &[StoryNode],
        preview_lines: usize,
    ) -> Element<'a, EditorMessage> {
        // Header: "Editing <title>" + Edit/Close normally; while a title
        // edit is in flight the label becomes an input and Edit/Close give
        // way to Save/Cancel. (No Delete button — the canvas hover-delete
        // already covers removing a node.)
        let header = match &self.title_draft {
            None => Row::new()
                .push(
                    title4(format!("{} {}", fl!("editor-label"), self.title))
                        .width(Length::Fill)
                        .wrapping(Wrapping::None)
                        .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1))),
                )
                .push(button::text(fl!("editor-edit")).on_press(EditorMessage::TitleEdit))
                .push(button::text(fl!("editor-close")).on_press(EditorMessage::Close)),
            Some(draft) => Row::new()
                .push(title4(fl!("editor-label")))
                .push(
                    text_input(fl!("editor-title-placeholder"), draft.as_str())
                        .on_input(EditorMessage::TitleDraftChanged)
                        .on_submit(|_| EditorMessage::TitleSave),
                )
                .push(
                    button::text(fl!("editor-save"))
                        .class(cosmic::theme::Button::Suggested)
                        .on_press(EditorMessage::TitleSave),
                )
                .push(button::text(fl!("editor-cancel")).on_press(EditorMessage::TitleCancel)),
        }
        .spacing(10)
        .align_y(Alignment::Center);

        // The add-block toolbar: one {icon + label} button per kind, in a
        // horizontally scrolling row so more kinds can join later without
        // wrapping the layout. Sized ~20% below the stock text button
        // (height 32/font 14/icon 16) so the toolbar stays unobtrusive.
        let toolbar_buttons: Vec<Element<'a, EditorMessage>> = BlockKind::ALL.iter()
            .map(|kind| {
                button::text(kind.label())
                    .leading_icon(icon::from_name(kind.icon_name()))
                    .height(26.0)
                    .padding([0, 8])
                    .font_size(12)
                    .line_height(16)
                    .icon_size(13)
                    .on_press(EditorMessage::AddBlock(*kind))
                    .into()
            })
            .collect();
        let toolbar = widget::scrollable::horizontal(
            Row::with_children(toolbar_buttons).spacing(8),
        )
        .width(Length::Fill);

        // The thread itself, with the drop-position hint line woven in
        // while a drag is in flight: the line sits above the hovered row
        // when dragging upward, below it when dragging downward — matching
        // where `DragDrop`'s remove+insert actually lands the block.
        let drag_from = self.drag.as_ref()
            .and_then(|drag| self.blocks.iter().position(|block| block.id == drag.block));
        let drag_over = self.drag.as_ref().and_then(|drag| drag.over_index);

        let mut list = Column::new().spacing(16);
        for (index, block) in self.blocks.iter().enumerate() {
            if let (Some(from), Some(over)) = (drag_from, drag_over)
                && over == index && over < from
            {
                list = list.push(insertion_line());
            }
            list = list.push(self.block_row(index, block, characters, nodes, preview_lines));
            if let (Some(from), Some(over)) = (drag_from, drag_over)
                && over == index && over > from
            {
                list = list.push(insertion_line());
            }
        }

        let list = widget::scrollable(list).width(Length::Fill).height(Length::Fill);

        // While a drag is in flight — or a press is still deciding whether
        // it's one — the list gets wrapped to track the gesture: cursor
        // moves feed the threshold check/ghost position, releases drop,
        // and leaving cancels. The ghost itself (pinned at the cursor,
        // stacked above everything — the "raised z-index") only exists
        // once a real drag started.
        let thread: Element<'a, EditorMessage> = if self.drag.is_some() || self.pending_drag.is_some() {
            let mut layers = Stack::new().push(list);

            if let Some(drag) = &self.drag
                && let Some(y) = drag.cursor_y
                && let Some(block) = self.blocks.iter().find(|block| block.id == drag.block)
            {
                layers = layers.push(pin(drag_ghost(block)).x(48.0).y(y - 16.0));
            }

            let interaction = if self.drag.is_some() {
                mouse::Interaction::Grabbing
            } else {
                mouse::Interaction::Idle
            };

            mouse_area(layers)
                .on_move(|position| EditorMessage::DragMoved(position.y))
                .on_release(EditorMessage::DragDrop)
                .on_exit(EditorMessage::DragCancel)
                .interaction(interaction)
                .into()
        } else {
            list.into()
        };

        let content: Element<'a, EditorMessage> = Column::new()
            .push(header)
            .push(toolbar)
            .push(
                // `clip(true)` hard-bounds the thread's rendering to its
                // own area, so scrolled-out content can't bleed over the
                // header/toolbar above.
                container(thread)
                    .clip(true)
                    .width(Length::Fill)
                    .height(Length::Fill),
            )
            .spacing(12)
            .padding(16)
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // The block delete-confirmation dialog dims and blocks the whole
        // editor while pending, same as the pages' delete dialogs.
        match &self.pending_delete {
            Some((_, dialog)) => {
                with_overlay(content, dialog.view().map(EditorMessage::ConfirmDelete), SHADE_ALPHA)
            }
            None => content,
        }
    }

    /// One block as a messaging-thread row: character dialogue sits left
    /// (avatar + speaker dropdown in the gutter), everything else sits
    /// right (fixed narrator avatar for narration, plain kind label
    /// otherwise). A drag pill rides the row's outer edge, and the bubble
    /// grows a hover-only "✕" while the cursor is over the row.
    fn block_row<'a>(
        &'a self,
        index: usize,
        block: &'a StoryBlock,
        characters: &[Character],
        nodes: &[StoryNode],
        preview_lines: usize,
    ) -> Element<'a, EditorMessage> {
        // The drag pill only materializes while its row is hovered (and no
        // drag is in flight — the row's own `on_release` handles drops);
        // otherwise an identically-sized placeholder keeps the row from
        // shifting sideways. The whole read-mode bubble is draggable too
        // (see below), so the pill is an affordance more than the only
        // handle.
        let pill: Element<'a, EditorMessage> =
            if self.hovered_block == Some(block.id) && self.drag.is_none() {
                mouse_area(
                    container(
                        Column::new()
                            .push(icon::from_name("pan-up-symbolic").icon().size(10))
                            .push(icon::from_name("pan-down-symbolic").icon().size(10))
                            .align_x(Horizontal::Center),
                    )
                    .width(Length::Fixed(PILL_WIDTH))
                    .height(Length::Fixed(PILL_HEIGHT))
                    .align_x(Horizontal::Center)
                    .align_y(Vertical::Center)
                    .class(cosmic::theme::Container::Card),
                )
                .on_press(EditorMessage::DragStart(block.id))
                .interaction(mouse::Interaction::Grab)
                .into()
            } else {
                Space::new()
                    .width(Length::Fixed(PILL_WIDTH))
                    .height(Length::Fixed(PILL_HEIGHT))
                    .into()
            };
        // Centered against the avatar band, mock-style, instead of
        // hugging the row's top edge.
        let pill: Element<'a, EditorMessage> = container(pill)
            .height(Length::Fixed(AVATAR_SIZE))
            .align_y(Vertical::Center)
            .into();

        let gutter: Element<'a, EditorMessage> = match &block.content {
            // Dialogue's gutter keeps a fixed width so the speaker
            // dropdown has stable room; right-side gutters shrink to
            // their label/avatar so the drag pill hugs the content the
            // way it does on the left.
            BlockContent::Dialogue { speaker, .. } => container(
                Column::new()
                    .push(dialogue_avatar(*speaker, characters))
                    .push(speaker_selector(
                        block.id,
                        *speaker,
                        characters,
                        self.open_dropdown == Some(DropdownId::Speaker(block.id)),
                    ))
                    .spacing(6),
            )
            .width(Length::Fixed(GUTTER_WIDTH))
            .into(),
            BlockContent::Narration { .. } => Column::new()
                .push(narrator_avatar())
                .push(caption(fl!("block-narrator-label")))
                .spacing(6)
                .into(),
            _ => caption(block.kind().label()).into(),
        };
        let gutter = container(gutter);

        let is_editing = self.editing_block == Some(block.id);
        let body: Element<'a, EditorMessage> = match &block.content {
            BlockContent::Narration { .. } => {
                self.prose_body(block, is_editing, fl!("block-narration-placeholder"), preview_lines)
            }
            BlockContent::Dialogue { .. } => {
                self.prose_body(block, is_editing, fl!("block-dialogue-placeholder"), preview_lines)
            }
            BlockContent::Note { .. } => {
                self.prose_body(block, is_editing, fl!("block-note-placeholder"), preview_lines)
            }
            BlockContent::Choice { options } => {
                choice_body(self.node_id, block.id, options, nodes, self.open_dropdown)
            }
            BlockContent::Directive { command, argument } => {
                let block_id = block.id;
                Row::new()
                    .push(
                        text_input(fl!("block-directive-command-placeholder"), command.as_str())
                            .on_input(move |value| EditorMessage::DirectiveCommandChanged(block_id, value))
                            .width(Length::FillPortion(2)),
                    )
                    .push(
                        text_input(fl!("block-directive-argument-placeholder"), argument.as_str())
                            .on_input(move |value| EditorMessage::DirectiveArgumentChanged(block_id, value))
                            .width(Length::FillPortion(3)),
                    )
                    .spacing(8)
                    .into()
            }
        };

        // Bubbles hold a constant width — filling the row up to
        // BUBBLE_MAX_WIDTH, messaging-app style — and grow downward with
        // their content; author notes keep their own tint so they never
        // read as story content at a glance.
        let bubble = container(body)
            .padding(14)
            .width(Length::FillPortion(4))
            .max_width(BUBBLE_MAX_WIDTH);
        let bubble: Element<'a, EditorMessage> = match block.kind() {
            BlockKind::Note => bubble.style(note_card_style).into(),
            _ => bubble.class(cosmic::theme::Container::Card).into(),
        };

        // A read-mode prose bubble is one big press target: a clean
        // click (release without crossing the drag threshold) enters edit
        // mode, movement while held becomes a drag-to-reorder. While a
        // drag is in flight it turns into a drop target instead —
        // `mouse_area` captures left releases unconditionally within its
        // bounds, so without `on_release(DragDrop)` here a release over
        // the bubble would never reach the wrapper and the ghost would
        // stay latched to the cursor.
        let bubble: Element<'a, EditorMessage> = if block.prose().is_some() && !is_editing {
            let area = mouse_area(bubble).interaction(mouse::Interaction::Pointer);
            if self.drag.is_some() {
                area.on_release(EditorMessage::DragDrop).into()
            } else {
                area.on_press(EditorMessage::BlockPressed(block.id))
                    .on_release(EditorMessage::BlockReleased(block.id))
                    .into()
            }
        } else {
            bubble
        };

        // The hover-only delete: floated over the bubble's top-right
        // corner (the Stack sizes itself to the bubble — its first child —
        // so the overlay never widens the row).
        let bubble: Element<'a, EditorMessage> =
            if self.hovered_block == Some(block.id) && self.drag.is_none() {
                let delete_button = button::icon(icon::from_name("window-close-symbolic"))
                    .extra_small()
                    .class(cosmic::theme::Button::Destructive)
                    .tooltip(fl!("tooltip-remove-block"))
                    .on_press(EditorMessage::RemoveBlock(block.id));

                Stack::new()
                    .push(bubble)
                    .push(container(delete_button).width(Length::Fill).align_x(Horizontal::Right))
                    .into()
            } else {
                bubble
            };

        // The bubble and the empty filler split the row's leftover 4:1
        // (iced divides purely by fill factor — a max-clamped Fill child
        // doesn't hand its surplus back), keeping bubbles wide and
        // near-uniform while still leaving the messaging-style off-side
        // gap.
        let filler = Space::new().width(Length::FillPortion(1));
        let row = if matches!(block.content, BlockContent::Dialogue { .. }) {
            Row::new()
                .push(pill)
                .push(gutter)
                .push(bubble)
                .push(filler)
        } else {
            Row::new()
                .push(filler)
                .push(bubble)
                .push(gutter)
                .push(pill)
        };
        let row = row.spacing(8).width(Length::Fill);

        // Row-level cursor tracking: hover (for the delete button)
        // normally, drop-target reporting while a drag is in flight. The
        // rows also handle the drop themselves — same capture reasoning
        // as the pill above: the innermost `mouse_area` under the cursor
        // swallows the release, so the one the release actually lands on
        // must be the one that ends the gesture.
        let area = mouse_area(row);
        let area = if self.drag.is_some() {
            area.on_enter(EditorMessage::DragOverRow(index))
                .on_release(EditorMessage::DragDrop)
        } else {
            area.on_enter(EditorMessage::BlockHoverEnter(block.id))
                .on_exit(EditorMessage::BlockHoverExit)
        };
        area.into()
    }

    /// A prose block's bubble content: plain text in read mode (the
    /// placeholder caption when empty), or the expanded `text_editor` with
    /// Save/Cancel while this block is the one being edited.
    fn prose_body<'a>(
        &'a self,
        block: &'a StoryBlock,
        is_editing: bool,
        placeholder: String,
        preview_lines: usize,
    ) -> Element<'a, EditorMessage> {
        let block_id = block.id;

        if !is_editing {
            let text = block.prose().unwrap_or_default();
            // `WordOrGlyph` + `Fill`: an unbroken run longer than the
            // bubble (e.g. "aaaa…") breaks mid-word instead of overflowing
            // the bubble sideways across the gutter. The line-count
            // ellipsize (the Preferences "Collapsed preview lines" value)
            // keeps long passages collapsed while browsing — only the
            // block being edited expands to full height.
            return if text.is_empty() {
                caption(placeholder).into()
            } else {
                body(text)
                    .width(Length::Fill)
                    .wrapping(Wrapping::WordOrGlyph)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(preview_lines)))
                    .into()
            };
        }

        let editor: Element<'a, EditorMessage> = match self.prose_contents.get(&block_id) {
            Some(content) => text_editor(content)
                .placeholder(placeholder)
                .on_action(move |action| EditorMessage::ProseAction(block_id, action))
                .min_height(64.0)
                // Same mid-word fallback as read mode, for the same
                // unbroken-run overflow.
                .wrapping(Wrapping::WordOrGlyph)
                .class(cosmic::theme::iced::TextEditor::Custom(Box::new(flat_prose_style)))
                .into(),
            // Contents are created/removed in lockstep with prose blocks,
            // so this shouldn't happen — but a hole in the UI beats a panic.
            None => Space::new().into(),
        };

        Column::new()
            .push(editor)
            .push(
                Row::new()
                    .push(Space::new().width(Length::Fill))
                    .push(small_button(
                        fl!("editor-save"),
                        cosmic::theme::Button::Suggested,
                        EditorMessage::ProseSave(block_id),
                    ))
                    .push(small_button(
                        fl!("editor-cancel"),
                        cosmic::theme::Button::Destructive,
                        EditorMessage::ProseCancel(block_id),
                    ))
                    .spacing(8),
            )
            .spacing(8)
            .into()
    }
}

/// A compact text button matching the toolbar's ~20%-reduced sizing; used
/// for the prose bubbles' Save/Cancel.
fn small_button(
    label: String,
    class: cosmic::theme::Button,
    message: EditorMessage,
) -> Element<'static, EditorMessage> {
    button::text(label)
        .class(class)
        .height(26.0)
        .padding([0, 8])
        .font_size(12)
        .line_height(16)
        .on_press(message)
        .into()
}

/// The floating miniature that follows the cursor during a drag: the
/// block's kind icon plus a truncated preview of its prose (or just the
/// kind label when it has none).
fn drag_ghost<'a>(block: &StoryBlock) -> Element<'a, EditorMessage> {
    let preview = block.prose()
        .filter(|text| !text.is_empty())
        .map_or_else(|| block.kind().label(), |text| display_title(text, GHOST_PREVIEW_CHARS));

    container(
        Row::new()
            .push(icon::from_name(block.kind().icon_name()).icon().size(13))
            .push(caption(preview))
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .padding([4, 8])
    .class(cosmic::theme::Container::Card)
    .into()
}

/// The drop-position hint drawn between rows while dragging.
fn insertion_line<'a>() -> Element<'a, EditorMessage> {
    container(Space::new().width(Length::Fill).height(Length::Fixed(2.0)))
        .width(Length::Fill)
        .style(|theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
            background: Some(Background::Color(theme.cosmic().accent_color().into())),
            ..Default::default()
        })
        .into()
}

/// The dialogue gutter's avatar: the picked speaker's portrait if they
/// have one, otherwise the generic placeholder icon (also used when no
/// speaker is assigned or the character was deleted).
fn dialogue_avatar<'a>(speaker: Option<Uuid>, characters: &[Character]) -> Element<'a, EditorMessage> {
    let avatar_path = speaker
        .and_then(|id| characters.iter().find(|character| character.id == id))
        .and_then(Character::avatar_path)
        .map(std::path::PathBuf::from);

    let content: Element<'a, EditorMessage> = match avatar_path {
        Some(path) => image(path)
            .width(Length::Fixed(AVATAR_SIZE))
            .height(Length::Fixed(AVATAR_SIZE))
            .content_fit(ContentFit::Cover)
            .border_radius(AVATAR_RADIUS)
            .into(),
        None => icon::from_name(BlockKind::Dialogue.icon_name()).icon().size(36).into(),
    };

    avatar_frame(content)
}

/// The narrator's fixed avatar for narration blocks — same footprint as a
/// character avatar, but always the narration icon.
fn narrator_avatar<'a>() -> Element<'a, EditorMessage> {
    avatar_frame(icon::from_name(BlockKind::Narration.icon_name()).icon().size(36).into())
}

/// The shared `AVATAR_SIZE` frame both avatar kinds sit centered in, so
/// dialogue and narration rows line up.
fn avatar_frame(content: Element<'_, EditorMessage>) -> Element<'_, EditorMessage> {
    container(content)
        .width(Length::Fixed(AVATAR_SIZE))
        .height(Length::Fixed(AVATAR_SIZE))
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .class(cosmic::theme::Container::Card)
        .into()
}

/// The closed face of an inline dropdown: "▼ <selected>", caret left of
/// the label, label wrapping to at most two lines (per the design mock's
/// "Character Two-line"). The speaker selector passes `Button::Text` so
/// it reads as a bare caret + name under the avatar; the choice target
/// keeps the boxed `Standard` look of a form field.
fn selector_toggle<'a>(
    selected: String,
    width: Length,
    class: cosmic::theme::Button,
    message: EditorMessage,
) -> Element<'a, EditorMessage> {
    button::custom(
        Row::new()
            .push(icon::from_name("pan-down-symbolic").icon().size(12))
            .push(
                caption(selected)
                    .wrapping(Wrapping::Word)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(2))),
            )
            .spacing(6)
            .align_y(Alignment::Center),
    )
    .class(class)
    .padding([4, 8])
    .width(width)
    .on_press(message)
    .into()
}

/// One row of an expanded inline dropdown's entry list.
fn dropdown_entry(label: String, message: EditorMessage) -> Element<'static, EditorMessage> {
    button::custom(body(label))
        .class(cosmic::theme::Button::MenuItem)
        .padding([4, 8])
        .width(Length::Fill)
        .on_press(message)
        .into()
}

/// The entry list panel shown under an expanded inline dropdown's toggle.
fn dropdown_panel(entries: Element<'_, EditorMessage>) -> Element<'_, EditorMessage> {
    container(entries)
        .class(cosmic::theme::Container::Card)
        .width(Length::Fill)
        .into()
}

/// A dialogue block's inline speaker selector: the "▼ <name>" face, and —
/// while `is_open` — a "(None)" entry plus the cast in Characters-tab
/// order right below it. A `speaker` id whose character was deleted
/// matches nothing and shows as "(None)".
fn speaker_selector<'a>(
    block_id: Uuid,
    speaker: Option<Uuid>,
    characters: &[Character],
    is_open: bool,
) -> Element<'a, EditorMessage> {
    let selected = speaker
        .and_then(|id| characters.iter().find(|character| character.id == id))
        .map_or_else(|| fl!("dropdown-none"), |character| character.name.clone());

    let toggle = selector_toggle(
        selected,
        Length::Fill,
        cosmic::theme::Button::Text,
        EditorMessage::DropdownToggled(DropdownId::Speaker(block_id)),
    );

    if !is_open {
        return toggle;
    }

    let mut entries = Column::new()
        .push(dropdown_entry(fl!("dropdown-none"), EditorMessage::SpeakerPicked(block_id, None)));
    for character in characters {
        entries = entries.push(dropdown_entry(
            character.name.clone(),
            EditorMessage::SpeakerPicked(block_id, Some(character.id)),
        ));
    }

    Column::new()
        .push(toggle)
        .push(dropdown_panel(entries.into()))
        .spacing(4)
        .into()
}

/// A choice block's body: one row per option (label input, inline target
/// selector, remove button) plus an "Add option" button at the bottom.
/// The open selector's entry list — "(None)", then every *other* node on
/// the canvas (`current_node` is where the reader already is, so a jump
/// to it is never offered; a pre-existing self-target still displays
/// truthfully on the closed face) — expands right below its row.
fn choice_body<'a>(
    current_node: Uuid,
    block_id: Uuid,
    options: &'a [ChoiceOption],
    nodes: &[StoryNode],
    open_dropdown: Option<DropdownId>,
) -> Element<'a, EditorMessage> {
    let mut column = Column::new().spacing(8);

    for option in options {
        let option_id = option.id;
        let dropdown_id = DropdownId::ChoiceTarget { block: block_id, option: option_id };

        let selected = option.target
            .and_then(|target| nodes.iter().find(|node| node.id == target))
            .map_or_else(|| fl!("dropdown-none"), |node| node.title.clone());

        let mut selector = Column::new()
            .push(selector_toggle(
                selected,
                Length::Fixed(TARGET_SELECTOR_WIDTH),
                cosmic::theme::Button::Standard,
                EditorMessage::DropdownToggled(dropdown_id),
            ))
            .spacing(4)
            .width(Length::Fixed(TARGET_SELECTOR_WIDTH));

        if open_dropdown == Some(dropdown_id) {
            let mut entries = Column::new().push(dropdown_entry(
                fl!("dropdown-none"),
                EditorMessage::ChoiceTargetPicked { block: block_id, option: option_id, target: None },
            ));
            for node in nodes.iter().filter(|node| node.id != current_node) {
                entries = entries.push(dropdown_entry(
                    node.title.clone(),
                    EditorMessage::ChoiceTargetPicked {
                        block: block_id,
                        option: option_id,
                        target: Some(node.id),
                    },
                ));
            }
            selector = selector.push(dropdown_panel(entries.into()));
        }

        column = column.push(
            Row::new()
                .push(
                    text_input(fl!("choice-option-placeholder"), option.label.as_str())
                        .on_input(move |label| EditorMessage::ChoiceLabelChanged {
                            block: block_id,
                            option: option_id,
                            label,
                        }),
                )
                .push(selector)
                .push(
                    button::icon(icon::from_name("edit-delete-symbolic"))
                        .extra_small()
                        .tooltip(fl!("tooltip-remove-option"))
                        .on_press(EditorMessage::RemoveChoiceOption {
                            block: block_id,
                            option: option_id,
                        }),
                )
                .spacing(8)
                // Start, not Center: the open selector's entry list grows
                // the row downward and shouldn't drag the input with it.
                .align_y(Alignment::Start),
        );
    }

    column
        .push(button::text(fl!("choice-add-option")).on_press(EditorMessage::AddChoiceOption(block_id)))
        .into()
}

/// Borderless, transparent styling for the prose bubbles' `text_editor`,
/// so the editing surface blends into its bubble instead of reading as a
/// boxed input — text/placeholder/selection colors stay cosmic's stock
/// ones (mirrors the `Catalog` impl in libcosmic's `theme/style/iced.rs`).
fn flat_prose_style(
    theme: &cosmic::Theme,
    _status: text_editor::Status,
) -> text_editor::Style {
    let palette = theme.cosmic();
    let value: Color = palette.palette.neutral_9.into();
    let placeholder = Color { a: 0.7, ..value };

    text_editor::Style {
        background: Background::Color(Color::TRANSPARENT),
        border: Border {
            radius: 0.0.into(),
            width: 0.0,
            color: Color::TRANSPARENT,
        },
        placeholder,
        value,
        selection: palette.accent.base.into(),
    }
}

/// The author-note bubble tint — see `block_row`.
fn note_card_style(_theme: &cosmic::Theme) -> cosmic::iced::widget::container::Style {
    cosmic::iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgba(1.0, 0.8, 0.3, 0.06))),
        border: Border {
            width: 1.0,
            color: Color::from_rgba(1.0, 0.8, 0.3, 0.35),
            radius: 8.0.into(),
        },
        ..Default::default()
    }
}
