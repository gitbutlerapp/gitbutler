use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use bstr::ByteSlice;
use but_workspace::stack_ext::StackExt;
use git2::build::CheckoutBuilder;
use gitbutler_branch_actions::internal::list_virtual_branches;
use gitbutler_branch_actions::{update_workspace_commit, RemoteBranchFile};
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt as _};
use gitbutler_command_context::{gix_repo_for_merging, CommandContext};
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use gitbutler_diff::hunks_by_filepath;
use gitbutler_operating_modes::{
    operating_mode, read_edit_mode_metadata, write_edit_mode_metadata, EditModeMetadata,
    OperatingMode, EDIT_BRANCH_REF, WORKSPACE_BRANCH_REF,
};
use gitbutler_oxidize::{
    git2_to_gix_object_id, gix_to_git2_index, GixRepositoryExt, ObjectIdExt, OidExt, RepoExt,
};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_reference::{ReferenceName, Refname};
use gitbutler_repo::RepositoryExt;
use gitbutler_repo::{signature, SignaturePurpose};
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use gitbutler_workspace::branch_trees::{update_uncommited_changes_with_tree, WorkspaceState};
#[allow(deprecated)]
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head};
use serde::Serialize;

pub mod commands;

const UNCOMMITED_CHANGES_REF: &str = "refs/heads/gitbutler/edit-uncommited-changes";

/// Returns an index of the the tree of `commit` if it is unconflicted, *or* produce a merged tree
/// if `commit` is conflicted. That tree is turned into an index that records the conflicts that occurred
/// during the merge.
fn get_commit_index(repository: &git2::Repository, commit: &git2::Commit) -> Result<git2::Index> {
    let commit_tree = commit.tree().context("Failed to get commit's tree")?;
    // Checkout the commit as unstaged changes
    if commit.is_conflicted() {
        let base = commit_tree
            .get_name(".conflict-base-0")
            .context("Failed to get base")?
            .id();
        let ours = commit_tree
            .get_name(".conflict-side-0")
            .context("Failed to get base")?
            .id();
        let theirs = commit_tree
            .get_name(".conflict-side-1")
            .context("Failed to get base")?
            .id();

        let gix_repo = gix_repo_for_merging(repository.path())?;
        // Merge without favoring a side this time to get a tree containing the actual conflicts.
        let mut merge_result = gix_repo.merge_trees(
            git2_to_gix_object_id(base),
            git2_to_gix_object_id(ours),
            git2_to_gix_object_id(theirs),
            gix_repo.default_merge_labels(),
            gix_repo.tree_merge_options()?,
        )?;
        let merged_tree_id = merge_result.tree.write()?;
        let mut index = gix_repo.index_from_tree(&merged_tree_id)?;
        if !merge_result.index_changed_after_applying_conflicts(
            &mut index,
            gix::merge::tree::TreatAsUnresolved::git(),
            gix::merge::tree::apply_index_entries::RemovalMode::Mark,
        ) {
            tracing::warn!("There must be an issue with conflict-commit creation as re-merging the conflicting trees didn't yield a conflicting index.");
        }
        gix_to_git2_index(&index)
    } else {
        let mut index = git2::Index::new()?;
        index.read_tree(&commit_tree)?;
        Ok(index)
    }
}

/// Returns a commit to be the HEAD of `gitbutler/edit`
///
/// This should a commit who's tree is what the commit getting edited
/// (the editee) is based on.
///
/// If the editee is conflicted:
/// We should checkout `.conflict-side-0`. This is because the resulting merge
/// is always based on top of `.conflict-side-0`, so this is the preferable
/// base.
///
/// If the parent is conflicted:
/// We should checkout the parent's `.auto-resolution` because that is what
/// the editee is based on
///
/// Otherwise:
/// We can simply return the parent commit.
fn find_or_create_base_commit<'a>(
    repository: &'a git2::Repository,
    commit: &git2::Commit<'a>,
) -> Result<git2::Commit<'a>> {
    let is_conflicted = commit.is_conflicted();
    let is_parent_conflicted = commit.parent(0)?.is_conflicted();

    // If neither is conflicted, we can use the old parent.
    if !(is_conflicted || is_parent_conflicted) {
        return Ok(commit.parent(0)?);
    };

    let base_tree = if is_conflicted {
        repository.find_real_tree(commit, ConflictedTreeKey::Ours)?
    } else {
        let parent = commit.parent(0)?;
        repository.find_real_tree(&parent, ConflictedTreeKey::AutoResolution)?
    };

    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;
    let base = repository.commit(
        None,
        &author_signature,
        &committer_signature,
        "Conflict base",
        &base_tree,
        &[],
    )?;

    Ok(repository.find_commit(base)?)
}

