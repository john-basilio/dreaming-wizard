//! The "Preferences" nav page: three sections of settings.
//!
//! - **Project** edits the *open project's* metadata (author, comment,
//!   repository link) — those live in `project.json`, not the app config,
//!   and dirty the project like any other edit.
//! - **Development** and **Editor** edit app-wide `Config` fields,
//!   persisted through `cosmic_config`.
//!
//! The page itself is almost stateless: `view()` renders straight from
//! the `Config`/`ProjectData` the caller passes in, and every message is
//! applied by `AppModel` (which owns both) — the only state held here is
//! the language row's "look here" glow, triggered by Help → Language.

use std::time::{Duration, Instant};

use cosmic::Element;
use cosmic::iced::Length;
use cosmic::widget::{self, container, settings, spin_button, text_input, toggler};

use crate::components::ProjectData;
use crate::components::overlay::{is_pulse_active, pulse_alpha};
use crate::config::Config;
use crate::fl;

/// Timing of the language row's "look here" glow (see `flash_language`);
/// same feel as the Find panel's found-it glow.
const GLOW_FADE_IN: Duration = Duration::from_millis(200);
const GLOW_VISIBLE: Duration = Duration::from_millis(1500);
const GLOW_FADE_OUT: Duration = Duration::from_millis(800);

/// Width of the text inputs on the right side of setting rows.
const INPUT_WIDTH: f32 = 260.0;

/// Page model for the Preferences page.
#[derive(Default)]
pub struct SettingsPage {
    /// `Some(started)` while the language row's glow is fading in/
    /// holding/fading out — set by Help → Language (see `flash_language`).
    language_glow: Option<Instant>,
}

/// Messages emitted by the Preferences page; all applied by `AppModel`,
/// which owns the `Config` and `ProjectData` being edited.
#[derive(Debug, Clone)]
pub enum SettingsMessage {
    // Section 1: project metadata (dirty the project).
    AuthorChanged(String),
    CommentChanged(String),
    RepositoryChanged(String),
    // Section 2: development.
    AutosaveToggled(bool),
    AutosaveIntervalChanged(u32),
    ReopenToggled(bool),
    // Section 3: editor.
    ZoomSensitivityChanged(u32),
    PreviewLinesChanged(u32),
    ConfirmNodesToggled(bool),
    ConfirmCharactersToggled(bool),
    ConfirmBlocksToggled(bool),
    /// A language radio was picked; `None` = follow the system locale.
    LanguagePicked(Option<String>),
    /// Drives the language row's glow while it's active.
    AnimationTick,
}

impl SettingsPage {
    /// Starts the language row's glow — Help → Language lands here after
    /// switching to this page, so the eye is led to the right row.
    pub fn flash_language(&mut self) {
        self.language_glow = Some(Instant::now());
    }

    /// Whether the glow is mid-animation; used by the app's subscription
    /// the same way as the Find glows.
    pub fn is_glow_active(&self) -> bool {
        self.language_glow
            .is_some_and(|started| is_pulse_active(started, GLOW_FADE_IN, GLOW_VISIBLE, GLOW_FADE_OUT))
    }

    pub fn update(&mut self, message: &SettingsMessage) {
        if matches!(message, SettingsMessage::AnimationTick)
            && !self.is_glow_active()
        {
            self.language_glow = None;
        }
    }

