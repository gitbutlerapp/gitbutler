use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice};
use but_core::{RepositoryExt, TreeChange, ref_metadata::StackId};
use but_ctx::{
    Context,
    access::{WorktreeReadPermission, WorktreeWritePermission},
};
use but_oxidize::{ObjectIdExt, OidExt, RepoExt, git2_to_gix_object_id, gix_to_git2_index};
use but_workspace::legacy::stack_ext::StackExt;
use git2::build::CheckoutBuilder;
use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_cherry_pick::{ConflictedTreeKey, RepositoryExt as _};
use gitbutler_commit::{
    commit_ext::CommitExt,
    commit_headers::{CommitHeadersV2, HasCommitHeaders},
};
use gitbutler_operating_modes::{
    EDIT_BRANCH_REF, EditModeMetadata, OperatingMode, WORKSPACE_BRANCH_REF, operating_mode,
    read_edit_mode_metadata, write_edit_mode_metadata,
};
use gitbutler_repo::{RepositoryExt as _, SignaturePurpose, signature};
use gitbutler_stack::VirtualBranchesHandle;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes_with_tree};
use serde::Serialize;

pub mod commands;

const UNCOMMITTED_CHANGES_REF: &str = "refs/gitbutler/edit-uncommitted-changes";

/// Returns an index of the tree of `commit` if it is unconflicted, *or* produce a merged tree
/// if `commit` is conflicted. That tree is turned into an index that records the conflicts that occurred
/// during the merge.
fn get_commit_index(ctx: &Context, commit: &git2::Commit) -> Result<git2::Index> {
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

        let gix_repo = ctx.clone_repo_for_merging()?;
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
            tracing::warn!(
                "There must be an issue with conflict-commit creation as re-merging the conflicting trees didn't yield a conflicting index."
            );
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

fn commit_uncommited_changes(ctx: &Context) -> Result<()> {
    let repository = &*ctx.git2_repo.get()?;
    let uncommited_changes = repository.create_wd_tree(0)?;
    repository.reference(UNCOMMITTED_CHANGES_REF, uncommited_changes.id(), true, "")?;
    Ok(())
}

fn get_uncommited_changes(ctx: &Context) -> Result<git2::Oid> {
    let repository = &*ctx.git2_repo.get()?;
    let uncommited_changes = repository
        .find_reference(UNCOMMITTED_CHANGES_REF)?
        .peel_to_tree()?
        .id();
    Ok(uncommited_changes)
}

fn checkout_edit_branch(ctx: &Context, commit: git2::Commit) -> Result<()> {
    let repository = &*ctx.git2_repo.get()?;

    // Checkout commits's parent
    let commit_parent = find_or_create_base_commit(repository, &commit)?;
    repository.reference(EDIT_BRANCH_REF, commit_parent.id(), true, "")?;
    repository.set_head(EDIT_BRANCH_REF)?;
    repository.checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))?;

    // Checkout the commit as unstaged changes
    let mut index = get_commit_index(ctx, &commit)?;

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

