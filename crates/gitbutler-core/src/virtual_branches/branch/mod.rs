mod file_ownership;
mod hunk;
mod ownership;

use anyhow::Result;
pub use file_ownership::OwnershipClaim;
pub use hunk::{Hunk, HunkHash};
pub use ownership::{reconcile_claims, BranchOwnershipClaims};
use serde::{Deserialize, Serialize};

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
    /// Previously used to determine if the branch should be shown on the board. If a branch is not shown on the board, it is converted to a real git branch.
    #[serde(rename = "applied")]
    pub old_applied: bool,
    pub upstream: Option<git::RemoteRefname>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    #[serde(with = "crate::serde::oid_opt", default)]
    pub upstream_head: Option<git2::Oid>,
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
    #[serde(with = "crate::serde::oid")]
    pub tree: git2::Oid,
    /// head is id of the last "virtual" commit in this branch
    #[serde(with = "crate::serde::oid")]
    pub head: git2::Oid,
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

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
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
