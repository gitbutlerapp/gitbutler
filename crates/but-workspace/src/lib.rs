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
use std::{collections::HashMap, path::Path};

use anyhow::Result;
use bstr::BString;
use but_core::TreeChange;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};

mod integrated;

/// Types specifically for the user-interface.
pub mod ui;

pub mod commit_engine;
/// Tools for manipulating trees
pub mod tree_manipulation;
pub use tree_manipulation::{
    MoveChangesResult,
    discard_worktree_changes::discard_workspace_changes,
    move_between_commits::move_changes_between_commits,
    remove_changes_from_commit_in_stack::remove_changes_from_commit_in_stack,
    split_branch::{split_branch, split_into_dependent_branch},
    split_commit::{CommitFiles, CommmitSplitOutcome, split_commit},
};
pub mod head;
pub use head::{
    merge_worktree_with_workspace, remerged_workspace_commit_v2, remerged_workspace_tree_v2,
};

/// 🚧utilities for applying and unapplying branches 🚧.
/// Ignore the name of this module; it's just a place to put code by now.
pub mod branch;

pub mod snapshot;

mod changeset;

/// Utility types for the [`WorkspaceCommit`].
pub(crate) mod commit;

/// Types used only when obtaining head-information.
///
/// Note that many of these types should eventually end up in the crate root.
pub mod ref_info;
pub use ref_info::function::{head_info, ref_info};

/// High level Stack funtions that use primitives from this crate (`but-workspace`)
pub mod stack_ext;

/// Functions related to retrieving stack information.
mod stacks;
// TODO: _v3 versions are specifically for the UI, so import them into `ui` instead.
pub use stacks::{
    local_and_remote_commits, stack_branches, stack_details, stack_details_v3, stack_heads_info,
    stacks, stacks_v3,
};

mod branch_details;
pub use branch_details::{branch_details, branch_details_v3, local_commits_for_branch};
use but_graph::SegmentIndex;

/// A change that should be used to create a new commit or alter an existing one, along with enough information to know where to find it.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffSpec {
    /// The previous location of the entry, the source of a rename if there was one.
    #[serde(rename = "previousPathBytes")]
    pub previous_path: Option<BString>,
    /// The worktree-relative path to the worktree file with the content to commit.
    ///
    /// If `hunks` is empty, this means the current content of the file should be committed.
    #[serde(rename = "pathBytes")]
    pub path: BString,
    /// If one or more hunks are specified, match them with actual changes currently in the worktree.
    /// Failure to match them will lead to the change being dropped.
    /// If empty, the whole file is taken as is if this seems to be an addition.
    /// Otherwise, the whole file is being deleted.
    pub hunk_headers: Vec<HunkHeader>,
}

impl From<&TreeChange> for DiffSpec {
    fn from(change: &but_core::TreeChange) -> Self {
        Self {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path.to_owned(),
            hunk_headers: vec![],
        }
    }
}

impl From<TreeChange> for DiffSpec {
    fn from(change: but_core::TreeChange) -> Self {
        Self {
            previous_path: change.previous_path().map(ToOwned::to_owned),
            path: change.path.to_owned(),
            hunk_headers: vec![],
        }
    }
}

/// The header of a hunk that represents a change to a file.
#[derive(Clone, Copy, PartialEq, Eq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkHeader {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero number of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero number of lines included in the new version of the file.
    pub new_lines: u32,
}

impl From<&but_core::unified_diff::DiffHunk> for HunkHeader {
    fn from(hunk: &but_core::unified_diff::DiffHunk) -> Self {
        Self {
            old_start: hunk.old_start,
            old_lines: hunk.old_lines,
            new_start: hunk.new_start,
            new_lines: hunk.new_lines,
        }
    }
}

impl HunkHeader {
    /// Returns the hunk header with the old and new ranges swapped.
    ///
    /// This is useful for applying the hunk in reverse.
    pub fn reverse(&self) -> Self {
        Self {
            old_start: self.new_start,
            old_lines: self.new_lines,
            new_start: self.old_start,
            new_lines: self.old_lines,
        }
    }
}

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
    pub target: Option<but_graph::projection::Target>,
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

/// An ID uniquely identifying stacks.
pub use gitbutler_stack::StackId;

/// A filter for the list of stacks.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum StacksFilter {
    /// Show all stacks
    All,
    /// Show only applied stacks
    #[default]
    InWorkspace,
    /// Show only unapplied stacks
    // TODO: figure out where this is used. V2 maybe? If so, it can be removed eventually.
    Unapplied,
}

/// Returns the last-seen fork-point that the workspace has with the target branch with which it wants to integrate.
// TODO: at some point this should be optional, integration branch doesn't have to be defined.
pub fn common_merge_base_with_target_branch(gb_dir: &Path) -> Result<gix::ObjectId> {
    Ok(VirtualBranchesHandle::new(gb_dir)
        .get_default_target()?
        .sha
        .to_gix())
}

/// Return a list of commits on the target branch
/// Starts either from the target branch or from the provided commit id, up to the limit provided.
///
/// Returns the commits in reverse order, i.e., from the most recent to the oldest.
/// The `Commit` type is the same as that of the other workspace endpoints - for that reason,
/// the fields `has_conflicts` and `state` are somewhat meaningless.
pub fn log_target_first_parent(
    ctx: &CommandContext,
    last_commit_id: Option<gix::ObjectId>,
    limit: usize,
) -> Result<Vec<ui::Commit>> {
    let repo = ctx.gix_repo()?;
    let traversal_root_id = match last_commit_id {
        Some(id) => {
            let commit = repo.find_commit(id)?;
            commit.parent_ids().next()
        }
        None => {
            let state = state_handle(&ctx.project().gb_dir());
            let default_target = state.get_default_target()?;
            Some(
                repo.find_reference(&default_target.branch.to_string())?
                    .peel_to_commit()?
                    .id(),
            )
        }
    };
    let traversal_root_id = match traversal_root_id {
        Some(id) => id,
        None => return Ok(vec![]),
    };

    let mut commits: Vec<ui::Commit> = vec![];
    for commit_info in traversal_root_id.ancestors().first_parent_only().all()? {
        if commits.len() == limit {
            break;
        }
        let commit = commit_info?.id().object()?.into_commit();

        commits.push(commit.try_into()?);
    }
    Ok(commits)
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
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
    use crate::{HunkHeader, commit_engine::HunkRange};

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

pub(crate) mod ext {
    use gix::objs::Write;

    pub trait ObjectStorageExt {
        /// Write all in-memory objects into the given writer.
        fn persist(&self, out: impl gix::objs::Write) -> anyhow::Result<()>;
    }

    impl ObjectStorageExt for gix::odb::memory::Storage {
        fn persist(&self, out: impl Write) -> anyhow::Result<()> {
            for (kind, data) in self.values() {
                out.write_buf(*kind, data)
                    .map_err(anyhow::Error::from_boxed)?;
            }
            Ok(())
        }
    }
}
