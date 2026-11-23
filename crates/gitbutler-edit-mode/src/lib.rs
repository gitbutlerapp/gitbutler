use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice};
use but_core::{
    Commit, RepositoryExt, TreeChange,
    commit::{Headers, SignCommit},
    ref_metadata::StackId,
    update_head_reference,
};
use but_ctx::{
    Context,
    access::{RepoExclusive, RepoShared},
};
use but_oxidize::{ObjectIdExt as _, gix_to_git2_index};
use but_rebase::graph_rebase::{Editor, Pick};
use git2::build::CheckoutBuilder;
use gitbutler_operating_modes::{
    EDIT_BRANCH_REF, EditModeMetadata, OperatingMode, WORKSPACE_BRANCH_REF,
    delete_edit_mode_metadata, operating_mode, read_edit_mode_metadata, write_edit_mode_metadata,
};
use gix::prelude::ObjectIdExt as _;
use serde::Serialize;
use workspace_tree::{WorkspaceTrees, checkout_uncommitted_changes};

pub mod commands;
mod workspace_tree;

const UNCOMMITTED_CHANGES_REF: &str = "refs/gitbutler/edit-uncommitted-changes";

fn commit_title_to_merge_conflict_label(commit: &gix::Commit<'_>) -> Result<String> {
    Ok(gix::objs::commit::MessageRef::from_bytes(
        but_core::commit::strip_conflict_markers(commit.message_raw()?).as_ref(),
    )
    .title
    // For some reason, the title sometimes contains newlines at the end, so trim it.
    .trim_ascii_end()
    .to_str_lossy()
    .chars()
    .take(80)
    .collect())
}

/// Returns an index of the tree of `commit` if it is unconflicted, *or* produce a merged tree
/// if `commit` is conflicted. That tree is turned into an index that records the conflicts that occurred
/// during the merge.
fn get_commit_index(ctx: &Context, commit_id: gix::ObjectId) -> Result<git2::Index> {
    let repo = &*ctx.repo.get()?;
    let commit = Commit::from_id(commit_id.attach(repo))?;

    if let Some((base, ours, theirs)) = commit.conflicted_tree_ids()? {
        let repo = repo.clone().for_tree_diffing()?;
        // Merge without favoring a side this time to get a tree containing the actual conflicts.
        let merge_result = repo.merge_trees(
            base,
            ours,
            theirs,
            repo.default_merge_labels(),
            repo.tree_merge_options()?,
        )?;
        let conflict_kind = gix::merge::tree::TreatAsUnresolved::git();
        if !merge_result.index.has_unmerged_entries(conflict_kind) {
            tracing::warn!(
                "There must be an issue with conflict-commit creation as re-merging the conflicting trees didn't yield a conflicting index."
            );
        }
        let index = merge_result.index(conflict_kind, None)?;
        gix_to_git2_index(&index)
    } else {
        let commit_tree_id = commit.tree_id_or_auto_resolution()?;
        let index = repo.index_from_tree(&commit_tree_id)?;
        gix_to_git2_index(&index)
    }
}

/// Returns a commit to be the HEAD of `gitbutler/edit`.
///
/// If the edited commit is unconflicted, this is the edited commit itself.
///
/// If the edited commit is conflicted, this is the edited commit except that
/// the root tree is `.conflict-side-0`. This is because the resulting merge is
/// always based on top of `.conflict-side-0`, so this is the preferable base.
fn find_or_create_base_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let commit = repo.find_commit(commit_id)?;
    let but_commit = but_core::Commit::try_from(commit.clone())?;

    if !but_commit.is_conflicted() {
        return Ok(commit_id);
    };

    let base_tree = but_commit.tree_id_or_kind(but_core::commit::TreeKind::Ours)?;

    let concrete_commit = gix::objs::Commit::try_from(commit.decode()?)?;
    let extra_headers: Vec<(BString, BString)> = Headers::try_from_commit(&concrete_commit)
        .map(|commit_headers| {
            let headers = Headers {
                conflicted: None,
                ..commit_headers
            };
            (&headers).into()
        })
        .unwrap_or_default();
    let message = but_core::commit::strip_conflict_markers(concrete_commit.message.as_ref());
    let commit = gix::objs::Commit {
        tree: base_tree.into(),
        extra_headers,
        message,
        ..concrete_commit
    };
    Ok(repo.write_object(&commit)?.detach())
}

