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
    Other(#[from] anyhow::Error),
}

impl TryFrom<&reader::Reader<'_>> for Session {
    type Error = SessionError;

    fn try_from(reader: &reader::Reader) -> Result<Self, Self::Error> {
        let results = reader
            .batch(&[
                path::Path::new("session/meta/id"),
                path::Path::new("session/meta/start"),
                path::Path::new("session/meta/last"),
                path::Path::new("session/meta/branch"),
                path::Path::new("session/meta/commit"),
            ])
            .context("failed to batch read")?;

        let id = &results[0];
        let start_timestamp_ms = &results[1];
        let last_timestamp_ms = &results[2];
        let branch = &results[3];
        let commit = &results[4];

        let id = id.clone().map_err(|error| match error {
            reader::Error::NotFound => SessionError::NoSession,
            error => SessionError::Other(error.into()),
        })?;
        let id: String = id
            .try_into()
            .context("failed to parse session id as string")
            .map_err(SessionError::Other)?;
        let id: SessionId = id.parse().context("failed to parse session id as uuid")?;

        let start_timestamp_ms = start_timestamp_ms.clone().map_err(|error| match error {
            reader::Error::NotFound => SessionError::NoSession,
            error => SessionError::Other(error.into()),
        })?;

        let start_timestamp_ms: u128 = start_timestamp_ms
            .try_into()
            .context("failed to parse session start timestamp as number")
            .map_err(SessionError::Other)?;

        let last_timestamp_ms = last_timestamp_ms.clone().map_err(|error| match error {
            reader::Error::NotFound => SessionError::NoSession,
            error => SessionError::Other(error.into()),
        })?;

        let last_timestamp_ms: u128 = last_timestamp_ms
            .try_into()
            .context("failed to parse session last timestamp as number")
            .map_err(SessionError::Other)?;

        let branch = match branch.clone() {
            Ok(branch) => {
                let branch = branch
                    .try_into()
                    .context("failed to parse session branch as string")?;
                Ok(Some(branch))
            }
            Err(reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
        .context("failed to parse session branch as string")?;

        let commit = match commit.clone() {
            Ok(commit) => {
                let commit = commit
                    .try_into()
                    .context("failed to parse session commit as string")?;
                Ok(Some(commit))
            }
            Err(reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }
        .context("failed to parse session commit as string")?;

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
