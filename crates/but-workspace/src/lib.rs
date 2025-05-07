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
use bstr::BString;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_error::error::Code;
use gitbutler_id::id::Id;
use gitbutler_oxidize::{ObjectIdExt, OidExt, git2_signature_to_gix_signature};
use gitbutler_stack::{Stack, StackBranch, VirtualBranchesHandle};
use integrated::IsCommitIntegrated;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::str::FromStr;

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
    Unapplied,
}

/// Returns the list of branch information for the branches in a stack.
pub fn stack_heads_info(stack: &Stack, repo: &gix::Repository) -> Result<Vec<ui::StackHeadInfo>> {
    let branches = stack
        .branches()
        .into_iter()
        .rev()
        .filter_map(|branch| {
            let tip = branch.head_oid(repo).ok()?;
            Some(ui::StackHeadInfo {
                name: branch.name().to_owned().into(),
                tip,
            })
        })
        .collect::<Vec<_>>();

    Ok(branches)
}

/// Returns the list of stacks that are currently part of the workspace.
/// If there are no applied stacks, the returned Vec is empty.
/// If the GitButler state file in the provided path is missing or invalid, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
pub fn stacks(
    ctx: &CommandContext,
    gb_dir: &Path,
    repo: &gix::Repository,
    filter: StacksFilter,
) -> Result<Vec<ui::StackEntry>> {
    let state = state_handle(gb_dir);

    let stacks = match filter {
        StacksFilter::All => state.list_all_stacks()?,
        StacksFilter::InWorkspace => state
            .list_all_stacks()?
            .into_iter()
            .filter(|s| s.in_workspace)
            .collect(),
        StacksFilter::Unapplied => state
            .list_all_stacks()?
            .into_iter()
            .filter(|s| !s.in_workspace)
            .collect(),
    };

    let stacks = stacks
        .into_iter()
        .filter_map(|mut stack| stack.migrate_change_ids(ctx).ok().map(|()| stack))
        .filter(|s| s.is_initialized());

    stacks
        .sorted_by_key(|s| s.order)
        .map(|stack| ui::StackEntry::try_new(repo, &stack))
        .collect()
}

fn try_from_stack_v3(
    repo: &gix::Repository,
    stack: branch::Stack,
) -> anyhow::Result<ui::StackEntry> {
    let heads = stack.segments.into_iter().map(|segment| {
        let ref_name = segment
            .ref_name
            .expect("This type can't represent this state and it shouldn't have to");
        ui::StackHeadInfo {
            tip: repo
                .find_reference(ref_name.as_ref())
                .ok()
                .and_then(|r| r.try_id())
                .map(|id| id.detach())
                .unwrap_or(repo.object_hash().null()),
            name: ref_name.into(),
        }
    });
    Ok(ui::StackEntry {
        id: Default::default(), // TODO: have a global evil mapping from ref-names to IDs
        heads: heads.collect(),
        tip: stack.tip.unwrap_or(repo.object_hash().null()),
    })
}

/// Returns the list of stacks that pass `filter`.
///
/// Use `repo` and `meta` to read branches data
// TODO: Let the UI get all stacks at once.
pub fn stacks_v3(
    repo: &gix::Repository,
    meta: &impl but_core::RefMetadata,
    filter: StacksFilter,
) -> Result<Vec<ui::StackEntry>> {
    let info = crate::head_info(
        repo,
        meta,
        head_info::Options {
            // TODO: set this to a good value for the UI to not slow down, and also a value that forces us to re-investigate this.
            stack_commit_limit: 100,
        },
    )?;

    info.stacks
        .into_iter()
        .filter_map(|stack| match filter {
            StacksFilter::All | StacksFilter::InWorkspace => Some(try_from_stack_v3(repo, stack)),
            // TODO: get all stacks, not just the applied ones. These are only by inspection of metadata.
            StacksFilter::Unapplied => None,
        })
        .collect::<Result<_, _>>()
}

/// Determines if a force push is required to push a branch to its remote.
fn requires_force(ctx: &CommandContext, branch: &StackBranch, remote: &str) -> Result<bool> {
    let upstream = branch.remote_reference(remote);

    let reference = match ctx.repo().refname_to_id(&upstream) {
        Ok(reference) => reference,
        Err(err) if err.code() == git2::ErrorCode::NotFound => return Ok(false),
        Err(other) => return Err(other).context("failed to find upstream reference"),
    };

    let upstream_commit = ctx
        .repo()
        .find_commit(reference)
        .context("failed to find upstream commit")?;

    let branch_head = branch.head_oid(&ctx.gix_repo()?)?;
    let merge_base = ctx
        .repo()
        .merge_base(upstream_commit.id(), branch_head.to_git2())?;

    Ok(merge_base != upstream_commit.id())
}