fn commit_uncommited_changes(repo: &gix::Repository) -> Result<()> {
    #[expect(deprecated)]
    let uncommitted_changes = repo.create_wd_tree(0)?;
    repo.reference(
        UNCOMMITTED_CHANGES_REF,
        uncommitted_changes,
        gix::refs::transaction::PreviousValue::Any,
        "",
    )?;
    Ok(())
}

fn get_uncommitted_changes(repo: &gix::Repository) -> Result<gix::ObjectId> {
    let uncommitted_changes = repo
        .find_reference(UNCOMMITTED_CHANGES_REF)?
        .peel_to_tree()?
        .id;
    Ok(uncommitted_changes)
}

fn checkout_edit_branch(ctx: &Context, commit_id: gix::ObjectId) -> Result<()> {
    let repo = &*ctx.repo.get()?;
    #[expect(deprecated, reason = "checkout/index materialization boundary")]
    let git2_repo = &*ctx.git2_repo.get()?;
    let commit = commit_id.attach(repo).object()?.try_into_commit()?;

    let base_commit_id = find_or_create_base_commit(repo, commit_id)?;
    let base_commit = base_commit_id.attach(repo).object()?.try_into_commit()?;
    let edit_branch_ref: gix::refs::FullName = EDIT_BRANCH_REF.try_into()?;
    repo.reference(
        edit_branch_ref.as_ref(),
        base_commit_id,
        gix::refs::transaction::PreviousValue::Any,
        "enter edit mode",
    )?;
    update_head_reference(
        repo,
        gix::refs::Target::Symbolic(edit_branch_ref),
        false,
        "enter edit mode",
        EDIT_BRANCH_REF.into(),
        0,
    )?;

    // Checkout the commit as unstaged changes
    // TODO this may not be necessary if the commit is unconflicted
    let mut index = get_commit_index(ctx, commit_id)?;

    let their_commit_msg = commit_title_to_merge_conflict_label(&commit)?;
    let their_label = format!("Current commit: {their_commit_msg}");

    let commit_parent_id = base_commit.parent_ids().next();
    let our_label = if let Some(commit_parent_id) = commit_parent_id {
        let our_commit_msg =
            commit_title_to_merge_conflict_label(&commit_parent_id.object()?.try_into_commit()?)?;
        format!("New base: {our_commit_msg}")
    } else {
        "New base: (no commit)".to_string()
    };

    git2_repo.checkout_index(
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
    repo.find_reference(WORKSPACE_BRANCH_REF).with_context(|| {
        format!("expected the open workspace ref to exist: {WORKSPACE_BRANCH_REF}")
    })
}

/// TODO: This function must go away as it recreates an artificial traversal from the workspace,
/// probably because at some point the surrounding workspace couldn't be found anymore and was strictly required.
/// By now, edit-mode won't fully detach the commit-to-edit anymore, so the surrounding workspace should still be
/// found.
#[deprecated = "extra traversals must not be done and shouldn't be needed here."]
fn workspace_from_workspace_ref(ctx: &Context) -> Result<but_graph::Workspace> {
    let repo = ctx.repo.get()?;
    let meta = ctx.meta()?;
    let mut workspace_ref = open_workspace_ref(&repo)?;
    let workspace_commit_id = workspace_ref.peel_to_id()?.detach();
    but_graph::nosegment::workspace(
        &repo,
        &meta,
        but_graph::init::Overlay::default(),
        ctx.project_meta()?,
        Some(workspace_ref.name()),
        Some(workspace_commit_id),
        [],
        Vec::new(),
        false,
        but_graph::init::Options::limited().commits_limit_hint,
    )
}

fn ensure_stack_in_workspace(ctx: &Context, stack_id: StackId) -> Result<()> {
    #[allow(deprecated)]
    workspace_from_workspace_ref(ctx)?.try_find_stack_by_id(stack_id)?;
    Ok(())
}

pub(crate) fn enter_edit_mode(
    ctx: &Context,
    commit_oid: gix::ObjectId,
    stack_id: StackId,
    _perm: &mut RepoExclusive,
) -> Result<EditModeMetadata> {
    let repo = &*ctx.repo.get()?;
    let edit_mode_metadata = EditModeMetadata {
        commit_oid,
        stack_id,
    };

    ensure_stack_in_workspace(ctx, stack_id)?;

    // Fail before writing anything if the index is already locked, as checkout would only
    // notice after refs and HEAD were moved. The probe is released so checkout can take it.
    drop(
        gix::lock::File::acquire(
            repo.index_path(),
            gix::lock::acquire::Fail::Immediately,
            None,
            gix::fs::SharedPermissions::Umask,
            Some(&gix::lock::acquire::resolve_symlink),
        )
        .context("Repository index is locked by another process")?,
    );

    commit_uncommited_changes(repo)?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit_oid).context("Failed to checkout edit branch")?;

    Ok(edit_mode_metadata)
}

