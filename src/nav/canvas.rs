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
    }, widget::{self, canvas},
};
// use cosmic::theme::Container as ContainerStyle;
use cosmic::iced::{Border, Background, Size};

use crate::components::{
    NodePosition, 
    StoryNode, 
    StoryNodeEditor, 
    display_title, 
    story_node_editor::{EditorEvent, EditorMessage},
};

/// Responsible for providing unique UUIDs for each node
/// so that they can be identified by other components.
use uuid::Uuid;

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
    /// Forwarded from the open `StoryNodeEditor`'s own `view()`.
    Editor(EditorMessage),
    /// One frame of an in-flight `CameraAnimation`; see `is_animating_camera`
    /// and `AppModel::subscription` for how these get scheduled.
    AnimationTick,

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

    pub fn view(&self) -> Element<'_, CanvasMessage> {
        use cosmic::iced::widget::{Stack, pin};

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

                stack.push(node_widget)
            }
        ).clip(true);


        // Now we return the stack with the canvas and all the nodes.
        // Redraw with the editor if SOME.
        match &self.editor {
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
                    editor.view().map(CanvasMessage::Editor),
                ].into()
            },

            None => widget::container(stack)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into(),
        }
    }

    pub fn update(&mut self, message: CanvasMessage) -> Option<Uuid> {
        match message {
            CanvasMessage::AddNode => {
                let center = self.world_center();
                let default_node = StoryNode::default();

                let top_left = NodePosition {
                    x: center.x - default_node.size.width / 2.0,
                    y: center.y - default_node.size.height / 2.0,
                };

                let node = StoryNode { position: top_left, ..default_node };
                self.nodes.push(node);
                self.geo_cache.clear();
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
                }
                self.geo_cache.clear();
                None
            },
            CanvasMessage::NodeClicked { id } => {
                if let Some(node) = self.nodes.iter().find(|n| n.id == id) {
                    let title = node.title.clone();
                    let size = node.size.clone();
                    let position = node.position.clone();

                    self.editor = Some(StoryNodeEditor::new(id, title));
                    self.saved_camera = Some((self.offset, self.zoom));

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
                    EditorEvent::TitleCommitted(new_title) => {
                        if let Some(node) = self.nodes.iter_mut().find(|n| n.id == node_id) {
                            node.title = new_title;
                        }

                        self.geo_cache.clear();
                    }
                    EditorEvent::Closed => {
                        self.editor = None;
                        if let Some((offset, zoom)) = self.saved_camera.take() {
                            self.start_camera_animation(offset, zoom);
                        }
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

        // Freeze all canvas interaction — panning, node dragging, clicking
        // another node to open a new editor session, and zoom — while the
        // editor is open. Otherwise those would fight the camera position
        // the click-to-edit animation just settled on. Control returns once
        // the editor is closed.
        if self.editor.is_some() {
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
                CanvasInteraction::Idle => None,
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

            for node in &self.nodes {
                node.draw(frame); // no text, just the rounded rectangle
            }
        })]
        
    }
}

