#![allow(clippy::cast_precision_loss)] // Precision loss from i32 to f32 conversion is practically harmless for normal use.
#![allow(clippy::cast_possible_truncation)] // Same case for f32 to i32

//! This page is responsible for visualizing nodes, aka `StoryNodes`
//! as well as letting the users modify them in place through the
//! editor interface when a node is clicked.

use std::cell::Cell;
use std::time::{Duration, Instant};
use cosmic::{
    Element, Renderer, Theme, iced::{
        Color, Length, Point, Radius, Rectangle, Vector, mouse,
        keyboard::{self, Modifiers},
        widget::{Stack, pin},
    }, widget::{self, button, canvas, icon},
};
// use cosmic::theme::Container as ContainerStyle;
use cosmic::iced::{Border, Background, Size};

use crate::components::{
    Character,
    NodePosition,
    StoryNode,
    StoryNodeEditor,
    ConfirmDialog,
    display_title,
    confirm_dialog::ConfirmDialogMessage,
    overlay::{with_corner_button, with_overlay, toast_box, HoverTooltip, pulse_alpha, is_pulse_active},
    story_node_editor::{EditorEvent, EditorMessage},
};
use crate::fl;

/// Responsible for providing unique UUIDs for each node
/// so that they can be identified by other components.
use uuid::Uuid;

/// Alpha of the dimming shade behind the delete-confirmation overlay.
const SHADE_ALPHA: f32 = 0.3;
/// Padding from both edges for the floating "add node" button.
const ADD_BUTTON_PADDING: f32 = 24.0;
/// Vertical gap (beyond `ADD_BUTTON_PADDING`) between the add button and its
/// hover tooltip floating above it.
const ADD_BUTTON_TOOLTIP_OFFSET: f32 = 56.0;
/// Max +/- world-space jitter applied to a newly-spawned node's position
/// (see `spawn_jitter`), so repeated "Add Node"s don't stack perfectly on
/// top of each other and hide one another.
const NODE_SPAWN_JITTER: f32 = 100.0;

/// Bend range for story-graph edge curves (the edge pass in `draw`): how
/// far, in world units, an edge's control points extend past its
/// endpoints along the routing axis — half the span, clamped to these
/// bounds so short hops still visibly curve and long ones don't balloon.
const EDGE_MIN_BEND: f32 = 30.0;
const EDGE_MAX_BEND: f32 = 120.0;
/// Sideways shift of an edge's anchors from the node-edge centers, signed
/// by travel direction — a reciprocal pair (A→B and B→A) lands on two
/// parallel lanes instead of overlapping/tangling.
const EDGE_LANE_OFFSET: f32 = 14.0;

/// Timing of the Find panel's "found it" glow ring (see `focus_node`):
/// gently fades in, holds at full brightness for a couple seconds, then
/// fades back out on its own — no user input needed to dismiss it.
const NODE_GLOW_FADE_IN: Duration = Duration::from_millis(200);
const NODE_GLOW_VISIBLE: Duration = Duration::from_millis(1500);
const NODE_GLOW_FADE_OUT: Duration = Duration::from_millis(800);

/// Derives a small pseudo-random offset from a node's own (freshly
/// generated) id — reusing its already-random bytes instead of pulling in
/// a `rand` dependency just for this.
fn spawn_jitter(id: &Uuid) -> Vector {
    let bytes = id.as_bytes();
    let to_offset = |byte: u8| (byte as f32 / 255.0) * (2.0 * NODE_SPAWN_JITTER) - NODE_SPAWN_JITTER;
    Vector::new(to_offset(bytes[0]), to_offset(bytes[1]))
}

/// An in-flight transition of `offset`/`zoom` from one camera state to
/// another, driven forward by repeated `CanvasMessage::AnimationTick`
/// messages while it is `Some`.
pub struct CameraAnimation {
    start: Instant,
    duration: Duration,
    start_offset: Vector,
    start_zoom: f32,
    target_offset: Vector,
    target_zoom: f32,
}

/// This page model, responsible for rendering the canvas,
/// where all the story node lives and are rendered.
pub struct CanvasPage {
    // shared fields — used by both draw() and view()
    pub offset: Vector, // Camera position: screen = world * zoom + offset.
    pub zoom: f32, // Camera zoom factor; see MIN_ZOOM/MAX_ZOOM.

    // draw() unique fields
    pub geo_cache: canvas::Cache,
    pub last_bounds: Cell<Size>, // updated every draw(), read by update() for world_center()

    // view() unique fields
    pub editor: Option<StoryNodeEditor>, // Some while a node's editor is open; freezes the canvas (see canvas::Program::update).
    pub nodes: Vec<StoryNode>, // Every node currently on the canvas.

    // camera animation fields
    camera_anim: Option<CameraAnimation>,
    saved_camera: Option<(Vector, f32)>, // (offset, zoom) from before the editor opened

