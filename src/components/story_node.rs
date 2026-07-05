use uuid::Uuid;

#[allow(unused_imports)]
use cosmic::iced::{
    core::text::Alignment as TextAlignment,
    Color, Point, Rectangle, Size, alignment, Pixels
};
#[allow(unused_imports)]
use cosmic::widget::{
    canvas::{self,Frame, Path, Stroke}
};

#[derive(Debug, Clone)]
pub struct StoryNode {
    pub id: Uuid,
    pub position: Point,
    pub size: Size,
    pub title: String,
}

impl Default for StoryNode {
    fn default() -> Self {
        Self { 
            id: Uuid::new_v4(), 
            position: Point::new(0.0, 0.0), 
            size: Size::new(200.0, 100.0), 
            title: "New Node".to_string() 
        }
    }
}

impl StoryNode {

    #[allow(dead_code)]
    pub fn new(id: Uuid, position: Point, size: Size, title: impl Into<String>) -> Self {
        Self { id, position, size, title: title.into() }
    }

    pub fn bounds(&self) -> Rectangle {
        Rectangle::new(self.position, self.size)
    }

    pub fn contains(&self, point: Point) -> bool {
        self.bounds().contains(point)
    }

    pub fn draw(&self, frame: &mut Frame) {
        let path = Path::rounded_rectangle(self.position, self.size, 8.0.into());

        frame.fill(&path, Color::from_rgb8(45, 45, 48));

        frame.stroke(
            &path,
            Stroke::default()
                .with_color(Color::from_rgb8(70, 70, 70))
                .with_width(2.0),
        );        
    }

}