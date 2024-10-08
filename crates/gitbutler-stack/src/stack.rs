use anyhow::Result;
use gitbutler_id::id::Id;
use gitbutler_patch_reference::PatchReference;
use gitbutler_reference::{normalize_branch_name, Refname, RemoteRefname, VirtualRefname};
use serde::{Deserialize, Serialize};

use crate::ownership::BranchOwnershipClaims;

pub type StackId = Id<Stack>;

// this is the struct for the virtual branch data that is stored in our data
// store. it is more or less equivalent to a git branch reference, but it is not
// stored or accessible from the git repository itself. it is stored in our
// session storage under the branches/ directory.
#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct Stack {
    pub id: StackId,
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
    head: git2::Oid,
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
    /// Represents the Stack state of pseudo-references ("heads").
    /// Do **NOT** edit this directly, instead use the `Stack` trait in gitbutler_stack.
    #[serde(default)]
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

impl Stack {
    /// DO NOT USE THIS DIRECTLY, use `StackActions::new` instead.
    /// Creates a new `Branch` with the given name. The `in_workspace` flag is set to `true`.
    #[allow(clippy::too_many_arguments)]
    pub fn new_uninitialized(
        name: String,
        source_refname: Option<Refname>,
        upstream: Option<RemoteRefname>,
        upstream_head: Option<git2::Oid>,
        tree: git2::Oid,
        head: git2::Oid,
        order: usize,
        selected_for_changes: Option<i64>,
        allow_rebasing: bool,
    ) -> Self {
        let now = gitbutler_time::time::now_ms();
        Self {
            id: StackId::generate(),
            name,
            notes: String::new(),
            source_refname,
            upstream,
            upstream_head,
            created_timestamp_ms: now,
            updated_timestamp_ms: now,
            tree,
            head,
            ownership: BranchOwnershipClaims::default(),
            order,
            selected_for_changes,
            allow_rebasing,
            in_workspace: true,
            not_in_workspace_wip_change_id: None,
            heads: Default::default(),
        }
    }

    pub fn refname(&self) -> anyhow::Result<VirtualRefname> {
        self.try_into()
    }

    pub fn head(&self) -> git2::Oid {
        self.head
    }

    pub fn set_head(&mut self, head: git2::Oid) {
        self.head = head;
    }
}

impl TryFrom<&Stack> for VirtualRefname {
    type Error = anyhow::Error;

    fn try_from(value: &Stack) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            branch: normalize_branch_name(&value.name)?,
        })
    }
}