    // hover/delete fields
    /// The node currently under the cursor (idle, not panning/dragging), if
    /// any — driven by `canvas::Program::update`'s own hit-testing, since
    /// the pinned node widgets in `view()` don't get raw mouse events of
    /// their own. Shows the hover-delete button on that node.
    hovered_node: Option<Uuid>,
    /// Some while a delete confirmation is pending for this node — set
    /// either by the hover-delete button or the open editor's own Delete
    /// button. The `ConfirmDialog` is built once at request time (rather
    /// than fresh in `view()`) so its `view()` borrow has somewhere
    /// long-lived (`self`) to borrow from.
    pending_delete: Option<(Uuid, ConfirmDialog)>,

    /// Drives the floating "add node" button's delayed-fade hover tooltip.
    add_button_tooltip: HoverTooltip,

    /// `Some((node_id, started_at))` while that node's Find-triggered
    /// "found it" glow ring is fading in/holding/fading out; see
    /// `focus_node`.
    glow: Option<(Uuid, Instant)>,

    /// Set whenever a node was added, moved, edited, or deleted since the
    /// last time this was taken — polled by `AppModel` (via
    /// `take_content_dirty`) after every message it forwards here, to
    /// update its own project-wide "is anything unsaved" flag. Doesn't
    /// reset itself; the caller must consume it.
    content_dirty: bool,
}

/// Messages emitted by the canvas page.
#[derive(Debug, Clone)]

pub enum CanvasMessage {
    /// Spawns a new default-sized `StoryNode` centered in the current view.
    AddNode,
    /// The camera was dragged by `delta` screen pixels (middle-click or
    /// Ctrl+left-click drag — see `canvas::Program::update`).
    Panned(Vector),
    /// Mouse wheel/trackpad scroll over the canvas, zooming around `at`.
    Zoomed {
        at: Point,
        scroll_amount: f32,
    },
    /// A node was moved by `delta` *world*-space units while being dragged.
    NodeDragged {
        id: Uuid,
        delta: Vector,
    },
    /// A node was pressed and released without much movement — opens (or
    /// re-targets) the editor for it and kicks off the click-to-edit camera
    /// animation.
    NodeClicked {id: Uuid},
    /// The node under the cursor changed (or the cursor left every node);
    /// see `hovered_node`.
    NodeHoverChanged(Option<Uuid>),
    /// The hover-delete button on a node was clicked; see `pending_delete`.
    NodeDeleteRequested(Uuid),
    /// Forwarded from the open `ConfirmDialog`'s own `view()`.
    ConfirmDelete(ConfirmDialogMessage),
    /// Forwarded from the open `StoryNodeEditor`'s own `view()`.
    Editor(EditorMessage),
    /// One frame of an in-flight `CameraAnimation`; see `is_animating_camera`
    /// and `AppModel::subscription` for how these get scheduled.
    AnimationTick,
    /// The cursor entered/left the floating "add node" button; see
    /// `add_button_tooltip`.
    AddButtonHoverEnter,
    AddButtonHoverExit,
    /// The canvas's on-screen bounds changed while a node's editor was
    /// open; re-centers that node against the new size (see
    /// `canvas::Program::update`'s resize handling).
    EditorBoundsChanged(Size),

}


impl Default for CanvasPage {
    fn default() -> Self {
        Self {
            geo_cache: canvas::Cache::new(),
            zoom: 1.0, // Needs to be at least 1.0 to avoid division of 0.
            offset: Vector::new(0.0, 0.0),
            editor: None,
            nodes: Vec::new(),
            last_bounds: Cell::new(Size::new(800.0, 600.0)), // fallback before first draw
            camera_anim: None,
            saved_camera: None,
            hovered_node: None,
            pending_delete: None,
            add_button_tooltip: HoverTooltip::default(),
            glow: None,
            content_dirty: false,
        }
    }
}


/// This is where we put other custom methods — `view`/`update` for the
/// COSMIC widget tree, plus the camera math (`screen_to_world` and friends)
/// shared by both `view` and the `canvas::Program` impl below.
impl CanvasPage {
    /// Furthest the user can zoom out.
    const MIN_ZOOM: f32 = 0.1;
    /// Furthest the user can zoom in.
    const MAX_ZOOM: f32 = 4.0;
    /// Multiplier applied to scroll input before it changes `zoom`; higher
    /// = faster zooming per wheel notch/trackpad pixel.
    const ZOOM_SENSITIVITY: f32 = 0.1;

    /// How much wider than the edited node the reserved canvas gap is, both
    /// for the editor's `Row` sizing in `view()` and for centering the node
    /// within that gap in the click-to-edit camera animation.
    const EDITOR_GAP_MULTIPLIER: f32 = 1.5;
    const CAMERA_ANIM_DURATION: Duration = Duration::from_millis(350);

    /// Whether a camera transition is currently in flight; used by the app's
    /// subscription to decide whether to keep ticking the animation forward.
    pub fn is_animating_camera(&self) -> bool {
        self.camera_anim.is_some()
    }

    /// Whether the add-button tooltip is mid-fade; used by the app's
    /// subscription the same way as `is_animating_camera`.
    pub fn is_add_button_tooltip_active(&self) -> bool {
        self.add_button_tooltip.is_active()
    }

