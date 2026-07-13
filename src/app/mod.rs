// SPDX-License-Identifier: AGPL-3.0-or-later

//! The application shell: window/header/nav-bar chrome, the top-level
//! `Message`/`update`/`view` loop, global keybind dispatch, and JSON
//! project save/load. The actual story canvas lives in `nav::canvas`; this
//! module mostly wires COSMIC's `Application` trait to it.
//!
//! Split into a few child modules so this file stays a thin orchestration
//! layer as the app grows:
//! - `chrome`: header/nav-bar presentation + the `MenuAction` family
//! - `project_io`: project file read/write + New/Load/Save orchestration
//! - `overlays`: popup/save-dialog/toast state glue (built on the reusable
//!   `components::overlay` helpers)

use crate::config::Config;
use cosmic::cosmic_config::{self, CosmicConfigEntry};

use crate::fl;
use cosmic::{
    prelude::*,
    Element,
    app::context_drawer,
    widget::{self, about::About, icon, menu::{self, action::MenuAction as _, key_bind::{KeyBind, Modifier}}, nav_bar},
    iced::{
        Subscription,
        Length,
        event::{self, Event},
        keyboard::{self, Key, Modifiers, key::Physical},
        alignment::{Horizontal, Vertical},
    },
};

use std::collections::HashMap;

use crate::nav::{CanvasPage, CanvasMessage, CharactersPage, CharactersMessage};

use crate::components::{
    ProjectData, SimplePopup, SaveProjectDialog, StoryNodeEditor, CharacterCardEditor, FindPanel, FindTarget,
    character_card_editor::EditorMessage,
    simple_popup::PopupMessage,
    save_project_dialog::SaveDialogMessage,
    story_node_editor::EditorMessage as NodeEditorMessage,
    unsaved_changes_dialog::UnsavedChangesMessage,
    find_panel::{FindMessage, query_input_id},
};
use crate::nav::characters::characters_scroll_id;

mod chrome;
mod project_io;
mod overlays;
mod find;

use chrome::{MenuAction, FileMenuAction, ActionMenuAction, HelpMenuAction};

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../../resources/icons/hicolor/scalable/apps/icon.svg");

/// The application model stores app-specific state used to describe its
/// interface and drive its logic.
pub struct AppModel {
    /// Application state which is managed by the COSMIC runtime.
    pub core: cosmic::Core,
    /// The about context for this app.
    pub about: About,
    /// Contains items assigned to the nav bar panel.
    pub nav: nav_bar::Model,
    /// Key bindings for the application's menu bar.
    pub key_binds: HashMap<menu::KeyBind, MenuAction>,
    /// Contains `ContextDrawer` pages
    pub drawer_page: DrawerPage,
    /// Configuration data that persists between application runs.
    config: Config,
    /// The `cosmic_config` handle backing `config`; `None` if it couldn't be
    /// opened (e.g. no writable config dir), in which case `config` changes
    /// still apply in-memory but silently aren't persisted. Kept separate
    /// from `config` because `write_entry` needs this handle, not the data.
    config_handle: Option<cosmic_config::Config>,

    /// Project metadata
    project_meta: ProjectData,

    /// Canvas Page
    pub canvas: CanvasPage,

    /// Characters Page
    pub characters: CharactersPage,

    /// Some while a `SimplePopup` notice is shown (e.g. a Load Project
    /// failure); see `overlays::apply_overlays` and `Message::Popup`.
    popup: Option<SimplePopup>,

    /// Some while the "save a brand-new project" dialog is shown (opened
    /// from `FileMenuAction::Save` when no project is open yet); see
    /// `overlays::apply_overlays` and `Message::SaveDialog`.
    save_dialog: Option<SaveProjectDialog>,