    pub fn view<'a>(&'a self, config: &'a Config, meta: &'a ProjectData) -> Element<'a, SettingsMessage> {
        let project_section = settings::section()
            .title(fl!("prefs-section-project"))
            .add(settings::item(
                fl!("prefs-author"),
                text_input(fl!("prefs-author-placeholder"), meta.author.as_str())
                    .on_input(SettingsMessage::AuthorChanged)
                    .width(Length::Fixed(INPUT_WIDTH)),
            ))
            .add(settings::item(
                fl!("prefs-comment"),
                text_input(fl!("prefs-comment-placeholder"), meta.comment.as_str())
                    .on_input(SettingsMessage::CommentChanged)
                    .width(Length::Fixed(INPUT_WIDTH)),
            ))
            .add(settings::item(
                fl!("prefs-repository"),
                text_input(fl!("prefs-repository-placeholder"), meta.repository.as_str())
                    .on_input(SettingsMessage::RepositoryChanged)
                    .width(Length::Fixed(INPUT_WIDTH)),
            ));

        let development_section = settings::section()
            .title(fl!("prefs-section-development"))
            .add(settings::item(
                fl!("prefs-autosave"),
                toggler(config.autosave).on_toggle(SettingsMessage::AutosaveToggled),
            ))
            .add(settings::item(
                fl!("prefs-autosave-interval"),
                spin_button(
                    config.autosave_interval_minutes.to_string(),
                    fl!("prefs-autosave-interval"),
                    config.autosave_interval_minutes,
                    1,
                    1,
                    60,
                    SettingsMessage::AutosaveIntervalChanged,
                ),
            ))
            .add(settings::item(
                fl!("prefs-reopen"),
                toggler(config.reopen_last_project).on_toggle(SettingsMessage::ReopenToggled),
            ));

        // The language row: "System default" plus every locale the binary
        // actually embeds, as radios keyed by index (0 = system). Wrapped
        // in a glow border while Help → Language's "look here" pulse is
        // active.
        let languages = available_languages();
        let selected_index = config.language.as_deref()
            .and_then(|current| languages.iter().position(|lang| lang == current).map(|i| i + 1))
            .unwrap_or(0);

        let mut language_row = widget::Row::new()
            .push(widget::radio(
                widget::text::body(fl!("prefs-language-system")),
                0_usize,
                Some(selected_index),
                |_| SettingsMessage::LanguagePicked(None),
            ))
            .spacing(12);
        for (index, lang) in languages.iter().enumerate() {
            let value = lang.clone();
            language_row = language_row.push(widget::radio(
                widget::text::body(display_language(lang)),
                index + 1,
                Some(selected_index),
                move |_| SettingsMessage::LanguagePicked(Some(value)),
            ));
        }

        let glow_alpha = self.language_glow.map_or(0.0, |started| {
            pulse_alpha(started, GLOW_FADE_IN, GLOW_VISIBLE, GLOW_FADE_OUT)
        });
        let language_item: Element<'a, SettingsMessage> = if glow_alpha > 0.0 {
            container(settings::item(fl!("prefs-language"), language_row))
                .padding(4)
                .style(move |_theme: &cosmic::Theme| cosmic::iced::widget::container::Style {
                    border: cosmic::iced::Border {
                        width: 3.0,
                        color: cosmic::iced::Color::from_rgba(1.0, 1.0, 1.0, glow_alpha),
                        radius: 8.0.into(),
                    },
                    ..Default::default()
                })
                .into()
        } else {
            settings::item(fl!("prefs-language"), language_row).into()
        };

        let editor_section = settings::section()
            .title(fl!("prefs-section-editor"))
            .add(settings::item(
                fl!("prefs-zoom-sensitivity"),
                widget::slider(5..=30, config.zoom_sensitivity_percent, SettingsMessage::ZoomSensitivityChanged)
                    .width(Length::Fixed(INPUT_WIDTH)),
            ))
            .add(settings::item(
                fl!("prefs-preview-lines"),
                spin_button(
                    config.preview_lines.to_string(),
                    fl!("prefs-preview-lines"),
                    config.preview_lines,
                    1,
                    2,
                    20,
                    SettingsMessage::PreviewLinesChanged,
                ),
            ))
            .add(settings::item(
                fl!("prefs-confirm-nodes"),
                toggler(config.confirm_delete_nodes).on_toggle(SettingsMessage::ConfirmNodesToggled),
            ))
            .add(settings::item(
                fl!("prefs-confirm-characters"),
                toggler(config.confirm_delete_characters).on_toggle(SettingsMessage::ConfirmCharactersToggled),
            ))
            .add(settings::item(
                fl!("prefs-confirm-blocks"),
                toggler(config.confirm_delete_blocks).on_toggle(SettingsMessage::ConfirmBlocksToggled),
            ))
            .add(language_item);

        widget::scrollable(
            settings::view_column(vec![
                project_section.into(),
                development_section.into(),
                editor_section.into(),
            ]),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}

/// Every locale embedded in the binary, as plain identifiers (e.g. "en").
fn available_languages() -> Vec<String> {
    crate::i18n::localizer()
        .available_languages()
        .map(|languages| languages.into_iter().map(|lang| lang.to_string()).collect())
        .unwrap_or_default()
}

/// A human-facing name for a language identifier; falls back to the raw
/// identifier for locales without a mapping yet.
fn display_language(id: &str) -> String {
    match id {
        "en" => "English".to_string(),
        other => other.to_string(),
    }
}
