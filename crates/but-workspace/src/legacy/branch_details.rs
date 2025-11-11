use std::{collections::HashSet, path::Path};

use anyhow::{Context, bail};
use but_error::Code;
use but_oxidize::OidExt;
use gitbutler_command_context::CommandContext;

use crate::{
    legacy::state_handle,
    ui,
    ui::{CommitState, PushStatus, UpstreamCommit},
};

/// Returns information about the current state of a branch.
pub fn branch_details(
    gb_dir: &Path,
    branch_name: &str,
    remote: Option<&str>,
    ctx: &CommandContext,
) -> anyhow::Result<ui::BranchDetails> {
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
        None => {
            // The branch can be remote even if we dont have the upstream set
            if is_remote_head {
                PushStatus::NothingToPush
            } else {
                PushStatus::CompletelyUnpushed
            }
        }
    };

    let merge_bases = repository.merge_bases(branch_oid, default_target.sha)?;
    let Some(base_commit) = merge_bases.last() else {
        bail!("Failed to find merge base");
    };

    let mut authors = HashSet::new();
    let commits = local_commits(repository, default_target.sha, branch_oid, &mut authors)?;
    let upstream_commits = upstream_oid
        .map(|upstream_oid| {
            upstream_commits(
                repository,
                upstream_oid,
                default_target.sha,
                branch_oid,
                &mut authors,
            )
        })
        .transpose()?
        .unwrap_or_default();

    Ok(ui::BranchDetails {
        name: branch_name.into(),
        linked_worktree_id: None, /* not implemented in legacy mode */
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

/// Traverse all commits that are reachable from the first parent of `upstream_id`, but not in `integration_branch_id` nor in `branch_id`.
/// While at it, collect the commiter and author of each commit into `authors`.
fn upstream_commits(
    repository: &git2::Repository,
    upstream_id: git2::Oid,
    integration_branch_id: git2::Oid,
    branch_id: git2::Oid,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<UpstreamCommit>> {
    let mut revwalk = repository.revwalk()?;
    revwalk.push(upstream_id)?;
    revwalk.hide(branch_id)?;
    revwalk.hide(integration_branch_id)?;
    revwalk.simplify_first_parent()?;
    Ok(revwalk
        .filter_map(Result::ok)
        .filter_map(|oid| repository.find_commit(oid).ok())
        .map(|commit| {
            let author: ui::Author = commit.author().into();
            let commiter: ui::Author = commit.committer().into();
            authors.insert(author.clone());
            authors.insert(commiter);
            UpstreamCommit {
                id: commit.id().to_gix(),
                message: commit.message().unwrap_or_default().into(),
                created_at: i128::from(commit.time().seconds()) * 1000,
                author,
            }
        })
        .collect())
}

/// Traverse all commits that are reachable from the first parent of `branch_id`, but not in `integration_branch`, and store all
/// commit authors and committers in `authors` while at it.
fn local_commits(
    repository: &git2::Repository,
    integration_branch_id: git2::Oid,
    branch_id: git2::Oid,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<ui::Commit>> {
    let mut revwalk = repository.revwalk()?;
    revwalk.push(branch_id)?;
    revwalk.hide(integration_branch_id)?;
    revwalk.simplify_first_parent()?;

    Ok(revwalk
        .filter_map(Result::ok)
        .filter_map(|oid| repository.find_commit(oid).ok())
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
                created_at: i128::from(commit.time().seconds()) * 1000,
                author,
                gerrit_review_url: None,
            }
        })
        .collect())
}