    /// `Some(shown_at)` while the "Saved" toast is visible or fading out;
    /// `overlays` derives the current fade alpha straight from how long ago
    /// this was, and `subscription` ticks `Message::ToastTick` to keep
    /// redrawing while it's set. A new save simply overwrites this with
    /// `Instant::now()`, restarting the toast from fully visible — no
    /// separate "cancel the old timer" bookkeeping needed.
    saved_toast: Option<std::time::Instant>,

    /// True while the "unsaved changes" warning is shown because
    /// `on_app_exit` found a dirty editor open when the app was about to
    /// close; see `Message::UnsavedExit` and `overlays::apply_overlays`.
    pending_exit_confirm: bool,

    /// Some while the Find panel (Ctrl+F) is open; see `find::apply_find_panel`.
    find_panel: Option<FindPanel>,
}

/// Which page (if any) `AppModel::context_drawer` should currently show.
pub enum DrawerPage {
    None,
    About,
}

/// The page to display in the application. Triggered by nav.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Page {
    Canvas,
    Characters,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    // Header menus
    HeaderFile(FileMenuAction),
    HeaderAction(ActionMenuAction),
    HeaderHelp(HelpMenuAction),
    // Nav pages
    Canvas(CanvasMessage),
    Characters(CharactersMessage),

    // Direct Actions
    CloseDrawer,
    // Opens a URL in the system's default handler; currently only reached
    // from links clicked inside the About context drawer.
    LaunchUrl(String),
    // Raw key presses, matched against `AppModel::key_binds`.
    Key(Modifiers, Key, Physical),
    // The Load Project directory picker (opened from `FileMenuAction::Load`)
    // resolved; `None` if it was cancelled or failed to open. `Some(dir)` is
    // the chosen project *directory*, not a file — see
    // `project_io::handle_load_dir_picked` for the `project.json`-on-
    // top-level check.
    LoadDirPicked(Option<std::path::PathBuf>),
    // Forwarded from the open `SimplePopup`'s own `view()`.
    Popup(PopupMessage),
    // Forwarded from the open `SaveProjectDialog`'s own `view()` (opened
    // from `FileMenuAction::Save` when there's no project open yet).
    SaveDialog(SaveDialogMessage),
    // Fired periodically by `subscription` while `AppModel::saved_toast` is
    // set, purely to force a redraw so `overlays` can recompute the toast's
    // fade alpha; the actual "hide it" decision happens in the handler,
    // once enough time has elapsed.
    ToastTick,
    // Forwarded from the app-exit `unsaved_changes_dialog` shown by
    // `on_app_exit`; see `pending_exit_confirm`.
    UnsavedExit(UnsavedChangesMessage),
    // A deliberate no-op; see `on_app_exit`.
    Noop,
    // Forwarded from the open `FindPanel`'s own `view()`.
    Find(FindMessage),
}

impl AppModel {
    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let mut window_title = fl!("app-title");

        if let Some(page) = self.nav.text(self.nav.active()) {
            window_title.push_str(" — ");
            window_title.push_str(page);
        }

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    /// Switches the active nav page to `page`, if it isn't already active.
    /// Used by the Action menu's "Add Node"/"Add Character" items, which
    /// should jump to the relevant page rather than silently adding to
    /// whichever one already happens to be open.
    fn activate_page(&mut self, page: Page) {
        if self.nav.active_data::<Page>() == Some(&page) {
            return;
        }

        let found = self.nav.iter().find(|&id| self.nav.data::<Page>(id) == Some(&page));
        if let Some(id) = found {
            self.nav.activate(id);
        }
    }

    /// Persists `self.config` to disk. Silently does nothing if the config
    /// handle couldn't be opened at startup (see `AppModel::config_handle`).
    fn save_config(&self) {
        if let Some(context) = &self.config_handle
            && let Err(err) = self.config.write_entry(context) {
            eprintln!("failed to write config: {err}");
        }
    }

