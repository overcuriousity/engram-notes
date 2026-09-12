//! engram-notes core: parser, vault, index, config. No window, no Tauri.

pub mod error;
pub mod index;
pub mod parse;
pub mod rename;
pub mod vault;

pub use error::{Error, Result};
