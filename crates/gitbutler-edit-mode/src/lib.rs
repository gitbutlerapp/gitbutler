use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use bstr::BString;
use but_core::{RepositoryExt, TreeChange, commit::Headers, ref_metadata::StackId};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_oxidize::{ObjectIdExt as _, OidExt, gix_to_git2_index};
use but_rebase::graph_rebase::{GraphExt as _, Step};
use git2::build::CheckoutBuilder;
use gitbutler_cherry_pick::{ConflictedTreeKey, GixRepositoryExt as _, RepositoryExt as _};
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr as _};
use gitbutler_operating_modes::{
    EDIT_BRANCH_REF, EditModeMetadata, OPEN_WORKSPACE_REFS, OperatingMode, WORKSPACE_BRANCH_REF,
    operating_mode, read_edit_mode_metadata, write_edit_mode_metadata,
};
use gitbutler_repo::RepositoryExt as _;
use gitbutler_workspace::branch_trees::{WorkspaceState, update_uncommitted_changes_with_tree};
use serde::Serialize;

pub mod commands;

const UNCOMMITTED_CHANGES_REF: &str = "refs/gitbutler/edit-uncommitted-changes";

/// Returns an index of the tree of `commit` if it is unconflicted, *or* produce a merged tree
/// if `commit` is conflicted. That tree is turned into an index that records the conflicts that occurred
/// during the merge.
fn get_commit_index(ctx: &Context, commit_id: gix::ObjectId) -> Result<git2::Index> {
    let git2_repo = ctx.git2_repo.get()?;
    let git2_commit = git2_repo
        .find_commit(commit_id.to_git2())
        .context("Failed to find commit")?;
    let commit_tree = git2_commit.tree().context("Failed to get commit's tree")?;
    let repo = ctx.repo.get()?;
    let commit = repo.find_commit(commit_id)?;
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

        let repo = repo.clone().for_tree_diffing()?;
        // Merge without favoring a side this time to get a tree containing the actual conflicts.
        let mut merge_result = repo.merge_trees(
            base.to_gix(),
            ours.to_gix(),
            theirs.to_gix(),
            repo.default_merge_labels(),
            repo.tree_merge_options()?,
        )?;
        let merged_tree_id = merge_result.tree.write()?;
        let mut index = repo.index_from_tree(&merged_tree_id)?;
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
fn find_or_create_base_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let commit = repo.find_commit(commit_id)?;
    let is_conflicted = commit.is_conflicted();
    let parent = commit
        .parent_ids()
        .next()
        .context("Expected commit to have a single parent")?
        .object()?
        .into_commit();
    let is_parent_conflicted = parent.is_conflicted();

    // If neither is conflicted, we can use the old parent.
    if !(is_conflicted || is_parent_conflicted) {
        return Ok(parent.id);
    };

    let base_tree = if is_conflicted {
        repo.find_real_tree(&commit, ConflictedTreeKey::Ours)?
    } else {
        repo.find_real_tree(&parent, ConflictedTreeKey::AutoResolution)?
    };

    let author = repo
        .author()
        .context("author must be configured")??
        .to_owned()?;
    let committer = repo
        .committer()
        .context("committer must be configured")??
        .to_owned()?;
    let commit = gix::objs::Commit {
        tree: base_tree.into(),
        parents: Default::default(),
        author,
        committer,
        encoding: None,
        message: parent.message_bstr().to_owned(),
        extra_headers: Vec::new(),
    };
    Ok(repo.write_object(&commit)?.detach())
}

fn commit_uncommited_changes(ctx: &Context) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    let uncommited_changes = repo.create_wd_tree(0)?;
    repo.reference(UNCOMMITTED_CHANGES_REF, uncommited_changes.id(), true, "")?;
    Ok(())
}

fn get_uncommited_changes(ctx: &Context) -> Result<gix::ObjectId> {
    let repo = &*ctx.git2_repo.get()?;
    let uncommited_changes = repo
        .find_reference(UNCOMMITTED_CHANGES_REF)?
        .peel_to_tree()?
        .id();
    Ok(uncommited_changes.to_gix())
}

