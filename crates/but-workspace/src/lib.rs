#![deny(missing_docs)]
#![deny(clippy::indexing_slicing)]

//! ### Terminology
//!
//! * **Workspace**
//!   - A GitButler concept of the combination of one or more branches into one worktree. This allows
//!     multiple branches to be perceived in one worktree, by merging multiple branches together.
//!   - Currently, there is only one workspace per repository, but this is something we intend to change
//!     in the future to facilitate new use cases.
//! * **Workspace Ref**
//!   - The reference that points to the merge-commit which integrates all *workspace* *stacks*.
//! * **Stack**
//!   - GitButler implements the concept of a branch stack. This is essentially a collection of "heads"
//!     (pseudo branches) that contain each other.
//!   - Always contains at least one branch.
//!   - High level documentation here: <https://docs.gitbutler.com/features/branch-management/stacked-branches>
//! * **Target Branch**
//!   - The branch every stack in the workspace wants to get merged into.
//!   - It's usually a local tracking branch, but doesn't have to if no Git *remote* is associated with it.
//!   - Git doesn't have a notion of such a branch.
//! * **DiffSpec**
//!   - A type that identifies changes, either as whole file, or as hunks in the file.
//!   - It doesn't specify if the change is in a commit, or in the worktree, so that information must be provided separately.
use std::collections::HashMap;

use but_core::DiffSpec;

/// **Do not use!**
/// A module with code that depends on `gitbutler-` crates. As such, it's supposed to be ported or rewritten.
/// The structure of the module mirrors the root of this crate, as code was moved here.
#[cfg(feature = "legacy")]
pub mod legacy;

/// Types specifically for the user-interface.
pub mod ui;

pub mod commit_engine;
/// Tools for manipulating trees
pub mod tree_manipulation;
pub use tree_manipulation::discard_worktree_changes::discard_workspace_changes;

/// 🚧utilities for applying and unapplying branches 🚧.
/// Ignore the name of this module; it's just a place to put code by now.
pub mod branch;

mod changeset;

/// Utility types for the [`WorkspaceCommit`].
pub mod commit;

/// Types used only when obtaining head-information.
///
/// Note that many of these types should eventually end up in the crate root.
pub mod ref_info;
pub use ref_info::function::{head_info, ref_info};

mod branch_details;
pub use branch_details::{branch_details, local_commits_for_branch};
use but_graph::{SegmentIndex, projection::TargetCommit};

