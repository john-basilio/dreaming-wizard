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

    const FONT_SIZE: f32 = 16.0;
    const TEXT_PADDING: f32 = 12.0;
    // Rough average glyph width for a typical UI sans-serif font, as a
    // fraction of font size — an estimate, not a real measurement.
    // See `truncate()` for why.
    const AVG_CHAR_WIDTH_RATIO: f32 = 0.55;

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

    pub fn display_title(&self) -> String {
        let available = (self.size.width - Self::TEXT_PADDING * 2.0).max(0.0);
        Self::truncate(&self.title, available)
    }


    fn truncate(title: &str, available_width: f32) -> String {
        let avg_char_width = Self::FONT_SIZE * Self::AVG_CHAR_WIDTH_RATIO;
        if avg_char_width <= 0.0 {
            return String::new();
        }

        let max_chars = (available_width / avg_char_width).floor() as usize;
        let char_count = title.chars().count();

        if char_count <= max_chars {
            return title.to_string();
        }
        if max_chars == 0 {
            return String::new();
        }

        let keep = max_chars.saturating_sub(1);
        let mut truncated: String = title.chars().take(keep).collect();
        truncated.push('…');
        truncated
    }
}