    /// Whether either page's open editor has unsaved draft changes; used by
    /// `on_app_exit` to decide whether to warn before quitting.
    fn has_unsaved_editor(&self) -> bool {
        self.canvas.editor.as_ref().is_some_and(StoryNodeEditor::is_dirty)
            || self.characters.editor.as_ref().is_some_and(CharacterCardEditor::is_dirty)
    }

    /// Commits whichever editor(s) are open into their node/character —
    /// used by the app-exit unsaved-changes warning's "Save", which (unlike
    /// the same warning shown from an editor's own Close) doesn't already
    /// know which single editor is open.
    fn commit_dirty_editors(&mut self) {
        self.canvas.commit_editor();
        self.characters.commit_editor();
    }

    /// Actually closes the main window (and, since this app has only the
    /// one, exits) — used once an exit is confirmed (nothing unsaved, or
    /// the user chose Discard/Save-then-exit).
    fn exit_app(&self) -> Task<cosmic::Action<Message>> {
        match self.core.main_window_id() {
            Some(id) => cosmic::iced::window::close(id),
            None => Task::none(),
        }
    }
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = ();

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = "com.inuxiuz.dreamingwizard";

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(
        core: cosmic::Core,
        _flags: Self::Flags,
    ) -> (Self, Task<cosmic::Action<Self::Message>>) {
        let key_binds = chrome::default_key_binds();

        // Create a nav bar with page items.
        let mut nav = nav_bar::Model::default();

        nav.insert()
            .text(fl!("nav-canvas-id"))
            .data::<Page>(Page::Canvas)
            .icon(icon::from_name("insert-drawing-symbolic"))
            .activate();

        nav.insert()
            .text(fl!("nav-characters-id"))
            .data::<Page>(Page::Characters)
            .icon(icon::from_name("system-users-symbolic"));

        // Create the about widget
        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .comments(fl!("about_comments"))
            .links([(fl!("repository"), REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));

        // Open the cosmic_config handle once so we can both read the initial
        // config and write updates back to it later (`write_entry` needs
        // the handle itself, not just the deserialized `Config`).
        let config_handle = cosmic_config::Config::new(Self::APP_ID, Config::VERSION).ok();
        let config = config_handle.as_ref()
            .map(|context| match Config::get_entry(context) {
                Ok(config) => config,
                Err((_errors, config)) => config,
            })
            .unwrap_or_default();

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            canvas: CanvasPage::default(),
            characters: CharactersPage::default(),
            about,
            nav,
            key_binds,
            drawer_page: DrawerPage::None,
            project_meta: ProjectData::default(),
            config,
            config_handle,
            popup: None,
            save_dialog: None,
            saved_toast: None,
            pending_exit_confirm: false,
            find_panel: None,
        };

        app.auto_load_last_project();

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Called before closing the application (the header bar's own close
    /// button — see `chrome::build_nav_bar`'s sibling, the header itself,
    /// via COSMIC's `Action::Close`). Returning `Some` overrides closing:
    /// if either page's editor has unsaved draft changes, show the same
    /// warning a Close-with-unsaved-changes shows, and block the exit until
    /// it's resolved (`Message::UnsavedExit`).
    fn on_app_exit(&mut self) -> Option<Self::Message> {
        if self.has_unsaved_editor() {
            self.pending_exit_confirm = true;
            // The state change already happened above; this message is a
            // deliberate no-op; it only exists because `on_app_exit`'s
            // contract is that returning `Some` (of *some* message) is what
            // blocks the default close, vs. `None` letting it proceed.
            Some(Message::Noop)
        } else {
            None
        }
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        chrome::header_start(&self.key_binds)
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        chrome::header_center(&self.project_meta.name)
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    fn nav_bar(&self) -> Option<Element<'_, cosmic::Action<Self::Message>>> {
        if !self.core.nav_bar_active() {
            return None;
        }

        Some(chrome::build_nav_bar(self.nav_model()?))
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        match self.drawer_page {
            DrawerPage::None => None,
            DrawerPage::About => Some(context_drawer::about(
                    &self.about,
                    |url| Message::LaunchUrl(url.to_string()),
                    Message::CloseDrawer,
                )),
        }
    }

    /// Describes the interface based on the current state of the application model.
    ///
    /// Application events will be processed through the view. Any messages emitted by
    /// events received by widgets will be passed to the update method.
    fn view(&self) -> Element<'_, Self::Message> {
        let content: Element<_> = match self.nav.active_data::<Page>().unwrap() {
            Page::Canvas => {
                self.canvas.view().map(Message::Canvas)
            },
            Page::Characters => {
                self.characters.view().map(Message::Characters)
            },
        };

        let content: Element<_> = widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .apply(widget::container)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into();

        let content = self.apply_find_panel(content);
        self.apply_overlays(content)
    }

    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            Message::HeaderFile(action) => return self.handle_file_menu(action),
            Message::LoadDirPicked(dir) => self.handle_load_dir_picked(dir),
            Message::Popup(PopupMessage::Close) => {
                self.popup = None;
            }
            Message::SaveDialog(msg) => return self.handle_save_dialog(msg),
            Message::ToastTick => self.handle_toast_tick(),

            Message::HeaderAction(action_intent) => { match action_intent {
                ActionMenuAction::AddNode => {
                    self.activate_page(Page::Canvas);
                    let title_task = self.update_title();
                    return Task::batch([
                        title_task,
                        Task::done(cosmic::Action::App(Message::Canvas(CanvasMessage::AddNode))),
                    ]);
                }
                ActionMenuAction::AddCharacter => {
                    self.activate_page(Page::Characters);
                    let title_task = self.update_title();
                    return Task::batch([
                        title_task,
                        Task::done(cosmic::Action::App(Message::Characters(CharactersMessage::AddCharacter))),
                    ]);
                }
                // Opens the panel fresh (defaulting to whichever page is
                // active) if it isn't already open; re-focusing the query
                // field is harmless either way. Unlike Add Node/Character,
                // this never switches pages itself.
                ActionMenuAction::Find => {
                    if self.find_panel.is_none() {
                        let target = match self.nav.active_data::<Page>() {
                            Some(Page::Characters) => FindTarget::Character,
                            _ => FindTarget::Node,
                        };
                        self.find_panel = Some(FindPanel::new(target));
                    }
                    return widget::text_input::focus(query_input_id());
                }
            }}

            Message::HeaderHelp(help_intent) => { match help_intent {
                HelpMenuAction::About => {
                    self.drawer_page = DrawerPage::About;
                    self.core.window.show_context = true;
                }
            }}

            // Other messages
            //
            // Both of these are intercepted here (rather than in
            // `CanvasPage::update`) because committing the draft is only
            // half the job — a "Save" anywhere in this app now writes the
            // *entire* project to disk (see `project_io::save_project`),
            // which needs state (`config`, `project_meta`) the page doesn't
            // have. The page-local half (committing the draft, and closing
            // the editor for `UnsavedClose`) still happens via the normal
            // `self.canvas.update(...)` forwarding below.
            Message::Canvas(CanvasMessage::Editor(NodeEditorMessage::Save)) => {
                self.canvas.update(CanvasMessage::Editor(NodeEditorMessage::Save));
                return self.save_project();
            }
            Message::Canvas(CanvasMessage::UnsavedClose(UnsavedChangesMessage::Save)) => {
                self.canvas.update(CanvasMessage::UnsavedClose(UnsavedChangesMessage::Save));
                return self.save_project();
            }

            Message::Canvas(msg) => {
                if let Some(_node_id) = self.canvas.update(msg) {
                    self.core_mut().nav_bar_set_toggled(false);
                    // A node's editor just opened — the Find panel (if
                    // open) should get out of the way; see `find`'s module
                    // doc and `FindMessage`'s doc comment on why nothing
                    // else closes it.
                    self.find_panel = None;
                }
            }

            // Intercepted here (rather than in `CharactersPage::update`)
            // because only the top-level `update` can return a `Task` — the
            // system file picker (xdg-portal, matching the `xdg-portal`
            // libcosmic feature already enabled in Cargo.toml) is async.
            // The result comes back around as `EditorMessage::AvatarPicked`,
            // which *does* flow through the normal `Message::Characters(msg)`
            // arm below.
            Message::Characters(CharactersMessage::Editor(EditorMessage::ChangeAvatar)) => {
                return cosmic::task::future(async {
                    let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
                        .filter(
                            cosmic::dialog::file_chooser::FileFilter::new(&fl!("dialog-image-filter-label"))
                                .glob("*.png")
                                .glob("*.PNG")
                                .glob("*.jpg")
                                .glob("*.JPG")
                                .glob("*.jpeg")
                                .glob("*.JPEG"),
                        );

                    let path = dialog.open_file().await.ok()
                        .and_then(|response| response.url().to_file_path().ok());

                    cosmic::Action::App(Message::Characters(CharactersMessage::Editor(
                        EditorMessage::AvatarPicked(path),
                    )))
                });
            }

            // See the matching `Message::Canvas` pair above for why these
            // two are intercepted here instead of forwarded straight
            // through.
            Message::Characters(CharactersMessage::Editor(EditorMessage::Save)) => {
                self.characters.update(CharactersMessage::Editor(EditorMessage::Save));
                return self.save_project();
            }
            Message::Characters(CharactersMessage::UnsavedClose(UnsavedChangesMessage::Save)) => {
                self.characters.update(CharactersMessage::UnsavedClose(UnsavedChangesMessage::Save));
                return self.save_project();
            }

            // A Find-triggered scroll animation needs a `scrollable::
            // snap_to` `Task` every tick it's in flight, which (like the
            // Save pair above) only the top-level `update` can return.
            Message::Characters(CharactersMessage::AnimationTick) => {
                self.characters.update(CharactersMessage::AnimationTick);

                if let Some(y) = self.characters.take_pending_scroll() {
                    return cosmic::iced::widget::scrollable::snap_to(
                        characters_scroll_id(),
                        cosmic::iced::widget::scrollable::RelativeOffset { x: None, y: Some(y) },
                    );
                }
            }

            Message::Characters(msg) => {
                if let Some(_character_id) = self.characters.update(msg) {
                    self.core_mut().nav_bar_set_toggled(false);
                    // See the matching note in `Message::Canvas` above.
                    self.find_panel = None;
                }
            }

            Message::CloseDrawer => {
                self.drawer_page = DrawerPage::None;
                self.core.window.show_context = false;
            }

            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },

