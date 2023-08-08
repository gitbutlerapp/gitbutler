use core::fmt;

use serde::{ser::SerializeMap, Serialize};

use crate::{app, keys, project_repository, virtual_branches};

#[derive(Debug)]
pub enum Code {
    Unknown,
    FetchFailed,
    PushFailed,
    Conflicting,
    ProjectCreateFailed,
    GitAutenticationFailed,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Code::Unknown => write!(f, "errors.unknown"),
            Code::PushFailed => write!(f, "errors.push"),
            Code::FetchFailed => write!(f, "errors.fetch"),
            Code::Conflicting => write!(f, "errors.conflict"),
            Code::GitAutenticationFailed => write!(f, "errors.git.authentication"),
            Code::ProjectCreateFailed => write!(f, "errors.projects.create"),
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

impl From<virtual_branches::controller::Error> for Error {
    fn from(e: virtual_branches::controller::Error) -> Self {
        match e {
            virtual_branches::controller::Error::PushError(
                project_repository::Error::AuthError,
            ) => Error::UserError {
                code: Code::GitAutenticationFailed,
                message: "Git authentication failed. Add your GitButler key to your provider and try again."
                    .to_string(),
            },
            virtual_branches::controller::Error::PushError(e) => Error::UserError {
                code: Code::PushFailed,
                message: e.to_string(),
            },
            virtual_branches::controller::Error::Conflicting => Error::UserError {
                code: Code::Conflicting,
                message: "Project is in conflicting state. Resolve all conflicts and try again."
                    .to_string(),
            },
            virtual_branches::controller::Error::LockError(_) => Error::Unknown,
            virtual_branches::controller::Error::Other(e) => Error::from(e),
        }
    }
}

impl From<app::Error> for Error {
    fn from(e: app::Error) -> Self {
        match e {
            app::Error::FetchError(project_repository::Error::AuthError) => Error::UserError {
                code: Code::GitAutenticationFailed,
                message: "Git authentication failed. Add your GitButler key to your provider and try again."
                    .to_string(),
            },
            app::Error::FetchError(e) => Error::UserError {
                code: Code::FetchFailed,
                message: e.to_string(),
            },
            app::Error::CreateProjectError(message) => Error::UserError {
                code: Code::ProjectCreateFailed,
                message,
            },
            app::Error::Other(e) => Error::from(e),
        }
    }
}

impl From<keys::Error> for Error {
    fn from(value: keys::Error) -> Self {
        anyhow::anyhow!(format!("keys: {0}", value.to_string())).into()
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        sentry_anyhow::capture_anyhow(&e);
        log::error!("{:#}", e);
        Error::Unknown
    }
}
