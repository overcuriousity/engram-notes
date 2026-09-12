#[derive(serde::Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl From<engram_core::Error> for CommandError {
    fn from(e: engram_core::Error) -> Self {
        use engram_core::Error;
        let code = match &e {
            Error::Io { .. } => "io",
            Error::Parse { .. } => "parse",
            Error::Index(_) => "index",
            Error::Config(_) => "config",
            Error::NotFound(_) => "not_found",
            Error::Exists(_) => "exists",
            Error::Base(_) => "base",
        };
        CommandError {
            code,
            message: e.to_string(),
        }
    }
}

impl CommandError {
    pub fn closed() -> Self {
        CommandError {
            code: "no_vault",
            message: "no vault is open".into(),
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;
