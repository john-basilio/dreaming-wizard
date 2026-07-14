//! The "Settings" nav page. Blank for now — a placeholder until there's
//! actually something to configure; follows the same `*Page`/`*Message`
//! shape as `canvas`/`characters` (see `nav`'s module doc) so adding real
//! settings later is a drop-in extension rather than a rework.

use cosmic::Element;
use cosmic::widget::text;

use crate::fl;

/// Page model for the Settings page.
#[derive(Default)]
pub struct SettingsPage {}

/// Messages emitted by the Settings page. Empty for now — there's nothing
/// on the page yet to emit one.
#[derive(Debug, Clone)]
pub enum SettingsMessage {}

impl SettingsPage {
    pub fn view(&self) -> Element<'_, SettingsMessage> {
        text::body(fl!("settings-placeholder")).into()
    }

    pub fn update(&mut self, message: SettingsMessage) {
        match message {}
    }
}
