use anyhow::Result;
use gitbutler_id::id::Id;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname, VirtualRefname};
use serde::{Deserialize, Serialize};

use crate::ownership::BranchOwnershipClaims;

pub type BranchId = Id<Branch>;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: BranchId,
    /// A user-specified name with no restrictions.
    /// It will be normalized except to be a valid [ref-name](Branch::refname()) if named `refs/gitbutler/<normalize(name)>`.
    pub name: String,
    pub notes: String,
    /// If set, this means this virtual branch was originally created from `Some(branch)`.
    /// It can be *any* branch.
    pub source_refname: Option<Refname>,
    /// The local tracking branch, holding the state of the remote.
    pub upstream: Option<RemoteRefname>,
    // upstream_head is the last commit on we've pushed to the upstream branch
    #[serde(with = "gitbutler_serde::oid_opt", default)]
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
    #[serde(with = "gitbutler_serde::oid")]
    pub tree: git2::Oid,
    /// head is id of the last "virtual" commit in this branch
    #[serde(with = "gitbutler_serde::oid")]
    pub head: git2::Oid,
    pub ownership: BranchOwnershipClaims,
    // order is the number by which UI should sort branches
    pub order: usize,
    // is Some(timestamp), the branch is considered a default destination for new changes.
    // if more than one branch is selected, the branch with the highest timestamp wins.
    pub selected_for_changes: Option<i64>,
    #[serde(default = "default_true")]
    pub allow_rebasing: bool,
    /// This is the old metric for determining whether the branch is in the workspace
    /// This is kept in sync with in_workspace
    /// There should only be one condition where `applied` is false and `in_workspace`
    /// true.
    ///
    /// This is after updating, the `in_workspace` property will have defaulted to true
    /// but the old `applied` property will have remained false.
    #[serde(default = "default_true")]
    pub applied: bool,
    /// This is the new metric for determining whether the branch is in the workspace, which means it's applied
    /// and its effects are available to the user.
    #[serde(default = "default_true")]
    pub in_workspace: bool,
    #[serde(default)]
    pub not_in_workspace_wip_change_id: Option<String>,
}

fn default_true() -> bool {
    true
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
    pub fn refname(&self) -> anyhow::Result<VirtualRefname> {
        self.try_into()
    }

    /// self.applied and self.in_workspace are kept in sync by the application
    ///
    /// There is only once case where this might not be the case which is when
    /// the user has upgraded to the new version for the fisrt time.
    ///
    /// In this state, the `in_workspace` property will have defaulted to true
    /// but the old `applied` property will have remained false.
    ///
    /// This function indicates this state
    pub fn is_old_unapplied(&self) -> bool {
        !self.applied && self.in_workspace
    }
}

impl TryFrom<&Branch> for VirtualRefname {
    type Error = anyhow::Error;

    fn try_from(value: &Branch) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            branch: normalize_branch_name(&value.name)?,
        })
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
    pub allow_rebasing: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct BranchCreateRequest {
    pub name: Option<String>,
    pub ownership: Option<BranchOwnershipClaims>,
    pub order: Option<usize>,
    pub selected_for_changes: Option<bool>,
}