fn checkout_edit_branch(ctx: &Context, commit_id: gix::ObjectId) -> Result<()> {
    let repo = &*ctx.git2_repo.get()?;
    let commit = repo.find_commit(commit_id.to_git2())?;

    // Checkout commits's parent
    let commit_parent_id = find_or_create_base_commit(&*ctx.repo.get()?, commit_id)?;
    let commit_parent = repo.find_commit(commit_parent_id.to_git2())?;
    repo.reference(EDIT_BRANCH_REF, commit_parent_id.to_git2(), true, "")?;
    repo.set_head(EDIT_BRANCH_REF)?;
    repo.checkout_head(Some(CheckoutBuilder::new().force().remove_untracked(true)))?;

    // Checkout the commit as unstaged changes
    let mut index = get_commit_index(ctx, commit_id)?;

    let their_commit_msg = commit
        .message()
        .and_then(|m| m.lines().next())
        .map(|l| l.chars().take(80).collect::<String>())
        .unwrap_or("".into());
    let their_label = format!("Current commit: {their_commit_msg}");

    let our_commit_msg = commit_parent
        .message()
        .and_then(|m| m.lines().next())
        .map(|l| l.chars().take(80).collect::<String>())
        .unwrap_or("".into());
    let our_label = format!("New base: {our_commit_msg}");

    repo.checkout_index(
        Some(&mut index),
        Some(
            CheckoutBuilder::new()
                .force()
                .remove_untracked(true)
                .conflict_style_diff3(true)
                .ancestor_label("Common ancestor")
                .our_label(&our_label)
                .their_label(&their_label),
        ),
    )?;

    Ok(())
}

fn open_workspace_ref<'repo>(repo: &'repo gix::Repository) -> Result<gix::Reference<'repo>> {
    OPEN_WORKSPACE_REFS
        .iter()
        .find_map(|&name| repo.find_reference(name).ok())
        .with_context(|| {
            format!(
                "expected one of the open workspace refs to exist: {}",
                OPEN_WORKSPACE_REFS.join(", ")
            )
        })
}

fn workspace_from_workspace_ref(ctx: &Context) -> Result<but_graph::projection::Workspace> {
    let repo = ctx.repo.get()?;
    let meta = ctx.meta()?;
    let mut workspace_ref = open_workspace_ref(&repo)?;
    let graph = but_graph::Graph::from_commit_traversal(
        workspace_ref.peel_to_id()?,
        Some(workspace_ref.inner.name.clone()),
        &meta,
        but_graph::init::Options::limited(),
    )?;
    graph.into_workspace()
}

fn ensure_stack_in_workspace(ctx: &Context, stack_id: StackId) -> Result<()> {
    workspace_from_workspace_ref(ctx)?.try_find_stack_by_id(stack_id)?;
    Ok(())
}

pub(crate) fn enter_edit_mode(
    ctx: &Context,
    commit_oid: gix::ObjectId,
    stack_id: StackId,
    _perm: &mut RepoExclusive,
) -> Result<EditModeMetadata> {
    let edit_mode_metadata = EditModeMetadata {
        commit_oid,
        stack_id,
    };

    ensure_stack_in_workspace(ctx, stack_id)?;

    commit_uncommited_changes(ctx)?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit_oid).context("Failed to checkout edit branch")?;

    Ok(edit_mode_metadata)
}

