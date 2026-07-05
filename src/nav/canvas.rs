
use std::cell::Cell;
use cosmic::{
    Element, Renderer, Theme, iced::{
        Color, Length, Point, Radius, Rectangle, Vector, mouse
    }, widget::{self, canvas},
};
// use cosmic::theme::Container as ContainerStyle;
use cosmic::iced::{Border, Background, Size};

use crate::components::{
    StoryNode, 
    StoryNodeEditor, 
    story_node_editor::{EditorEvent, EditorMessage},};

/// Responsible for providing unique UUIDs for each node
/// so that they can be identified by other components.
use uuid::Uuid;

/// This page model, responsible for rendering the canvas,
/// where all the story node lives and are rendered.
pub struct CanvasPage {
    // shared fields — used by both draw() and view()
    pub offset: Vector,
    pub zoom: f32,

    // draw() unique fields
    pub geo_cache: canvas::Cache,
    pub last_bounds: Cell<Size>, // updated every draw(), read by update() for world_center()

    // view() unique fields
    pub nodes: Option<Vec<StoryNode>>,
    pub editor: Option<StoryNodeEditor>,
}

/// Messages emitted by the canvas page. 
#[derive(Debug, Clone)]

pub enum CanvasMessage {
    AddNode,
    Panned(Vector),
    Zoomed {
        at: Point,
        scroll_amount: f32,
    },
    NodeDragged { 
        id: Uuid,
        delta: Vector,
    },
    NodeClicked {id: Uuid},
    Editor(EditorMessage),

}


impl Default for CanvasPage {
    fn default() -> Self {
        Self {
            geo_cache: canvas::Cache::new(),
            zoom: 1.0, // Needs to be at least 1.0 to avoid division of 0.
            offset: Vector::new(0.0, 0.0),
            nodes: Some(vec![StoryNode::default()]), // temporary, default NONE,
            editor: None,
            last_bounds: Cell::new(Size::new(800.0, 600.0)), // fallback before first draw
        }
    }
}


/// This is where we put other custom methods. 
impl CanvasPage {
    const MIN_ZOOM: f32 = 0.1;
    const MAX_ZOOM: f32 = 4.0;
    const ZOOM_SENSITIVITY: f32 = 0.1;

    pub fn view(&self) -> Element<'_, CanvasMessage> {
        use cosmic::iced::widget::{Stack, pin};


        let canvas_element = widget::canvas(self)
            .width(Length::Fill)
            .height(Length::Fill);

        let nodes = self.nodes.as_deref().unwrap_or(&[]);

        let stack = nodes.iter().fold(
            Stack::new().push(canvas_element),
            |stack, node| {
                let screen = self.world_to_screen(node.position);
                let screen_width = node.size.width * self.zoom;
                let screen_height = node.size.height * self.zoom;

                let node_widget = pin(
                    widget::container(
                        widget::text::body(node.display_title())
                            .width(Length::Fill)
                            .align_x(cosmic::iced::alignment::Horizontal::Center)
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
        )
        .clip(true);


        // Now we return the stack with the canvas and all the nodes.
        // Redraw with the editor if SOME.
        match &self.editor {
            Some(editor) => widget::row![
                widget::container(stack)
                    .width(Length::Fill)
                    .height(Length::Fill),
                editor.view().map(CanvasMessage::Editor),
            ].into(),

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

                let top_left = Point::new(
                    center.x - default_node.size.width / 2.0,
                    center.y - default_node.size.height / 2.0,
                );

                let node = StoryNode { position: top_left, ..default_node };
                self.nodes.get_or_insert_with(Vec::new).push(node);
                self.geo_cache.clear();
                None
            },
            CanvasMessage::Panned(delta) => {
                self.offset += delta;
                self.geo_cache.clear(); // Clear the cache to force a redraw with the new offset.
                None
            },
            CanvasMessage::NodeDragged { id, delta } => {
                // You have to check the Option<> first before you can iterate
                if let Some(nodes) = &mut self.nodes {
                    if let Some(node) = nodes.iter_mut().find(|node| node.id == id) {
                        node.position += delta;
                }}
                self.geo_cache.clear();
                None
            },
            CanvasMessage::NodeClicked { id } => {
                if let Some(nodes) = &mut self.nodes {
                    if let Some(node) = nodes.iter().find(|n| n.id == id) {
                        self.editor = Some(StoryNodeEditor::new(id, node.title.clone()))
                    }
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
                        if let Some(nodes) = &mut self.nodes {
                            if let Some(node) = nodes.iter_mut().find(|n| n.id == node_id) {
                                node.title = new_title;
                            }
                        }

                        self.geo_cache.clear();
                    }
                    EditorEvent::Closed => {self.editor = None}
                }
                None
            }
        }
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
    type State = CanvasInteraction;

    fn update(
        &self,
        state: &mut CanvasInteraction,
        event: &canvas::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<CanvasMessage>> {
        let cursor_position = cursor.position_in(bounds)?;

        match event {
            // TODO: Add doc later
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let world_position = self.screen_to_world(cursor_position);

                if let Some(node) = self.nodes.as_deref().unwrap_or(&[]).iter().rev().find(|n| n.contains(world_position)) {
                    *state = CanvasInteraction::DraggingNode {
                        id: node.id,
                        last: cursor_position,
                        start: cursor_position,
                    };
                } else {
                    // If no node was clicked, we start panning the canvas.
                    *state = CanvasInteraction::Panning { last: cursor_position };
                }

                Some(canvas::Action::capture())
            }
            // This is where we create match arms for `state: &mut CanvasInteraction` to handle the panning logic.
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => match *state {
                CanvasInteraction::Idle => None,
                // This updates practically every frame while your mouse moves on the canvas.
                CanvasInteraction::Panning { last } => {
                    let delta = cursor_position - last;
                    *state = CanvasInteraction::Panning { last: cursor_position };
                    Some(canvas::Action::publish(CanvasMessage::Panned(delta)).and_capture())
                },

                CanvasInteraction::DraggingNode { id, last, start } => {
                    let screen_delta = cursor_position - last;
                    let world_delta = Vector::new(
                        screen_delta.x / self.zoom,
                        screen_delta.y / self.zoom,
                    );
                    *state = CanvasInteraction::DraggingNode { id, last: cursor_position, start };
                    Some(
                        canvas::Action::publish(CanvasMessage::NodeDragged { id, delta: world_delta })
                        .and_capture()
                    )
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let click_threshold: f32 = 5.0;
                // Decide, only now that the gesture is over, whether it was a
                // click or a drag — by checking total distance traveled since
                // the original press.
                let message = match *state {
                    CanvasInteraction::DraggingNode { id, start, .. } => {
                        let distance = cursor_position - start;
                        let moved = (distance.x * distance.x + distance.y * distance.y).sqrt();
                        (moved < click_threshold).then_some(CanvasMessage::NodeClicked { id })
                    }
                    _ => None,
                };

                *state = CanvasInteraction::Idle;

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
        _state: &CanvasInteraction,
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

            for node in self.nodes.as_deref().unwrap_or(&[]) {
                node.draw(frame); // no text, just the rounded rectangle
            }
        })]
        
    }
}

