//! engram-notes core: parser, vault, index, config. No window, no Tauri.

pub mod config;
pub mod error;
pub mod frontmatter;
pub mod index;
pub mod parse;
pub mod rename;
pub mod vault;
pub mod watch;

pub use error::{Error, Result};
