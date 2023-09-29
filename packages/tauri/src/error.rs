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
    InvalidHead,
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Code::Unknown => write!(f, "errors.unknown"),
            Code::PushFailed => write!(f, "errors.push"),
            Code::FetchFailed => write!(f, "errors.fetch"),
            Code::Conflicting => write!(f, "errors.conflict"),
            Code::GitAutenticationFailed => write!(f, "errors.git.authentication"),
            Code::InvalidHead => write!(f, "errors.git.head"),
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
            virtual_branches::controller::Error::PushError(project_repository::Error::NoUrl) => Error::UserError {
                code: Code::PushFailed,
                message: "Project URL not found. Please check your project's git config and try again."
                    .to_string(),
            },
            virtual_branches::controller::Error::PushError(project_repository::Error::NonSSHUrl(_)) => Error::UserError {
                code: Code::PushFailed,
                message: "Project URL is not supported. Please set it to either ssh or https and try again."
                    .to_string(),
            },
            virtual_branches::controller::Error::PushError(project_repository::Error::Other(e)) => Error::from(e),
            virtual_branches::controller::Error::Conflicting => Error::UserError {
                code: Code::Conflicting,
                message: "Project is in conflicting state. Resolve all conflicts and try again."
                    .to_string(),
            },
            virtual_branches::controller::Error::DetachedHead => Error::UserError {
                code: Code::InvalidHead,
                message: format!("Project in detached head state. Please checkout {0} to continue.", virtual_branches::GITBUTLER_INTEGRATION_BRANCH_NAME),
            },
            virtual_branches::controller::Error::InvalidHead(head_name) => Error::UserError {
                code: Code::InvalidHead,
                message: format!("Project is on {0}. Please checkout {1} to continue.", head_name.replace("refs/heads/", ""), virtual_branches::GITBUTLER_INTEGRATION_BRANCH_NAME),
            },
            virtual_branches::controller::Error::NoIntegrationCommit => Error::UserError {
                code: Code::InvalidHead,
                message: "GibButler's integration commit not found on head.".to_string(),
            },
            virtual_branches::controller::Error::LockError(e) => Error::from(anyhow::anyhow!(e)),
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
            app::Error::FetchError(project_repository::Error::NoUrl) => Error::UserError {
                code: Code::FetchFailed,
                message: "Project URL not found. Please check your project's git config and try again."
                    .to_string(),
            },
            app::Error::FetchError(project_repository::Error::NonSSHUrl(_)) => Error::UserError {
                code: Code::FetchFailed,
                message: "Project URL is not supported. Please set it to either ssh or https and try again."
                    .to_string(),
            },
            app::Error::FetchError(project_repository::Error::Other(e)) => Error::from(e),
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
    fn from(error: anyhow::Error) -> Self {
        sentry_anyhow::capture_anyhow(&error);
        tracing::error!(?error);
        Error::Unknown
    }
}
