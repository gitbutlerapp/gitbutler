use anyhow::Result;
use bstr::{BStr, ByteSlice};
use gitbutler_id::id::Id;
use gitbutler_patch_reference::PatchReference;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname, VirtualRefname};
use serde::{Deserialize, Serialize, Serializer};
use std::ops::Deref;

use crate::{ownership::BranchOwnershipClaims, reference::ChangeReference};

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
    /// This is the new metric for determining whether the branch is in the workspace, which means it's applied
    /// and its effects are available to the user.
    #[serde(default = "default_true")]
    pub in_workspace: bool,
    #[serde(default)]
    pub not_in_workspace_wip_change_id: Option<String>,
    /// TODO: This is now obsolete in favor of the heads field before.
    #[serde(default)]
    pub references: Vec<ChangeReference>,
    /// Represents the Stack state of pseudo-references ("heads").
    /// Do **NOT** edit this directly, instead use the `Stack` trait in gitbutler_stack.
    #[serde(default)]
    // pub heads: StackHeads,
    pub heads: Vec<PatchReference>,
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

/// The identity of a branch as to allow to group similar branches together.
///
/// * For *local* branches, it is what's left without the standard prefix, like `refs/heads`, e.g. `main`
///   for `refs/heads/main` or `feat/one` for `refs/heads/feat/one`.
/// * For *remote* branches, it is what's without the prefix and remote name, like `main` for `refs/remotes/origin/main`.
///   or `feat/one` for `refs/remotes/my/special/remote/feat/one`.
/// * For virtual branches, it's either the above if there is a `source_refname` or an `upstream`, or it's the normalized
///   name of the virtual branch.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct BranchIdentity(
    /// The identity is always a valid reference name, full or partial.
    pub gix::refs::PartialName,
);

impl Serialize for BranchIdentity {
    fn serialize<S>(&self, s: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.as_ref().as_bstr().to_str_lossy().serialize(s)
    }
}

impl Deref for BranchIdentity {
    type Target = BStr;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().as_bstr()
    }
}

/// Facilitate obtaining this type from the UI.
impl TryFrom<String> for BranchIdentity {
    type Error = gix::refs::name::Error;

    fn try_from(value: String) -> std::result::Result<Self, Self::Error> {
        gix::refs::PartialName::try_from(value).map(BranchIdentity)
    }
}

/// Used in testing, and **panics** if the value isn't a valid partial ref name
impl From<&str> for BranchIdentity {
    fn from(value: &str) -> Self {
        gix::refs::PartialName::try_from(value)
            .map(BranchIdentity)
            .expect("BUG: value must be valid ref name")
    }
}

/// Used in for short-name conversions
impl TryFrom<&BStr> for BranchIdentity {
    type Error = gix::refs::name::Error;

    fn try_from(value: &BStr) -> std::result::Result<Self, Self::Error> {
        gix::refs::PartialName::try_from(value.to_owned()).map(BranchIdentity)
    }
}
