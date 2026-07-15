// SPDX-License-Identifier: AGPL-3.0-or-later

//! Entry point: wires up localization, then hands off to the COSMIC/iced
//! runtime with `app::AppModel` as the application. Most of the actual
//! logic lives in `app` (window/menu/state) and `nav::canvas` (the story
//! canvas + node editor).

mod app;
mod config;
mod i18n;
mod nav;
mod components;

fn main() -> cosmic::iced::Result {
    // Get the system's preferred languages.
    let mut requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();

    // The Preferences page's language override wins over the system
    // locale when set — read it straight from cosmic_config, since the
    // app model (and its own config load) doesn't exist yet.
    {
        use cosmic::cosmic_config::CosmicConfigEntry;

        if let Ok(handle) = cosmic::cosmic_config::Config::new(
            <app::AppModel as cosmic::Application>::APP_ID,
            config::Config::VERSION,
        ) {
            let config = match config::Config::get_entry(&handle) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            };
            if let Some(language) = config.language.as_deref().and_then(|s| s.parse().ok()) {
                requested_languages = vec![language];
            }
        }
    }

    // Enable localizations to be applied.
    i18n::init(&requested_languages);

    // Settings for configuring the application window and iced runtime.
    let settings = cosmic::app::Settings::default().size_limits(
        cosmic::iced::Limits::NONE
            .min_width(360.0)
            .min_height(180.0),
    );

    // Starts the application's event loop with `()` as the application's flags.
    cosmic::app::run::<app::AppModel>(settings, ())
}
