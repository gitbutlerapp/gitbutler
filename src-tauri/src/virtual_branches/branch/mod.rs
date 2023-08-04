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

use serde::{Deserialize, Serialize};

use anyhow::Result;

use crate::project_repository::branch;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub applied: bool,
    pub upstream: Option<branch::RemoteName>,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
    // tree is the last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub tree: git2::Oid,
    pub head: git2::Oid,
    pub ownership: Ownership,
    // order is the number by which UI should sort branches
    pub order: u32,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchUpdateRequest {
    pub id: String,
    pub name: Option<String>,
    pub ownership: Option<Ownership>,
    pub order: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchCreateRequest {
    pub name: Option<String>,
    pub ownership: Option<Ownership>,
    pub order: Option<u32>,
}

impl TryFrom<&dyn crate::reader::Reader> for Branch {
    type Error = crate::reader::Error;

    fn try_from(reader: &dyn crate::reader::Reader) -> Result<Self, Self::Error> {
        let id = reader.read_string("id").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("id: {}", e),
            ))
        })?;
        let name = reader.read_string("meta/name").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/name: {}", e),
            ))
        })?;
        let applied = reader
            .read_bool("meta/applied")
            .map_err(|e| {
                crate::reader::Error::IOError(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("meta/applied: {}", e),
                ))
            })
            .or(Ok(false))?;

        let order = match reader.read_u32("meta/order") {
            Ok(order) => Ok(order),
            Err(crate::reader::Error::NotFound) => Ok(0),
            Err(e) => Err(e),
        }
        .map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/order: {}", e),
            ))
        })?;

        let upstream = match reader.read_string("meta/upstream") {
            Ok(upstream) => {
                if upstream.is_empty() {
                    Ok(None)
                } else {
                    branch::RemoteName::try_from(upstream.as_str())
                        .map(Some)
                        .map_err(|e| {
                            crate::reader::Error::IOError(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                format!("meta/upstream: {}", e),
                            ))
                        })
                }
            }
            Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        let tree = reader.read_string("meta/tree").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/tree: {}", e),
            ))
        })?;
        let head = reader.read_string("meta/head").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/head: {}", e),
            ))
        })?;
        let created_timestamp_ms = reader.read_u128("meta/created_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/created_timestamp_ms: {}", e),
            ))
        })?;
        let updated_timestamp_ms = reader.read_u128("meta/updated_timestamp_ms").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/updated_timestamp_ms: {}", e),
            ))
        })?;

        let ownership_string = reader.read_string("meta/ownership").map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/ownership: {}", e),
            ))
        })?;
        let ownership = Ownership::try_from(ownership_string.as_str()).map_err(|e| {
            crate::reader::Error::IOError(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("meta/ownership: {}", e),
            ))
        })?;

        Ok(Self {
            id,
            name,
            applied,
            upstream,
            tree: git2::Oid::from_str(&tree).unwrap(),
            head: git2::Oid::from_str(&head).unwrap(),
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
            order,
        })
    }
}