/// Returns information about the current state of a stack.
/// If the stack is not found, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
/// - `stack_id`: The ID of the stack to get information about.
/// - `ctx`: The command context for the project.
pub fn stack_details(
    gb_dir: &Path,
    stack_id: StackId,
    ctx: &CommandContext,
) -> Result<ui::StackDetails> {
    #[derive(Debug, Default)]
    struct BranchState {
        is_integrated: bool,
        is_dirty: bool,
        requires_force: bool,
        has_pushed_commits: bool,
    }

    impl From<BranchState> for PushStatus {
        fn from(state: BranchState) -> Self {
            match (
                state.is_integrated,
                state.is_dirty,
                state.requires_force,
                state.has_pushed_commits,
            ) {
                (true, _, _, _) => PushStatus::Integrated,
                (_, true, _, false) => PushStatus::CompletelyUnpushed,
                (_, _, true, _) => PushStatus::UnpushedCommitsRequiringForce,
                (_, true, _, _) => PushStatus::UnpushedCommits,
                (_, false, _, _) => PushStatus::NothingToPush,
            }
        }
    }

    let state = state_handle(gb_dir);
    let mut stack = state.get_stack(stack_id)?;
    let branches = stack.branches();
    let branches = branches.iter().filter(|b| !b.archived);
    let repo = ctx.gix_repo()?;
    let remote = state
        .get_default_target()
        .context("failed to get default target")?
        .push_remote_name();

    let mut stack_state = BranchState::default();
    let mut stack_is_conflicted = false;
    let mut branch_details = vec![];
    let mut current_base = stack.merge_base(ctx)?;

    for branch in branches {
        let upstream_reference = ctx
            .repo()
            .find_reference(&branch.remote_reference(remote.as_str()))
            .ok()
            .map(|_| branch.remote_reference(remote.as_str()));

        let mut branch_state = BranchState {
            requires_force: requires_force(ctx, branch, &remote)?,
            ..Default::default()
        };

        let mut is_conflicted = false;
        let mut authors = HashSet::new();
        let commits = local_and_remote_commits(ctx, &repo, branch, &stack)?;
        let upstream_commits = upstream_only_commits(ctx, &repo, branch, &stack, Some(&commits))?;

        // If there are commits in the remote, we can assume that commits have been pushed. *Like, literally*.
        branch_state.has_pushed_commits |= !upstream_commits.is_empty();

        for commit in &commits {
            is_conflicted |= commit.has_conflicts;
            branch_state.is_dirty |= matches!(commit.state, ui::CommitState::LocalOnly);
            branch_state.has_pushed_commits |=
                matches!(commit.state, CommitState::LocalAndRemote(_));
            authors.insert(commit.author.clone());
        }

        // We can assume that if the child-most commit is integrated, the whole branch is integrated
        branch_state.is_integrated = matches!(
            commits.first().map(|c| &c.state),
            Some(CommitState::Integrated)
        );

        stack_is_conflicted |= is_conflicted;
        stack_state.is_dirty |= branch_state.is_dirty;
        stack_state.requires_force |= branch_state.requires_force;
        stack_state.has_pushed_commits |= branch_state.has_pushed_commits;

        // If all branches are integrated, the stack is integrated
        stack_state.is_integrated &= branch_state.is_integrated;

        branch_details.push(ui::BranchDetails {
            name: branch.name().to_owned().into(),
            remote_tracking_branch: upstream_reference.map(Into::into),
            description: branch.description.clone(),
            pr_number: branch.pr_number,
            review_id: branch.review_id.clone(),
            tip: branch.head_oid(&repo)?,
            base_commit: current_base,
            push_status: branch_state.into(),
            last_updated_at: commits.first().map(|c| c.created_at),
            authors: authors.into_iter().collect(),
            is_conflicted,
            commits,
            upstream_commits,
            is_remote_head: false,
        });

        current_base = branch.head_oid(&repo)?;
    }

    stack.migrate_change_ids(ctx).ok(); // If it fails thats ok - best effort migration
    branch_details.reverse();

    let push_status = stack_state.into();

    Ok(ui::StackDetails {
        derived_name: stack.derived_name()?,
        push_status,
        branch_details,
        is_conflicted: stack_is_conflicted,
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

/// Represents a branch in a [`Stack`]. It contains commits derived from the local pseudo branch and it's respective remote
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// The name of the branch.
    #[serde(with = "gitbutler_serde::bstring_lossy")]
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    #[serde(with = "gitbutler_serde::bstring_opt_lossy")]
    pub remote_tracking_branch: Option<BString>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// Indicates that the branch was previously part of a stack but it has since been integrated.
    /// In other words, the merge base of the stack is now above this branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    /// An archived branch will not have any commits associated with it.
    pub archived: bool,
    /// This is the last commit in the branch, aka the tip of the branch.
    /// If this is the only branch in the stack or the top-most branch, this is the tip of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub tip: gix::ObjectId,
    /// This is the base commit from the perspective of this branch.
    /// If the branch is part of a stack and is on top of another branch, this is the head of the branch below it.
    /// If this branch is at the bottom of the stack, this is the merge base of the stack.
    #[serde(with = "gitbutler_serde::object_id")]
    pub base_commit: gix::ObjectId,
}

/// Returns the branches that belong to a particular [`gitbutler_stack::Stack`]
/// The entries are ordered from newest to oldest.
pub fn stack_branches(stack_id: String, ctx: &CommandContext) -> Result<Vec<Branch>> {
    let state = state_handle(&ctx.project().gb_dir());
    let remote = state
        .get_default_target()
        .context("failed to get default target")?
        .push_remote_name();

    let mut stack_branches = vec![];
    let mut stack = state.get_stack(Id::from_str(&stack_id)?)?;
    let mut current_base = stack.merge_base(ctx)?;
    let repo = ctx.gix_repo()?;
    for internal in stack.branches() {
        let upstream_reference = ctx
            .repo()
            .find_reference(&internal.remote_reference(remote.as_str()))
            .ok()
            .map(|_| internal.remote_reference(remote.as_str()));
        let result = Branch {
            name: internal.name().to_owned().into(),
            remote_tracking_branch: upstream_reference.map(Into::into),
            description: internal.description.clone(),
            pr_number: internal.pr_number,
            review_id: internal.review_id.clone(),
            archived: internal.archived,
            tip: internal.head_oid(&repo)?,
            base_commit: current_base,
        };
        current_base = internal.head_oid(&repo)?;
        stack_branches.push(result);
    }
    stack.migrate_change_ids(ctx).ok(); // If it fails thats ok - best effort migration
    stack_branches.reverse();
    Ok(stack_branches)
}

/// Returns a list of commits beloning to this branch. Ordered from newest to oldest (child-most to parent-most).
///
/// These are the commits that are currently part of the workspace (applied).
/// Created from the local pseudo branch (head currently stored in the TOML file)
///
/// When there is only one branch in the stack, this includes the commits
/// from the tip of the stack to the merge base with the trunk / target branch (not including the merge base).
///
/// When there are multiple branches in the stack, this includes the commits from the branch head to the next branch in the stack.
///
/// In either case, this is effectively a list of commits that in the working copy which may or may not have been pushed to the remote.
pub fn stack_branch_local_and_remote_commits(
    stack_id: String,
    branch_name: String,
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> Result<Vec<ui::Commit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let stack = state.get_stack(Id::from_str(&stack_id)?)?;

    let branches = stack.branches();
    let branch = branches
        .iter()
        .find(|b| b.name() == &branch_name)
        .ok_or_else(|| anyhow::anyhow!("Could not find branch {:?}", branch_name))?;
    if branch.archived {
        return Ok(vec![]);
    }
    local_and_remote_commits(ctx, repo, branch, &stack)
}

/// Returns a fift of commits belonging to this branch. Ordered from newest to oldest (child-most to parent-most).
///
/// These are the commits that exist **only** on the upstream branch. Ordered from newest to oldest.
/// Created from the tip of the local tracking branch eg. refs/remotes/origin/my-branch -> refs/heads/my-branch
///
/// This does **not** include the commits that are in the commits list (local)
/// This is effectively the list of commits that are on the remote branch but are not in the working copy.
pub fn stack_branch_upstream_only_commits(
    stack_id: String,
    branch_name: String,
    ctx: &CommandContext,
    repo: &gix::Repository,
) -> Result<Vec<ui::UpstreamCommit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let stack = state.get_stack(Id::from_str(&stack_id)?)?;

    let branches = stack.branches();
    let branch = branches
        .iter()
        .find(|b| b.name() == &branch_name)
        .with_context(|| format!("Could not find branch {branch_name:?}"))?;
    if branch.archived {
        return Ok(vec![]);
    }
    upstream_only_commits(ctx, repo, branch, &stack, None)
}

fn upstream_only_commits(
    ctx: &CommandContext,
    repo: &gix::Repository,
    stack_branch: &gitbutler_stack::StackBranch,
    stack: &Stack,
    current_local_and_remote_commits: Option<&Vec<ui::Commit>>,
) -> Result<Vec<ui::UpstreamCommit>> {
    let branch_commits = stack_branch.commits(ctx, stack)?;

    let local_and_remote = if let Some(current_local_and_remote) = current_local_and_remote_commits
    {
        current_local_and_remote
    } else {
        &local_and_remote_commits(ctx, repo, stack_branch, stack)?
    };

    // Upstream only
    let mut upstream_only = vec![];
    for commit in branch_commits.upstream_only.iter() {
        let matches_known_commit = local_and_remote.iter().any(|c| {
            // If the id matches verbatim or if there is a known remote_id (in the case of LocalAndRemote) that matchies
            c.id == commit.id().to_gix() || matches!(&c.state, CommitState::LocalAndRemote(remote_id) if remote_id == &commit.id().to_gix())
        });
        // Ignore commits that strictly speaking are remote only, but they match a known local commit (rebase etc)
        if !matches_known_commit {
            let created_at = u128::try_from(commit.time().seconds())? * 1000;
            let upstream_commit = ui::UpstreamCommit {
                id: commit.id().to_gix(),
                message: commit.message_bstr().into(),
                created_at,
                author: commit.author().into(),
            };
            upstream_only.push(upstream_commit);
        }
    }
    upstream_only.reverse();

    Ok(upstream_only)
}

fn local_and_remote_commits(
    ctx: &CommandContext,
    repo: &gix::Repository,
    stack_branch: &gitbutler_stack::StackBranch,
    stack: &Stack,
) -> Result<Vec<ui::Commit>> {
    let state = state_handle(&ctx.project().gb_dir());
    let default_target = state
        .get_default_target()
        .context("failed to get default target")?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut check_commit = IsCommitIntegrated::new(ctx, &default_target, repo, &mut graph)?;

    let branch_commits = stack_branch.commits(ctx, stack)?;
    let mut local_and_remote: Vec<ui::Commit> = vec![];
    let mut is_integrated = false;

    let remote_commit_data = branch_commits
        .remote_commits
        .iter()
        .filter_map(|commit| {
            let data = CommitData::try_from(commit).ok()?;
            Some((data, commit.id()))
        })
        .collect::<HashMap<_, _>>();

    // Local and remote
    // Reverse first instead of later, so that we catch the first integrated commit
    for commit in branch_commits.clone().local_commits.iter().rev() {
        if !is_integrated {
            is_integrated = check_commit.is_integrated(commit)?;
        }
        let copied_from_remote_id = CommitData::try_from(commit)
            .ok()
            .and_then(|data| remote_commit_data.get(&data).copied());

        let state = if is_integrated {
            CommitState::Integrated
        } else {
            // Can we find this as a remote commit by any of these options:
            // - the commit is copied from a remote commit
            // - the commit has an identical sha as the remote commit (the no brainer case)
            // - the commit has a change id that matches a remote commit
            if let Some(remote_id) = copied_from_remote_id {
                CommitState::LocalAndRemote(remote_id.to_gix())
            } else if let Some(remote_id) = branch_commits
                .remote_commits
                .iter()
                .find(|c| c.id() == commit.id() || c.change_id() == commit.change_id())
                .map(|c| c.id())
            {
                CommitState::LocalAndRemote(remote_id.to_gix())
            } else {
                CommitState::LocalOnly
            }
        };

        let created_at = u128::try_from(commit.time().seconds())? * 1000;

        let api_commit = ui::Commit {
            id: commit.id().to_gix(),
            parent_ids: commit.parents().map(|p| p.id().to_gix()).collect(),
            message: commit.message_bstr().into(),
            has_conflicts: commit.is_conflicted(),
            state,
            created_at,
            author: commit.author().into(),
        };
        local_and_remote.push(api_commit);
    }

    Ok(local_and_remote)
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

/// The commit-data we can use for comparison to see which remote-commit was used to craete
/// a local commit from.
/// Note that trees can't be used for comparison as these are typically rebased.
#[derive(Debug, Hash, Eq, PartialEq)]
pub(crate) struct CommitData {
    message: BString,
    author: gix::actor::Signature,
}

impl TryFrom<&git2::Commit<'_>> for CommitData {
    type Error = anyhow::Error;

    fn try_from(commit: &git2::Commit<'_>) -> std::result::Result<Self, Self::Error> {
        Ok(CommitData {
            message: commit.message_raw_bytes().into(),
            author: git2_signature_to_gix_signature(commit.author()),
        })
    }
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
