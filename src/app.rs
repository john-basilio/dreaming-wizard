// SPDX-License-Identifier: AGPL-3.0-or-later

// Not used for now, removed related stuff that can be added later
// may copy from libcosmic template later
// use crate::config::Config;
// use cosmic::cosmic_config::{self, CosmicConfigEntry};

use crate::nav::{CanvasPage, CanvasMessage};
use crate::fl;
use cosmic::{
    prelude::*,
    Element,
    app::context_drawer,
    widget::{
        self, 
        about::About, 
        icon, 
        menu::{self,key_bind::{KeyBind,Modifier}}, 
        nav_bar
    },
    iced::{
        Length,
        keyboard::Key,
        alignment::{Horizontal, Vertical}},

};
use std::{
    collections::HashMap,
    io::BufWriter,
    fs::File,
};
use crate::components::ProjectData;



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

    /// Canvas Page
    pub canvas: CanvasPage,

}

pub enum DrawerPage {
    None,
    About,
}

/// The page to display in the application. Triggered by nav.
pub enum Page {
    Canvas,
    //Characters,
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
    // Characters,
    
    // Direct Actions
    CloseDrawer,
    // TODO: Unknown yet
    LaunchUrl(String),
}

/// For future purposes, we're expecting interactions to be often with 
/// header menus, so each menu are their own enum.
// TODO: Complete the implementation of the header menu
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
// Canvas menu
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
            .icon(icon::from_name("applications-science-symbolic"))
            .activate();

        // Create the about widget
        let about = About::default()
            .name(fl!("app-title"))
            .icon(widget::icon::from_svg_bytes(APP_ICON))
            .version(env!("CARGO_PKG_VERSION"))
            .comments(fl!("about_comments"))
            .links([(fl!("repository"), REPOSITORY)])
            .license(env!("CARGO_PKG_LICENSE"));
            

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            core,
            canvas: CanvasPage::default(),
            about,
            nav,
            key_binds: HashMap::new(),
            drawer_page: DrawerPage::None,
        };

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
        )]);

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
        )]);

        vec![file_menu.into(), canvas_menu.into(), help_menu.into()]
    }

    /// Enables the COSMIC application to create a nav bar with this model.
    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav)
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
                FileMenuAction::Load => {
                    let path = dirs::download_dir().unwrap().join("dw.json");
                    let json =  std::fs::read_to_string(&path).expect("Failed to read file.");
                    // Use ProjecData as deserialization shape.
                    let project: ProjectData = serde_json::from_str(&json).expect("Failed to deserialize.");
                    
                    self.canvas.nodes = project.canvas.nodes;
                    self.canvas.geo_cache.clear();

                    println!("Loaded from {}", path.display());
                },

                FileMenuAction::Save => {
                    let project = ProjectData::new(
                        self.canvas.nodes.clone(),
                    );

                    let path = dirs::download_dir().unwrap().join("dw.json");
                    let file = File::create(&path).expect("Failed to create savefile.");
                    let writer =  BufWriter::new(file);

                    serde_json::to_writer_pretty(writer, &project).expect("failed to serialize");
                    println!("Saved to {}", path.display());
                },

                
            }}
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

            Message::CloseDrawer => {
                self.drawer_page = DrawerPage::None;
                self.core.window.show_context = false;            }

            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    eprintln!("failed to open {url:?}: {err}");
                }
            },
        }
        Task::none()
    }

    // TODO: Keybinds subscription for keyboard shortcuts
    // fn subscription(&self) -> cosmic::iced::Subscription<Self::Message> {
        
    // }

    /// Called when a nav item is selected.
    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Self::Message>> {
        // Activate the page in the model.
        self.nav.activate(id);

        self.update_title()
    }
}
