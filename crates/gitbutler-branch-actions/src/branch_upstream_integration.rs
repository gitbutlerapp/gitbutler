use std::borrow::Cow;

use anyhow::{anyhow, Context, Result};
use gitbutler_branch::{Branch, BranchId};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::commit_ext::CommitExt as _;
use gitbutler_error::error::Marker;
use gitbutler_repo::{rebase::cherry_rebase_group, RepoActionsExt as _, RepositoryExt as _};

use crate::{conflicts, integration::get_workspace_head, VirtualBranchesExt as _};

/// Integrates upstream work from a remote branch.
///
/// First we determine strategy based on preferences and branch state. If you
/// have allowed force push then it is likely branch commits frequently get
/// rebased, meaning we want to cherry pick new upstream work onto our rebased
/// commits.
///
/// If your local branch has been rebased, but you have new local only commits,
/// we _must_ rebase the upstream commits on top of the last rebased commit. We
/// do this to avoid duplicate commits, but we then need to let the user decide
/// if the local only commits get rebased on top of new upstream work or merged
/// with the new commits. The latter is sometimes preferable because you have
/// at most one merge conflict to resolve, while rebasing requires a multi-step
/// interactive process (currently not supported, so we abort).
///
/// If you do not allow force push then first validate the remote branch and
/// your local branch have the same merge base. A different merge base means
/// means either you or the remote branch has been rebased, and merging the
/// two would introduce duplicate commits (same changes, different hash).
///
/// Additionally, if we succeed in integrating the upstream commit, we still
/// need to merge the new branch tree with the working directory tree. This
/// might introduce more conflicts, but there is no need to commit at the
/// end since there will only be one parent commit.
///
pub fn integrate_upstream_commits(ctx: &CommandContext, branch_id: BranchId) -> Result<()> {
    conflicts::is_conflicting(ctx, None)?;

    let repo = ctx.repository();
    let project = ctx.project();
    let vb_state = project.virtual_branches();

    let mut branch = vb_state.get_branch_in_workspace(branch_id)?;
    let default_target = vb_state.get_default_target()?;

    let upstream_branch = branch.upstream.as_ref().context("upstream not found")?;
    let upstream_oid = repo.refname_to_id(&upstream_branch.to_string())?;
    let upstream_commit = repo.find_commit(upstream_oid)?;

    if upstream_commit.id() == branch.head() {
        return Ok(());
    }

    let upstream_commits = repo.list_commits(upstream_commit.id(), default_target.sha)?;
    let branch_commits = repo.list_commits(branch.head(), default_target.sha)?;

    let branch_commit_ids = branch_commits.iter().map(|c| c.id()).collect::<Vec<_>>();

    let branch_change_ids = branch_commits
        .iter()
        .filter_map(|c| c.change_id())
        .collect::<Vec<_>>();

    let mut unknown_commits: Vec<git2::Oid> = upstream_commits
        .iter()
        .filter(|c| {
            (!c.change_id()
                .is_some_and(|cid| branch_change_ids.contains(&cid)))
                && !branch_commit_ids.contains(&c.id())
        })
        .map(|c| c.id())
        .collect::<Vec<_>>();

    let rebased_commits = upstream_commits
        .iter()
        .filter(|c| {
            c.change_id()
                .is_some_and(|cid| branch_change_ids.contains(&cid))
                && !branch_commit_ids.contains(&c.id())
        })
        .map(|c| c.id())
        .collect::<Vec<_>>();

    // If there are no new commits then there is nothing to do.
    if unknown_commits.is_empty() {
        return Ok(());
    };

    let merge_base = repo.merge_base(default_target.sha, upstream_oid)?;

    // Booleans needed for a decision on how integrate upstream commits.
    // let is_same_base = default_target.sha == merge_base;
    let can_use_force = branch.allow_rebasing;
    let has_rebased_commits = !rebased_commits.is_empty();

    // We can't proceed if we rebased local commits but no permission to force push. In this
    // scenario we would need to "cherry rebase" new upstream commits onto the last rebased
    // local commit.
    if has_rebased_commits && !can_use_force {
        return Err(anyhow!("Cannot merge rebased commits without force push")
            .context("Aborted because force push is disallowed and commits have been rebased")
            .context(Marker::ProjectConflict));
    }

    let integration_result = match can_use_force {
        true => integrate_with_rebase(ctx, &mut branch, &mut unknown_commits),
        false => {
            if has_rebased_commits {
                return Err(anyhow!("Cannot merge rebased commits without force push")
                    .context(
                        "Aborted because force push is disallowed and commits have been rebased",
                    )
                    .context(Marker::ProjectConflict));
            }
            integrate_with_merge(ctx, &mut branch, &upstream_commit, merge_base).map(Into::into)
        }
    };

    if integration_result.as_ref().err().map_or(false, |err| {
        err.downcast_ref()
            .is_some_and(|marker: &Marker| *marker == Marker::ProjectConflict)
    }) {
        return Ok(());
    };

    let new_head = integration_result?;
    let new_head_tree = repo.find_commit(new_head)?.tree()?;
    let head_commit = repo.find_commit(new_head)?;

    let wd_tree = ctx.repository().create_wd_tree()?;
    let workspace_tree = repo.find_commit(get_workspace_head(ctx)?)?.tree()?;

    let mut merge_index = repo.merge_trees(&workspace_tree, &new_head_tree, &wd_tree, None)?;

    if merge_index.has_conflicts() {
        repo.checkout_index_builder(&mut merge_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()?;
    } else {
        branch.set_head(new_head);
        branch.tree = head_commit.tree()?.id();
        vb_state.set_branch(branch.clone())?;
        repo.checkout_index_builder(&mut merge_index)
            .force()
            .checkout()?;
    };

    crate::integration::update_workspace_commit(&vb_state, ctx)?;
    Ok(())
}

fn integrate_with_rebase(
    ctx: &CommandContext,
    branch: &mut Branch,
    unknown_commits: &mut Vec<git2::Oid>,
) -> Result<git2::Oid> {
    cherry_rebase_group(
        ctx.repository(),
        branch.head(),
        unknown_commits.as_mut_slice(),
    )
}

fn integrate_with_merge(
    ctx: &CommandContext,
    branch: &mut Branch,
    upstream_commit: &git2::Commit,
    merge_base: git2::Oid,
) -> Result<git2::Oid> {
    let wd_tree = ctx.repository().create_wd_tree()?;
    let repo = ctx.repository();
    let remote_tree = upstream_commit.tree().context("failed to get tree")?;
    let upstream_branch = branch.upstream.as_ref().context("upstream not found")?;
    // let merge_tree = repo.find_commit(merge_base).and_then(|c| c.tree())?;
    let merge_tree = repo.find_commit(merge_base)?;
    let merge_tree = merge_tree.tree()?;

    let mut merge_index = repo.merge_trees(&merge_tree, &wd_tree, &remote_tree, None)?;

    if merge_index.has_conflicts() {
        let conflicts = merge_index.conflicts()?;
        let merge_conflicts = conflicts
            .flatten()
            .filter_map(|c| c.our)
            .map(|our| gix::path::try_from_bstr(Cow::Owned(our.path.into())))
            .collect::<Result<Vec<_>, _>>()?;
        conflicts::mark(ctx, merge_conflicts, Some(upstream_commit.id()))?;
        repo.checkout_index_builder(&mut merge_index)
            .allow_conflicts()
            .conflict_style_merge()
            .force()
            .checkout()?;
        return Err(anyhow!("merge problem")).context(Marker::ProjectConflict);
    }

    let merge_tree_oid = merge_index.write_tree_to(ctx.repository())?;
    let merge_tree = repo.find_tree(merge_tree_oid)?;
    let head_commit = repo.find_commit(branch.head())?;

    ctx.commit(
        format!(
            "Merged {}/{} into {}",
            upstream_branch.remote(),
            upstream_branch.branch(),
            branch.name
        )
        .as_str(),
        &merge_tree,
        &[&head_commit, upstream_commit],
        None,
    )
}
