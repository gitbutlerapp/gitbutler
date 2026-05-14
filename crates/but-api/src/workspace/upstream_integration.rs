//! Types for upstream integration status reporting.

use anyhow::{Result, bail};
use but_core::ref_metadata::StackId;
use but_serde::BStringForFrontend;
use serde::{Deserialize, Serialize};

/// A branch name paired with its upstream integration status.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct NameAndStatus {
    /// The short branch name.
    pub name: String,
    /// The integration status of this branch.
    pub status: BranchStatus,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(NameAndStatus);

/// A selector pointing to the bottom-most commit or reference of a stack,
/// used to construct a `BottomUpdate` for the `workspace_integrate_upstream` API.
///
/// Serializes to the same JSON shape as `crate::commit::json::RelativeTo`
/// so the frontend can pass it directly as `BottomUpdate.selector`.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase", tag = "type", content = "subject")]
pub enum StackSelector {
    /// Points to the bottom-most non-integrated commit in the stack.
    #[serde(with = "but_serde::object_id")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    Commit(gix::ObjectId),
    /// Points to the bottom branch reference (when the branch has no commits).
    #[serde(with = "but_serde::fullname_lossy")]
    #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
    Reference(gix::refs::FullName),
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackSelector);

/// Per-stack upstream integration status, including branch-level statuses and a
/// bottom selector for constructing a `BottomUpdate`.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct StackStatus {
    /// The stack's identifier, if available.
    #[cfg_attr(feature = "export-schema", schemars(with = "Option<String>"))]
    pub stack_id: Option<StackId>,
    /// The worktree-level tree status for this stack.
    pub tree_status: UpstreamTreeStatus,
    /// Per-branch statuses within the stack.
    pub branch_statuses: Vec<NameAndStatus>,
    /// Selector for the bottom-most commit/reference of the stack,
    /// suitable for use as `BottomUpdate.selector`.
    /// `None` when the entire stack is fully integrated.
    pub bottom_selector: Option<StackSelector>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackStatus);

impl StackStatus {
    /// Create a new `StackStatus`, returning an error if `branch_statuses` is empty.
    pub fn create(
        stack_id: Option<StackId>,
        tree_status: UpstreamTreeStatus,
        branch_statuses: Vec<NameAndStatus>,
        bottom_selector: Option<StackSelector>,
    ) -> Result<Self> {
        if branch_statuses.is_empty() {
            bail!("Branch statuses must not be empty")
        }

        Ok(Self {
            stack_id,
            tree_status,
            branch_statuses,
            bottom_selector,
        })
    }
}

/// Whether the worktree tree can be cleanly updated against the new target.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum UpstreamTreeStatus {
    /// The tree can be updated without conflicts.
    SafelyUpdatable,
    /// The tree update would produce conflicts.
    Conflicted,
    /// The stack has no commits.
    Empty,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(UpstreamTreeStatus);

/// The upstream integration status of a single branch.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BranchStatus {
    /// The branch can be rebased onto the new target without conflicts.
    SafelyUpdatable,
    /// All commits in the branch are already integrated upstream.
    Integrated,
    /// Rebasing the branch onto the new target produces conflicts.
    Conflicted {
        /// If the branch can be rebased onto the target without conflicts.
        rebasable: bool,
    },
    /// The branch has no commits.
    Empty,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BranchStatus);

/// Top-level upstream integration status for all stacks in the workspace.
#[derive(Serialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum StackStatuses {
    /// All stacks are up to date with the target branch.
    UpToDate,
    /// One or more stacks need to be updated.
    UpdatesRequired {
        /// Worktree paths that would conflict when integrating upstream.
        #[serde(rename = "worktreeConflicts")]
        #[cfg_attr(feature = "export-schema", schemars(with = "Vec<String>"))]
        worktree_conflicts: Vec<BStringForFrontend>,
        /// Per-stack integration statuses.
        statuses: Vec<StackStatus>,
    },
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(StackStatuses);

/// How a diverged base branch should be resolved.
#[derive(Serialize, Deserialize, PartialEq, Debug)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum BaseBranchResolutionApproach {
    /// Rebase local changes onto the remote target.
    Rebase,
    /// Merge the remote target into the local branch.
    Merge,
    /// Hard-reset to the remote target, discarding local changes.
    HardReset,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(BaseBranchResolutionApproach);

/// How a conflicting stack should be resolved during upstream integration.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
pub enum ResolutionApproach {
    /// Rebase the stack onto the new target.
    Rebase,
    /// Merge upstream into the stack.
    Merge,
    /// Unapply the stack.
    Unapply,
    /// Delete the stack.
    Delete,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ResolutionApproach);
