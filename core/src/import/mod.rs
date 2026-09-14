//! Bringing another tool's notes into the vault. A mapping is pure and
//! tested on a fixture graph; only its `run` touches the disk.

mod report;

pub use report::{Report, Unmapped};
