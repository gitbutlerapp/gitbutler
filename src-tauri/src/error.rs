use serde::{ser::SerializeMap, Serialize};

use crate::{app, keys, virtual_branches};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
    #[error("Something went wrong")]
    Unknown,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("message", &self.to_string())?;
        map.end()
    }
}

impl From<virtual_branches::controller::Error> for Error {
    fn from(e: virtual_branches::controller::Error) -> Self {
        match e {
            virtual_branches::controller::Error::PushError(e) => Error::Message(e.to_string()),
            virtual_branches::controller::Error::Conflicting => {
                Error::Message("Project is in conflicting state".to_string())
            }
            virtual_branches::controller::Error::LockError(_) => Error::Unknown,
            virtual_branches::controller::Error::Other(e) => Error::from(e),
        }
    }
}

impl From<app::Error> for Error {
    fn from(e: app::Error) -> Self {
        match e {
            app::Error::Message(msg) => Error::Message(msg),
            app::Error::Other(e) => Error::from(e),
        }
    }
}

impl From<keys::Error> for Error {
    fn from(value: keys::Error) -> Self {
        app::Error::Other(anyhow::Error::from(value)).into()
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        sentry_anyhow::capture_anyhow(&e);
        log::error!("{:#}", e);
        Error::Unknown
    }
}
