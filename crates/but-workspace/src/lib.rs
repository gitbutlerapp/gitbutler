#![deny(missing_docs, rust_2018_idioms)]
#![deny(clippy::indexing_slicing)]

//! ### Terminology
//!
//! * **Workspace**
//!     - A GitButler concept of the combination of one or more branches into one worktree. This allows
//!       multiple branches to be perceived in one worktree, by merging multiple branches together.
//!     - Currently, there is only one workspace per repository, but this is something we intend to change
//!       in the future to facilitate new use cases.
//!
//! * **Stack**
//!   - GitButler implements the concept of a branch stack. This is essentially a collection of "heads"
//!     (pseudo branches) that contain each other.
//!   - Always contains at least one branch.
//!   - High level documentation here: https://docs.gitbutler.com/features/stacked-branches
//!

use anyhow::{Context, Result};
use bstr::BString;
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt;
use gitbutler_id::id::Id;
use gitbutler_oxidize::OidExt;
use gitbutler_stack::stack_context::CommandContextExt;
use gitbutler_stack::{stack_context::StackContext, Stack, Target, VirtualBranchesHandle};
use gix::ObjectId;
use integrated::IsCommitIntegrated;
use itertools::Itertools;
use std::path::Path;
use std::str::FromStr;

mod integrated;

/// Represents a lightweight version of a [`gitbutler_stack::Stack`] for listing.
#[derive(Debug, Clone)]
pub struct StackEntry {
    /// The ID of the stack.
    pub id: Id<gitbutler_stack::Stack>,
    /// The list of the branch names that are part of the stack.
    /// The list is never empty.
    /// The first entry in the list is always the most recent branch on top the stack.
    pub branch_names: Vec<BString>,
}

/// Returns the list of stacks that are currently part of the workspace.
/// If there are no applied stacks, the returned Vec is empty.
/// If the GitButler state file in the provided path is missing or invalid, an error is returned.
///
/// - `gb_dir`: The path to the GitButler state for the project. Normally this is `.git/gitbutler` in the project's repository.
pub fn stacks(gb_dir: &Path) -> Result<Vec<StackEntry>> {
    let state = state_handle(gb_dir);
    Ok(state
        .list_stacks_in_workspace()?
        .into_iter()
        .sorted_by_key(|s| s.order)
        .map(|stack| StackEntry {
            id: stack.id,
            branch_names: stack.heads().into_iter().map(Into::into).collect(),
        })
        .collect())
}

/// Represents the state a commit could be in.
#[derive(Debug, Clone)]
pub enum CommitState {
    /// The commit is only local
    LocalOnly,
    /// The commit is also present at the remote tracking branch.
    /// This is the commit state if:
    ///  - The commit has been pushed to the remote
    ///  - The commit has been copied from a remote commit (when applying a remote branch)
    ///
    /// This variant carries the remote commit id.
    /// The `remote_commit_id` may be the same as the `id` or it may be different if the local commit has been rebased or updated in another way.
    LocalAndRemote(gix::ObjectId),
    /// The commit is considered integrated.
    /// This should happen when this commit or the contents of this commit is already part of the base.
    Integrated,
}

/// Commit that is a part of a [`StackBranch`] and, as such, containing state derived in relation to the specific branch.
#[derive(Debug, Clone)]
pub struct Commit {
    /// The OID of the commit.
    pub id: gix::ObjectId,
    /// The message of the commit.
    pub message: BString,
    /// Whether the commit is in a conflicted state.
    /// Conflicted state of a commit is a GitButler concept.
    /// GitButler will perform rebasing/reordering etc without interruptions and flag commits as conflicted if needed.
    /// Conflicts are resolved via the Edit Mode mechanism.
    pub has_conflicts: bool,
    /// Represents wether the the commit is considered integrated, local only,
    /// or local and remote with respect to the branch it belongs to.
    /// Note that remote only commits in the context of a branch are expressed with the [`UpstreamCommit`] struct instead of this.
    pub state: CommitState,
}

/// Commit that is only at the remote.
/// Unlike the `Commit` struct, there is no knowledge of GitButler concepts like conflicted state etc.
#[derive(Debug, Clone)]
pub struct UpstreamCommit {
    /// The OID of the commit.
    pub id: gix::ObjectId,
    /// The message of the commit.
    pub message: BString,
}

impl Commit {
    fn matches(&self, id: ObjectId) -> bool {
        self.id == id
            || match self.state {
                CommitState::LocalAndRemote(remote_commit_id) => remote_commit_id == id,
                _ => false,
            }
    }
}

