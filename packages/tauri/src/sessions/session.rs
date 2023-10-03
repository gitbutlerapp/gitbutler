use std::path;

use anyhow::{Context, Result};
use serde::Serialize;
use thiserror::Error;

use crate::reader;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    // timestamp of when the session was created
    pub start_timestamp_ms: u128,
    // timestamp of when the session was last active
    pub last_timestamp_ms: u128,
    // session branch name
    pub branch: Option<String>,
    // session commit hash
    pub commit: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    // if hash is not set, the session is not saved aka current
    pub hash: Option<String>,
    pub meta: Meta,
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("session does not exist")]
    NoSession,
    #[error("{0}")]
    Err(anyhow::Error),
}

impl TryFrom<&dyn reader::Reader> for Session {
    type Error = SessionError;

    fn try_from(reader: &dyn reader::Reader) -> Result<Self, Self::Error> {
        if !reader.exists(path::Path::new("session/meta")) {
            return Err(SessionError::NoSession);
        }

        let id: String = reader
            .read(path::Path::new("session/meta/id"))
            .context("failed to read session id")
            .map_err(SessionError::Err)?
            .try_into()
            .context("failed to parse session id")
            .map_err(SessionError::Err)?;
        let start_timestamp_ms = reader
            .read(path::Path::new("session/meta/start"))
            .context("failed to read session start timestamp")
            .map_err(SessionError::Err)?
            .try_into()
            .context("failed to parse session start timestamp")
            .map_err(SessionError::Err)?;
        let last_timestamp_ms = reader
            .read(path::Path::new("session/meta/last"))
            .context("failed to read session last timestamp")
            .map_err(SessionError::Err)?
            .try_into()
            .context("failed to parse session last timestamp")
            .map_err(SessionError::Err)?;
        let branch = match reader.read(path::Path::new("session/meta/branch")) {
            Ok(reader::Content::UTF8(branch)) => Some(branch.to_string()),
            _ => None,
        };
        let commit = match reader.read(path::Path::new("session/meta/commit")) {
            Ok(reader::Content::UTF8(commit)) => Some(commit.to_string()),
            _ => None,
        };

        Ok(Self {
            id,
            hash: None,
            meta: Meta {
                start_timestamp_ms,
                last_timestamp_ms,
                branch,
                commit,
            },
        })
    }
}

impl TryFrom<reader::DirReader> for Session {
    type Error = SessionError;

    fn try_from(reader: reader::DirReader) -> Result<Self, Self::Error> {
        let session = Session::try_from(&reader as &dyn reader::Reader)?;
        Ok(session)
    }
}

impl<'reader> TryFrom<reader::CommitReader<'reader>> for Session {
    type Error = SessionError;

    fn try_from(reader: reader::CommitReader<'reader>) -> Result<Self, Self::Error> {
        let commit_oid = reader.get_commit_oid().to_string();
        let session = Session::try_from(&reader as &dyn reader::Reader)?;
        Ok(Session {
            hash: Some(commit_oid),
            ..session
        })
    }
}
