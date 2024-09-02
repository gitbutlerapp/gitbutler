use std::str::FromStr;

use anyhow::{bail, Context, Result};
use bstr::ByteSlice;
use git2::build::CheckoutBuilder;
use gitbutler_branch::{signature, Branch, SignaturePurpose, VirtualBranchesHandle};
use gitbutler_branch_actions::{list_virtual_branches, update_workspace_commit, RemoteBranchFile};
use gitbutler_cherry_pick::RepositoryExt as _;
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
use gitbutler_repo::{
    rebase::{cherry_rebase, cherry_rebase_group},
    RepositoryExt,
};

pub mod commands;

pub const EDIT_UNCOMMITED_FILES_REF: &str = "refs/gitbutler/edit_uncommited_files";
pub const EDIT_INITIAL_STATE_REF: &str = "refs/gitbutler/edit_initial_state";

fn save_uncommited_files(ctx: &CommandContext) -> Result<()> {
    let repository = ctx.repository();

    // Create a tree of all uncommited files
    let tree = repository.create_wd_tree()?;

    // Commit tree and reference it
    let author_signature =
        signature(SignaturePurpose::Author).context("Failed to get gitbutler signature")?;
    let committer_signature =
        signature(SignaturePurpose::Committer).context("Failed to get gitbutler signature")?;
    let head = repository.head().context("Failed to get head")?;
    let head_commit = head.peel_to_commit().context("Failed to get head commit")?;
    let commit = repository
        .commit(
            None,
            &author_signature,
            &committer_signature,
            "Edit mode saved changes",
            &tree,
            &[&head_commit],
        )
        .context("Failed to write stash commit")?;

    repository
        .reference(EDIT_UNCOMMITED_FILES_REF, commit, true, "")
        .context("Failed to reference uncommited files")?;

    Ok(())
}

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

fn checkout_edit_branch(ctx: &CommandContext, commit: &git2::Commit) -> Result<()> {
    let repository = ctx.repository();

    // Checkout commits's parent
    let commit_parent = commit.parent(0)?;
    repository.reference(EDIT_BRANCH_REF, commit_parent.id(), true, "")?;
    repository.reference(EDIT_INITIAL_STATE_REF, commit_parent.id(), true, "")?;
    repository.set_head(EDIT_BRANCH_REF)?;
    repository.checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))?;

    // Checkout the commit as unstaged changes
    let mut index = get_commit_index(repository, commit)?;

    repository.checkout_index(
        Some(&mut index),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true),
        ),
    )?;

    let tree = repository.create_wd_tree()?;

    let author_signature = signature(SignaturePurpose::Author)?;
    let committer_signature = signature(SignaturePurpose::Committer)?;
    // Commit initial state commit
    repository.commit(
        Some(EDIT_INITIAL_STATE_REF),
        &author_signature,
        &committer_signature,
        "Initial state commit",
        &tree,
        &[&commit_parent],
    )?;

    Ok(())
}

