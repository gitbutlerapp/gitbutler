use std::path;

use anyhow::{Context, Result};
use serde::Serialize;
use thiserror::Error;

use crate::{git, id::Id, reader};

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

pub type SessionId = Id<Session>;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: SessionId,
    // if hash is not set, the session is not saved aka current
    pub hash: Option<git::Oid>,
    pub meta: Meta,
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("session does not exist")]
    NoSession,
    #[error("{0}")]
    Other(anyhow::Error),
}

impl TryFrom<&reader::Reader<'_>> for Session {
    type Error = SessionError;

    fn try_from(reader: &reader::Reader) -> Result<Self, Self::Error> {
        let id: String = reader
            .read(path::Path::new("session/meta/id"))
            .map_err(|error| match error {
                reader::Error::NotFound => SessionError::NoSession,
                _ => SessionError::Other(error.into()),
            })?
            .try_into()
            .context("failed to parse session id as string")
            .map_err(SessionError::Other)?;

        let id: SessionId = id
            .parse()
            .context("failed to parse session id as id")
            .map_err(SessionError::Other)?;

        let start_timestamp_ms = reader
            .read(path::Path::new("session/meta/start"))
            .map_err(|error| match error {
                reader::Error::NotFound => SessionError::NoSession,
                _ => SessionError::Other(error.into()),
            })?
            .try_into()
            .context("failed to parse session start timestamp as number")
            .map_err(SessionError::Other)?;

        let last_timestamp_ms = reader
            .read(path::Path::new("session/meta/last"))
            .map_err(|error| match error {
                reader::Error::NotFound => SessionError::NoSession,
                _ => SessionError::Other(error.into()),
            })?
            .try_into()
            .context("failed to parse session last timestamp as number")
            .map_err(SessionError::Other)?;

        let branch = match reader.read(path::Path::new("session/meta/branch")) {
            Ok(reader::Content::UTF8(branch)) => Some(branch.clone()),
            Err(reader::Error::NotFound) => None,
            _ => {
                return Err(SessionError::Other(anyhow::anyhow!(
                    "failed to read branch"
                )))
            }
        };

        let commit = match reader.read(path::Path::new("session/meta/commit")) {
            Ok(reader::Content::UTF8(commit)) => Some(commit.clone()),
            Err(reader::Error::NotFound) => None,
            _ => {
                return Err(SessionError::Other(anyhow::anyhow!(
                    "failed to read commit"
                )))
            }
        };

        Ok(Self {
            id,
            hash: reader.commit_id(),
            meta: Meta {
                start_timestamp_ms,
                last_timestamp_ms,
                branch,
                commit,
            },
        })
    }
}