            Message::Key(modifiers, key, physical_key) => {
                // Ctrl+A is contextual to whichever page is active (Add
                // Node on canvas, Add Character on characters) rather than
                // a single static `MenuAction`, so it's matched directly
                // here instead of through `self.key_binds` — see
                // `ActionMenuAction`'s doc comment. Unlike the Action menu's
                // items, this doesn't switch pages: it always targets
                // whichever page the user is already on.
                let add_key_bind = KeyBind { modifiers: vec![Modifier::Ctrl], key: Key::Character("a".into()) };
                if add_key_bind.matches(modifiers, &key, Some(&physical_key)) {
                    return match self.nav.active_data::<Page>() {
                        Some(Page::Canvas) => self.update(Message::Canvas(CanvasMessage::AddNode)),
                        Some(Page::Characters) => self.update(Message::Characters(CharactersMessage::AddCharacter)),
                        None => Task::none(),
                    };
                }

                for (key_bind, action) in &self.key_binds {
                    if key_bind.matches(modifiers, &key, Some(&physical_key)) {
                        return self.update(action.message());
                    }
                }

                // Up/Down move the Find panel's highlighted result, and
                // Enter confirms it — all three matched directly here
                // rather than left to the query field's own `on_submit`.
                // `on_submit` alone isn't enough for Enter: it only fires
                // while the query `text_input` itself has focus, but
                // clicking a result row (or the target dropdown) shifts
                // focus away from it without closing the panel, and Enter
                // should still confirm the highlighted row at that point.
                // Up/Down reach here regardless either way — a focused
                // `text_input` never captures them itself.
                if let Some(highlighted) = self.find_panel.as_ref().map(|panel| panel.highlighted) {
                    let len = self.find_results().len();

                    match key {
                        Key::Named(keyboard::key::Named::ArrowDown) if len > 0 => {
                            if let Some(panel) = &mut self.find_panel {
                                panel.highlighted = (highlighted + 1) % len;
                            }
                        }
                        Key::Named(keyboard::key::Named::ArrowUp) if len > 0 => {
                            if let Some(panel) = &mut self.find_panel {
                                panel.highlighted = (highlighted + len - 1) % len;
                            }
                        }
                        Key::Named(keyboard::key::Named::Enter) => {
                            self.find_select(highlighted);
                        }
                        _ => {}
                    }
                }
            }