fn commit_uncommited_changes(ctx: &CommandContext, parent: git2::Oid) -> Result<()> {
    let repository = ctx.repo();
    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;
    let parent = repository.find_commit(parent)?;

    let uncommited_changes = repository.create_wd_tree(0)?;
    let uncommited_changes_commit = repository.commit(
        None,
        &author_signature,
        &committer_signature,
        "Conflict base",
        &uncommited_changes,
        &[&parent],
    )?;

    repository.reference(UNCOMMITED_CHANGES_REF, uncommited_changes_commit, true, "")?;
    Ok(())
}

fn get_uncommited_changes(ctx: &CommandContext) -> Result<git2::Oid> {
    let repository = ctx.repo();
    let uncommited_changes = repository
        .find_reference(UNCOMMITED_CHANGES_REF)?
        .peel_to_tree()?
        .id();
    Ok(uncommited_changes)
}

fn checkout_edit_branch(ctx: &CommandContext, commit: git2::Commit) -> Result<()> {
    let repository = ctx.repo();

    // Checkout commits's parent
    let commit_parent = find_or_create_base_commit(repository, &commit)?;
    repository.reference(EDIT_BRANCH_REF, commit_parent.id(), true, "")?;
    repository.set_head(EDIT_BRANCH_REF)?;
    repository.checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))?;

    // Checkout the commit as unstaged changes
    let mut index = get_commit_index(repository, &commit)?;

    repository.checkout_index(
        Some(&mut index),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    Ok(())
}

fn find_virtual_branch_by_reference(
    ctx: &CommandContext,
    reference: &ReferenceName,
) -> Result<Option<Stack>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let all_stacks = vb_state
        .list_stacks_in_workspace()
        .context("Failed to read virtual branches")?;

    Ok(all_stacks.into_iter().find(|virtual_branch| {
        let Ok(refname) = virtual_branch.refname() else {
            return false;
        };

        let Ok(checked_out_refname) = Refname::from_str(reference) else {
            return false;
        };

        checked_out_refname == refname.into()
    }))
}