    /// Whether a Find-triggered glow ring is still fading in/holding/
    /// fading out; used by the app's subscription the same way as
    /// `is_animating_camera`.
    pub fn is_glow_active(&self) -> bool {
        self.glow.is_some_and(|(_, started)| {
            is_pulse_active(started, NODE_GLOW_FADE_IN, NODE_GLOW_VISIBLE, NODE_GLOW_FADE_OUT)
        })
    }

    /// Pans the camera to center node `id` on screen — keeping the current
    /// zoom, unlike the click-to-edit animation, which also resets zoom to
    /// 1.0 — and starts its "found it" glow ring. Used by the Find panel;
    /// does nothing if `id` isn't a node on this canvas (e.g. a stale
    /// result after it was deleted).
    pub fn focus_node(&mut self, id: Uuid) {
        let Some(node) = self.nodes.iter().find(|n| n.id == id) else { return };

        let node_center = Point::new(
            node.position.x + node.size.width / 2.0,
            node.position.y + node.size.height / 2.0,
        );
        let bounds = self.last_bounds.get();
        let screen_center = Point::new(bounds.width / 2.0, bounds.height / 2.0);

        // screen = world * zoom + offset  =>  offset = screen - world * zoom
        let target_offset = Vector::new(
            screen_center.x - node_center.x * self.zoom,
            screen_center.y - node_center.y * self.zoom,
        );

        self.start_camera_animation(target_offset, self.zoom);
        self.glow = Some((id, Instant::now()));
    }

    /// `characters` is the Characters tab's cast, only threaded through to
    /// the open editor's dialogue speaker dropdowns (see
    /// `StoryNodeEditor::view`).
    pub fn view(&self, characters: &[Character]) -> Element<'_, CanvasMessage> {
        let canvas_element = widget::canvas(self)
            .width(Length::Fill)
            .height(Length::Fill);

