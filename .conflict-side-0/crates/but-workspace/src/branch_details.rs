use crate::head_info::function::workspace_data_of_default_workspace_branch;
use crate::ui::{CommitState, PushStatus};
use crate::{state_handle, ui};
use anyhow::{Context, bail};
use but_core::RefMetadata;
use gitbutler_command_context::CommandContext;
use gitbutler_error::error::Code;
use gitbutler_oxidize::OidExt;
use gix::remote::Direction;
use std::collections::HashSet;
use std::path::Path;

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

/// Returns information about the current state of a branch identified by its `name`.
/// This branch is assumed to not be in the workspace, but it will still be assumed to want to integrate with the workspace target reference if set.
///
/// ### Implementation
#[allow(unused_variables)]
pub fn branch_details_v3(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
    meta: &impl RefMetadata,
) -> anyhow::Result<ui::BranchDetails> {
    let integration_ref_name = workspace_data_of_default_workspace_branch(meta)?
        .context(
            "TODO: cannot run in non-workspace mode yet.\
        It would need a way to deal with limiting the commit traversal",
        )?
        .target_ref
        .context("TODO: a target to integrate with is currently needed for a workspace commit")?;
    let mut integration_ref = repo
        .find_reference(&integration_ref_name)
        .context("The branch to integrate with must be present")?;
    let integration_ref_target_id = integration_ref.peel_to_id_in_place()?;

    let mut branch = repo.find_reference(name)?;
    let branch_target_id = branch.peel_to_id_in_place()?;

    let mut remote_tracking_branch = repo
        .branch_remote_tracking_ref_name(name, Direction::Fetch)
        .transpose()?
        .and_then(|remote_tracking_ref| repo.find_reference(remote_tracking_ref.as_ref()).ok());
    let remote_tracking_target_id = remote_tracking_branch
        .as_mut()
        .map(|remote_ref| remote_ref.peel_to_id_in_place())
        .transpose()?;
    let push_status = remote_tracking_target_id
        .map(|remote_target_id| {
            if remote_target_id == branch_target_id {
                PushStatus::NothingToPush
            } else {
                PushStatus::UnpushedCommits
            }
        })
        .unwrap_or(PushStatus::CompletelyUnpushed);

    let meta = meta.branch(name)?;
    let meta: &but_core::ref_metadata::Branch = &meta;

    let base_commit = {
        let cache = repo.commit_graph_if_enabled()?;
        let mut graph = repo.revision_graph(cache.as_ref());
        let merge_bases = repo.merge_bases_many_with_graph(
            branch_target_id,
            &[integration_ref_target_id.detach()],
            &mut graph,
        )?;
        // TODO: have a test that shows why this must/should be last. Then maybe make it easy to do
        //       the right thing whenever the mergebase with the integration branch is needed.
        merge_bases.last().map(|id| id.detach())
    };

    todo!()
    // Ok(ui::BranchDetails {
    //     name: name.as_bstr().into(),
    //     remote_tracking_branch: remote_tracking_branch.map(|b| b.name().as_bstr().to_owned()),
    //     description: meta.description.clone(),
    //     pr_number: meta.review.pull_request,
    //     review_id: meta.review.review_id.clone(),
    //     base_commit: todo!(),
    //     push_status,
    //     last_updated_at: todo!(),
    //     authors: todo!(),
    //     is_conflicted: todo!(),
    //     commits: todo!(),
    //     upstream_commits: todo!(),
    //     tip: branch_target_id.detach(),
    //     is_remote_head: name.category() == Some(Category::RemoteBranch),
    // })
}
