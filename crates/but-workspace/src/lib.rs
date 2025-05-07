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

use anyhow::{Context, Result, bail};
use but_core::RefMetadata;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Code;
use gitbutler_oxidize::OidExt;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

mod integrated;

/// Types specifically for the user-interface.
pub mod ui;

pub mod commit_engine;
pub mod tree_manipulation;
pub use tree_manipulation::function::discard_workspace_changes;
pub mod head;
pub use head::{head, merge_worktree_with_workspace};

/// 🚧utilities for applying and unapplying branches 🚧.
pub mod branch;

/// 🚧Deal with worktree changes 🚧.
mod stash {
    /// Information about a stash which is associated with the tip of a stack.
    #[derive(Debug, Copy, Clone)]
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
pub mod head_info;
pub use head_info::function::head_info;

/// High level Stack funtions that use primitives from this crate (`but-workspace`)
pub mod stack_ext;

/// Functions related to retrieving stack information.
mod stacks;
pub use stacks::{
    stack_branch_local_and_remote_commits, stack_branch_upstream_only_commits, stack_branches,
    stack_details, stack_heads_info, stacks, stacks_v3,
};

/// Information about where the user is currently looking at.
#[derive(Debug, Clone)]
pub struct HeadInfo {
    /// The stacks visible in the current workspace.
    ///
    /// This is an empty array if the `HEAD` is detached.
    /// Otherwise, there is one or more stacks.
    pub stacks: Vec<branch::Stack>,
    /// The full name to the target reference that we should integrate with, if present.
    pub target_ref: Option<gix::refs::FullName>,
}

mod virtual_branches_metadata;
pub use virtual_branches_metadata::VirtualBranchesTomlMetadata;

/// A representation of the commit that is the tip of the workspace, i.e., usually what `HEAD` points to,
/// possibly in its managed form in which it merges two or more stacks together, and we can rewrite it at will.
pub struct WorkspaceCommit<'repo> {
    /// The id of the commit itself.
    pub id: gix::Id<'repo>,
    /// The decoded commit for direct access.
    pub inner: gix::objs::Commit,
}

use crate::ui::{CommitState, PushStatus};
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

/// Returns information about the current state of a branch.
pub fn branch_details(
    gb_dir: &Path,
    branch_name: &str,
    remote: Option<&str>,
    ctx: &CommandContext,
) -> Result<ui::BranchDetails> {
    let state = state_handle(gb_dir);
    let repository = ctx.repo();

    let default_target = state.get_default_target()?;

    let (branch, is_remote_head) = match remote {
        None => repository
            .find_branch(branch_name, git2::BranchType::Local)
            .map(|b| (b, false)),
        Some(remote) => repository
            .find_branch(
                format!("{remote}/{branch_name}").as_str(),
                git2::BranchType::Remote,
            )
            .map(|b| (b, true)),
    }
    .context(format!("Could not find branch {branch_name}"))
    .context(Code::BranchNotFound)?;

    let Some(branch_oid) = branch.get().target() else {
        bail!("Branch points to nothing");
    };
    let upstream = branch.upstream().ok();
    let upstream_oid = upstream.as_ref().and_then(|u| u.get().target());

    let push_status = match upstream.as_ref() {
        Some(upstream) => {
            if upstream.get().target() == branch.get().target() {
                PushStatus::NothingToPush
            } else {
                PushStatus::UnpushedCommits
            }
        }
        None => PushStatus::CompletelyUnpushed,
    };

    let merge_bases = repository.merge_bases(branch_oid, default_target.sha)?;
    let Some(base_commit) = merge_bases.last() else {
        bail!("Failed to find merge base");
    };

    let mut revwalk = repository.revwalk()?;
    revwalk.push(branch_oid)?;
    revwalk.hide(default_target.sha)?;
    revwalk.simplify_first_parent()?;

    let commits = revwalk
        .filter_map(|oid| repository.find_commit(oid.ok()?).ok())
        .collect::<Vec<_>>();

    let upstream_commits = if let Some(upstream_oid) = upstream_oid {
        let mut revwalk = repository.revwalk()?;
        revwalk.push(upstream_oid)?;
        revwalk.hide(branch_oid)?;
        revwalk.hide(default_target.sha)?;
        revwalk.simplify_first_parent()?;
        revwalk
            .filter_map(|oid| repository.find_commit(oid.ok()?).ok())
            .collect::<Vec<_>>()
    } else {
        vec![]
    };

    let mut authors = HashSet::new();

    let commits = commits
        .into_iter()
        .map(|commit| {
            let author: ui::Author = commit.author().into();
            let commiter: ui::Author = commit.committer().into();
            authors.insert(author.clone());
            authors.insert(commiter);
            ui::Commit {
                id: commit.id().to_gix(),
                parent_ids: commit.parent_ids().map(|id| id.to_gix()).collect(),
                message: commit.message().unwrap_or_default().into(),
                has_conflicts: false,
                state: CommitState::LocalAndRemote(commit.id().to_gix()),
                created_at: u128::try_from(commit.time().seconds()).unwrap_or(0) * 1000,
                author,
            }
        })
        .collect::<Vec<_>>();

    let upstream_commits = upstream_commits
        .into_iter()
        .map(|commit| {
            let author: ui::Author = commit.author().into();
            let commiter: ui::Author = commit.committer().into();
            authors.insert(author.clone());
            authors.insert(commiter);
            ui::UpstreamCommit {
                id: commit.id().to_gix(),
                message: commit.message().unwrap_or_default().into(),
                created_at: u128::try_from(commit.time().seconds()).unwrap_or(0) * 1000,
                author,
            }
        })
        .collect::<Vec<_>>();

    Ok(ui::BranchDetails {
        name: branch_name.into(),
        remote_tracking_branch: upstream
            .as_ref()
            .and_then(|upstream| upstream.get().name())
            .map(Into::into),
        description: None,
        pr_number: None,
        review_id: None,
        base_commit: base_commit.to_gix(),
        push_status,
        last_updated_at: commits
            .first()
            .map(|c| c.created_at)
            .or(upstream_commits.first().map(|c| c.created_at)),
        authors: authors.into_iter().collect(),
        is_conflicted: false,
        commits,
        upstream_commits,
        tip: branch_oid.to_gix(),
        is_remote_head,
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
/// Returns the commits in reverse order, i.e. from the most recent to the oldest.
/// The `Commit` type is the same as that of the other workspace endpoints - for that reason
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

        commits.push(ui::Commit {
            id: commit.id,
            parent_ids: commit.parent_ids().map(|id| id.detach()).collect(),
            message: commit.message_raw_sloppy().into(),
            has_conflicts: false,
            state: CommitState::LocalAndRemote(commit.id),
            created_at: u128::try_from(commit.time()?.seconds)? * 1000,
            author: commit.author()?.into(),
        });
    }
    Ok(commits)
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
}

#[cfg(test)]
pub(crate) mod utils {
    use crate::commit_engine::{HunkHeader, HunkRange};

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