        // Stack indirectly helps with occlusion by stacking nodes in order of first-bottom to last-top.
        let stack = self.nodes.iter().fold(
            Stack::new().push(canvas_element),
            |stack, node| {
                let screen = self.world_to_screen(
                    Point::new(node.position.x, node.position.y));
                let screen_width = node.size.width * self.zoom;
                let screen_height = node.size.height * self.zoom;

                let node_widget = pin(
                    widget::container(
                        widget::text::body(display_title(&node.title, 15))
                            .width(Length::Fill)
                            .align_x(cosmic::iced::alignment::Horizontal::Center)
                            .size(14.0 * self.zoom) // The `14` comes from: /libcosmic-41009aea1d72760b/e7f278d/src/widget/text.rs -> pub fn heading
                    )
                    .width(Length::Fixed(screen_width))
                    .height(Length::Fixed(screen_height))
                    .align_y(cosmic::iced::alignment::Vertical::Center)
                    .style(|_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
                        background: Some(Background::Color(Color::from_rgb8(45, 45, 48))),
                        border: Border {
                            width: 1.0,
                            color: Color::from_rgb8(70, 70, 70),
                            radius: Radius::new(8.0),
                        },
                        ..Default::default()
                    })
                )
                .x(screen.x)
                .y(screen.y);

                let stack = stack.push(node_widget);

                // A real `Button`, not a `mouse_area` — it only intercepts
                // clicks within its own small bounds, so the rest of the
                // node's area (and the raw canvas beneath it) still work
                // normally for dragging/click-to-edit. Never shown while an
                // editor is open (on top of `NodeClicked` clearing
                // `hovered_node`, this also covers it staying visible for
                // whichever node's editor is currently open).
                if self.editor.is_none() && self.hovered_node == Some(node.id) {
                    let delete_button = button::icon(icon::from_name("edit-delete-symbolic"))
                        .extra_small()
                        .class(cosmic::theme::Button::Destructive)
                        .tooltip(fl!("tooltip-delete"))
                        .on_press(CanvasMessage::NodeDeleteRequested(node.id));

                    stack.push(pin(delete_button).x(screen.x + screen_width - 14.0).y(screen.y - 14.0))
                } else {
                    stack
                }
            }
        ).clip(true);


        // Now we build the content with the editor if SOME, plus the
        // floating "add node" button when it's not (see below), then layer
        // the delete-confirmation dialog over the very top of either.
        let content: Element<'_, CanvasMessage> = match &self.editor {
            Some(editor) => {
                // Reserve screen-space equal to `EDITOR_GAP_MULTIPLIER`x the
                // edited node's width *at 1x zoom* for the canvas; the editor
                // fills the rest. Using the node's raw world-space width (not
                // `* self.zoom`) keeps this gap — and the editor's width —
                // stable while the click-to-edit camera animation interpolates
                // zoom, instead of visibly shrinking/growing along with it.
                // Iced re-resolves this Row on every layout pass, so the
                // editor's width stays correct across window resizes and nav
                // bar toggles without any manual tracking.
                let node_width = self.nodes.iter()
                    .find(|n| n.id == editor.node_id)
                    .map_or(0.0, |n| n.size.width);

                widget::row![
                    widget::container(stack)
                        .width(Length::Fixed(node_width * Self::EDITOR_GAP_MULTIPLIER))
                        .height(Length::Fill),
                    editor.view(characters, &self.nodes).map(CanvasMessage::Editor),
                ].into()
            },

            None => {
                let canvas_area: Element<'_, CanvasMessage> = widget::container(stack)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into();

                let add_button = button::icon(icon::from_name("list-add-symbolic"))
                    .large()
                    .icon_size(25)
                    .class(cosmic::theme::Button::Suggested)
                    .on_press(CanvasMessage::AddNode);

                // No built-in `.tooltip(...)`: that shows instantly on
                // hover, but this one should only fade in after a pause
                // (see `HoverTooltip`) — so hover tracking/the tooltip label
                // are both handled by hand instead.
                let add_button: Element<'_, CanvasMessage> = widget::mouse_area(add_button)
                    .on_enter(CanvasMessage::AddButtonHoverEnter)
                    .on_exit(CanvasMessage::AddButtonHoverExit)
                    .into();

                let tooltip = toast_box(fl!("tooltip-add-node"), self.add_button_tooltip.alpha());

                with_corner_button(canvas_area, add_button, tooltip, ADD_BUTTON_PADDING, ADD_BUTTON_TOOLTIP_OFFSET)
            },
        };

        match &self.pending_delete {
            Some((_, dialog)) => with_overlay(content, dialog.view().map(CanvasMessage::ConfirmDelete), SHADE_ALPHA),
            None => content,
        }
    }

    pub fn update(&mut self, message: CanvasMessage) -> Option<Uuid> {
        match message {
            CanvasMessage::AddNode => {
                let center = self.world_center();
                let default_node = StoryNode::default();
                let jitter = spawn_jitter(&default_node.id);

                let top_left = NodePosition {
                    x: center.x - default_node.size.width / 2.0 + jitter.x,
                    y: center.y - default_node.size.height / 2.0 + jitter.y,
                };

                let node = StoryNode { position: top_left, ..default_node };
                self.nodes.push(node);
                self.geo_cache.clear();
                self.content_dirty = true;
                None
            },
            CanvasMessage::Panned(delta) => {
                self.offset += delta;
                self.geo_cache.clear(); // Clear the cache to force a redraw with the new offset.
                None
            },
            CanvasMessage::NodeDragged { id, delta } => {
                if let Some(node) = self.nodes.iter_mut().find(|node| node.id == id) {
                    node.position.x += delta.x;
                    node.position.y += delta.y;
                    self.content_dirty = true;
                }
                self.geo_cache.clear();
                None
            },
            CanvasMessage::NodeClicked { id } => {
                if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                    let editor = StoryNodeEditor::new(node);
                    let size = node.size.clone();
                    let position = node.position.clone();

                    self.editor = Some(editor);
                    self.saved_camera = Some((self.offset, self.zoom));
                    // Otherwise the last-hovered node's delete button stays
                    // pinned (and clickable) for the rest of the editing
                    // session, since nothing else clears `hovered_node`
                    // while the cursor doesn't move.
                    self.hovered_node = None;

                    // Target zoom is 1.0, so world and screen units coincide
                    // and `screen = world + offset`.
                    let canvas_width = size.width * Self::EDITOR_GAP_MULTIPLIER;
                    let canvas_height = self.last_bounds.get().height;
                    let node_center = Point::new(
                        position.x + size.width / 2.0,
                        position.y + size.height / 2.0,
                    );
                    let target_offset = Vector::new(
                        canvas_width / 2.0 - node_center.x,
                        canvas_height / 2.0 - node_center.y,
                    );

                    self.start_camera_animation(target_offset, 1.0);
                }
                Some(id)
            }

            CanvasMessage::NodeHoverChanged(id) => {
                self.hovered_node = id;
                None
            }

            CanvasMessage::NodeDeleteRequested(id) => {
                self.request_delete(id);
                None
            }

            CanvasMessage::ConfirmDelete(ConfirmDialogMessage::Cancel) => {
                self.pending_delete = None;
                None
            }

            CanvasMessage::ConfirmDelete(ConfirmDialogMessage::Confirm) => {
                if let Some((id, _)) = self.pending_delete.take() {
                    self.nodes.retain(|n| n.id != id);
                    self.geo_cache.clear();
                    self.hovered_node = None;
                    self.content_dirty = true;

                    // Deleted the node currently being edited — close the
                    // editor and restore the camera, same as a normal Close.
                    if self.editor.as_ref().is_some_and(|e| e.node_id == id) {
                        self.close_editor();
                    }
                }
                None
            }

            CanvasMessage::Zoomed { at, scroll_amount } => {
                let old_zoom = self.zoom;
                let new_zoom = (old_zoom * (1.0 + scroll_amount * Self::ZOOM_SENSITIVITY))
                    // Clamp the zoom level to prevent it from going too far in or out.
                    .clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);

                let world_x = (at.x - self.offset.x) / old_zoom;
                let world_y = (at.y - self.offset.y) / old_zoom;

                self.offset = Vector::new(
                    at.x - world_x * new_zoom,
                    at.y - world_y * new_zoom,
                );

                self.zoom = new_zoom;
                self.geo_cache.clear();

                None
            }

            CanvasMessage::Editor(message) => {
                let Some(editor) = &mut self.editor else {
                    return None;
                };

                let event = editor.update(message);
                let node_id = editor.node_id;

                match event {
                    EditorEvent::None => {}
                    EditorEvent::TitleChanged(new_title) => {
                        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                            node.title = new_title;
                        }

                        self.geo_cache.clear();
                        self.content_dirty = true;
                    }
                    EditorEvent::BlocksChanged(blocks) => {
                        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                            node.blocks = blocks;
                        }

                        // Choice targets may have changed, and those are
                        // drawn (as edges) by the cached geometry pass.
                        self.geo_cache.clear();
                        self.content_dirty = true;
                    }
                    EditorEvent::Closed(pending_blocks) => {
                        // Closing auto-commits an in-flight prose draft;
                        // write it through like any other block change.
                        if let Some(blocks) = pending_blocks {
                            if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                                node.blocks = blocks;
                            }
                            self.geo_cache.clear();
                            self.content_dirty = true;
                        }
                        self.close_editor();
                    }
                }
                None
            }

            CanvasMessage::AnimationTick => {
                if let Some(anim) = &self.camera_anim {
                    let elapsed = anim.start.elapsed().as_secs_f32();
                    let duration = anim.duration.as_secs_f32();
                    let t = if duration > 0.0 { (elapsed / duration).clamp(0.0, 1.0) } else { 1.0 };

                    let offset = Vector::new(
                        cosmic::anim::slerp(anim.start_offset.x, anim.target_offset.x, t),
                        cosmic::anim::slerp(anim.start_offset.y, anim.target_offset.y, t),
                    );
                    let zoom = cosmic::anim::slerp(anim.start_zoom, anim.target_zoom, t);
                    let finished = t >= 1.0;

                    self.offset = offset;
                    self.zoom = zoom;
                    self.geo_cache.clear();

                    if finished {
                        self.camera_anim = None;
                    }
                }

                if let Some((_, started)) = self.glow {
                    if is_pulse_active(started, NODE_GLOW_FADE_IN, NODE_GLOW_VISIBLE, NODE_GLOW_FADE_OUT) {
                        // Time-based, not offset/zoom-based, so nothing
                        // else already clears the cache for it — the ring's
                        // alpha wouldn't otherwise update frame-to-frame.
                        self.geo_cache.clear();
                    } else {
                        self.glow = None;
                    }
                }

                self.add_button_tooltip.tick();
                None
            }

            CanvasMessage::AddButtonHoverEnter => {
                self.add_button_tooltip.enter();
                None
            }

            CanvasMessage::AddButtonHoverExit => {
                self.add_button_tooltip.exit();
                None
            }

            CanvasMessage::EditorBoundsChanged(new_size) => {
                self.last_bounds.set(new_size);

                // Only the vertical center ever depends on the canvas's
                // bounds — `canvas_width` (see `NodeClicked`) is derived
                // purely from the node's own width, so it can't go stale.
                if let Some(editor) = &self.editor
                    && let Some(node) = self.nodes.iter().find(|n| n.id == editor.node_id)
                {
                    let node_center_y = node.position.y + node.size.height / 2.0;
                    let target_y = new_size.height / 2.0 - node_center_y;

                    if let Some(anim) = &mut self.camera_anim {
                        // A resize mid-animation retargets it instead of
                        // fighting it frame-by-frame.
                        anim.target_offset.y = target_y;
                    } else {
                        self.offset.y = target_y;
                        self.geo_cache.clear();
                    }
                }
                None
            }
        }
    }

    fn start_camera_animation(&mut self, target_offset: Vector, target_zoom: f32) {
        self.camera_anim = Some(CameraAnimation {
            start: Instant::now(),
            duration: Self::CAMERA_ANIM_DURATION,
            start_offset: self.offset,
            start_zoom: self.zoom,
            target_offset,
            target_zoom,
        });
    }

    /// Drops the open editor and restores the pre-edit camera — the shared
    /// second half of every path that ends an edit session (a clean Close,
    /// Discard from the unsaved-changes warning, a Save-and-close, or the
    /// edited node being deleted out from under it).
    fn close_editor(&mut self) {
        self.editor = None;
        if let Some((offset, zoom)) = self.saved_camera.take() {
            self.start_camera_animation(offset, zoom);
        }
    }

    /// Takes whatever `content_dirty` has accumulated since the last time
    /// this was called — `AppModel` folds it into its own project-wide
    /// dirty flag after every message it forwards here.
    pub fn take_content_dirty(&mut self) -> bool {
        std::mem::take(&mut self.content_dirty)
    }

    /// Builds the delete-confirmation dialog for node `id` (using its
    /// current title) and stores it as `pending_delete`; shared by the
    /// hover-delete button and the open editor's own Delete button.
    fn request_delete(&mut self, id: Uuid) {
        let title = self.nodes.iter().find(|n| n.id == id).map(|n| n.title.clone()).unwrap_or_default();
        let dialog = ConfirmDialog::new(
            fl!("confirm-delete-node-title"),
            fl!("confirm-delete-node-message", title = title.as_str()),
        );

        self.pending_delete = Some((id, dialog));
    }

    // Outputs the current screen's coordinates to where it is
    // positioned to World coordinates. Useful for ading elements
    // to the canvas based on screen position.
    fn screen_to_world(&self, point: Point) -> Point {
        Point::new(
            (point.x - self.offset.x) / self.zoom,
            (point.y - self.offset.y) / self.zoom,
        )
    }

    // This is what we use to immediately gaint center coords
    fn world_center(&self) -> Point {
        let size = self.last_bounds.get();
        let screen_center = Point::new(size.width / 2.0, size.height / 2.0);
        self.screen_to_world(screen_center)
    }

    // Separate inverse function to convert world coordinates to
    // screen so widgets in the view() can be positioned correctly.
    fn world_to_screen(&self, point: Point) -> Point {
    Point::new(
        point.x * self.zoom + self.offset.x,
        point.y * self.zoom + self.offset.y,
    )
}
}

