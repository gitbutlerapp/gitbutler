mod file_ownership;
mod hunk;
mod ownership;
mod reader;
mod writer;

pub use file_ownership::FileOwnership;
pub use hunk::Hunk;
pub use ownership::Ownership;
pub use reader::BranchReader as Reader;
pub use writer::BranchWriter as Writer;

use std::path;

use serde::{Deserialize, Serialize};

use anyhow::Result;

use crate::git;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub notes: String,
    pub applied: bool,
    pub upstream: Option<git::RemoteBranchName>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    pub upstream_head: Option<git::Oid>,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    // tree is the last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub tree: git::Oid,
    pub head: git::Oid,
    pub ownership: Ownership,
    // order is the number by which UI should sort branches
    pub order: usize,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchUpdateRequest {
    pub id: String,
    pub name: Option<String>,
    pub notes: Option<String>,
    pub ownership: Option<Ownership>,
    pub order: Option<usize>,
    pub upstream: Option<String>, // just the branch name, so not refs/remotes/origin/branchA, just branchA
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchCreateRequest {
    pub name: Option<String>,
    pub ownership: Option<Ownership>,
    pub order: Option<usize>,
}

impl TryFrom<&dyn crate::reader::Reader> for Branch {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let id: String = reader.read(&path::PathBuf::from("id"))?.try_into()?;
        let name: String = reader.read(&path::PathBuf::from("meta/name"))?.try_into()?;

        let notes: String = match reader.read(&path::PathBuf::from("meta/notes")) {
            Ok(notes) => Ok(notes.try_into()?),
            Err(crate::reader::Error::NotFound) => Ok("".to_string()),
            Err(e) => Err(e),
        }?;

        let applied = match reader.read(&path::PathBuf::from("meta/applied")) {
            Ok(applied) => applied.try_into(),
            _ => Ok(false),
        }
        .unwrap_or(false);

        let order: usize = match reader.read(&path::PathBuf::from("meta/order")) {
            Ok(order) => Ok(order.try_into()?),
            Err(crate::reader::Error::NotFound) => Ok(0),
            Err(e) => Err(e),
        }?;

        let upstream_head = match reader.read(&path::PathBuf::from("meta/upstream_head")) {
            Ok(crate::reader::Content::UTF8(upstream_head)) => {
                upstream_head.parse().map(Some).map_err(|e| {
                    crate::reader::Error::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("meta/upstream_head: {}", e),
                    ))
                })
            }
            Ok(_) => Ok(None),
            Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        let upstream = match reader.read(&path::PathBuf::from("meta/upstream")) {
            Ok(crate::reader::Content::UTF8(upstream)) => {
                if upstream.is_empty() {
                    Ok(None)
                } else {
                    upstream
                        .parse::<git::RemoteBranchName>()
                        .map(Some)
                        .map_err(|e| {
                            crate::reader::Error::Io(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("meta/upstream: {}", e),
                            ))
                        })
                }
            }
            Ok(_) => Ok(None),
            Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        let tree: String = reader.read(&path::PathBuf::from("meta/tree"))?.try_into()?;
        let head: String = reader.read(&path::PathBuf::from("meta/head"))?.try_into()?;
        let created_timestamp_ms = reader
            .read(&path::PathBuf::from("meta/created_timestamp_ms"))?
            .try_into()?;
        let updated_timestamp_ms = reader
            .read(&path::PathBuf::from("meta/updated_timestamp_ms"))?
            .try_into()?;

        let ownership_string: String = reader
            .read(&path::PathBuf::from("meta/ownership"))?
            .try_into()?;
        let ownership = ownership_string.parse().map_err(|e| {
            crate::reader::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/ownership: {}", e),
            ))
        })?;

        Ok(Self {
            id,
            name,
            notes,
            applied,
            upstream,
            upstream_head,
            tree: tree.parse().map_err(|e| {
                crate::reader::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("meta/tree: {}", e),
                ))
            })?,
            head: head.parse().map_err(|e| {
                crate::reader::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("meta/head: {}", e),
                ))
            })?,
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
            order,
        })
    }
}
