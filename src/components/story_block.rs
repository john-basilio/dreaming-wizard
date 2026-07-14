// SPDX-License-Identifier: AGPL-3.0-or-later

//! The building blocks that make up a `StoryNode`'s content: an ordered
//! list of typed `StoryBlock`s — narration, character dialogue, player
//! choices, game-engine directives, and author-only notes. Dialogue refers
//! to a `Character` and choices refer to a target `StoryNode` by `Uuid`
//! rather than by name, so renames propagate for free; a reference whose
//! entity was deleted is kept as-is and shown as unassigned in the editor
//! (and skipped when drawing edges) instead of being cascaded away.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::fl;

/// One entry in a `StoryNode`'s ordered content list. The `id` is the
/// block's stable identity for reorder/remove targeting in the editor —
/// list indices shift, ids don't.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryBlock {
    pub id: Uuid,
    #[serde(flatten)]
    pub content: BlockContent,
}

/// What a `StoryBlock` actually holds, tagged by kind. Serialized
/// internally tagged (flattened into the block), so a block reads as e.g.
/// `{"id": ..., "type": "dialogue", "speaker": ..., "text": ...}` on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BlockContent {
    /// Speaker-less prose: scene description, narrator text.
    Narration { text: String },
    /// A spoken line. `speaker` is a `Character::id`; `None` means no
    /// speaker assigned (yet, or the character was deleted — see the
    /// dangling-reference note in the module doc).
    Dialogue { speaker: Option<Uuid>, text: String },
    /// Player-facing branching: each option can point at another node,
    /// which is what forms the story graph's edges.
    Choice { options: Vec<ChoiceOption> },
    /// A game-engine hook (e.g. `play_music` / `tavern_theme`) — meant for
    /// the machine, not the reader.
    Directive { command: String, argument: String },
    /// Writer-only comment; excluded from any future export.
    Note { text: String },
}

/// One option of a `Choice` block. `target` is a `StoryNode::id`; `None`
/// means not linked (yet, or the node was deleted).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChoiceOption {
    pub id: Uuid,
    pub label: String,
    pub target: Option<Uuid>,
}

impl Default for ChoiceOption {
    fn default() -> Self {
        Self { id: Uuid::new_v4(), label: String::new(), target: None }
    }
}

/// The five block kinds, without their payloads — what the editor's
/// "add block" buttons pick from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    Narration,
    Dialogue,
    Choice,
    Directive,
    Note,
}

impl BlockKind {
    /// Every kind, in the order the editor's add-block buttons show them.
    pub const ALL: [Self; 5] = [
        Self::Narration,
        Self::Dialogue,
        Self::Choice,
        Self::Directive,
        Self::Note,
    ];

    /// The kind's user-facing name — block gutter labels and add-block
    /// toolbar buttons.
    pub fn label(self) -> String {
        match self {
            Self::Narration => fl!("block-narration"),
            Self::Dialogue => fl!("block-dialogue"),
            Self::Choice => fl!("block-choice"),
            Self::Directive => fl!("block-directive"),
            Self::Note => fl!("block-note"),
        }
    }

    /// The freedesktop icon name representing the kind — the add-block
    /// toolbar buttons, and (for `Narration`) the narrator's fixed avatar
    /// in the editor's block gutter.
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Narration => "media-view-subtitles-symbolic",
            Self::Dialogue => "avatar-default-symbolic",
            Self::Choice => "object-select-symbolic",
            Self::Directive => "utilities-terminal-symbolic",
            Self::Note => "text-x-generic-symbolic",
        }
    }
}

impl StoryBlock {
    /// A fresh, empty block of the given kind. A new `Choice` starts with
    /// one empty option so it doesn't render as a bare header.
    pub fn new(kind: BlockKind) -> Self {
        let content = match kind {
            BlockKind::Narration => BlockContent::Narration { text: String::new() },
            BlockKind::Dialogue => BlockContent::Dialogue { speaker: None, text: String::new() },
            BlockKind::Choice => BlockContent::Choice { options: vec![ChoiceOption::default()] },
            BlockKind::Directive => BlockContent::Directive {
                command: String::new(),
                argument: String::new(),
            },
            BlockKind::Note => BlockContent::Note { text: String::new() },
        };

        Self { id: Uuid::new_v4(), content }
    }

    pub fn kind(&self) -> BlockKind {
        match self.content {
            BlockContent::Narration { .. } => BlockKind::Narration,
            BlockContent::Dialogue { .. } => BlockKind::Dialogue,
            BlockContent::Choice { .. } => BlockKind::Choice,
            BlockContent::Directive { .. } => BlockKind::Directive,
            BlockContent::Note { .. } => BlockKind::Note,
        }
    }

    /// The block's free-flowing prose body, if its kind has one
    /// (narration/dialogue/note — the kinds the editor gives a
    /// `text_editor` to).
    pub fn prose(&self) -> Option<&str> {
        match &self.content {
            BlockContent::Narration { text }
            | BlockContent::Dialogue { text, .. }
            | BlockContent::Note { text } => Some(text),
            BlockContent::Choice { .. } | BlockContent::Directive { .. } => None,
        }
    }

    /// Writes a new prose body into the block; a no-op for kinds without
    /// one (see `prose`).
    pub fn set_prose(&mut self, new_text: String) {
        match &mut self.content {
            BlockContent::Narration { text }
            | BlockContent::Dialogue { text, .. }
            | BlockContent::Note { text } => *text = new_text,
            BlockContent::Choice { .. } | BlockContent::Directive { .. } => {}
        }
    }

    /// Whether any user-visible text in this block contains `query`, which
    /// the caller must already have lowercased — used by the Find panel's
    /// node search alongside the title match.
    pub fn matches_query(&self, query: &str) -> bool {
        match &self.content {
            BlockContent::Narration { text }
            | BlockContent::Dialogue { text, .. }
            | BlockContent::Note { text } => text.to_lowercase().contains(query),
            BlockContent::Choice { options } => {
                options.iter().any(|option| option.label.to_lowercase().contains(query))
            }
            BlockContent::Directive { command, argument } => {
                command.to_lowercase().contains(query) || argument.to_lowercase().contains(query)
            }
        }
    }
}