            Message::Find(msg) => return self.handle_find(msg),

            Message::UnsavedExit(UnsavedChangesMessage::Cancel) => {
                self.pending_exit_confirm = false;
            }

            Message::UnsavedExit(UnsavedChangesMessage::Discard) => {
                self.pending_exit_confirm = false;
                return self.exit_app();
            }

            Message::UnsavedExit(UnsavedChangesMessage::Save) => {
                self.commit_dirty_editors();
                let save_task = self.save_project();
                self.pending_exit_confirm = false;

                // Saving a brand-new project opens the name/location dialog
                // instead of writing immediately — don't force an exit out
                // from under that; the user can close again once it's done.
                return if self.save_dialog.is_none() {
                    Task::batch([save_task, self.exit_app()])
                } else {
                    save_task
                };
            }

            Message::Noop => {}
        }
        Task::none()
    }

    /// Register subscriptions for this application.
    ///
    /// Subscriptions are long-running async tasks running in the background which
    /// emit messages to the application through a channel. They can be dynamically
    /// stopped and started conditionally based on application state, or persist
    /// indefinitely.
    fn subscription(&self) -> Subscription<Self::Message> {
        let mut subscriptions = vec![
            event::listen_with(|event, status, _window| {
                if status != event::Status::Ignored {
                    return None;
                }

                match event {
                    Event::Keyboard(keyboard::Event::KeyPressed {
                        key, modifiers, physical_key, ..
                    }) => Some(Message::Key(modifiers, key, physical_key)),
                    _ => None,
                }
            }),
        ];

        // Only tick while a click-to-edit (or Find-triggered) camera
        // animation, the add-button tooltip, or a Find "found it" glow ring
        // is in flight, so we're not redrawing every frame the rest of the
        // time.
        if self.canvas.is_animating_camera() || self.canvas.is_add_button_tooltip_active() || self.canvas.is_glow_active() {
            subscriptions.push(
                cosmic::iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::Canvas(CanvasMessage::AnimationTick)),
            );
        }

        // Only tick while the characters page's add-button tooltip, a
        // Find-triggered scroll animation, or its "found it" glow ring is
        // in flight, same reasoning as above.
        if self.characters.is_add_button_tooltip_active()
            || self.characters.is_animating_scroll()
            || self.characters.is_glow_active()
        {
            subscriptions.push(
                cosmic::iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::Characters(CharactersMessage::AnimationTick)),
            );
        }

        // Only tick while the "Saved" toast is visible/fading, so we're not
        // redrawing every frame the rest of the time.
        if self.toast_is_active() {
            subscriptions.push(
                cosmic::iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::ToastTick),
            );
        }

        Subscription::batch(subscriptions)
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}
