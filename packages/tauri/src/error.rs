use core::fmt;

use serde::{ser::SerializeMap, Serialize};

#[derive(Debug)]
pub enum Code {
    Unknown,
    Validation,
    Projects,
    Branches,
    ProjectGitAuth,
    ProjectGitRemote,
    ProjectConflict,
    ProjectHead,
    Menu,
    Hook,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Code::Menu => write!(f, "errors.menu"),
            Code::Unknown => write!(f, "errors.unknown"),
            Code::Validation => write!(f, "errors.validation"),
            Code::Projects => write!(f, "errors.projects"),
            Code::Branches => write!(f, "errors.branches"),
            Code::ProjectGitAuth => write!(f, "errors.projects.git.auth"),
            Code::ProjectGitRemote => write!(f, "errors.projects.git.remote"),
            Code::ProjectHead => write!(f, "errors.projects.head"),
            Code::ProjectConflict => write!(f, "errors.projects.conflict"),
            Code::Hook => write!(f, "errors.hook"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("[{code}]: {message}")]
    UserError { code: Code, message: String },
    #[error("[errors.unknown]: Something went wrong")]
    Unknown,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let (code, message) = match self {
            Error::UserError { code, message } => (code.to_string(), message.to_string()),
            Error::Unknown => (
                Code::Unknown.to_string(),
                "Something went wrong".to_string(),
            ),
        };

        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("code", &code)?;
        map.serialize_entry("message", &message)?;
        map.end()
    }
}

impl From<anyhow::Error> for Error {
    fn from(error: anyhow::Error) -> Self {
        tracing::error!(?error);
        Error::Unknown
    }
}
