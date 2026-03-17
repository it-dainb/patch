use std::path::PathBuf;

/// Every error patch can produce. Displayed as user-facing messages with suggestions.
#[derive(Debug)]
pub enum PatchError {
    AlreadyReported {
        exit_code: i32,
    },
    Clap {
        message: String,
        exit_code: i32,
    },
    NotFound {
        path: PathBuf,
        suggestion: Option<String>,
    },
    PermissionDenied {
        path: PathBuf,
    },
    InvalidQuery {
        query: String,
        reason: String,
    },
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        reason: String,
    },
}

impl std::fmt::Display for PatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AlreadyReported { .. } => Ok(()),
            Self::Clap { message, .. } => write!(f, "{message}"),
            Self::NotFound { path, suggestion } => {
                write!(f, "not found: {}", path.display())?;
                if let Some(s) = suggestion {
                    write!(f, " — did you mean: {s}")?;
                }
                Ok(())
            }
            Self::PermissionDenied { path } => {
                write!(f, "{} [permission denied]", path.display())
            }
            Self::InvalidQuery { query, reason } => {
                write!(f, "invalid query \"{query}\": {reason}")
            }
            Self::IoError { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::ParseError { path, reason } => {
                write!(f, "parse error in {}: {reason}", path.display())
            }
        }
    }
}

impl std::error::Error for PatchError {}

impl PatchError {
    /// Exit code matching the spec.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::AlreadyReported { exit_code } => *exit_code,
            Self::Clap { exit_code, .. } => *exit_code,
            Self::NotFound { .. } | Self::IoError { .. } => 2,
            Self::InvalidQuery { .. } | Self::ParseError { .. } => 3,
            Self::PermissionDenied { .. } => 4,
        }
    }
}
