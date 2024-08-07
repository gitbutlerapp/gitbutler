use anyhow::{Context, Result};
use bstr::ByteSlice;
use git2::build::CheckoutBuilder;
use gitbutler_branch::{signature, SignaturePurpose, VirtualBranchesHandle};
use gitbutler_branch_actions::{list_virtual_branches, update_gitbutler_integration};
use gitbutler_command_context::CommandContext;
use gitbutler_commit::{commit_ext::CommitExt, commit_headers::HasCommitHeaders};
use gitbutler_operating_modes::{
    read_edit_mode_metadata, write_edit_mode_metadata, EditModeMetadata, EDIT_BRANCH_REF,
    INTEGRATION_BRANCH_REF,
};
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_repo::{rebase::cherry_rebase, RepositoryExt};

pub const EDIT_UNCOMMITED_FILES_REF: &str = "refs/gitbutler/edit_uncommited_files";

fn save_uncommited_files(ctx: &CommandContext) -> Result<()> {
    let repository = ctx.repository();

    // Create a tree of all uncommited files
    let mut index = repository.index().context("Failed to get index")?;
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .context("Failed to add all to index")?;
    index.write().context("Failed to write index")?;
    let tree_oid = index
        .write_tree()
        .context("Failed to create tree from index")?;
    let tree = repository
        .find_tree(tree_oid)
        .context("Failed to find tree")?;

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

fn checkout_edit_branch(ctx: &CommandContext, editee: &git2::Commit) -> Result<()> {
    let repository = ctx.repository();

    // Checkout editee's parent
    let editee_parent = editee.parent(0).context("Failed to get editee's parent")?;
    repository
        .reference(EDIT_BRANCH_REF, editee_parent.id(), true, "")
        .context("Failed to update edit branch reference")?;
    repository
        .set_head(EDIT_BRANCH_REF)
        .context("Failed to set head reference")?;
    repository
        .checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))
        .context("Failed to checkout head")?;

    // Checkout the editee as unstaged changes
    let editee_tree = editee.tree().context("Failed to get editee's tree")?;
    repository
        .checkout_tree(
            editee_tree.as_object(),
            Some(CheckoutBuilder::new().force().remove_untracked(true)),
        )
        .context("Failed to checkout editee")?;

    Ok(())
}

pub fn transition_from_in_workspace_to_edit_mode(
    ctx: &CommandContext,
    editee: &git2::Commit,
    editee_branch: String,
    _perm: &mut WorktreeWritePermission,
) -> Result<EditModeMetadata> {
    let edit_mode_metadata = EditModeMetadata {
        editee_commit_sha: editee.id(),
        editee_branch,
    };

    save_uncommited_files(ctx).context("Failed to save uncommited files")?;
    checkout_edit_branch(ctx, editee).context("Failed to checkout edit branch")?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;

    Ok(edit_mode_metadata)
}

pub fn transition_from_edit_to_in_workspace_mode(
    ctx: &CommandContext,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repository = ctx.repository();
    let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());

    let editee_reference = repository
        .find_reference(&edit_mode_metadata.editee_branch)
        .context("Failed to find editee branch reference")?;
    let editee_branch_head = editee_reference
        .peel_to_commit()
        .context("Failed to find editee branch head")?;
    let editee = repository
        .find_commit(edit_mode_metadata.editee_commit_sha)
        .context("Failed to find editee")?;
    let editee_parent = editee.parent(0).context("Failed to get editee's parent")?;
    let stashed_integration_changes_reference = repository
        .find_reference(EDIT_UNCOMMITED_FILES_REF)
        .context("Failed to find stashed integration changes")?;
    let stashed_integration_changes_tree = stashed_integration_changes_reference
        .peel_to_tree()
        .context("Failed to get stashed changes tree")?;

    // Recommit editee
    let mut index = repository.index().context("Failed to get index")?;
    index
        .add_all(["*"], git2::IndexAddOption::DEFAULT, None)
        .context("Failed to add all to index")?;
    index.write().context("Failed to write index")?;
    let tree_oid = index
        .write_tree()
        .context("Failed to create tree from index")?;
    let tree = repository
        .find_tree(tree_oid)
        .context("Failed to find tree")?;
    let new_editee_oid = ctx
        .repository()
        .commit_with_signature(
            None,
            &editee.author(),
            &editee.committer(),
            &editee.message_bstr().to_str_lossy(),
            &tree,
            &[&editee_parent],
            editee.gitbutler_headers(),
        )
        .context("Failed to commit new editee")?;

    // Rebase all all commits on top of the new editee and update reference
    let new_editee_branch_head =
        cherry_rebase(ctx, new_editee_oid, editee_branch_head.id(), editee.id())
            .context("Failed to rebase commits onto new editee")?
            .unwrap_or(new_editee_oid);
    repository
        .reference(
            &edit_mode_metadata.editee_branch,
            new_editee_branch_head,
            true,
            "",
        )
        .context("Failed to reference new editee branch head")?;

    // Move back to gitbutler/integration and restore stashed changes
    repository
        .set_head(INTEGRATION_BRANCH_REF)
        .context("Failed to set head reference")?;
    repository
        .checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))
        .context("Failed to checkout gitbutler/integration")?;
    repository
        .checkout_tree(
            stashed_integration_changes_tree.as_object(),
            Some(CheckoutBuilder::new().force().remove_untracked(true)),
        )
        .context("Failed to checkout stashed changes tree")?;

    update_gitbutler_integration(&vb_state, ctx)
        .context("Failed to update gitbutler integration")?;
    list_virtual_branches(ctx, perm).context("Failed to list virtual branches")?;

    Ok(())
}