pub(crate) fn enter_edit_mode(
    ctx: &Context,
    commit: git2::Commit,
    stack_id: StackId,
    _perm: &mut WorktreeWritePermission,
) -> Result<EditModeMetadata> {
    let edit_mode_metadata = EditModeMetadata {
        commit_oid: commit.id(),
        stack_id,
    };

    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());
    // Validate the stack_id
    vb_state.get_stack_in_workspace(stack_id)?;

    commit_uncommited_changes(ctx)?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit).context("Failed to checkout edit branch")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &Context,
    _perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let repository = &*ctx.git2_repo.get()?;

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
    ctx: &Context,
    perm: &mut WorktreeWritePermission,
) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repository = &*ctx.git2_repo.get()?;
    let vb_state = VirtualBranchesHandle::new(ctx.project_data_dir());

    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    // Get important references
    let commit = repository
        .find_commit(edit_mode_metadata.commit_oid)
        .context("Failed to find commit")?;

    let mut stack = vb_state.get_stack_in_workspace(edit_mode_metadata.stack_id)?;

    let parents = commit.parents().collect::<Vec<_>>();

    // Write out all the changes, including unstaged changes to a tree for re-committing
    let mut index = repository.index()?;
    index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
    index.write()?;
    let tree = repository.create_wd_tree(0)?;

    let (_, committer) = repository.signatures()?;
    let commit_headers = commit
        .gitbutler_headers()
        .map(|commit_headers| CommitHeadersV2 {
            conflicted: None,
            ..commit_headers
        });
    let new_commit_oid = ctx
        .git2_repo
        .get()?
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

    let gix_repo = repository.to_gix_repo()?;

    let mut steps = stack.as_rebase_steps(ctx, &gix_repo)?;
    // swap out the old commit with the new, updated one
    steps.iter_mut().for_each(|step| {
        if let but_rebase::RebaseStep::Pick { commit_id, .. } = step
            && commit.id() == commit_id.to_git2()
        {
            *commit_id = new_commit_oid.to_gix();
        }
    });
    let merge_base = stack.merge_base(ctx)?;
    let mut rebase = but_rebase::Rebase::new(&gix_repo, Some(merge_base), None)?;
    rebase.rebase_noops(false);
    rebase.steps(steps)?;
    let output = rebase.rebase()?;

    stack.set_heads_from_rebase_output(ctx, output.references)?;

    // Switch branch to gitbutler/workspace
    repository
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;
    repository.checkout_head(Some(CheckoutBuilder::new().force()))?;

    update_workspace_commit(&vb_state, ctx, false)?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let uncommtied_changes = get_uncommited_changes(ctx)?;

    update_uncommitted_changes_with_tree(
        ctx,
        old_workspace,
        new_workspace,
        Some(uncommtied_changes),
        Some(true),
        perm,
    )?;

    // Currently if the index goes wonky then files don't appear quite right.
    // This just makes sure the index is all good.
    let mut index = repository.index()?;
    index.read_tree(&repository.head()?.peel_to_tree()?)?;
    index.write()?;

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
    ctx: &Context,
    _perm: &WorktreeReadPermission,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repository = &*ctx.git2_repo.get()?;

    let commit = repository.find_commit(metadata.commit_oid)?;
    let commit_parent_tree = if commit.is_conflicted() {
        repository.find_real_tree(&commit, ConflictedTreeKey::Base)?
    } else {
        commit.parent(0)?.tree()?
    };

    let index = get_commit_index(ctx, &commit)?;

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
                .map(|entry| BString::new(entry.path.clone()))?;

            Some((
                path,
                ConflictEntryPresence {
                    ours: conflict.our.is_some(),
                    theirs: conflict.their.is_some(),
                    ancestor: conflict.ancestor.is_some(),
                },
            ))
        })
        .collect::<HashMap<BString, ConflictEntryPresence>>();

    let gix_repo = ctx.repo.get()?;

    let tree_changes = but_core::diff::tree_changes(
        &gix_repo,
        Some(commit_parent_tree.id().to_gix()),
        repository
            .find_real_tree(&commit, ConflictedTreeKey::Theirs)?
            .id()
            .to_gix(),
    )?;

    let outcome = tree_changes
        .into_iter()
        .map(|tc| (tc.clone(), conflicts.get(&tc.path).cloned()))
        .collect();

    Ok(outcome)
}

pub(crate) fn changes_from_initial(
    ctx: &Context,
    _perm: &WorktreeReadPermission,
) -> Result<Vec<TreeChange>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx) else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repository = &*ctx.git2_repo.get()?;
    let commit = repository.find_commit(metadata.commit_oid)?;
    let base = repository
        .find_real_tree(&commit, Default::default())?
        .id()
        .to_gix();
    let head = repository.create_wd_tree(0)?.id().to_gix();

    let gix_repo = ctx.repo.get()?;
    let tree_changes = but_core::diff::tree_changes(&gix_repo, Some(base), head)?;
    Ok(tree_changes)
}
