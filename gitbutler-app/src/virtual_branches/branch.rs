mod file_ownership;
mod hunk;
mod ownership;
mod reader;
mod writer;

pub use file_ownership::OwnershipClaim;
pub use hunk::Hunk;
pub use ownership::BranchOwnershipClaims;
pub use reader::BranchReader as Reader;
pub use writer::BranchWriter as Writer;

use serde::{Deserialize, Serialize};

use anyhow::Result;

use crate::{git, id::Id};

pub type BranchId = Id<Branch>;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: BranchId,
    pub name: String,
    pub notes: String,
    pub applied: bool,
    pub upstream: Option<git::RemoteRefname>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    pub upstream_head: Option<git::Oid>,
    #[serde(
        serialize_with = "serialize_u128",
        deserialize_with = "deserialize_u128"
    )]
    pub created_timestamp_ms: u128,
    #[serde(
        serialize_with = "serialize_u128",
        deserialize_with = "deserialize_u128"
    )]
    pub updated_timestamp_ms: u128,
    /// tree is the last git tree written to a session, or merge base tree if this is new. use this for delta calculation from the session data
    pub tree: git::Oid,
    /// head is id of the last "virtual" commit in this branch
    pub head: git::Oid,
    pub ownership: BranchOwnershipClaims,
    // order is the number by which UI should sort branches
    pub order: usize,
    // is Some(timestamp), the branch is considered a default destination for new changes.
    // if more than one branch is selected, the branch with the highest timestamp wins.
    pub selected_for_changes: Option<i64>,
}

fn serialize_u128<S>(x: &u128, s: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    s.serialize_str(&x.to_string())
}

fn deserialize_u128<'de, D>(d: D) -> Result<u128, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    let x: u128 = s.parse().map_err(serde::de::Error::custom)?;
    Ok(x)
}

impl Branch {
    pub fn refname(&self) -> git::VirtualRefname {
        self.into()
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchUpdateRequest {
    pub id: BranchId,
    pub name: Option<String>,
    pub notes: Option<String>,
    pub ownership: Option<BranchOwnershipClaims>,
    pub order: Option<usize>,
    pub upstream: Option<String>, // just the branch name, so not refs/remotes/origin/branchA, just branchA
    pub selected_for_changes: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchCreateRequest {
    pub name: Option<String>,
    pub ownership: Option<BranchOwnershipClaims>,
    pub order: Option<usize>,
    pub selected_for_changes: Option<bool>,
}

impl Branch {
    pub fn from_reader(reader: &crate::reader::Reader<'_>) -> Result<Self, crate::reader::Error> {
        let results = reader.batch(&[
            "id",
            "meta/name",
            "meta/notes",
            "meta/applied",
            "meta/order",
            "meta/upstream",
            "meta/upstream_head",
            "meta/tree",
            "meta/head",
            "meta/created_timestamp_ms",
            "meta/updated_timestamp_ms",
            "meta/ownership",
            "meta/selected_for_changes",
        ])?;

        let id: String = results[0].clone()?.try_into()?;
        let id: BranchId = id.parse().map_err(|e| {
            crate::reader::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("id: {}", e)).into(),
            )
        })?;
        let name: String = results[1].clone()?.try_into()?;

        let notes: String = match results[2].clone() {
            Ok(notes) => Ok(notes.try_into()?),
            Err(crate::reader::Error::NotFound) => Ok(String::new()),
            Err(e) => Err(e),
        }?;

        let applied = match results[3].clone() {
            Ok(applied) => applied.try_into(),
            _ => Ok(false),
        }
        .unwrap_or(false);

        let order: usize = match results[4].clone() {
            Ok(order) => Ok(order.try_into()?),
            Err(crate::reader::Error::NotFound) => Ok(0),
            Err(e) => Err(e),
        }?;

        let upstream = match results[5].clone() {
            Ok(crate::reader::Content::UTF8(upstream)) => {
                if upstream.is_empty() {
                    Ok(None)
                } else {
                    upstream
                        .parse::<git::RemoteRefname>()
                        .map(Some)
                        .map_err(|e| {
                            crate::reader::Error::Io(
                                std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("meta/upstream: {}", e),
                                )
                                .into(),
                            )
                        })
                }
            }
            Ok(_) | Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        let upstream_head = match results[6].clone() {
            Ok(crate::reader::Content::UTF8(upstream_head)) => {
                upstream_head.parse().map(Some).map_err(|e| {
                    crate::reader::Error::Io(
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("meta/upstream_head: {}", e),
                        )
                        .into(),
                    )
                })
            }
            Ok(_) | Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        let tree: String = results[7].clone()?.try_into()?;
        let head: String = results[8].clone()?.try_into()?;
        let created_timestamp_ms = results[9].clone()?.try_into()?;
        let updated_timestamp_ms = results[10].clone()?.try_into()?;

        let ownership_string: String = results[11].clone()?.try_into()?;
        let ownership = ownership_string.parse().map_err(|e| {
            crate::reader::Error::Io(
                std::io::Error::new(std::io::ErrorKind::Other, format!("meta/ownership: {}", e))
                    .into(),
            )
        })?;

        let selected_for_changes = match results[12].clone() {
            Ok(raw_ts) => {
                let ts = raw_ts.try_into().map_err(|e| {
                    crate::reader::Error::Io(
                        std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("meta/selected_for_changes: {}", e),
                        )
                        .into(),
                    )
                })?;
                Ok(Some(ts))
            }
            Err(crate::reader::Error::NotFound) => Ok(None),
            Err(e) => Err(e),
        }?;

        Ok(Self {
            id,
            name,
            notes,
            applied,
            upstream,
            upstream_head,
            tree: tree.parse().map_err(|e| {
                crate::reader::Error::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("meta/tree: {}", e))
                        .into(),
                )
            })?,
            head: head.parse().map_err(|e| {
                crate::reader::Error::Io(
                    std::io::Error::new(std::io::ErrorKind::Other, format!("meta/head: {}", e))
                        .into(),
                )
            })?,
            created_timestamp_ms,
            updated_timestamp_ms,
            ownership,
            order,
            selected_for_changes,
        })
    }
}
