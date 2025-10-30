use std::{collections::HashSet, path::Path};

use anyhow::{Context, bail};
use but_core::RefMetadata;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Code;
use gitbutler_oxidize::OidExt;
use gix::{date::parse::TimeBuf, prelude::ObjectIdExt, reference::Category, remote::Direction};
use itertools::Itertools;

use crate::{
    ref_info::function::workspace_data_of_default_workspace_branch,
    state_handle, ui,
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

/// Returns information about the current state of a branch identified by its `name`.
/// This branch is assumed to not be in the workspace, but it will still be assumed to want to integrate with the workspace target
/// reference if set.
/// Note that for stacks, we shouldn't call `stack_details_v3`, but instead [`head_info()`](crate::head_info()) to get all stacks
/// reachable from the current HEAD.
///
/// ### Implementation
///
/// Note that the following fields aren't computed or are only partially computed.
///
/// - `push_status` - `Integrated` variant is not computed for now (but it's conceivably doable later).
/// - `is_conflicted` - only local commits contribute.
// TODO: make use of `but-graph` here: traverse with `name` as entrypoint, maybe even try to cache the
//       graph and find its segment in there first before traversing unnecessarily.
pub fn branch_details_v3(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
    meta: &impl RefMetadata,
) -> anyhow::Result<ui::BranchDetails> {
    let integration_branch_name = workspace_data_of_default_workspace_branch(meta)?
        .context(
            "TODO: cannot run in non-workspace mode yet.\
        It would need a way to deal with limiting the commit traversal",
        )?
        .target_ref
        .context("TODO: a target to integrate with is currently needed for a workspace commit")?;
    let mut integration_branch = repo
        .find_reference(&integration_branch_name)
        .context("The branch to integrate with must be present")?;
    let integration_branch_id = integration_branch.peel_to_id()?;

    let mut branch = repo.find_reference(name)?;
    let branch_id = branch.peel_to_id()?;

    let mut remote_tracking_branch = repo
        .branch_remote_tracking_ref_name(name, Direction::Fetch)
        .transpose()?
        .and_then(|remote_tracking_ref| repo.find_reference(remote_tracking_ref.as_ref()).ok());
    let remote_tracking_branch_id = remote_tracking_branch
        .as_mut()
        .map(|r| r.peel_to_id())
        .transpose()?;

    let meta = meta.branch(name)?;
    let meta: &but_core::ref_metadata::Branch = &meta;

    let cache = repo.commit_graph_if_enabled()?;
    let mut graph = repo.revision_graph(cache.as_ref());
    let base_commit = {
        let merge_bases = repo.merge_bases_many_with_graph(
            branch_id,
            &[integration_branch_id.detach()],
            &mut graph,
        )?;
        // TODO: have a test that shows why this must/should be last. Then maybe make it easy to do
        //       the right thing whenever the mergebase with the integration branch is needed.
        merge_bases
            .last()
            .map(|id| id.detach())
            .unwrap_or_else(|| {
            tracing::warn!("No merge-base found between {name} and the integration branch {integration_branch_name}");
                // default to the tip just like the code previously did, resulting in no information
                // TODO: we should probably indicate that there is no merge-base instead of just glossing over it.
                branch_id.detach()
        })
    };

    let mut authors = HashSet::new();
    let (mut commits, upstream_commits) = {
        let commits = local_commits_gix(branch_id, integration_branch_id.detach(), &mut authors)?;

        let upstream_commits = if let Some(remote_tracking_branch) = remote_tracking_branch.as_mut()
        {
            let remote_id = remote_tracking_branch.peel_to_id()?;
            upstream_commits_gix(
                remote_id,
                integration_branch_id.detach(),
                branch_id.detach(),
                &mut authors,
            )?
        } else {
            Vec::new()
        };
        (commits, upstream_commits)
    };

    let is_remote_head = name.category() == Some(Category::RemoteBranch);
    let push_status = match remote_tracking_branch_id {
        Some(remote_tracking_branch_id) => {
            let merge_base = repo
                .merge_bases_many_with_graph(
                    branch_id,
                    &[remote_tracking_branch_id.detach()],
                    &mut graph,
                )?
                .first()
                .copied();
            if merge_base == Some(remote_tracking_branch_id) {
                if merge_base == Some(branch_id) {
                    PushStatus::NothingToPush
                } else {
                    PushStatus::UnpushedCommits
                }
            } else {
                PushStatus::UnpushedCommitsRequiringForce
            }
        }
        None => {
            if is_remote_head {
                // Make remotes appears neutral, like there is nothing to do.
                PushStatus::NothingToPush
            } else {
                // likely that no remote tracking branch existed in the first place.
                PushStatus::CompletelyUnpushed
            }
        }
    };

    Ok(ui::BranchDetails {
        name: name.as_bstr().into(),
        remote_tracking_branch: remote_tracking_branch.map(|b| b.name().as_bstr().to_owned()),
        description: meta.description.clone(),
        pr_number: meta.review.pull_request,
        review_id: meta.review.review_id.clone(),
        base_commit,
        last_updated_at: meta.ref_info.updated_at.map(|d| d.seconds as i128 * 1_000),
        authors: authors
            .into_iter()
            .sorted_by(|a, b| a.name.cmp(&b.name).then_with(|| a.email.cmp(&b.email)))
            .collect(),
        is_conflicted: compute_is_conflicted(
            repo,
            commits.iter_mut().map(|c| (c.id, &mut c.has_conflicts)),
        )?,
        commits,
        upstream_commits,
        tip: branch_id.detach(),
        is_remote_head,
        push_status,
    })
}

fn compute_is_conflicted<'a>(
    repo: &gix::Repository,
    commits_and_flags: impl Iterator<Item = (gix::ObjectId, &'a mut bool)>,
) -> anyhow::Result<bool> {
    let mut is_conflicted = false;
    for (id, flag) in commits_and_flags {
        *flag = but_core::Commit::from_id(id.attach(repo))?.is_conflicted();
        is_conflicted |= *flag;
    }
    Ok(is_conflicted)
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

/// Traverse all commits that are reachable from the first parent of `upstream_id`, but not in `integration_branch_id` nor in `branch_id`.
/// While at it, collect the commiter and author of each commit into `authors`.
fn upstream_commits_gix(
    upstream_id: gix::Id<'_>,
    integration_branch_id: gix::ObjectId,
    branch_id: gix::ObjectId,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<UpstreamCommit>> {
    let revwalk = upstream_id
        .ancestors()
        .with_hidden([branch_id, integration_branch_id])
        .first_parent_only()
        .all()?;
    let mut out = Vec::new();
    for info in revwalk {
        let info = info?;
        let commit = info.id().object()?.into_commit();
        let commit = commit.decode()?;
        let author: ui::Author = commit.author().into();
        let commiter: ui::Author = commit.committer().into();
        authors.insert(author.clone());
        authors.insert(commiter);
        out.push(UpstreamCommit {
            id: info.id,
            message: commit.message.into(),
            created_at: i128::from(commit.time().seconds) * 1000,
            author,
        });
    }
    Ok(out)
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

/// Traverse all commits that are reachable from the first parent of `branch_id`, but not in `integration_branch`, and store all
/// commit authors and committers in `authors` while at it.
fn local_commits_gix(
    branch_id: gix::Id<'_>,
    integration_branch_id: gix::ObjectId,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<ui::Commit>> {
    let revwalk = branch_id
        .ancestors()
        .with_hidden([integration_branch_id])
        .first_parent_only()
        .all()?;

    let mut out = Vec::new();
    for info in revwalk {
        let info = info?;
        let commit = but_core::Commit::from_id(info.id())?;

        let mut buf = TimeBuf::default();
        let author: ui::Author = commit.author.to_ref(&mut buf).into();
        let commiter: ui::Author = commit.committer.to_ref(&mut buf).into();
        authors.insert(author.clone());
        authors.insert(commiter);
        out.push(ui::Commit {
            id: info.id,
            parent_ids: commit.parents.iter().cloned().collect(),
            message: commit.message.clone(),
            has_conflicts: commit.is_conflicted(),
            state: CommitState::LocalAndRemote(info.id),
            created_at: i128::from(commit.committer.time.seconds) * 1000,
            author,
            gerrit_review_url: None,
        });
    }
    Ok(out)
}