pub(crate) fn enter_edit_mode(
    ctx: &CommandContext,
    commit: git2::Commit,
    branch: &git2::Reference,
    _perm: &mut WorktreeWritePermission,
) -> Result<EditModeMetadata> {
    let Some(branch_reference) = branch.name() else {
        bail!("Failed to get branch reference name");
    };

    let edit_mode_metadata = EditModeMetadata {
        commit_oid: commit.id(),
        branch_reference: branch_reference.to_string().into(),
    };

    if find_virtual_branch_by_reference(ctx, &edit_mode_metadata.branch_reference)?.is_none() {
        bail!("Can not enter edit mode for a reference which does not have a cooresponding virtual branch")
    }

    commit_uncommited_changes(ctx, commit.id())?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit).context("Failed to checkout edit branch")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &CommandContext,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = ctx.repo();

    // Checkout gitbutler workspace branch
    repository
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    let uncommited_changes = get_uncommited_changes(ctx)?;
    let uncommited_changes = repository.find_tree(uncommited_changes)?;

    repository.checkout_tree(
        uncommited_changes.as_object(),
        Some(CheckoutBuilder::new().force().remove_untracked(true)),
    )?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repository = ctx.repo();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    // Get important references
    let commit = repository
        .find_commit(edit_mode_metadata.commit_oid)
        .context("Failed to find commit")?;

    let Some(mut stack) =
        find_virtual_branch_by_reference(ctx, &edit_mode_metadata.branch_reference)?
    else {
        bail!("Failed to find virtual branch for this reference. Entering and leaving edit mode for non-virtual branches is unsupported")
    };

    let parents = commit.parents().collect::<Vec<_>>();

    // Write out all the changes, including unstaged changes to a tree for re-committing
    let tree = repository.create_wd_tree(0)?;

    let (_, committer) = repository.signatures()?;
    let commit_headers = commit
        .gitbutler_headers()
        .map(|commit_headers| CommitHeadersV2 {
            conflicted: None,
            ..commit_headers
        });
    let new_commit_oid = ctx
        .repo()
        .commit_with_signature(
            None,
            &commit.author(),
            &committer, // Use a new committer
            &commit.message_bstr().to_str_lossy(),
            &tree,
            &parents.iter().collect::<Vec<_>>(),
            commit_headers,
        )
        .context("Failed to commit new commit")?;

    let gix_repo = repository.to_gix()?;

    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
    // swap out the old commit with the new, updated one
    steps.iter_mut().for_each(|step| {
        if let but_rebase::RebaseStep::Pick { commit_id, .. } = step {
            if commit.id() == commit_id.to_git2() {
                *commit_id = new_commit_oid.to_gix();
            }
        }
    });
    let merge_base = stack.merge_base(ctx)?;
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;
    let new_branch_head = output.top_commit.to_git2();

    // Update virtual_branch
    let (new_branch_head, new_branch_tree) = if ctx.app_settings().feature_flags.v3 {
        (new_branch_head, None)
    } else {
        #[allow(deprecated)]
        let res = compute_updated_branch_head(ctx.repo(), &gix_repo, &stack, new_branch_head, ctx)?;
        (res.head, Some(res.tree))
    };

    stack.set_stack_head(&vb_state, &gix_repo, new_branch_head, new_branch_tree)?;
    stack.set_heads_from_rebase_output(ctx, output.references)?;

    // Switch branch to gitbutler/workspace
    repository
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let uncommtied_changes = get_uncommited_changes(ctx)?;

    if ctx.app_settings().feature_flags.v3 {
        update_uncommited_changes_with_tree(
            ctx,
            old_workspace,
            new_workspace,
            uncommtied_changes,
            perm,
        )?;
    } else {
        // Checkout the applied branches
        #[allow(deprecated)]
        checkout_branch_trees(ctx, perm)?;
    }
    update_workspace_commit(&vb_state, ctx)?;
    list_virtual_branches(ctx, perm)?;

    Ok(())
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntryPresence {
    pub ours: bool,
    pub theirs: bool,
    pub ancestor: bool,
}

pub(crate) fn starting_index_state(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<Vec<(RemoteBranchFile, Option<ConflictEntryPresence>)>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repository = ctx.repo();

    let commit = repository.find_commit(metadata.commit_oid)?;
    let commit_parent_tree = if commit.is_conflicted() {
        repository.find_real_tree(&commit, ConflictedTreeKey::Ours)?
    } else {
        commit.parent(0)?.tree()?
    };

    let index = get_commit_index(repository, &commit)?;

    let conflicts = index
        .conflicts()?
        .filter_map(|conflict| {
            let Ok(conflict) = conflict else {
                return None;
            };

            let path = conflict
                .ancestor
                .as_ref()
                .or(conflict.our.as_ref())
                .or(conflict.their.as_ref())
                .map(|entry| PathBuf::from(entry.path.to_str_lossy().to_string()))?;

            Some((
                path,
                ConflictEntryPresence {
                    ours: conflict.our.is_some(),
                    theirs: conflict.their.is_some(),
                    ancestor: conflict.ancestor.is_some(),
                },
            ))
        })
        .collect::<HashMap<PathBuf, ConflictEntryPresence>>();

    let diff = repository.diff_tree_to_index(Some(&commit_parent_tree), Some(&index), None)?;

    let diff_files = hunks_by_filepath(Some(repository), &diff)?
        .into_iter()
        .map(|(path, file)| (file.into(), conflicts.get(&path).cloned()))
        .collect();

    Ok(diff_files)
}
