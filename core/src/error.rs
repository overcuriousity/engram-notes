use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("parse error in {path}: {message}")]
    Parse { path: String, message: String },
    #[error("index error: {0}")]
    Index(String),
    #[error("config error: {0}")]
    Config(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("already exists: {0}")]
    Exists(String),
    #[error("base error: {0}")]
    Base(String),
    #[error("embedding error: {0}")]
    Embed(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }
}
