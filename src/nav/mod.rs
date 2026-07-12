//! Top-level nav-bar pages (see `app::Page`). Each page gets its own
//! submodule with a `*Page` model and `*Message` enum, following the same
//! shape as `canvas::{CanvasPage, CanvasMessage}`.
//!
//! `characters.rs` exists alongside this but isn't declared as a `mod` here
//! yet — it's an unwired stub for a future "Characters" nav page (see the
//! commented-out `Page::Characters` / `nav-characters-id` scaffolding in
//! `app.rs` and the i18n file), not dead code left over by accident.

pub mod canvas;
pub use canvas::{CanvasPage, CanvasMessage};