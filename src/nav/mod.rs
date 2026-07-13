//! Top-level nav-bar pages (see `app::Page`). Each page gets its own
//! submodule with a `*Page` model and `*Message` enum, following the same
//! shape as `canvas::{CanvasPage, CanvasMessage}`.

pub mod canvas;
pub use canvas::{CanvasPage, CanvasMessage};

pub mod characters;
pub use characters::{CharactersPage, CharactersMessage};