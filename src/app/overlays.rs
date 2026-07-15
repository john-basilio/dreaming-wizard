//! Ties `AppModel`'s popup/save-dialog/toast state to the reusable
//! `components::overlay` building blocks.

use std::time::{Duration, Instant};

use cosmic::Element;

use crate::components::overlay::{fade_alpha, toast_box, with_overlay, with_toast};
use crate::components::unsaved_changes_dialog::unsaved_changes_dialog;
use crate::fl;

use super::{AppModel, Message};

/// How long the "Saved" toast stays fully visible before it starts fading.
const TOAST_VISIBLE: Duration = Duration::from_secs(2);
/// How long the fade-out itself takes once `TOAST_VISIBLE` has elapsed.
const TOAST_FADE: Duration = Duration::from_secs(1);

/// Alpha of the dimming shade behind the popup/save-dialog overlays.
const SHADE_ALPHA: f32 = 0.3;

impl AppModel {
    /// Shows the "Saved" toast, starting its visible-then-fade timeline over
    /// from now. Called after every successful `write_project_file`.
    pub(super) fn show_saved_toast(&mut self) {
        self.saved_toast = Some(Instant::now());
    }

    /// Handles `Message::ToastTick`.
    pub(super) fn handle_toast_tick(&mut self) {
        if let Some(shown_at) = self.saved_toast
            && shown_at.elapsed() >= TOAST_VISIBLE + TOAST_FADE
        {
            self.saved_toast = None;
        }
    }

    /// Whether `subscription` should keep ticking `Message::ToastTick` to
    /// drive the toast's fade animation.
    pub(super) fn toast_is_active(&self) -> bool {
        self.saved_toast.is_some()
    }

    /// Layers the popup/save-dialog/toast overlays (whichever are active)
    /// over `content`, in that order.
    pub(super) fn apply_overlays<'a>(&'a self, content: Element<'a, Message>) -> Element<'a, Message> {
        let content = match &self.popup {
            Some(popup) => with_overlay(content, popup.view().map(Message::Popup), SHADE_ALPHA),
            None => content,
        };

        let content = match &self.new_project_dialog {
            Some(dialog) => with_overlay(content, dialog.view().map(Message::NewProject), SHADE_ALPHA),
            None => content,
        };

        // Highest priority of the modal overlays — shown on top of
        // whichever page/editor was open when the app tried to exit.
        let content = if self.pending_exit_confirm {
            with_overlay(content, unsaved_changes_dialog().map(Message::UnsavedExit), SHADE_ALPHA)
        } else {
            content
        };

        let Some(shown_at) = self.saved_toast else {
            return content;
        };

        // `Message::ToastTick` (see `subscription`) is what keeps driving
        // redraws so this alpha actually animates instead of jumping
        // straight to invisible.
        let alpha = fade_alpha(shown_at, TOAST_VISIBLE, TOAST_FADE);
        with_toast(content, toast_box(fl!("toast-saved"), alpha))
    }
}
