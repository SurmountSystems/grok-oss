#![deny(clippy::indexing_slicing)]

#![cfg_attr(feature = "test-support", allow(dead_code, unused_imports))]
pub mod appearance;
pub mod clipboard;
pub mod glyphs;
pub mod host;
pub mod input;
pub mod link_opener;
mod location_path;
pub mod modal_window_state;
pub mod prompt_images;
pub mod render;
pub mod search;
pub mod syntax;
pub mod terminal;
pub mod theme;
pub mod tui_screenshot;
pub mod util;
