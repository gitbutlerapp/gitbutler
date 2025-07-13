#![deny(missing_docs, rust_2018_idioms)]
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
//!   - High level documentation here: <https://docs.gitbutler.com/features/stacked-branches>
//! * **Target Branch**
//!   - The branch every stack in the workspace wants to get merged into.
//!   - It's usually a local tracking branch, but doesn't have to if no Git *remote* is associated with it.
//!   - Git doesn't have a notion of such a branch.
//! * **DiffSpec**
//!   - A type that identifies changes, either as whole file, or as hunks in the file.
//!   - It doesn't specify if the change is in a commit, or in the worktree, so that information must be provided separately.
use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use bstr::BString;
use serde::{Deserialize, Serialize};

use but_core::{RefMetadata, TreeChange};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_stack::VirtualBranchesHandle;

mod integrated;

/// Types specifically for the user-interface.
pub mod ui;

pub mod commit_engine;
/// Tools for manipulating trees
pub mod tree_manipulation;
pub use tree_manipulation::MoveChangesResult;
pub use tree_manipulation::discard_worktree_changes::discard_workspace_changes;
pub use tree_manipulation::move_between_commits::move_changes_between_commits;
pub use tree_manipulation::remove_changes_from_commit_in_stack::remove_changes_from_commit_in_stack;
pub use tree_manipulation::split_branch::split_branch;
pub mod head;
pub use head::{head, merge_worktree_with_workspace};
mod relapath;

/// 🚧utilities for applying and unapplying branches 🚧.
///‼️To be superseded by `but-graph` ‼️- the research in there is valuable and should still be migrated.
/// Ignore the name of this module; it's just a place to put code by now.
pub mod branch;

/// 🚧Deal with worktree changes 🚧.
mod stash {
    /// Information about a stash which is associated with the tip of a stack.
    #[derive(Debug, Eq, PartialEq, Copy, Clone)]
    pub enum StashStatus {
        /// The parent reference is still present, but it doesn't point to the first parent of the *stash commit* anymore.
        Desynced,
        /// The parent reference could not be found. Maybe it was removed, maybe it was renamed.
        Orphaned,
    }
}
pub use stash::StashStatus;

mod commit;

/// Types used only when obtaining head-information.
///
/// Note that many of these types should eventually end up in the crate root.
pub mod ref_info;
pub use ref_info::function::{head_info2, ref_info2};

/// High level Stack funtions that use primitives from this crate (`but-workspace`)
pub mod stack_ext;

/// Functions related to retrieving stack information.
mod stacks;
// TODO: _v3 versions are specifically for the UI, so import them into `ui` instead.
pub use stacks::{
    local_and_remote_commits, stack_branch_local_and_remote_commits,
    stack_branch_upstream_only_commits, stack_branches, stack_details, stack_details_v3,
    stack_heads_info, stacks, stacks_v3,
};

mod branch_details;
pub use branch_details::{branch_details, branch_details_v3};
use but_graph::VirtualBranchesTomlMetadata;

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
#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
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
/// TODO: this should become the UI version of [`but_graph::projection::Workspace`].
///       This should also include base-branch data, see `get_base_branch_data()`.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct RefInfo {
    /// The name of the ref that points to a workspace commit,
    /// *or* the name of the first stack segment.
    pub workspace_ref_name: Option<gix::refs::FullName>,
    /// The stacks visible in the current workspace.
    ///
    /// This is an empty array if the `HEAD` is detached.
    /// Otherwise, there is one or more stacks.
    pub stacks: Vec<branch::Stack>,
    /// The full name to the target reference that we should integrate with, if present.
    /// It's never present in single-branch mode.
    pub target_ref: Option<gix::refs::FullName>,
    /// The `workspace_ref_name` is `Some(_)` and belongs to GitButler, because it had metadata attached.
    pub is_managed_ref: bool,
    /// The `workspace_ref_name` points to a commit that was specifically created by us.
    /// If the user advanced the workspace head by hand, this would be `false`.
    pub is_managed_commit: bool,
    /// The workspace represents what `HEAD` is pointing to.
    pub is_entrypoint: bool,
}

/// A representation of the commit that is the tip of the workspace i.e., usually what `HEAD` points to,
/// possibly in its managed form in which it merges two or more stacks together, and we can rewrite it at will.
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

