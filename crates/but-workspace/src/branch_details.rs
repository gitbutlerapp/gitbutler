use std::collections::HashSet;

use crate::{
    ref_info::function::workspace_data_of_default_workspace_branch,
    ui,
    ui::{CommitState, PushStatus, UpstreamCommit},
};
use anyhow::Context;
use but_core::RefMetadata;
use but_oxidize::{ObjectIdExt as _, OidExt};
use gix::{
    date::parse::TimeBuf, prelude::ObjectIdExt as _, reference::Category, remote::Direction,
};
use itertools::Itertools;

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
pub fn branch_details(
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
        linked_worktree_id: None, /* probably not needed here */
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
/// TODO: can we use the Graph for this?
fn upstream_commits_gix(
    upstream_id: gix::Id<'_>,
    integration_branch_id: gix::ObjectId,
    branch_id: gix::ObjectId,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<UpstreamCommit>> {
    let git2_repo = git2::Repository::open(upstream_id.repo.path())?;
    let mut revwalk = git2_repo.revwalk()?;
    revwalk.simplify_first_parent()?;
    revwalk.push(upstream_id.to_git2())?;
    revwalk.hide(branch_id.to_git2())?;
    revwalk.hide(integration_branch_id.to_git2())?;

    let mut out = Vec::new();
    for id in revwalk {
        let id = id?.to_gix().attach(upstream_id.repo);
        let commit = id.object()?.into_commit();
        let commit = commit.decode()?;
        let author: ui::Author = commit.author().into();
        let commiter: ui::Author = commit.committer().into();
        authors.insert(author.clone());
        authors.insert(commiter);
        out.push(UpstreamCommit {
            id: id.detach(),
            message: commit.message.into(),
            created_at: i128::from(commit.time().seconds) * 1000,
            author,
        });
    }
    Ok(out)
}

/// Traverse all commits that are reachable from the first parent of `branch_id`, but not in `integration_branch`, and store all
/// commit authors and committers in `authors` while at it.
fn local_commits_gix(
    branch_id: gix::Id<'_>,
    integration_branch_id: gix::ObjectId,
    authors: &mut HashSet<ui::Author>,
) -> anyhow::Result<Vec<ui::Commit>> {
    // TODO(gix): make this work in `gix` or use the Graph traversal for this.
    let git2_repo = git2::Repository::open(branch_id.repo.path())?;
    let mut revwalk = git2_repo.revwalk()?;
    revwalk.push(branch_id.to_git2())?;
    revwalk.hide(integration_branch_id.to_git2())?;
    revwalk.simplify_first_parent()?;

    let mut out = Vec::new();
    for id in revwalk {
        let id = id?.to_gix().attach(branch_id.repo);
        let commit = but_core::Commit::from_id(id)?;

        let mut buf = TimeBuf::default();
        let author: ui::Author = commit.author.to_ref(&mut buf).into();
        let commiter: ui::Author = commit.committer.to_ref(&mut buf).into();
        authors.insert(author.clone());
        authors.insert(commiter);
        out.push(ui::Commit {
            id: id.detach(),
            parent_ids: commit.parents.iter().cloned().collect(),
            message: commit.message.clone(),
            has_conflicts: commit.is_conflicted(),
            state: CommitState::LocalAndRemote(id.detach()),
            created_at: i128::from(commit.committer.time.seconds) * 1000,
            author,
            gerrit_review_url: None,
        });
    }
    Ok(out)
}

/// Returns all local commits for the branch identified by `branch_id` that are not in `integration_branch_id`.
pub fn local_commits_for_branch(
    branch_id: gix::Id<'_>,
    integration_branch_id: gix::ObjectId,
) -> anyhow::Result<Vec<ui::Commit>> {
    let mut authors = HashSet::new();
    local_commits_gix(branch_id, integration_branch_id, &mut authors)
}
