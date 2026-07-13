//! A custom "confirm" button style for the story node/character card
//! editors' Save buttons. `cosmic::theme::Button::Suggested` (the built-in
//! "primary action" style) draws its resting-state background straight
//! from the desktop's configured accent color, which can end up close
//! enough to the disabled-button color that Save reads as unclickable —
//! this uses a fixed, always-legible off-white instead (with dark text),
//! staying in the app's black/white/red scheme rather than introducing a
//! new accent color, and unambiguous regardless of theme/accent.

use cosmic::iced::{Background, Color};
use cosmic::widget::button::Style;

const BASE: Color = Color::from_rgb(0.850, 0.850, 0.850);
const HOVER: Color = Color::from_rgb(0.950, 0.950, 0.950);
const PRESSED: Color = Color::from_rgb(0.720, 0.720, 0.720);
const TEXT: Color = Color::from_rgb(0.100, 0.100, 0.100);

fn style(background: Color) -> Style {
    let radius = cosmic::theme::active().cosmic().corner_radii.radius_xl;

    Style {
        background: Some(Background::Color(background)),
        text_color: Some(TEXT),
        icon_color: Some(TEXT),
        border_radius: radius.into(),
        ..Style::new()
    }
}

/// The `.class(...)` value for a Save button; see the module doc for why
/// this exists instead of `cosmic::theme::Button::Suggested`.
pub fn save_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, _theme| style(BASE)),
        hovered: Box::new(|_focused, _theme| style(HOVER)),
        pressed: Box::new(|_focused, _theme| style(PRESSED)),
        disabled: Box::new(|_theme| {
            let mut faded = BASE;
            faded.a *= 0.5;
            style(faded)
        }),
    }
}
