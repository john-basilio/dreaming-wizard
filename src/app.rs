// SPDX-License-Identifier: AGPL-3.0-or-later

//! The application shell: window/header/nav-bar chrome, the top-level
//! `Message`/`update`/`view` loop, global keybind dispatch, and JSON
//! project save/load. The actual story canvas lives in `nav::canvas`;
//! this file mostly wires COSMIC's `Application` trait to it.

use crate::config::Config;
use cosmic::cosmic_config::{self, CosmicConfigEntry};

use crate::fl;
use cosmic::{
    prelude::*,
    Element,
    app::context_drawer,
    widget::{
        self, 
        about::About, 
        icon, 
        menu::{self,key_bind::{KeyBind,Modifier},action::MenuAction as _,ItemWidth},
        nav_bar,
        text,
    },
    iced::{
        Subscription,
        Length,
        Vector,
        event::{self, Event},
        keyboard::{self, Key, Modifiers, key::Physical},
        alignment::{Horizontal, Vertical},
        advanced::text::{Wrapping, Ellipsize, EllipsizeHeightLimit},
    },

};

use std::{
    collections::HashMap,
    io::BufWriter,
    fs::File,
};

use crate::nav::{
    CanvasPage, CanvasMessage,
    CharactersPage, CharactersMessage,
};

use crate::components::{
    ProjectFile, ProjectData,
    character_card_editor::EditorMessage,
};



const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_ICON: &[u8] = include_bytes!("../resources/icons/hicolor/scalable/apps/icon.svg");


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
    // The Load file picker (opened from `FileMenuAction::Load`) resolved;
    // `None` if it was cancelled or failed to open.
    LoadPathPicked(Option<std::path::PathBuf>),
    // The Save file picker (opened from `FileMenuAction::Save`) resolved;
    // `None` if it was cancelled or failed to open.
    SavePathPicked(Option<std::path::PathBuf>),
}

/// For future purposes, we're expecting interactions to be often with 
/// header menus, so each menu are their own enum.
// TODO: Complete the implementation of the header menu
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// File menu
pub enum FileMenuAction {
    Load,
    Save,
}


#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Canvas menu
pub enum CanvasMenuAction {
    AddNode, 
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Help menu
pub enum HelpMenuAction {
    About,
} 



#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Header Start top-level menus
pub enum MenuAction {
    File(FileMenuAction),
    Canvas(CanvasMenuAction),
    Help(HelpMenuAction),
}

impl menu::action::MenuAction for MenuAction {
    type Message = Message;

    fn message(&self) -> Self::Message {
        match self {
            MenuAction::Canvas(intent) => match intent {
                CanvasMenuAction::AddNode => Message::HeaderCanvas(*intent)
            },
            MenuAction::Help(intent) => match intent {
                HelpMenuAction::About => Message::HeaderHelp(*intent)
            }
            MenuAction::File(intent) => match intent {
                FileMenuAction::Load | FileMenuAction::Save => Message::HeaderFile(*intent),
            }
        }
    }
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

    /// Reads and applies a project file from `path` onto canvas/metadata
    /// state. Returns whether it succeeded (missing file, unreadable, or
    /// invalid JSON all just report `false` rather than panicking) — on
    /// failure this leaves existing state untouched, so callers decide what
    /// "new session" fallback means for their context.
    fn try_load_project(&mut self, path: &std::path::Path) -> bool {
        let Some(project) = std::fs::read_to_string(path).ok()
            .and_then(|json| serde_json::from_str::<ProjectFile>(&json).ok())
        else {
            return false;
        };

        self.canvas.nodes = project.canvas.nodes;
        self.canvas.geo_cache.clear();
        self.project_meta = project.metadata;
        self.canvas.offset = Vector::new(
            project.canvas.last_camera.0,
            project.canvas.last_camera.1,
        );
        self.characters.characters = project.characters.characters;
        self.characters.editor = None;

        true
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
        // Declare Keybinds here.
        let mut key_binds = HashMap::new();

        key_binds.insert(
            KeyBind {
                modifiers: vec![Modifier::Ctrl],
                key: Key::Character("s".into()),
            },
            MenuAction::File(FileMenuAction::Save),
        );

        key_binds.insert(
            KeyBind {
                modifiers: vec![Modifier::Ctrl],
                key: Key::Character("o".into()),  // "o" for open/load
            },
            MenuAction::File(FileMenuAction::Load),
        );

        key_binds.insert(
            KeyBind {
                modifiers: vec![Modifier::Ctrl],
                key: Key::Character("a".into()),
            },
            MenuAction::Canvas(CanvasMenuAction::AddNode),
        );
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
                Err((_errors, config)) => {
                    // for why in errors {
                    //     tracing::error!(%why, "error loading app config");
                    // }

                    config
                }
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
        };

