use crate::{app::reader, git::activity};
use anyhow::{anyhow, Context, Result};
use serde::Serialize;
use std::path::Path;
use thiserror::Error;

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
    // TODO: make this a method instead
    pub activity: Vec<activity::Activity>,
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("session does not exist")]
    NoSession,
    #[error("{0}")]
    Err(anyhow::Error),
}

impl<'reader> TryFrom<Box<dyn reader::Reader + 'reader>> for Session {
    type Error = SessionError;

    fn try_from(reader: Box<dyn reader::Reader + 'reader>) -> Result<Self, Self::Error> {
        if !reader.exists("session") {
            return Err(SessionError::NoSession);
        }

        let id = reader
            .read_to_string("session/meta/id")
            .with_context(|| "failed to read session id")
            .map_err(|err| SessionError::Err(err))?;
        let start_timestamp_ms = reader
            .read_to_string("session/meta/start")
            .with_context(|| "failed to read session start timestamp")
            .map_err(|err| SessionError::Err(err))?
            .parse::<u128>()
            .with_context(|| "failed to parse session start timestamp")
            .map_err(|err| SessionError::Err(err))?;
        let last_timestamp_ms = reader
            .read_to_string("session/meta/last")
            .with_context(|| "failed to read session last timestamp")
            .map_err(|err| SessionError::Err(err))?
            .parse::<u128>()
            .with_context(|| "failed to parse session last timestamp")
            .map_err(|err| SessionError::Err(err))?;
        let branch = reader.read_to_string("session/meta/branch");
        let commit = reader.read_to_string("session/meta/commit");

        Ok(Self {
            id,
            hash: None,
            meta: Meta {
                start_timestamp_ms,
                last_timestamp_ms,
                branch: if branch.is_err() {
                    None
                } else {
                    Some(branch.unwrap())
                },
                commit: if commit.is_err() {
                    None
                } else {
                    Some(commit.unwrap())
                },
            },
            activity: vec![],
        })
    }
}

impl<'reader> TryFrom<reader::DirReader> for Session {
    type Error = SessionError;

    fn try_from(reader: reader::DirReader) -> Result<Self, Self::Error> {
        let session = Session::try_from(Box::new(reader) as Box<dyn reader::Reader + 'reader>)?;
        Ok(session)
    }
}

impl<'reader> TryFrom<reader::CommitReader<'reader>> for Session {
    type Error = SessionError;

    fn try_from(reader: reader::CommitReader<'reader>) -> Result<Self, Self::Error> {
        let commit_oid = reader.get_commit_oid().to_string();
        let session = Session::try_from(Box::new(reader) as Box<dyn reader::Reader + 'reader>)?;
        Ok(Session {
            hash: Some(commit_oid),
            ..session
        })
    }
}

pub fn id_from_commit(repo: &git2::Repository, commit: &git2::Commit) -> Result<String> {
    let tree = commit.tree().unwrap();
    let session_id_path = Path::new("session/meta/id");
    if !tree.get_path(session_id_path).is_ok() {
        return Err(anyhow!("commit does not have a session id"));
    }
    let id = read_as_string(repo, &tree, session_id_path)?;
    return Ok(id);
}

fn read_as_string(repo: &git2::Repository, tree: &git2::Tree, path: &Path) -> Result<String> {
    let tree_entry = tree.get_path(path)?;
    let blob = tree_entry.to_object(repo)?.into_blob().unwrap();
    let contents = String::from_utf8(blob.content().to_vec()).with_context(|| {
        format!(
            "failed to read file {} as string",
            path.to_str().unwrap_or("unknown")
        )
    })?;
    Ok(contents)
}
