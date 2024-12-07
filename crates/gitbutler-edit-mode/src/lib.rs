use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{bail, Context, Result};
use bstr::ByteSlice;
use git2::build::CheckoutBuilder;
use gitbutler_branch_actions::internal::list_virtual_branches;
use gitbutler_branch_actions::{update_workspace_commit, RemoteBranchFile};
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt as _};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use gitbutler_diff::hunks_by_filepath;
use gitbutler_operating_modes::{
    operating_mode, read_edit_mode_metadata, write_edit_mode_metadata, EditModeMetadata,
    OperatingMode, EDIT_BRANCH_REF, WORKSPACE_BRANCH_REF,
};
use gitbutler_project::access::{WorktreeReadPermission, WorktreeWritePermission};
use gitbutler_reference::{ReferenceName, Refname};
use gitbutler_repo::{rebase::cherry_rebase, RepositoryExt};
use gitbutler_repo::{signature, SignaturePurpose};
use gitbutler_stack::{Stack, VirtualBranchesHandle};
use gitbutler_workspace::{checkout_branch_trees, compute_updated_branch_head, BranchHeadAndTree};
use serde::Serialize;

pub mod commands;

fn get_commit_index(repository: &git2::Repository, commit: &git2::Commit) -> Result<git2::Index> {
    let commit_tree = commit.tree().context("Failed to get commit's tree")?;
    // Checkout the commit as unstaged changes
    if commit.is_conflicted() {
        let base = commit_tree
            .get_name(".conflict-base-0")
            .context("Failed to get base")?;
        let base = repository
            .find_tree(base.id())
            .context("Failed to find base tree")?;
        // Ours
        let ours = commit_tree
            .get_name(".conflict-side-0")
            .context("Failed to get base")?;
        let ours = repository
            .find_tree(ours.id())
            .context("Failed to find base tree")?;
        // Theirs
        let theirs = commit_tree
            .get_name(".conflict-side-1")
            .context("Failed to get base")?;
        let theirs = repository
            .find_tree(theirs.id())
            .context("Failed to find base tree")?;

        let index = repository
            .merge_trees(&base, &ours, &theirs, None)
            .context("Failed to merge trees")?;

        Ok(index)
    } else {
        let mut index = git2::Index::new()?;
        index
            .read_tree(&commit_tree)
            .context("Failed to set index tree")?;

        Ok(index)
    }
}

fn checkout_edit_branch(ctx: &CommandContext, commit: git2::Commit) -> Result<()> {
    let repository = ctx.repo();

    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;
    let maybe_conflicted_parent_commit = if commit.is_conflicted() {
        Err(commit.clone())
    } else if commit.parent(0)?.is_conflicted() {
        Err(commit.parent(0)?)
    } else {
        Ok(commit.parent(0)?)
    };

    // Checkout commits's parent
    let commit_parent = match maybe_conflicted_parent_commit {
        Err(conflicted) => {
            let base_tree = repository.find_real_tree(&conflicted, ConflictedTreeKey::Ours)?;
            let base = repository.commit(
                None,
                &author_signature,
                &committer_signature,
                "Conflict base",
                &base_tree,
                &[],
            )?;
            repository.find_commit(base)?
        }
        Ok(unconflicted) => unconflicted,
    };
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

    checkout_edit_branch(ctx, commit).context("Failed to checkout edit branch")?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = ctx.repo();

    // Checkout gitbutler workspace branch
    repository
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    checkout_branch_trees(ctx, perm)?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repository = ctx.repo();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    // Get important references
    let commit = repository
        .find_commit(edit_mode_metadata.commit_oid)
        .context("Failed to find commit")?;

    let Some(mut virtual_branch) =
        find_virtual_branch_by_reference(ctx, &edit_mode_metadata.branch_reference)?
    else {
        bail!("Failed to find virtual branch for this reference. Entering and leaving edit mode for non-virtual branches is unsupported")
    };

    let parents = commit.parents().collect::<Vec<_>>();

    // Recommit commit
    let tree = repository.create_wd_tree()?;

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

    // Rebase all all commits on top of the new commit and update reference
    let new_branch_head = cherry_rebase(ctx, new_commit_oid, commit.id(), virtual_branch.head())
        .context("Failed to rebase commits onto new commit")?
        .unwrap_or(new_commit_oid);

    // Update virtual_branch
    let BranchHeadAndTree {
        head: new_branch_head,
        tree: new_branch_tree,
    } = compute_updated_branch_head(repository, &virtual_branch, new_branch_head)?;

    virtual_branch.set_stack_head(ctx, new_branch_head, Some(new_branch_tree))?;

    // Switch branch to gitbutler/workspace
    repository
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    // Checkout the applied branches
    checkout_branch_trees(ctx, perm)?;
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
