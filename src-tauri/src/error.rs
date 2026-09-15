#[derive(Debug, serde::Serialize)]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
    /// Where a stopped import left its report, for the UI to open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<String>,
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
            Error::Embed(_) => "embed",
            Error::Import { .. } => "import",
        };
        let report = match &e {
            Error::Import { report, .. } => Some(report.clone()),
            _ => None,
        };
        CommandError {
            code,
            message: e.to_string(),
            report,
        }
    }
}

impl CommandError {
    pub fn closed() -> Self {
        CommandError {
            code: "no_vault",
            message: "no vault is open".into(),
            report: None,
        }
    }
}

pub type CmdResult<T> = Result<T, CommandError>;