/// Replesents a branch in a [`gitbutler_stack::Stack`]. It contains commits derived from the local pseudo branch and it's respective remote
#[derive(Debug, Clone)]
pub struct Branch {
    /// The name of the branch.
    pub name: BString,
    /// Upstream reference, e.g. `refs/remotes/origin/base-branch-improvements`
    pub remote_tracking_branch: Option<BString>,
    /// List of commits beloning to this branch. Ordered from newest to oldest.
    /// Created from the local pseudo branch (head currently stored in the TOML file)
    /// This includes the commits from the tip of the stack to the merge base with the trunk / target branch (not including the merge base).
    pub commits: Vec<Commit>,
    /// List of commits that exist **only** on the upstream branch. Ordered from newest to oldest.
    /// Created from the tip of the local tracking branch eg. refs/remotes/origin/my-branch -> refs/heads/my-branch
    /// This does **not** include the commits that are in the commits list (local)
    pub upstream_commits: Vec<UpstreamCommit>,
    /// Description of the branch.
    /// Can include arbitrary utf8 data, eg. markdown etc.
    pub description: Option<String>,
    /// The pull(merge) request associated with the branch, or None if no such entity has not been created.
    pub pr_number: Option<usize>,
    /// A unique identifier for the GitButler review associated with the branch, if any.
    pub review_id: Option<String>,
    /// Archived represents the state when series/branch has been integrated and is below the merge base of the branch.
    /// This would occur when the branch has been merged at the remote and the workspace has been updated with that change.
    pub archived: bool,
}

/// Provides the relevant details of a particular [`gitbutler_stack::Stack`]
/// The entries are ordered from newest to oldest.
pub fn stack_branches(stack_id: String, ctx: &CommandContext) -> Result<Vec<Branch>> {
    let state = state_handle(&ctx.project().gb_dir());
    let default_target = state
        .get_default_target()
        .context("failed to get default target")?;
    let stack_ctx = &ctx.to_stack_context()?;

    let repo = ctx.gix_repository()?;
    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let mut check_commit = IsCommitIntegrated::new(ctx, &default_target, &repo, &mut graph)?;

    let mut stack_branches = vec![];
    let stack = state.get_stack(Id::from_str(&stack_id)?)?;
    for internal in stack.branches() {
        let result = convert(
            stack_ctx,
            internal,
            &stack,
            &default_target,
            &mut check_commit,
            stack_branches.clone(),
        )?;
        stack_branches.push(result);
    }
    stack_branches.reverse();
    Ok(stack_branches)
}

fn convert(
    ctx: &StackContext<'_>,
    stack_branch: gitbutler_stack::StackBranch,
    stack: &Stack,
    default_target: &Target,
    check_commit: &mut IsCommitIntegrated<'_, '_, '_>,
    parent_series: Vec<Branch>,
) -> Result<Branch> {
    let branch_commits = stack_branch.commits(ctx, stack)?;
    let remote = default_target.push_remote_name();
    let mut patches: Vec<Commit> = vec![];
    let mut is_integrated = false;

    // Reverse first instead of later, so that we catch the first integrated commit
    for commit in branch_commits.clone().local_commits.iter().rev() {
        if !is_integrated {
            is_integrated = check_commit.is_integrated(commit)?;
        }
        let api_commit = Commit {
            id: commit.id().to_gix(),
            message: commit.message_bstr().into(),
            has_conflicts: commit.is_conflicted(),
            state: CommitState::LocalOnly, // TODO: implement this
        };
        patches.push(api_commit);
    }
    // There should be no duplicates, but dedup just in case
    patches.dedup_by(|a, b| a.id == b.id);

    let mut upstream_patches = vec![];
    for commit in branch_commits.remote_commits.iter().rev() {
        if patches.iter().any(|p| p.matches(commit.id().to_gix())) {
            // Skip if we already have this commit in the list
            continue;
        }

        if parent_series.iter().any(|series| {
            if series.archived {
                return false;
            };

            series
                .commits
                .iter()
                .any(|p| p.matches(commit.id().to_gix()))
        }) {
            // Skip if we already have this commit in the list
            continue;
        }

        let upstream_commit = UpstreamCommit {
            id: commit.id().to_gix(),
            message: commit.message_bstr().into(),
        };
        upstream_patches.push(upstream_commit);
    }
    upstream_patches.reverse();
    // There should be no duplicates, but dedup just in case
    upstream_patches.dedup_by(|a, b| a.id == b.id);

    let upstream_reference = ctx
        .repository()
        .find_reference(&stack_branch.remote_reference(remote.as_str()))
        .ok()
        .map(|_| stack_branch.remote_reference(remote.as_str()));
    Ok(Branch {
        name: stack_branch.name.into(),
        remote_tracking_branch: upstream_reference.map(Into::into),
        commits: patches,
        upstream_commits: upstream_patches,
        description: stack_branch.description.map(Into::into),
        pr_number: stack_branch.pr_number,
        review_id: stack_branch.review_id,
        archived: stack_branch.archived,
    })
}

fn state_handle(gb_state_path: &Path) -> VirtualBranchesHandle {
    VirtualBranchesHandle::new(gb_state_path)
}