/// The mouse gesture currently in progress on the canvas, if any. Lives in
/// `CanvasState::interaction`; see `canvas::Program::update` for how a
/// button press picks one of these and later events (`CursorMoved`,
/// `ButtonReleased`) act on whichever variant is active.
#[derive(Default, Clone, Copy)]

pub enum CanvasInteraction {
    #[default]
    Idle,
    Panning {last: Point}, // Where the last mouse position was when panning started.
    DraggingNode {
        id: Uuid,
        last: Point,
        start: Point,
    }
}

/// Persistent canvas widget state: the current gesture plus the live
/// keyboard modifiers, tracked independently since `Ctrl` needs to be known
/// the moment a mouse button is pressed, not just while a gesture is active.
#[derive(Default, Clone, Copy)]
pub struct CanvasState {
    interaction: CanvasInteraction,
    modifiers: Modifiers,
}

/// The renderer for the canvas page. `fn draw()` uses
/// immediate mode rendering. This gets redrawn every frame.
//
//  Note: Do NOT use fill_text() here or render anything that has it. While it is
//  suppported, it renders on top of all other elements and does not respect the
//  z-ordering of the canvas. If you need to render text, use the view() instead.
//
//  Read more here:
//  https://pop-os.github.io/libcosmic/cosmic/iced/daemon/program/graphics/geometry/struct.Frame.html#method.fill_text
impl canvas::Program<CanvasMessage, Theme, Renderer> for CanvasPage {
    type State = CanvasState;