/// Get a stable `StackId` for the given `name`. It's fetched from `meta`, assuming it's backed by a toml file
/// and assuming that `name` is stored there as applied or unapplied branch.
fn id_from_name_v2_to_v3(
    name: &gix::refs::FullNameRef,
    meta: &VirtualBranchesTomlMetadata,
) -> Result<StackId> {
    let ref_meta = meta.branch(name)?;
    ref_meta.stack_id().with_context(|| {
        format!(
            "{name:?} didn't have a stack-id even though \
        it was supposed to be in virtualbranches.toml"
        )
    })
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
mod tests {
    use super::*;
    use bstr::BString;

    #[test]
    fn test_flatten_diff_specs_empty() {
        let input = vec![];
        let result = flatten_diff_specs(input);
        assert!(result.is_empty());
    }

    #[test]
    fn test_flatten_diff_specs_single() {
        let spec = DiffSpec {
            path: BString::from("file.txt"),
            previous_path: None,
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
            }],
        };
        let input = vec![spec.clone()];
        let result = flatten_diff_specs(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result.first().unwrap(), &spec);
    }

    #[test]
    fn test_flatten_diff_specs_different_files() {
        let spec1 = DiffSpec {
            path: BString::from("file1.txt"),
            previous_path: None,
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
            }],
        };
        let spec2 = DiffSpec {
            path: BString::from("file2.txt"),
            previous_path: None,
            hunk_headers: vec![HunkHeader {
                old_start: 5,
                old_lines: 1,
                new_start: 5,
                new_lines: 2,
            }],
        };
        let input = vec![spec1.clone(), spec2.clone()];
        let result = flatten_diff_specs(input);
        assert_eq!(result.len(), 2);
        assert!(result.contains(&spec1));
        assert!(result.contains(&spec2));
    }

    #[test]
    fn test_flatten_diff_specs_same_file_merge_hunks() {
        let hunk1 = HunkHeader {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 3,
        };
        let hunk2 = HunkHeader {
            old_start: 10,
            old_lines: 1,
            new_start: 11,
            new_lines: 2,
        };

        let spec1 = DiffSpec {
            path: BString::from("file.txt"),
            previous_path: None,
            hunk_headers: vec![hunk1],
        };
        let spec2 = DiffSpec {
            path: BString::from("file.txt"),
            previous_path: None,
            hunk_headers: vec![hunk2],
        };

        let input = vec![spec1, spec2];
        let result = flatten_diff_specs(input);

        assert_eq!(result.len(), 1);
        assert_eq!(result.first().unwrap().path, BString::from("file.txt"));
        assert_eq!(result.first().unwrap().previous_path, None);
        assert_eq!(result.first().unwrap().hunk_headers.len(), 2);
        assert!(result.first().unwrap().hunk_headers.contains(&hunk1));
        assert!(result.first().unwrap().hunk_headers.contains(&hunk2));
    }

    #[test]
    fn test_flatten_diff_specs_with_previous_path() {
        let spec1 = DiffSpec {
            path: BString::from("new_file.txt"),
            previous_path: Some(BString::from("old_file.txt")),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 3,
            }],
        };
        let spec2 = DiffSpec {
            path: BString::from("new_file.txt"),
            previous_path: None,
            hunk_headers: vec![HunkHeader {
                old_start: 5,
                old_lines: 1,
                new_start: 5,
                new_lines: 2,
            }],
        };

        let input = vec![spec1.clone(), spec2.clone()];
        let result = flatten_diff_specs(input);

        // These should remain separate because they have different previous_path values
        assert_eq!(result.len(), 2);
        assert!(result.contains(&spec1));
        assert!(result.contains(&spec2));
    }

    #[test]
    fn test_flatten_diff_specs_same_previous_path() {
        let hunk1 = HunkHeader {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 3,
        };
        let hunk2 = HunkHeader {
            old_start: 10,
            old_lines: 1,
            new_start: 11,
            new_lines: 2,
        };

        let spec1 = DiffSpec {
            path: BString::from("new_file.txt"),
            previous_path: Some(BString::from("old_file.txt")),
            hunk_headers: vec![hunk1],
        };
        let spec2 = DiffSpec {
            path: BString::from("new_file.txt"),
            previous_path: Some(BString::from("old_file.txt")),
            hunk_headers: vec![hunk2],
        };

        let input = vec![spec1, spec2];
        let result = flatten_diff_specs(input);

        assert_eq!(result.len(), 1);
        assert_eq!(result.first().unwrap().path, BString::from("new_file.txt"));
        assert_eq!(
            result.first().unwrap().previous_path,
            Some(BString::from("old_file.txt"))
        );
        assert_eq!(result.first().unwrap().hunk_headers.len(), 2);
        assert!(result.first().unwrap().hunk_headers.contains(&hunk1));
        assert!(result.first().unwrap().hunk_headers.contains(&hunk2));
    }
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
