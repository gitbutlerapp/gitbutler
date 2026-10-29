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
use but_rebase::graph_rebase::{Editor, Pick};
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr};
use gitbutler_operating_modes::{
    EDIT_BRANCH_REF, EditModeMetadata, INTEGRATION_BRANCH_REF, OperatingMode, WORKSPACE_BRANCH_REF,
    delete_edit_mode_metadata, operating_mode, read_edit_mode_metadata, write_edit_mode_metadata,
};
use gitbutler_workspace::branch_trees::{WorkspaceState, move_tree_between_workspaces};
use gix::merge::blob::builtin_driver::text::Labels;
use gix::prelude::ObjectIdExt as _;
use serde::Serialize;

pub mod commands;

const UNCOMMITTED_CHANGES_REF: &str = "refs/gitbutler/edit-uncommitted-changes";

fn commit_title_to_merge_conflict_label(commit: &gix::Commit<'_>) -> String {
    gix::objs::commit::MessageRef::from_bytes(
        but_core::commit::strip_conflict_markers(commit.message_bstr()).as_ref(),
    )
    .title
    // For some reason, the title sometimes contains newlines at the end, so trim it.
    .trim_ascii_end()
    .to_str_lossy()
    .chars()
    .take(80)
    .collect()
}

/// Return the materialized tree of `commit` and its index, retaining unresolved stages separately
/// from the conflict markers in the worktree tree.
fn get_commit_index(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    labels: Labels<'_>,
) -> Result<(gix::ObjectId, gix::index::File)> {
    let commit = Commit::from_id(commit_id.attach(repo))?;

    if let Some((base, ours, theirs)) = commit.conflicted_tree_ids()? {
        let mut repo = repo.clone().for_tree_diffing()?;
        repo.config_snapshot_mut()
            .set_value(&gix::config::tree::Merge::CONFLICT_STYLE, "diff3")?;
        // Merge without favoring a side this time to get a tree containing the actual conflicts.
        let mut merge_result =
            repo.merge_trees(base, ours, theirs, labels, repo.tree_merge_options()?)?;
        let merged_tree_id = merge_result.tree.write()?;
        let mut index = repo.index_from_tree(&merged_tree_id)?;
        if !merge_result.index_changed_after_applying_conflicts(
            &mut index,
            gix::merge::tree::TreatAsUnresolved::git(),
            gix::merge::tree::apply_index_entries::RemovalMode::Prune,
        ) {
            tracing::warn!(
                "There must be an issue with conflict-commit creation as re-merging the conflicting trees didn't yield a conflicting index."
            );
        }
        Ok((merged_tree_id.detach(), index))
    } else {
        let commit_tree_id = commit.tree_id_or_auto_resolution()?;
        Ok((
            commit_tree_id.detach(),
            repo.index_from_tree(&commit_tree_id)?,
        ))
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

    if !commit.is_conflicted() {
        return Ok(commit_id);
    };

    let base_tree = but_core::Commit::try_from(commit.clone())?
        .tree_id_or_kind(but_core::commit::TreeKind::Ours)?;

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

fn commit_uncommited_changes(repo: &gix::Repository) -> Result<gix::ObjectId> {
    #[expect(deprecated)]
    let uncommitted_changes = repo.create_wd_tree(0)?;
    repo.reference(
        UNCOMMITTED_CHANGES_REF,
        uncommitted_changes,
        gix::refs::transaction::PreviousValue::Any,
        "",
    )?;
    Ok(uncommitted_changes)
}

fn get_uncommitted_changes(repo: &gix::Repository) -> Result<gix::ObjectId> {
    let uncommitted_changes = repo
        .find_reference(UNCOMMITTED_CHANGES_REF)?
        .peel_to_tree()?
        .id;
    Ok(uncommitted_changes)
}

fn checkout_edit_branch(
    ctx: &Context,
    commit_id: gix::ObjectId,
    worktree_id: gix::ObjectId,
    index_lock: gix::lock::File,
) -> Result<()> {
    let repo = &*ctx.repo.get()?;
    let commit = commit_id.attach(repo).object()?.try_into_commit()?;

    // A conflicted edit commit uses its ours tree as HEAD.
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

    let their_commit_msg = commit_title_to_merge_conflict_label(&commit);
    let their_label = format!("Current commit: {their_commit_msg}");

    let commit_parent_id = base_commit.parent_ids().next();
    let our_label = if let Some(commit_parent_id) = commit_parent_id {
        let our_commit_msg =
            commit_title_to_merge_conflict_label(&commit_parent_id.object()?.try_into_commit()?);
        format!("New base: {our_commit_msg}")
    } else {
        "New base: (no commit)".to_string()
    };

    let (tree_id, index) = get_commit_index(
        repo,
        commit_id,
        Labels {
            ancestor: Some("Common ancestor".into()),
            current: Some(our_label.as_str().into()),
            other: Some(their_label.as_str().into()),
        },
    )?;
    checkout_tree_and_index(repo, worktree_id, tree_id, &index, index_lock)?;

    Ok(())
}

fn lock_index(repo: &gix::Repository) -> Result<gix::lock::File> {
    gix::lock::File::acquire(
        repo.index_path(),
        gix::lock::acquire::Fail::Immediately,
        None,
        0,
        Some(&gix::lock::acquire::resolve_symlink),
    )
    .context("Repository index is locked by another process")
}

/// Materialize a saved tree while independently replacing the index owned by edit mode.
fn checkout_tree_and_index(
    repo: &gix::Repository,
    worktree_id: gix::ObjectId,
    tree_id: gix::ObjectId,
    index: &gix::index::File,
    index_lock: gix::lock::File,
) -> Result<()> {
    let baseline = repo.index_from_tree(&worktree_id)?;
    let mut target = repo.index_from_tree(&tree_id)?;
    // All captured worktree content is owned by this transition; the final index is supplied separately.
    let mut staging = baseline.clone();
    repo.checkout_index_with_baseline(
        &baseline,
        &mut target,
        &mut staging,
        gix::progress::Discard,
        &Default::default(),
        gix::worktree::checkout::Options {
            overwrite_tracked: true,
            overwrite_staged: true,
            overwrite_ignored: true,
        },
    )?;
    let mut index_lock = std::io::BufWriter::with_capacity(64 * 1024, index_lock);
    index.write_to(&mut index_lock, Default::default())?;
    index_lock
        .into_inner()
        .map_err(|err| err.into_error())?
        .commit()?;
    Ok(())
}

fn open_workspace_ref<'repo>(repo: &'repo gix::Repository) -> Result<gix::Reference<'repo>> {
    [WORKSPACE_BRANCH_REF, INTEGRATION_BRANCH_REF]
        .iter()
        .find_map(|&name| repo.find_reference(name).ok())
        .with_context(|| {
            format!(
                "expected one of the open workspace refs to exist: {}",
                [WORKSPACE_BRANCH_REF, INTEGRATION_BRANCH_REF].join(", ")
            )
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
    let mut db = ctx.db.get_cache_mut()?;
    let graph = but_graph::Graph::from_commit_traversal(
        workspace_ref.peel_to_id()?,
        Some(workspace_ref.inner.name.clone()),
        &meta,
        ctx.project_meta()?,
        &mut db,
        but_graph::init::Options::limited(),
    )?;
    graph.into_workspace()
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

    // Hold the index lock before changing refs, metadata, or worktree content.
    let index_lock = lock_index(repo)?;
    let worktree_id = commit_uncommited_changes(repo)?;
    write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
    checkout_edit_branch(ctx, commit_oid, worktree_id, index_lock)
        .context("Failed to checkout edit branch")?;

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

    let repo = &*ctx.repo.get()?;
    let index_lock = lock_index(repo)?;
    #[expect(deprecated)]
    let worktree_id = repo.create_wd_tree(0)?;

    // Checkout gitbutler workspace branch
    update_head_reference(
        repo,
        gix::refs::Target::Symbolic(WORKSPACE_BRANCH_REF.try_into()?),
        false,
        "abort edit mode",
        WORKSPACE_BRANCH_REF.into(),
        0,
    )?;
    let uncommitted_changes = get_uncommitted_changes(repo)?;
    let index = repo.index_from_tree(&uncommitted_changes)?;
    checkout_tree_and_index(repo, worktree_id, uncommitted_changes, &index, index_lock)?;

    cleanup_edit_mode(ctx, repo)?;

    Ok(())
}

pub(crate) fn save_and_return_to_workspace(ctx: &Context, perm: &mut RepoExclusive) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repo = &*ctx.repo.get()?;
    let index_lock = lock_index(repo)?;

    #[allow(deprecated)]
    let old_workspace_projection = workspace_from_workspace_ref(ctx)?;
    let old_target_base_oid = old_workspace_projection
        .target_commit
        .as_ref()
        .map(|target| target.commit_id)
        .context("failed to get target base oid")?;
    let old_head_oids = old_workspace_projection
        .stacks
        .iter()
        .map(|stack| stack.tip_skip_empty().unwrap_or(old_target_base_oid))
        .collect::<Vec<_>>();
    let old_workspace =
        WorkspaceState::create_from_heads_and_target(repo, &old_head_oids, old_target_base_oid)?;

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

    let workspace_commit = repo
        .find_reference(WORKSPACE_BRANCH_REF)?
        .peel_to_commit()?;
    let mut meta = ctx.meta()?;
    let mut db = ctx.db.get_cache_mut()?;
    let mut workspace = but_graph::Graph::from_commit_traversal(
        workspace_commit.id(),
        Some(gix::refs::FullName::try_from(WORKSPACE_BRANCH_REF)?),
        &meta,
        ctx.project_meta()?,
        &mut db,
        but_graph::init::Options::limited(),
    )?
    .into_workspace()?;
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
    // Nothing below needs the database, and `WorkspaceState::create` re-enters `ctx`
    // for a shared handle - which the exclusive one taken above would refuse.
    drop(db);
    ctx.invalidate_workspace_cache()?;

    // Switch branch to gitbutler/workspace
    update_head_reference(
        repo,
        gix::refs::Target::Symbolic(WORKSPACE_BRANCH_REF.try_into()?),
        false,
        "save edit mode",
        WORKSPACE_BRANCH_REF.into(),
        0,
    )?;

    let new_workspace = WorkspaceState::create(ctx, perm.read_permission())?;
    let uncommitted_changes = get_uncommitted_changes(repo)?;
    let mut merge_repo = ctx.clone_repo_for_merging()?;
    merge_repo
        .config_snapshot_mut()
        .set_value(&gix::config::tree::Merge::CONFLICT_STYLE, "diff3")?;
    let mut merge = move_tree_between_workspaces(
        &merge_repo,
        uncommitted_changes,
        &old_workspace,
        &new_workspace,
    )?;
    let restored_tree_id = merge.tree.write()?.detach();
    // Restored uncommitted changes, including conflicts, must remain unstaged.
    let index = repo.index_from_tree(&repo.head_tree_id()?)?;
    checkout_tree_and_index(repo, tree_id, restored_tree_id, &index, index_lock)?;

    // Write the commit evolution.
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
    let commit_parent_tree = if commit.is_conflicted() {
        but_core::Commit::try_from(commit.clone())?
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

    let (_, index) = get_commit_index(repo, metadata.commit_oid, repo.default_merge_labels())?;

    let mut conflicts = HashMap::<BString, ConflictEntryPresence>::new();
    for entry in index.entries() {
        use gix::index::entry::Stage;
        if entry.stage() == Stage::Unconflicted {
            continue;
        }
        let presence =
            conflicts
                .entry(entry.path(&index).to_owned())
                .or_insert(ConflictEntryPresence {
                    ours: false,
                    theirs: false,
                    ancestor: false,
                });
        match entry.stage() {
            Stage::Base => presence.ancestor = true,
            Stage::Ours => presence.ours = true,
            Stage::Theirs => presence.theirs = true,
            Stage::Unconflicted => unreachable!("unconflicted entries were skipped"),
        }
    }

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