    fn update(
        &self,
        state: &mut CanvasState,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasMessage>> {
        // Keep modifier tracking live even while the editor has the canvas
        // frozen below, so Ctrl-panning is correct the instant control
        // returns rather than lagging one keypress behind.
        if let canvas::Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
            return None;
        }

        // The click-to-edit animation's target offset is computed once, from
        // the canvas bounds *at click time* — so if those bounds change
        // while the editor is open (a window resize or maximize/restore —
        // both just resize the surface under the hood, plus a nav bar
        // toggle) the edited node is left off-center unless something
        // re-centers it. Comparing against `last_bounds` on every event
        // (rather than matching a specific `Event::Window` variant) catches
        // all of those the same way, without needing to know which exact
        // event a given compositor/gesture happens to deliver.
        if self.editor.is_some() && bounds.size() != self.last_bounds.get() {
            return Some(canvas::Action::publish(CanvasMessage::EditorBoundsChanged(bounds.size())).and_capture());
        }

        // Freeze all canvas interaction — panning, node dragging, clicking
        // another node to open a new editor session, zoom, and hover — while
        // the editor or a delete confirmation is open. Otherwise those would
        // fight the camera position the click-to-edit animation just settled
        // on, or let the user keep interacting with a canvas that's about to
        // lose a node. Control returns once both are closed.
        if self.editor.is_some() || self.pending_delete.is_some() {
            return Some(canvas::Action::capture());
        }

        let cursor_position = cursor.position_in(bounds)?;

        match event {
            // Middle-click-drag always pans, regardless of what's under the cursor.
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Middle)) => {
                state.interaction = CanvasInteraction::Panning { last: cursor_position };
                Some(canvas::Action::capture())
            }

            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if state.modifiers.control() {
                    // Ctrl+drag always pans, even with the cursor on top of a node.
                    state.interaction = CanvasInteraction::Panning { last: cursor_position };
                } else {
                    let world_position = self.screen_to_world(cursor_position);

                    // With no modifiers, a plain left click only ever drags or
                    // clicks a node — it no longer pans when it misses one.
                    if let Some(node) = self.nodes.iter().rev().find(|n| n.contains(world_position)) {
                        state.interaction = CanvasInteraction::DraggingNode {
                            id: node.id,
                            last: cursor_position,
                            start: cursor_position,
                        };
                    }
                }

