use uuid::Uuid;

use cosmic::iced::{
    Color, Point, Rectangle, Size,
};
use cosmic::widget::{
    canvas::{Frame, Path, Stroke}
};
use serde::{Serialize, Deserialize};
use crate::components::story_block::{BlockContent, StoryBlock};
use crate::fl;


/// A single node on the story canvas. `position`/`size` are in *world*
/// space (unaffected by `CanvasPage::zoom`) — `nav::canvas` is what
/// converts between world and screen coordinates for drawing/hit-testing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryNode {
    pub id: Uuid,
    pub position: NodePosition,
    /// Fixed in-app (see `NodeSize::default`) and deliberately not
    /// persisted — `#[serde(skip)]` also makes legacy project files'
    /// stored size an ignored leftover.
    #[serde(skip)]
    pub size: NodeSize,
    pub title: String,
    /// The passage's actual content, in reading order — see
    /// `components::story_block`. `#[serde(default)]` so project files
    /// saved before this field existed still load (same trick as
    /// `ProjectFile::characters`).
    #[serde(default)]
    pub blocks: Vec<StoryBlock>,
}

/// A node's top-left corner, in world-space units.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32
}

/// A node's footprint, in world-space units (i.e. at 1x zoom). Every node
/// shares the `Default` footprint — there's no resize UI, and the project
/// files don't store one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSize {
    pub width: f32,
    pub height: f32,
}

impl Default for NodeSize {
    fn default() -> Self {
        Self { width: 200.0, height: 100.0 }
    }
}

// Conversion impl, so we only need to use .into() when when using values
impl From<NodePosition> for Point {
    fn from(p: NodePosition) -> Self {
        Point::new(p.x, p.y)
    }
}

impl From<NodeSize> for Size {
    fn from(s: NodeSize) -> Self {
        Size::new(s.width, s.height)
    }
}

impl Default for StoryNode {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            position: NodePosition {x: 0.0, y:0.0},
            size: NodeSize::default(),
            title: fl!("node-default-title"),
            blocks: Vec::new(),
        }
    }
}

impl StoryNode {

    #[allow(dead_code)]
    // TODO: decide use
    pub fn new(id: Uuid, position: NodePosition, size: NodeSize, title: impl Into<String>) -> Self {
        Self { id, position, size, title: title.into(), blocks: Vec::new() }
    }

    /// Every node id this node's choice options link out to (unassigned
    /// options skipped, dangling ids included — the caller decides what a
    /// missing target means). This is what the canvas draws edges from.
    pub fn outgoing_targets(&self) -> impl Iterator<Item = Uuid> + '_ {
        self.blocks.iter()
            .filter_map(|block| match &block.content {
                BlockContent::Choice { options } => Some(options.iter().filter_map(|option| option.target)),
                _ => None,
            })
            .flatten()
    }

    /// World-space rectangle occupied by this node, used for both drawing
    /// and hit-testing (see `contains`).
    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.position.clone().into(), self.size.clone().into())

    }

    /// Whether a world-space `point` falls inside this node. Callers must
    /// convert screen coordinates (e.g. mouse position) to world space via
    /// `CanvasPage::screen_to_world` first — this does no zoom/offset math
    /// itself.
    pub fn contains(&self, point: Point) -> bool {
        self.bounds().contains(point)
    }

    /// Draws just the node's rounded-rectangle shape onto the canvas frame.
    /// The title label is rendered separately as an overlaid `view()`
    /// widget in `CanvasPage::view` — see the note on `canvas::Program` in
    /// `nav/canvas.rs` for why text can't be drawn here.
    pub fn draw(&self, frame: &mut Frame) {
        let path = Path::rounded_rectangle(
            self.position.clone().into(),
            self.size.clone().into(),
            8.0.into()
        );
        frame.fill(&path, Color::from_rgb8(45, 45, 48));

        frame.stroke(
            &path,
            Stroke::default()
                .with_color(Color::from_rgb8(70, 70, 70))
                .with_width(2.0),
        );
    }

    /// The Find panel's "found it" highlight: a ring inflated beyond the
    /// node's own bounds (so it isn't hidden by the node's opaque fill, or
    /// by the pinned title label in `CanvasPage::view` that exactly matches
    /// those bounds), at `alpha` — see `CanvasPage::focus_node`.
    pub fn draw_glow(&self, frame: &mut Frame, alpha: f32) {
        const PADDING: f32 = 6.0;

        let position = Point::new(self.position.x - PADDING, self.position.y - PADDING);
        let size = Size::new(self.size.width + PADDING * 2.0, self.size.height + PADDING * 2.0);
        let path = Path::rounded_rectangle(position, size, (8.0 + PADDING).into());

        frame.stroke(
            &path,
            Stroke::default()
                .with_color(Color::from_rgba(1.0, 1.0, 1.0, alpha))
                .with_width(3.0),
        );
    }

}