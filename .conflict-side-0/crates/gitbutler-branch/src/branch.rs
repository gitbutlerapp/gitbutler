use bstr::{BStr, ByteSlice};
use gitbutler_stack::{BranchOwnershipClaims, StackId};
use serde::{Deserialize, Serialize, Serializer};
use std::ops::Deref;

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct BranchUpdateRequest {
    pub id: StackId,
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
