//! Small, reusable floating-UI helpers: a dimming backdrop for modal
//! overlays and a fading toast notice. Pure presentation — no knowledge of
//! `AppModel` or any other app-specific state; callers pass in whatever
//! content and timing they need. `app::overlays` is the current consumer
//! (popup/save-dialog/toast layering), but nothing here is app-shell
//! specific.

use std::time::{Duration, Instant};

use cosmic::Element;
use cosmic::iced::{Length, alignment::{Horizontal, Vertical}};
use cosmic::widget::{self, text};

/// A full-bleed black backdrop at `alpha`. `mouse_area` (no `on_press`)
/// unconditionally captures left-clicks within its bounds, so this also
/// blocks interaction with whatever's behind it without needing to handle
/// the click itself.
pub fn dimming_shade<'a, Message: Clone + 'a>(alpha: f32) -> Element<'a, Message> {
    widget::mouse_area(
        widget::container(widget::Space::new().width(Length::Fill).height(Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
                background: Some(cosmic::iced::Background::Color(
                    cosmic::iced::Color::from_rgba(0.0, 0.0, 0.0, alpha),
                )),
                ..Default::default()
            }),
    )
    .into()
}

/// Layers `overlay_content` centered over `content` behind a `dimming_shade`
/// at `shade_alpha`. The shade only blocks clicks from reaching `content`
/// underneath; it doesn't dismiss anything itself — each overlay's own
/// close/cancel button is the only way to dismiss it.
pub fn with_overlay<'a, Message: Clone + 'a>(
    content: Element<'a, Message>,
    overlay_content: Element<'a, Message>,
    shade_alpha: f32,
) -> Element<'a, Message> {
    let overlay_box = widget::container(overlay_content)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center);

    cosmic::iced::widget::Stack::new()
        .push(content)
        .push(dimming_shade(shade_alpha))
        .push(overlay_box)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Places `toast` horizontally centered near the top of `content`. Unlike
/// `with_overlay`, this has no dimming shade and nothing captures clicks —
/// a toast is non-modal and has no close button, so the rest of the page
/// must stay fully usable underneath it.
pub fn with_toast<'a, Message: 'a>(
    content: Element<'a, Message>,
    toast: Element<'a, Message>,
) -> Element<'a, Message> {
    let toast_layer = widget::container(toast)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding([20.0, 0.0, 0.0, 0.0])
        .align_x(Horizontal::Center)
        .align_y(Vertical::Top);

    cosmic::iced::widget::Stack::new()
        .push(content)
        .push(toast_layer)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Linear fade: fully opaque for `visible`, then ramps down to `0.0` over
/// `fade`, clamped so it never goes negative once `fade` has fully elapsed.
pub fn fade_alpha(shown_at: Instant, visible: Duration, fade: Duration) -> f32 {
    let elapsed = shown_at.elapsed();
    if elapsed < visible {
        return 1.0;
    }

    let fade_elapsed = (elapsed - visible).as_secs_f32();
    (1.0 - fade_elapsed / fade.as_secs_f32()).clamp(0.0, 1.0)
}

/// A small pill-shaped notice box (e.g. "Saved"), faded to `alpha`.
///
/// Doesn't use `cosmic::theme::Container::Tooltip` directly because that
/// class's colors can't be faded (its `text_color` is `None`, meaning
/// "inherit", and its style closure has no alpha parameter) — so its
/// neutral-surface background/text colors are reproduced here manually,
/// scaled by `alpha`.
pub fn toast_box<'a, Message: 'a>(label: impl Into<String>, alpha: f32) -> Element<'a, Message> {
    let cosmic = cosmic::theme::active().cosmic().clone();

    let mut background: cosmic::iced::Color = cosmic.palette.neutral_2.into();
    background.a *= alpha;
    let mut text_color: cosmic::iced::Color = cosmic.on_bg_color().into();
    text_color.a *= alpha;

    widget::container(text::body(label.into()).class(cosmic::theme::Text::Color(text_color)))
        .padding([8, 16])
        .style(move |_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
            background: Some(cosmic::iced::Background::Color(background)),
            border: cosmic::iced::Border {
                radius: cosmic.corner_radii.radius_l.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}