/// Information about refs, as seen from within or outsie of a workspace.
///
/// We always try to deduce a set of stacks that are currently applied to a workspace,
/// even though it's possible to look at refs that are outside a workspace as well.
/// TODO: There should be a UI version of [`but_graph::projection::Workspace`].
///       This should also include base-branch data, see `get_base_branch_data()`.
#[derive(Debug, Clone)]
pub struct RefInfo {
    /// The name of the ref that points to a workspace commit,
    /// *or* the name of the first stack segment, along with worktree information.
    pub workspace_ref_info: Option<but_graph::RefInfo>,
    /// The stacks visible in the current workspace.
    ///
    /// This is an empty array if the `HEAD` is unborn.
    /// Otherwise, there is one or more stacks.
    pub stacks: Vec<branch::Stack>,
    /// The target to integrate workspace stacks into.
    ///
    /// If `None`, this is a local workspace that doesn't know when possibly pushed branches are considered integrated.
    /// This happens when there is a local branch checked out without a remote tracking branch.
    pub target_ref: Option<but_graph::projection::TargetRef>,
    /// A commit reachable by [`Self::target_ref`] which we chose to keep as base. That way we can extend the workspace
    /// past its computed lower bound.
    ///
    /// Indeed, it's valid to not set the reference, and to only set the commit which should act as an integration base.
    pub target_commit: Option<TargetCommit>,
    /// The segment index of the extra target as provided for traversal,
    /// useful for AdHoc workspaces, but generally applicable to all workspaces to keep the lower bound lower than it
    /// otherwise would be.
    pub extra_target: Option<SegmentIndex>,
    /// The bound can be imagined as the segment from which all other commits in the workspace originate.
    /// It can also be imagined to be the delimiter at the bottom beyond which nothing belongs to the workspace,
    /// as antagonist to the first commit in tip of the segment with `id`, serving as first commit that is
    /// inside the workspace.
    ///
    /// As such, it's always the longest path to the first shared commit with the target among
    /// all of our stacks, or it is the first commit that is shared among all of our stacks in absence of a target.
    /// One can also think of it as the starting point from which all workspace commits can be reached when
    /// following all incoming connections and stopping at the tip of the workspace.
    ///
    /// It is `None` there is only a single stack and no target, so nothing was integrated.
    pub lower_bound: Option<SegmentIndex>,
    /// The `workspace_ref_name` is `Some(_)` and belongs to GitButler, because it had metadata attached.
    pub is_managed_ref: bool,
    /// The `workspace_ref_name` points to a commit that was specifically created by us.
    /// If the user advanced the workspace head by hand, this would be `false`.
    /// See if `ancestor_workspace_commit` is `Some()` to understand if anything could be fixed here.
    /// If there is no managed commits, we have to be extra careful as to what we allow, but setting
    /// up stacks and dependent branches is usually fine, and limited commit creation. Play it safe though,
    /// this is mainly for graceful handling of special cases.
    pub is_managed_commit: bool,
    /// The workspace commit as it exists in the past of `workspace_ref_name`.
    ///
    /// **Warning**: If `Some()`, only fixing this issue should be allowed.
    pub ancestor_workspace_commit: Option<AncestorWorkspaceCommit>,
    /// The workspace represents what `HEAD` is pointing to.
    pub is_entrypoint: bool,
}

/// Describes a workspace commit that is in the ancestry of a managed workspace reference,
/// probably because it was advanced by user commits.
#[derive(Debug, Clone)]
pub struct AncestorWorkspaceCommit {
    /// The commits along the first parent that are between the managed workspace reference and the managed workspace commit.
    /// The vec *should* not be empty, but it can be empty in practice for reasons yet to be discovered.
    pub commits_outside: Vec<ref_info::Commit>,
    /// The index of the segment that actually holds the managed workspace commit.
    pub segment_with_managed_commit: SegmentIndex,
    /// The index of the workspace commit within the `commits` array in its parent segment.
    pub commit_index_of_managed_commit: but_graph::CommitIndex,
}

/// A representation of the commit that is the tip of the workspace i.e., usually what `HEAD` points to,
/// possibly in its managed form in which it merges two or more stacks together, and we can rewrite it at will.
#[derive(Debug, Clone)]
pub struct WorkspaceCommit<'repo> {
    /// The id of the commit itself.
    pub id: gix::Id<'repo>,
    /// The decoded commit for direct access.
    pub inner: gix::objs::Commit,
}

/// If there are multiple diffs spces where path and previous_path are the same, collapse them into one.
pub fn flatten_diff_specs(input: Vec<DiffSpec>) -> Vec<DiffSpec> {
    let mut output: HashMap<String, DiffSpec> = HashMap::new();
    for spec in input {
        let key = format!(
            "{}:{}",
            spec.path,
            spec.previous_path
                .clone()
                .map(|p| p.to_string())
                .unwrap_or_default()
        );
        output
            .entry(key)
            .and_modify(|e| e.hunk_headers.extend(spec.hunk_headers.clone()))
            .or_insert(spec);
    }
    output.into_values().collect()
}

#[cfg(test)]
pub(crate) mod utils {
    use but_core::{HunkHeader, HunkRange};

    pub fn range(start: u32, lines: u32) -> HunkRange {
        HunkRange { start, lines }
    }
    pub fn hunk_header(old: &str, new: &str) -> HunkHeader {
        let ((old_start, old_lines), (new_start, new_lines)) =
            but_testsupport::hunk_header(old, new);
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }
}
