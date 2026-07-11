use uuid::Uuid;

use cosmic::iced::{
    Color, Point, Rectangle, Size,
};
use cosmic::widget::{
    canvas::{Frame, Path, Stroke}
};
use serde::{Serialize, Deserialize};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryNode {
    pub id: Uuid,
    pub position: NodePosition,
    pub size: NodeSize,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodePosition {
    pub x: f32,
    pub y: f32
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSize {
    pub width: f32,
    pub height: f32,
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
            size: NodeSize {width: 200.0, height: 100.0},
            title: "New Node".to_string() 
        }
    }
}

impl StoryNode {


    #[allow(dead_code)]
    // TODO: decide use
    pub fn new(id: Uuid, position: NodePosition, size: NodeSize, title: impl Into<String>) -> Self {
        Self { id, position, size, title: title.into() }
    }

    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.position.clone().into(), self.size.clone().into())

    }

    pub fn contains(&self, point: Point) -> bool {
        self.bounds().contains(point)
    }

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

}