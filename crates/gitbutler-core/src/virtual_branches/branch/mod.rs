mod file_ownership;
mod hunk;
mod ownership;

use std::time::Duration;

pub use file_ownership::OwnershipClaim;
pub use hunk::{Hunk, HunkHash};
pub use ownership::{reconcile_claims, BranchOwnershipClaims};
use serde::{Deserialize, Serialize};

use crate::time::duration_int_string_serde;
use crate::{git, id::Id};

pub type BranchId = Id<Branch>;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone, Default)]
pub struct Branch {
    pub id: BranchId,
    pub name: String,
    pub notes: String,
    pub applied: bool,
    pub upstream: Option<git::RemoteRefname>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    pub upstream_head: Option<git::Oid>,
    #[serde(rename = "created_timestamp_ms", with = "duration_int_string_serde")]
    pub created_at: Duration,
    #[serde(rename = "updated_timestamp_ms", with = "duration_int_string_serde")]
    pub updated_at: Duration,
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