                Some(canvas::Action::capture())
            }
            // This is where we create match arms for `state.interaction` to handle the panning logic.
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => match state.interaction {
                // Idle just tracks which node (if any) is under the cursor,
                // so `view()` can show a hover-delete button on it — see
                // `hovered_node`. This is computed here (rather than via a
                // `mouse_area` on each pinned node widget in `view()`)
                // because a capturing `mouse_area` on top of the raw canvas
                // in the `Stack` would swallow the `ButtonPressed` events
                // this very function needs for dragging/click-to-edit.
                CanvasInteraction::Idle => {
                    let world_position = self.screen_to_world(cursor_position);
                    let hovered = self.nodes.iter().rev()
                        .find(|n| n.contains(world_position))
                        .map(|n| n.id);

                    (hovered != self.hovered_node)
                        .then(|| canvas::Action::publish(CanvasMessage::NodeHoverChanged(hovered)))
                }
                // This updates practically every frame while your mouse moves on the canvas.
                CanvasInteraction::Panning { last } => {
                    let delta = cursor_position - last;
                    state.interaction = CanvasInteraction::Panning { last: cursor_position };
                    Some(canvas::Action::publish(CanvasMessage::Panned(delta)).and_capture())
                },

                CanvasInteraction::DraggingNode { id, last, start } => {
                    let screen_delta = cursor_position - last;
                    let world_delta = Vector::new(
                        screen_delta.x / self.zoom,
                        screen_delta.y / self.zoom,
                    );
                    state.interaction = CanvasInteraction::DraggingNode { id, last: cursor_position, start };
                    Some(
                        canvas::Action::publish(CanvasMessage::NodeDragged { id, delta: world_delta })
                        .and_capture()
                    )
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left | mouse::Button::Middle)) => {
                let click_threshold: f32 = 5.0;
                // Decide, only now that the gesture is over, whether it was a
                // click or a drag — by checking total distance traveled since
                // the original press.
                let message = match state.interaction {
                    CanvasInteraction::DraggingNode { id, start, .. } => {
                        let distance = cursor_position - start;
                        let moved = (distance.x * distance.x + distance.y * distance.y).sqrt();
                        (moved < click_threshold).then_some(CanvasMessage::NodeClicked { id })
                    }
                    _ => None,
                };

                state.interaction = CanvasInteraction::Idle;

                match message {
                    Some(message) => Some(canvas::Action::publish(message).and_capture()),
                    None => Some(canvas::Action::capture()),
                }
            }

            canvas::Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                // Identify if scroll is from trackpag or mouse,
                // and handle balancing values accordingly.
                let y = match *delta {
                    // Mouse wheel reports integer "lines".
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    // Trackpads report "pixels" with larger numbers.
                    mouse::ScrollDelta::Pixels { y, .. } => y / 60.0,
                };

                // Send message to update the zoom values.
                Some(
                    canvas::Action::publish(CanvasMessage::Zoomed {
                        at: cursor_position,
                        scroll_amount: y,
                    })
                    .and_capture(),
                )
            }
            _ => None
        }
    }


    /// We draw primitive geometry here.
    /// In this method, things are drawn in immediate mode.
    fn draw(
        &self,
        _state: &CanvasState,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>>
    {
        self.last_bounds.set(bounds.size());
        vec![self.geo_cache.draw(renderer, bounds.size(), |frame: &mut canvas::Frame| {
            // Apply the current offset to the frame before drawing the grid.
            frame.translate(self.offset);

            // Must be applied AFTER translation, otherwise the grid will not scale correctly.
            frame.scale(self.zoom);

            // Grid settings
            let grid_color = Color::from_rgb8(46, 46, 46);
            let grid_width = 1.0;

            //  Adaptive grid to zoom and spacing settings
            let base_spacing = 50.0;
            let level = self.zoom.log2().floor();
            let scale = 2.0_f32.powf(level);
            let spacing = base_spacing / scale;

            // Calculate world coordinates for visible area.
            // screen = world * zoom + offset  =>  world = (screen - offset) / zoom
            let world_min_x = -self.offset.x / self.zoom;
            let world_min_y = -self.offset.y / self.zoom;
            let world_max_x = (bounds.width - self.offset.x) / self.zoom;
            let world_max_y = (bounds.height - self.offset.y) / self.zoom;

            // Colums and Rows calculation to evenly render grid lines.
            // floor/ceil + 1 cell of padding so dots don't visibly pop in
            // right at the screen edge as you pan.
            let col_start = (world_min_x / spacing).floor() as i32 - 1;
            let col_end = (world_max_x / spacing).ceil() as i32 + 1;
            let row_start = (world_min_y / spacing).floor() as i32 - 1;
            let row_end = (world_max_y / spacing).ceil() as i32 + 1;

            // Render vertical grid lines
            for col in col_start..=col_end {
                let x = col as f32 * spacing;

                let line = canvas::Path::line(
                    Point::new(x, world_min_y),
                    Point::new(x, world_max_y),
                );

                frame.stroke(
                    &line,
                    canvas::Stroke::default()
                    .with_color(grid_color)
                    .with_width(grid_width)
                );
            }

            // Render horizontal grid lines
            for row in row_start..=row_end {
                let y = row as f32 * spacing;

                let line = canvas::Path::line(
                    Point::new(world_min_x, y),
                    Point::new(world_max_x, y),
                );

                frame.stroke(
                    &line,
                    canvas::Stroke::default()
                    .with_color(grid_color)
                    .with_width(grid_width)
                );
            }

            // Story-graph edges: one arrow per assigned choice-option
            // target, drawn before the nodes so the curves pass under
            // their opaque fills. Dangling targets (deleted nodes) and
            // self-loops draw nothing.
            let edge_color = Color::from_rgb8(120, 120, 130);
            for node in &self.nodes {
                for target_id in node.outgoing_targets() {
                    if target_id == node.id {
                        continue;
                    }
                    let Some(target) = self.nodes.iter().find(|n| n.id == target_id) else {
                        continue;
                    };

                    let source_center = Point::new(
                        node.position.x + node.size.width / 2.0,
                        node.position.y + node.size.height / 2.0,
                    );
                    let target_center = Point::new(
                        target.position.x + target.size.width / 2.0,
                        target.position.y + target.size.height / 2.0,
                    );
                    let dx = target_center.x - source_center.x;
                    let dy = target_center.y - source_center.y;

                    // Route along the dominant axis, leaving/entering the
                    // facing node edges, with the anchors shifted sideways
                    // by travel direction (EDGE_LANE_OFFSET) — so A→B and
                    // B→A run on two parallel lanes instead of tangling,
                    // however the nodes are arranged. `dir` is the
                    // axis-aligned travel direction at both endpoints.
                    let (start, end, dir) = if dy.abs() >= dx.abs() {
                        let sign = if dy >= 0.0 { 1.0 } else { -1.0 };
                        let start = Point::new(
                            source_center.x + sign * EDGE_LANE_OFFSET,
                            if sign > 0.0 { node.position.y + node.size.height } else { node.position.y },
                        );
                        let end = Point::new(
                            target_center.x + sign * EDGE_LANE_OFFSET,
                            if sign > 0.0 { target.position.y } else { target.position.y + target.size.height },
                        );
                        (start, end, Vector::new(0.0, sign))
                    } else {
                        let sign = if dx >= 0.0 { 1.0 } else { -1.0 };
                        let start = Point::new(
                            if sign > 0.0 { node.position.x + node.size.width } else { node.position.x },
                            source_center.y + sign * EDGE_LANE_OFFSET,
                        );
                        let end = Point::new(
                            if sign > 0.0 { target.position.x } else { target.position.x + target.size.width },
                            target_center.y + sign * EDGE_LANE_OFFSET,
                        );
                        (start, end, Vector::new(sign, 0.0))
                    };

                    // An S-curve that leaves and arrives along `dir`; the
                    // control points sit `bend` past each endpoint on the
                    // routing axis.
                    let span = (dir.x * (end.x - start.x) + dir.y * (end.y - start.y)).abs();
                    let bend = (span / 2.0).clamp(EDGE_MIN_BEND, EDGE_MAX_BEND);
                    let curve = canvas::Path::new(|builder| {
                        builder.move_to(start);
                        builder.bezier_curve_to(
                            Point::new(start.x + dir.x * bend, start.y + dir.y * bend),
                            Point::new(end.x - dir.x * bend, end.y - dir.y * bend),
                            end,
                        );
                    });
                    frame.stroke(
                        &curve,
                        canvas::Stroke::default()
                            .with_color(edge_color)
                            .with_width(1.5),
                    );

                    // Arrowhead at `end`, pointing along `dir` — the last
                    // control point sits straight behind the tip, so the
                    // curve always arrives axis-aligned.
                    let base = Point::new(end.x - dir.x * 9.0, end.y - dir.y * 9.0);
                    let perp = Vector::new(-dir.y, dir.x);
                    let arrow = canvas::Path::new(|builder| {
                        builder.move_to(end);
                        builder.line_to(Point::new(base.x + perp.x * 5.0, base.y + perp.y * 5.0));
                        builder.line_to(Point::new(base.x - perp.x * 5.0, base.y - perp.y * 5.0));
                        builder.close();
                    });
                    frame.fill(&arrow, edge_color);
                }
            }

            for node in &self.nodes {
                // Drawn *before* the node's own opaque fill/stroke, inflated
                // beyond its bounds, so the ring peeks out around the edges
                // instead of being covered by it (or by the pinned title
                // label in `view()`, which exactly matches the node's own
                // bounds too).
                if let Some((glow_id, started)) = self.glow
                    && glow_id == node.id
                    && is_pulse_active(started, NODE_GLOW_FADE_IN, NODE_GLOW_VISIBLE, NODE_GLOW_FADE_OUT)
                {
                    let alpha = pulse_alpha(started, NODE_GLOW_FADE_IN, NODE_GLOW_VISIBLE, NODE_GLOW_FADE_OUT);
                    node.draw_glow(frame, alpha);
                }

                node.draw(frame); // no text, just the rounded rectangle
            }
        })]

    }
}