/// Remove the references and metadata that [`enter_edit_mode`] created.
///
/// `HEAD` must already point back at the workspace branch before this is called, otherwise
/// deleting [`EDIT_BRANCH_REF`] would leave `HEAD` dangling.
fn cleanup_edit_mode(ctx: &Context, repo: &gix::Repository) -> Result<()> {
    for ref_name in [EDIT_BRANCH_REF, UNCOMMITTED_CHANGES_REF] {
        // Tolerate an already-absent ref, but surface real lookup failures.
        if let Some(reference) = repo.try_find_reference(ref_name)? {
            reference
                .delete()
                .with_context(|| format!("Failed to delete reference {ref_name}"))?;
        }
    }
    delete_edit_mode_metadata(ctx)?;
    Ok(())
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

    #[expect(deprecated, reason = "checkout/materialization boundary")]
    let repo = &*ctx.git2_repo.get()?;

    repo.set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;

    let uncommited_changes = get_uncommitted_changes(&*ctx.repo.get()?)?;
    let uncommited_changes = repo.find_tree(uncommited_changes.to_git2())?;

    repo.checkout_tree(
        uncommited_changes.as_object(),
        Some(CheckoutBuilder::new().force().remove_untracked(true)),
    )?;

    cleanup_edit_mode(ctx, &*ctx.repo.get()?)?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    #[expect(deprecated, reason = "checkout/index materialization boundary")]
    let git2_repo = &*ctx.git2_repo.get()?;
    let repo = &*ctx.repo.get()?;

    #[allow(deprecated)]
    let old_workspace = WorkspaceTrees::from_workspace(repo, &workspace_from_workspace_ref(ctx)?)?;

    let head_commit = repo.head_commit()?;
    let decoded_head_commit = head_commit.decode()?;
    // Write out all the changes, including unstaged changes to a tree for re-committing
    #[expect(deprecated)]
    let tree_id = repo.create_wd_tree(0)?;
    let new_commit_oid = if decoded_head_commit.tree() == tree_id {
        head_commit.id
    } else {
        let commit = gix::objs::Commit::try_from(decoded_head_commit.clone())?;
        but_rebase::commit::create(
            repo,
            gix::objs::Commit {
                tree: tree_id,
                ..commit
            },
            but_rebase::commit::DateMode::CommitterUpdateAuthorKeep,
            SignCommit::IfSignCommitsEnabled,
            None,
        )?
    };

    {
        let workspace_commit_id = repo
            .find_reference(WORKSPACE_BRANCH_REF)?
            .peel_to_commit()?
            .id;
        let workspace_ref = gix::refs::FullName::try_from(WORKSPACE_BRANCH_REF)?;
        let mut meta = ctx.meta()?;
        let mut db = ctx.db.get_cache_mut()?;
        let mut workspace = but_graph::nosegment::workspace(
            repo,
            &meta,
            but_graph::init::Overlay::default(),
            ctx.project_meta()?,
            Some(workspace_ref.as_ref()),
            Some(workspace_commit_id),
            [],
            Vec::new(),
            false,
            but_graph::init::Options::limited().commits_limit_hint,
        )?;
        let mut editor = Editor::create(&mut workspace, &mut meta, repo, &mut db)?;
        let (target_selector, _commit) =
            editor.find_selectable_commit(edit_mode_metadata.commit_oid)?;

        editor.update_pick(target_selector, |_editor, _pick| {
            let mut pick = Pick::new_pick(new_commit_oid);
            // Do not replace new_commit_oid's parents with the parents of
            // edit_mode_metadata.commit_oid
            pick.preserved_parents = Some(decoded_head_commit.parents().collect());
            Ok(pick)
        })?;
        let outcome = editor.rebase()?;
        // HEAD is EDIT_BRANCH_REF and we do not need to re-checkout it (we
        // are checking out WORKSPACE_BRANCH_REF after this). As for needing to
        // "cherry pick" uncommitted changes from the old HEAD, we do not need to,
        // because there are none (they have been written to a tree earlier in this
        // function). Therefore, use `materialize_without_checkout`.
        outcome.materialize_without_checkout()?;
    }
    ctx.invalidate_workspace_cache()?;

    git2_repo
        .set_head(WORKSPACE_BRANCH_REF)
        .context("Failed to set head reference")?;
    git2_repo.checkout_head(Some(CheckoutBuilder::new().force()))?;

    let new_workspace = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        WorkspaceTrees::from_workspace(repo, &ws)?
    };
    let uncommitted_changes = get_uncommitted_changes(repo)?;
    checkout_uncommitted_changes(
        ctx,
        &old_workspace,
        &new_workspace,
        uncommitted_changes,
        perm,
    )?;

    // Currently if the index goes wonky then files don't appear quite right.
    // This just makes sure the index is all good.
    let mut index = git2_repo.index()?;
    index.read_tree(&git2_repo.head()?.peel_to_tree()?)?;
    index.write()?;

    let session = git_meta_lib::Session::open(repo.path())?;
    session
        .target(&git_meta_lib::Target::commit(
            &new_commit_oid.to_hex().to_string(),
        )?)
        .set_add(
            "evolution-parent",
            &edit_mode_metadata.commit_oid.to_hex().to_string(),
        )?;

    cleanup_edit_mode(ctx, repo)?;

    Ok(())
}

