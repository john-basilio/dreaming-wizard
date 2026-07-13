//! Small, reusable floating-UI helpers: a dimming backdrop for modal
//! overlays, a fading toast notice, and `HoverTooltip` (a delayed-fade
//! tooltip timer for the canvas/characters pages' floating "add" buttons).
//! Pure presentation — no knowledge of `AppModel` or any other app-specific
//! state; callers pass in whatever content and timing they need.
//! `app::overlays` is the current consumer of the popup/save-dialog/toast
//! layering, but nothing here is app-shell specific.

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

/// Layers `button` in the bottom-right corner of `content`, `padding` away
/// from both edges, with `tooltip` floating just above it (also
/// bottom-right anchored, `tooltip_offset` further up than `padding`).
/// `tooltip` is expected to already have its own alpha baked in (see
/// `toast_box`/`HoverTooltip`) — at `alpha == 0.0` it's fully transparent,
/// so it's safe to always include it here without shifting the button's
/// own layout when it fades in/out. Used for the canvas/characters pages'
/// floating "add" buttons.
pub fn with_corner_button<'a, Message: 'a>(
    content: Element<'a, Message>,
    button: Element<'a, Message>,
    tooltip: Element<'a, Message>,
    padding: f32,
    tooltip_offset: f32,
) -> Element<'a, Message> {
    let corner = widget::container(button)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Bottom);

    let tooltip_layer = widget::container(tooltip)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(cosmic::iced::Padding {
            top: 0.0,
            right: padding,
            bottom: padding + tooltip_offset,
            left: padding,
        })
        .align_x(Horizontal::Right)
        .align_y(Vertical::Bottom);

    cosmic::iced::widget::Stack::new()
        .push(content)
        .push(corner)
        .push(tooltip_layer)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Layers `panel` in the top-right corner of `content`, `padding` away from
/// both edges — unlike `with_overlay`, there's no dimming shade and nothing
/// captures clicks, so `content` underneath stays fully interactive (pan,
/// drag, scroll...). A plain layout `container` never intercepts pointer
/// events on its own; only `panel`'s own interactive widgets do. Used for
/// the Find panel.
pub fn with_corner_panel<'a, Message: 'a>(
    content: Element<'a, Message>,
    panel: Element<'a, Message>,
    padding: f32,
) -> Element<'a, Message> {
    let corner = widget::container(panel)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(padding)
        .align_x(Horizontal::Right)
        .align_y(Vertical::Top);

    cosmic::iced::widget::Stack::new()
        .push(content)
        .push(corner)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

/// Delay before a hover-triggered tooltip starts to appear, and how long
/// its fade (in on delayed-hover, out on unhover) takes. Shared by the
/// canvas/characters pages' floating "add" buttons via `HoverTooltip`.
pub const TOOLTIP_HOVER_DELAY: Duration = Duration::from_millis(1500);
pub const TOOLTIP_FADE: Duration = Duration::from_millis(300);

/// Drives a hover-triggered tooltip's fade timing: invisible until the
/// cursor has rested on its target for `TOOLTIP_HOVER_DELAY`, then fades in
/// over `TOOLTIP_FADE`; fades back out over `TOOLTIP_FADE` on unhover — or,
/// if the cursor left before the tooltip ever finished its delay, simply
/// resets with no fade-out (nothing was shown yet to fade).
#[derive(Default)]
pub struct HoverTooltip {
    /// Set on hover-enter, cleared on hover-exit.
    hover_started: Option<Instant>,
    /// Set on hover-exit, but only once the tooltip had actually started
    /// appearing; cleared once its fade-out finishes (see `tick`).
    hover_ended: Option<Instant>,
}

impl HoverTooltip {
    pub fn enter(&mut self) {
        self.hover_started = Some(Instant::now());
        self.hover_ended = None;
    }

    pub fn exit(&mut self) {
        if let Some(started) = self.hover_started.take()
            && started.elapsed() >= TOOLTIP_HOVER_DELAY
        {
            self.hover_ended = Some(Instant::now());
        }
    }

    /// Called on every animation tick; clears a finished fade-out so
    /// `is_active` can go back to `false` and the driving subscription can
    /// stop ticking.
    pub fn tick(&mut self) {
        if let Some(ended) = self.hover_ended
            && ended.elapsed() >= TOOLTIP_FADE
        {
            self.hover_ended = None;
        }
    }

    /// Whether a subscription needs to keep ticking this to drive its
    /// animation forward.
    pub fn is_active(&self) -> bool {
        self.hover_started.is_some() || self.hover_ended.is_some()
    }

    /// Current opacity, `0.0` to `1.0`.
    pub fn alpha(&self) -> f32 {
        if let Some(ended) = self.hover_ended {
            return (1.0 - ended.elapsed().as_secs_f32() / TOOLTIP_FADE.as_secs_f32()).clamp(0.0, 1.0);
        }

        let Some(started) = self.hover_started else {
            return 0.0;
        };

        let elapsed = started.elapsed();
        if elapsed < TOOLTIP_HOVER_DELAY {
            return 0.0;
        }
        ((elapsed - TOOLTIP_HOVER_DELAY).as_secs_f32() / TOOLTIP_FADE.as_secs_f32()).clamp(0.0, 1.0)
    }
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

/// One-shot fade-in/hold/fade-out curve: ramps up over `fade_in`, holds at
/// full opacity for `visible`, then ramps back down to `0.0` over
/// `fade_out`. Used for the Find feature's "found it" glow highlight around
/// a node/character card, which (unlike `fade_alpha`'s toast use) needs a
/// gentle fade-in rather than snapping straight to fully visible.
pub fn pulse_alpha(started_at: Instant, fade_in: Duration, visible: Duration, fade_out: Duration) -> f32 {
    let elapsed = started_at.elapsed();

    if elapsed < fade_in {
        return (elapsed.as_secs_f32() / fade_in.as_secs_f32()).clamp(0.0, 1.0);
    }

    let elapsed = elapsed - fade_in;
    if elapsed < visible {
        return 1.0;
    }

    let elapsed = elapsed - visible;
    (1.0 - elapsed.as_secs_f32() / fade_out.as_secs_f32()).clamp(0.0, 1.0)
}

/// Whether `pulse_alpha` still has anything left to show — used to gate the
/// animation-tick subscription while a glow highlight is active.
pub fn is_pulse_active(started_at: Instant, fade_in: Duration, visible: Duration, fade_out: Duration) -> bool {
    started_at.elapsed() < fade_in + visible + fade_out
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