        // Auto-load the last remembered project, if any. Only attempt this
        // when a path was actually remembered — an absent path means "no
        // prior session," not "try the fallback location," so a fresh
        // install doesn't pick up an unrelated leftover file there.
        if let Some(path) = app.config.last_project_path.clone() {
            let path = std::path::PathBuf::from(path);

            if app.try_load_project(&path) {
                println!("Loaded from {}", path.display());
            } else {
                // `app.canvas`/`app.project_meta`/`app.characters` are
                // already fresh defaults from the struct literal above, so
                // there's nothing to reset here beyond forgetting the bad
                // path.
                app.config.last_project_path = None;
                app.save_config();

                eprintln!(
                    "Could not load remembered project from {}; starting a new session.",
                    path.display()
                );
            }
        }

        // Create a startup command that sets the window title.
        let command = app.update_title();

        (app, command)
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let file_menu = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("hs_file")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(
                        fl!("item_load"), 
                        None, 
                        MenuAction::File(FileMenuAction::Load)),
                    menu::Item::Button(
                        fl!("item_save"),
                        None,
                        MenuAction::File(FileMenuAction::Save))
                ]
            ),
        )]);

        let canvas_menu = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("hs_canvas")).apply(Element::from),
            menu::items(
                &self.key_binds, 
                vec![
                    menu::Item::Button(
                        fl!("item_add_node"), 
                        None,
                        MenuAction::Canvas(CanvasMenuAction::AddNode)
                    )
                ]
            )
        )])
        // "New Story Node" is the longest label in any of our menus, so it
        // keeps the widest item width to fit alongside its "Ctrl + A" hint.
        .item_width(ItemWidth::Uniform(200));

        let help_menu = menu::bar(vec![menu::Tree::with_children(
            menu::root(fl!("hs_help")).apply(Element::from),
            menu::items(
                &self.key_binds,
                vec![
                    menu::Item::Button(
                        fl!("item_about"), 
                        None, 
                        MenuAction::Help(HelpMenuAction::About))],
            ),
        )])
        // "About" has no keybind hint at all, so it only needs to fit the label.
        .item_width(ItemWidth::Uniform(160));

        vec![file_menu.into(), canvas_menu.into(), help_menu.into()]
    }

    fn header_center(&self) -> Vec<Element<'_, Self::Message>> {
        // Update `Header Title` with file's project name or the fallback title.

        let display_name = if self.project_meta.name.is_empty() {
            fl!("project-title-fallback")
        } else {
            self.project_meta.name.clone()
        };

        let title = text::heading(format!(
            "{} {}", fl!("project_title_prefix"), display_name
        ))
            .width(Length::Fill)
            .center()
            .wrapping(Wrapping::None)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)));

        let title = widget::container(title)
            .width(Length::Fill)
            .padding([0, 64]);

        vec![title.into()]
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
    }

    /// Builds the nav bar, sized to hug its widest item instead of always
    /// taking the default fixed width.
    fn nav_bar(&self) -> Option<Element<'_, cosmic::Action<Self::Message>>> {
        if !self.core.nav_bar_active() {
            return None;
        }

        let nav_model = self.nav_model()?;

        let theme = cosmic::theme::active();
        let space_xxs = theme.cosmic().space_xxs();
        let space_s = theme.cosmic().space_s();
        let space_l = theme.cosmic().space_l();

        // Ideally this would match the width of the nav's shortest item, but
        // that requires measuring shaped text against a live renderer, which
        // isn't available while just building the widget tree. `space_l`
        // gives comfortable, deliberate breathing room instead.
        let nav = cosmic::widget::segmented_button::vertical(nav_model)
            .on_activate(|id| cosmic::Action::Cosmic(cosmic::app::Action::NavBar(id)))
            .button_height(32)
            .button_padding([space_l, space_xxs, space_l, space_xxs])
            .button_spacing(space_xxs)
            .spacing(space_xxs)
            .style(cosmic::theme::SegmentedButton::NavBar)
            .width(Length::Shrink)
            .apply(widget::container)
            .padding(space_s)
            .apply(widget::scrollable)
            .class(cosmic::style::iced::Scrollable::Minimal)
            .height(Length::Fill)
            .apply(widget::container)
            .width(Length::Shrink)
            .height(Length::Fill)
            .class(cosmic::theme::Container::custom(nav_bar::nav_bar_style));

        Some(Element::from(nav))
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

        widget::container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .apply(widget::container)
            .width(Length::Fill)
            .align_x(Horizontal::Center)
            .align_y(Vertical::Center)
            .into()
    }


    /// Handles messages emitted by the application and its widgets.
    ///
    /// Tasks may be returned for asynchronous execution of code in the background
    /// on the application's async runtime.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            // Header Start Messages
            Message::HeaderFile(file_intent) => { match file_intent {
                // Both Load and Save now always prompt via the system file
                // picker (xdg-portal) rather than silently reusing the
                // remembered path — the actual load/save work happens once
                // the picker resolves, in the `LoadPathPicked`/
                // `SavePathPicked` arms below.
                FileMenuAction::Load => {
                    return cosmic::task::future(async {
                        let dialog = cosmic::dialog::file_chooser::open::Dialog::new()
                            .title(fl!("dialog-load-title"))
                            // ashpd's open-file portal request doesn't expose a
                            // way to set a starting directory yet (see the
                            // `directory` field's own doc comment in
                            // `cosmic::dialog::file_chooser::open::Dialog`), so
                            // unlike the Save dialog below, there's no
                            // `.directory(...)` call to make here.
                            .filter(
                                cosmic::dialog::file_chooser::FileFilter::new(&fl!("dialog-json-filter-label"))
                                    .glob("*.json")
                                    .glob("*.JSON"),
                            );

                        let path = dialog.open_file().await.ok()
                            .and_then(|response| response.url().to_file_path().ok());

                        cosmic::Action::App(Message::LoadPathPicked(path))
                    });
                },

                FileMenuAction::Save => {
                    let now = jiff::Timestamp::now().to_string();

                    if self.project_meta.created_at.is_empty() {
                        self.project_meta.created_at = now.clone();
                    }
                    self.project_meta.updated_at = now;
                    self.project_meta.app_version = env!("CARGO_PKG_VERSION").to_string();

                    // No UI yet to set these, so default them until project settings exist.
                    if self.project_meta.name.is_empty() {
                        self.project_meta.name = fl!("project-name-fallback");
                    }
                    if self.project_meta.author.is_empty() {
                        self.project_meta.author = fl!("project-author-fallback");
                    }

                    // `config.project_dir` is the single hardcoded source for
                    // this (see its doc comment) — nothing else needs its own
                    // copy of the path.
                    let starting_dir = std::path::PathBuf::from(&self.config.project_dir);

                    return cosmic::task::future(async move {
                        let dialog = cosmic::dialog::file_chooser::save::Dialog::new()
                            .title(fl!("dialog-save-title"))
                            .file_name("project.json".to_string())
                            .directory(starting_dir)
                            .filter(
                                cosmic::dialog::file_chooser::FileFilter::new(&fl!("dialog-json-filter-label"))
                                    .glob("*.json")
                                    .glob("*.JSON"),
                            );

                        let path = match dialog.save_file().await {
                            Ok(response) => response.url().and_then(|url| url.to_file_path().ok()),
                            Err(_) => None,
                        };

                        cosmic::Action::App(Message::SavePathPicked(path))
                    });
                },


            }}

            Message::LoadPathPicked(None) => {
                // Dialog was cancelled or failed to open; nothing to do.
            }
            Message::LoadPathPicked(Some(path)) => {
                // Any failure here (unreadable, corrupt JSON) falls back to
                // a fresh session instead of the `.expect()` panics this
                // used to have — and forgets the bad remembered path so we
                // don't keep retrying it.
                if self.try_load_project(&path) {
                    self.config.last_project_path = Some(path.display().to_string());
                    self.save_config();

                    println!("Loaded from {}", path.display());
                } else {
                    self.canvas = CanvasPage::default();
                    self.project_meta = ProjectData::default();
                    self.characters = CharactersPage::default();

                    self.config.last_project_path = None;
                    self.save_config();

                    eprintln!(
                        "Could not load project from {}; starting a new session.",
                        path.display()
                    );
                }
            }

            Message::SavePathPicked(None) => {
                // Dialog was cancelled or failed to open; nothing to do.
            }
            Message::SavePathPicked(Some(path)) => {
                // Built fresh here (rather than passed through the message)
                // so the write reflects whatever the latest state is by the
                // time the picker resolves, not a snapshot from before the
                // (async) dialog was even shown.
                let project = ProjectFile::new(
                    self.canvas.nodes.clone(),
                    (self.canvas.offset.x, self.canvas.offset.y),
                    self.project_meta.clone(),
                    self.characters.characters.clone(),
                );

                match File::create(&path) {
                    Ok(file) => {
                        let writer = BufWriter::new(file);

                        match serde_json::to_writer_pretty(writer, &project) {
                            Ok(()) => {
                                self.config.last_project_path = Some(path.display().to_string());
                                self.save_config();

                                println!("Saved to {}", path.display());
                            }
                            Err(err) => {
                                eprintln!("failed to serialize project to {}: {err}", path.display());
                            }
                        }
                    }
                    Err(err) => {
                        eprintln!("failed to create savefile at {}: {err}", path.display());
                    }
                }
            }

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
                self.core.window.show_context = false;            }

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

        Subscription::batch(subscriptions)
    }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}