#[derive(Serialize, Debug, Clone)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntryPresence {
    pub ours: bool,
    pub theirs: bool,
    pub ancestor: bool,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(ConflictEntryPresence);

pub(crate) fn starting_index_state(
    ctx: &Context,
    perm: &RepoShared,
) -> Result<Vec<(TreeChange, Option<ConflictEntryPresence>)>> {
    let OperatingMode::Edit(metadata) = operating_mode(ctx, perm)? else {
        bail!("Starting index state can only be fetched while in edit mode")
    };

    let repo = &*ctx.repo.get()?;
    let commit = repo.find_commit(metadata.commit_oid)?;
    let but_commit = but_core::Commit::try_from(commit.clone())?;
    let commit_parent_tree = if but_commit.is_conflicted() {
        but_commit
            .tree_id_or_kind(but_core::commit::TreeKind::Base)?
            .detach()
    } else {
        commit
            .parent_ids()
            .next()
            .context("edited commit had no parent")?
            .object()?
            .try_into_commit()?
            .tree_id()?
            .detach()
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
        Some(commit_parent_tree),
        but_core::Commit::try_from(commit.clone())?
            .tree_id_or_kind(but_core::commit::TreeKind::Theirs)?
            .detach(),
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

    let repo = &*ctx.repo.get()?;
    let commit = repo.find_commit(metadata.commit_oid)?;
    let base = but_core::Commit::try_from(commit)?.tree_id_or_auto_resolution()?;
    #[expect(deprecated)]
    let head = repo.create_wd_tree(0)?;

    let repo = ctx.repo.get()?;
    let tree_changes = but_core::diff::tree_changes(&repo, Some(base.into()), head)?;
    Ok(tree_changes)
}