fn find_virtual_branch_by_reference(
    ctx: &CommandContext,
    reference: &ReferenceName,
) -> Result<Option<Branch>> {
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let all_virtual_branches = vb_state
        .list_branches_in_workspace()
        .context("Failed to read virtual branches")?;

    Ok(all_virtual_branches.into_iter().find(|virtual_branch| {
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
    commit: &git2::Commit,
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

    save_uncommited_files(ctx).context("Failed to save uncommited files")?;
    checkout_edit_branch(ctx, commit).context("Failed to checkout edit branch")?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &CommandContext,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = ctx.repository();

    // Checkout gitbutler workspace branch
    {
        repository
            .set_head(WORKSPACE_BRANCH_REF)
            .context("Failed to set head reference")?;
        repository
            .checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))
            .context("Failed to checkout gitbutler/workspace")?;
    }

    // Checkout any stashed changes.
    {
        let stashed_workspace_changes_reference = repository
            .find_reference(EDIT_UNCOMMITED_FILES_REF)
            .context("Failed to find stashed workspace changes")?;
        let stashed_workspace_changes_commit = stashed_workspace_changes_reference
            .peel_to_commit()
            .context("Failed to get stashed changes commit")?;

        repository
            .checkout_tree(
                stashed_workspace_changes_commit.tree()?.as_object(),
                Some(CheckoutBuilder::new().force().remove_untracked(true)),
            )
            .context("Failed to checkout workspace changes tree")?;
    }

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repository = ctx.repository();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    // Get important references
    let commit = repository
        .find_commit(edit_mode_metadata.commit_oid)
        .context("Failed to find commit")?;
    let commit_parent = commit.parent(0).context("Failed to get commit's parent")?;
    let stashed_workspace_changes_reference = repository
        .find_reference(EDIT_UNCOMMITED_FILES_REF)
        .context("Failed to find stashed workspace changes")?;
    let stashed_workspace_changes_commit = stashed_workspace_changes_reference
        .peel_to_commit()
        .context("Failed to get stashed changes commit")?;

    let Some(mut virtual_branch) =
        find_virtual_branch_by_reference(ctx, &edit_mode_metadata.branch_reference)?
    else {
        bail!("Failed to find virtual branch for this reference. Entering and leaving edit mode for non-virtual branches is unsupported")
    };

    // Recommit commit
    let tree = repository.create_wd_tree()?;
    let commit_headers = commit
        .gitbutler_headers()
        .map(|commit_headers| CommitHeadersV2 {
            conflicted: None,
            ..commit_headers
        });
    let new_commit_oid = ctx
        .repository()
        .commit_with_signature(
            None,
            &commit.author(),
            &commit.committer(),
            &commit.message_bstr().to_str_lossy(),
            &tree,
            &[&commit_parent],
            commit_headers,
        )
        .context("Failed to commit new commit")?;

    // Rebase all all commits on top of the new commit and update reference
    let new_branch_head = cherry_rebase(ctx, new_commit_oid, commit.id(), virtual_branch.head)
        .context("Failed to rebase commits onto new commit")?
        .unwrap_or(new_commit_oid);
    repository
        .reference(
            &edit_mode_metadata.branch_reference,
            new_branch_head,
            true,
            "",
        )
        .context("Failed to reference new commit branch head")?;

    // Move back to gitbutler/workspace and restore stashed changes
    {
        repository
            .set_head(WORKSPACE_BRANCH_REF)
            .context("Failed to set head reference")?;
        repository
            .checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))
            .context("Failed to checkout gitbutler/workspace")?;

        virtual_branch.head = new_branch_head;
        virtual_branch.updated_timestamp_ms = gitbutler_time::time::now_ms();
        vb_state
            .set_branch(virtual_branch)
            .context("Failed to update vbstate")?;

        let workspace_commit_oid = update_workspace_commit(&vb_state, ctx)
            .context("Failed to update gitbutler workspace")?;

        let rebased_stashed_workspace_changes_commit = cherry_rebase_group(
            ctx,
            workspace_commit_oid,
            &mut [stashed_workspace_changes_commit.id()],
        )
        .context("Failed to rebase stashed workspace commit changes")?;

        let commit_thing = repository
            .find_commit(rebased_stashed_workspace_changes_commit)
            .context("Failed to find commit of rebased stashed workspace changes commit oid")?;

        let tree_thing = repository
            .find_real_tree(&commit_thing, Default::default())
            .context("Failed to get tree of commit of rebased stashed workspace changes")?;

        repository
            .checkout_tree(
                tree_thing.as_object(),
                Some(CheckoutBuilder::new().force().remove_untracked(true)),
            )
            .context("Failed to checkout stashed changes tree")?;

        list_virtual_branches(ctx, perm).context("Failed to list virtual branches")?;
    }

    Ok(())
}

pub(crate) fn starting_index_state(
    ctx: &CommandContext,
    _perm: &WorktreeReadPermission,
) -> Result<Vec<RemoteBranchFile>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repository = ctx.repository();

    let commit = repository.find_commit(metadata.commit_oid)?;
    let commit_parent = commit.parent(0)?;
    let commit_parent_tree = repository.find_real_tree(&commit_parent, Default::default())?;

    let initial_state = repository
        .find_reference(EDIT_INITIAL_STATE_REF)?
        .peel_to_tree()?;

    let diff =
        repository.diff_tree_to_tree(Some(&commit_parent_tree), Some(&initial_state), None)?;

    let diff_files = hunks_by_filepath(Some(repository), &diff)?
        .into_iter()
        .map(|(path, file)| {
            let binary = file.hunks.iter().any(|h| h.binary);
            RemoteBranchFile {
                path,
                hunks: file.hunks,
                binary,
            }
        })
        .collect();

    Ok(diff_files)
}
