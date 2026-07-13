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
    widget::{self, about::About, icon, menu::{self, action::MenuAction as _}, nav_bar},
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
    ProjectData, SimplePopup, SaveProjectDialog,
    character_card_editor::EditorMessage,
    simple_popup::PopupMessage,
    save_project_dialog::SaveDialogMessage,
};

mod chrome;
mod project_io;
mod overlays;

use chrome::{MenuAction, FileMenuAction, CanvasMenuAction, HelpMenuAction};

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
}

/// Which page (if any) `AppModel::context_drawer` should currently show.
pub enum DrawerPage {
    None,
    About,
}

/// The page to display in the application. Triggered by nav.
pub enum Page {
    Canvas,
    Characters,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    // Header menus
    HeaderFile(FileMenuAction),
    HeaderCanvas(CanvasMenuAction),
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

    /// Persists `self.config` to disk. Silently does nothing if the config
    /// handle couldn't be opened at startup (see `AppModel::config_handle`).
    fn save_config(&self) {
        if let Some(context) = &self.config_handle
            && let Err(err) = self.config.write_entry(context) {
            eprintln!("failed to write config: {err}");
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
        };

        app.auto_load_last_project();

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
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

            Message::HeaderCanvas(canvas_intent) => { match canvas_intent {
                CanvasMenuAction::AddNode => {
                    return Task::done(cosmic::Action::App(
                        Message::Canvas(CanvasMessage::AddNode)
                    ))
                }
            }}

            Message::HeaderHelp(help_intent) => { match help_intent {
                HelpMenuAction::About => {
                    self.drawer_page = DrawerPage::About;
                    self.core.window.show_context = true;
                }
            }}

            // Other messages
            Message::Canvas(msg) => {
                if let Some(_node_id) = self.canvas.update(msg) {
                    self.core_mut().nav_bar_set_toggled(false);
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

            Message::Characters(msg) => {
                if let Some(_character_id) = self.characters.update(msg) {
                    self.core_mut().nav_bar_set_toggled(false);
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
                for (key_bind, action) in &self.key_binds {
                    if key_bind.matches(modifiers, &key, Some(&physical_key)) {
                        return self.update(action.message());
                    }
                }
            }
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

        // Only tick while a click-to-edit camera animation is in flight, so
        // we're not redrawing every frame the rest of the time.
        if self.canvas.is_animating_camera() {
            subscriptions.push(
                cosmic::iced::time::every(std::time::Duration::from_millis(16))
                    .map(|_| Message::Canvas(CanvasMessage::AnimationTick)),
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