pub(crate) fn abort_and_return_to_workspace(
    ctx: &Context,
    force: bool,
    perm: &mut RepoExclusive,
) -> Result<()> {
    if !force && !changes_from_initial(ctx, perm.read_permission())?.is_empty() {
        bail!(
            "The working tree differs from the original commit. A forced abort is necessary.\nIf you are seeing this message, please report it as a bug. The UI should have prevented this line getting hit."
        );
    }

    let repo = &*ctx.git2_repo.get()?;

    // Checkout gitbutler workspace branch
    repo.set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    let uncommited_changes = get_uncommited_changes(ctx)?;
    let uncommited_changes = repo.find_tree(uncommited_changes.to_git2())?;

    repo.checkout_tree(
        uncommited_changes.as_object(),
        Some(CheckoutBuilder::new().force().remove_untracked(true)),
    )?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let git2_repo = &*ctx.git2_repo.get()?;
    let repo = &*ctx.repo.get()?;

    let old_workspace = WorkspaceState::create(ctx, perm.read_permission())?;

    // Write out all the changes, including unstaged changes to a tree for re-committing
    let tree_id = but_status::create_wd_tree(repo, 0)?;

    let workspace_commit = repo
        .find_reference(WORKSPACE_BRANCH_REF)?
        .peel_to_commit()?;
    let meta = ctx.meta()?;
    let graph = but_graph::Graph::from_commit_traversal(
        workspace_commit.id(),
        Some(gix::refs::FullName::try_from(WORKSPACE_BRANCH_REF)?),
        &meta,
        but_graph::init::Options::limited(),
    )?;
    let mut editor = graph.to_editor(repo)?;
    let (target_selector, commit) = editor.find_selectable_commit(edit_mode_metadata.commit_oid)?;

    let extra_headers: Vec<(BString, BString)> = Headers::try_from_commit(&commit.inner)
        .map(|commit_headers| {
            let headers = Headers {
                conflicted: None,
                ..commit_headers
            };
            (&headers).into()
        })
        .unwrap_or_default();
    let new_commit_oid = but_rebase::commit::create(
        repo,
        gix::objs::Commit {
            tree: tree_id,
            extra_headers,
            ..commit.inner
        },
        but_rebase::commit::DateMode::CommitterUpdateAuthorKeep,
        true,
    )?;

    editor.replace(target_selector, Step::new_pick(new_commit_oid))?;
    let outcome = editor.rebase()?;
    // HEAD is EDIT_BRANCH_REF and we do not need to re-checkout it (we
    // are checking out WORKSPACE_BRANCH_REF after this). As for needing to
    // "cherry pick" uncommitted changes from the old HEAD, we do not need to,
    // because there are none (they have been written to a tree earlier in this
    // function). Therefore, use `materialize_without_checkout`.
    outcome.materialize_without_checkout()?;

    // Switch branch to gitbutler/workspace
    git2_repo
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;
    git2_repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let uncommtied_changes = get_uncommited_changes(ctx)?;

    update_uncommitted_changes_with_tree(
        ctx,
        old_workspace,
        new_workspace,
        Some(uncommtied_changes.to_git2()),
        Some(true),
        perm,
    )?;

    // Currently if the index goes wonky then files don't appear quite right.
    // This just makes sure the index is all good.
    let mut index = git2_repo.index()?;
    index.read_tree(&git2_repo.head()?.peel_to_tree()?)?;
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
    perm: &RepoShared,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx, perm)? else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let git2_repo = &*ctx.git2_repo.get()?;
    let repo = &*ctx.repo.get()?;

    let commit = git2_repo.find_commit(metadata.commit_oid.to_git2())?;
    let gix_commit = repo.find_commit(commit.id().to_gix())?;
    let commit_parent_tree = if gix_commit.is_conflicted() {
        git2_repo.find_real_tree(&commit, ConflictedTreeKey::Base)?
    } else {
        commit.parent(0)?.tree()?
    };

    let index = get_commit_index(ctx, metadata.commit_oid)?;

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

    let repo = ctx.repo.get()?;

    let tree_changes = but_core::diff::tree_changes(
        &repo,
        Some(commit_parent_tree.id().to_gix()),
        git2_repo
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

pub(crate) fn changes_from_initial(ctx: &Context, perm: &RepoShared) -> Result<Vec<TreeChange>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx, perm)? else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let git2_repo = &*ctx.git2_repo.get()?;
    let commit = git2_repo.find_commit(metadata.commit_oid.to_git2())?;
    let base = git2_repo
        .find_real_tree(&commit, Default::default())?
        .id()
        .to_gix();
    let head = git2_repo.create_wd_tree(0)?.id().to_gix();

    let repo = ctx.repo.get()?;
    let tree_changes = but_core::diff::tree_changes(&repo, Some(base), head)?;
    Ok(tree_changes)
}
