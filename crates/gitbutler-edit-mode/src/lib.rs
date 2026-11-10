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

/// Which sides of each conflicted path the merge of a conflicted `commit_id` records as index
/// stages, by path. An unconflicted commit has none.
fn conflict_presence(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
) -> Result<HashMap<BString, ConflictEntryPresence>> {
    let Some((base, ours, theirs)) =
        Commit::from_id(commit_id.attach(repo))?.conflicted_tree_ids()?
    else {
        return Ok(HashMap::new());
    };
    let repo = repo.clone().for_tree_diffing()?;
    let mut merge = repo.merge_trees(
        base,
        ours,
        theirs,
        repo.default_merge_labels(),
        repo.tree_merge_options()?,
    )?;
    let mut index = repo.index_from_tree(&merge.tree.write()?)?;
    merge.index_changed_after_applying_conflicts(
        &mut index,
        gix::merge::tree::TreatAsUnresolved::git(),
        gix::merge::tree::apply_index_entries::RemovalMode::Mark,
    );
    let mut presence = HashMap::<BString, ConflictEntryPresence>::new();
    for entry in index.entries() {
        let stage = entry.stage();
        if stage == gix::index::entry::Stage::Unconflicted {
            continue;
        }
        let sides =
            presence
                .entry(entry.path(&index).to_owned())
                .or_insert(ConflictEntryPresence {
                    ours: false,
                    theirs: false,
                    ancestor: false,
                });
        match stage {
            gix::index::entry::Stage::Base => sides.ancestor = true,
            gix::index::entry::Stage::Ours => sides.ours = true,
            gix::index::entry::Stage::Theirs => sides.theirs = true,
            gix::index::entry::Stage::Unconflicted => {}
        }
    }
    Ok(presence)
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

/// Write the files of `commit_id` to the worktree for editing, with its conflicts merged again if
/// it is conflicted. `base_commit_id` is what `HEAD` points to while editing.
fn checkout_edit_commit(
    repo: &gix::Repository,
    commit_id: gix::ObjectId,
    base_commit_id: gix::ObjectId,
) -> Result<()> {
    let commit = commit_id.attach(repo).object()?.try_into_commit()?;
    let base_commit = base_commit_id.attach(repo).object()?.try_into_commit()?;

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
    let labels = gix::merge::blob::builtin_driver::text::Labels {
        ancestor: Some("Common ancestor".into()),
        current: Some(our_label.as_str().into()),
        other: Some(their_label.as_str().into()),
    };

    {
        let repo = repo.clone().for_tree_diffing()?;
        let commit = Commit::from_id(commit_id.attach(&repo))?;
        match commit.conflicted_tree_ids()? {
            Some((base, ours, theirs)) => {
                let mut merge =
                    repo.merge_trees(base, ours, theirs, labels, diff3_options(&repo)?)?;
                for path in conflicted_submodule_paths(&merge) {
                    merge.tree.remove(path.as_bstr())?;
                }
                let tree = merge.tree.write()?.detach();
                but_core::worktree::overwrite_worktree(&repo, tree, Some(&merge))?;
            }
            None => {
                let tree = commit.tree_id_or_auto_resolution()?.detach();
                but_core::worktree::overwrite_worktree(&repo, tree, None)?;
            }
        }
    }
    Ok(())
}

/// Point `HEAD` at a new edit branch at `base_commit_id`.
fn point_head_at_edit_branch(repo: &gix::Repository, base_commit_id: gix::ObjectId) -> Result<()> {
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
    Ok(())
}

/// The paths of unresolved conflicts with a submodule on any side. Nothing is checked out there,
/// so the user puts the side they want in place.
fn conflicted_submodule_paths(merge: &gix::merge::tree::Outcome<'_>) -> Vec<BString> {
    merge
        .conflicts
        .iter()
        .filter(|conflict| conflict.is_unresolved(gix::merge::tree::TreatAsUnresolved::git()))
        .filter(|conflict| {
            conflict
                .entries()
                .into_iter()
                .flatten()
                .any(|entry| entry.mode.is_commit())
        })
        .map(|conflict| conflict.ours.location().to_owned())
        .collect()
}

/// Tree merge options that keep content conflicts as markers in diff3 style, which shows the
/// common ancestor, whatever `merge.conflictStyle` says.
fn diff3_options(repo: &gix::Repository) -> Result<gix::merge::tree::Options> {
    let mut options: gix::merge::plumbing::tree::Options = repo.tree_merge_options()?.into();
    options.blob_merge.text.conflict = gix::merge::blob::builtin_driver::text::Conflict::Keep {
        style: gix::merge::blob::builtin_driver::text::ConflictStyle::Diff3,
        marker_size: gix::merge::blob::builtin_driver::text::Conflict::DEFAULT_MARKER_SIZE
            .try_into()?,
    };
    Ok(options.into())
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

    // Fail before writing anything if the index, or the file it links to, is already locked.
    // The probe is released so checkout can take it.
    drop(
        gix::lock::File::acquire(
            repo.index_path(),
            gix::lock::acquire::Fail::Immediately,
            None,
            0,
            Some(&gix::lock::acquire::resolve_symlink),
        )
        .context("Repository index is locked by another process")?,
    );

    let head_name = repo
        .head_name()?
        .context("HEAD must point to the workspace")?;
    commit_uncommited_changes(repo)?;
    let entered = (|| -> Result<()> {
        let base_commit_id = find_or_create_base_commit(repo, commit_oid)?;
        point_head_at_edit_branch(repo, base_commit_id)?;
        write_edit_mode_metadata(ctx, &edit_mode_metadata).context("Failed to persist metadata")?;
        checkout_edit_commit(repo, commit_oid, base_commit_id)
            .context("Failed to checkout edit branch")
    })();
    if let Err(err) = entered {
        update_head_reference(
            repo,
            gix::refs::Target::Symbolic(head_name.clone()),
            false,
            "abort entering edit mode",
            head_name.as_bstr(),
            0,
        )?;
        cleanup_edit_mode(ctx, repo)?;
        return Err(err);
    }

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
    let uncommited_changes = get_uncommitted_changes(repo)?;
    but_core::worktree::overwrite_worktree(repo, uncommited_changes, None)?;
    point_head_at_workspace(repo, "abort edit mode")?;
    but_core::worktree::reset_index(repo, repo.head_tree_id()?.detach())?;
    cleanup_edit_mode(ctx, repo)?;

    Ok(())
}

/// Merge `uncommitted_changes`, the worktree on top of the `old_workspace` tree when edit mode
/// was entered, onto the `new_workspace` tree, and return the resulting tree. Conflicts stay in it
/// as markers.
fn uncommitted_changes_on_workspace(
    repo: &gix::Repository,
    uncommitted_changes: gix::ObjectId,
    old_workspace: gix::ObjectId,
    new_workspace: gix::ObjectId,
) -> Result<gix::ObjectId> {
    let labels = gix::merge::blob::builtin_driver::text::Labels {
        ancestor: None,
        current: Some("ours".into()),
        other: Some("theirs".into()),
    };
    let mut merge = repo.merge_trees(
        old_workspace,
        uncommitted_changes,
        new_workspace,
        labels,
        diff3_options(repo)?,
    )?;
    Ok(merge.tree.write()?.detach())
}

/// Put the edit-mode worktree `tree` and `index` back after finishing wrote the workspace's files,
/// so edit mode holds what the user left in it.
fn restore_edit_worktree(
    repo: &gix::Repository,
    tree: gix::ObjectId,
    mut index: gix::index::File,
) -> Result<()> {
    but_core::worktree::overwrite_worktree(repo, tree, None)?;
    index.write(Default::default())?;
    Ok(())
}

/// Point `HEAD` at the workspace branch again.
fn point_head_at_workspace(repo: &gix::Repository, reason: &str) -> Result<()> {
    update_head_reference(
        repo,
        gix::refs::Target::Symbolic(WORKSPACE_BRANCH_REF.try_into()?),
        false,
        reason,
        WORKSPACE_BRANCH_REF.into(),
        0,
    )
    .context("Failed to set head reference")?;
    Ok(())
}

pub(crate) fn save_and_return_to_workspace(ctx: &Context, _perm: &mut RepoExclusive) -> Result<()> {
    let edit_mode_metadata = read_edit_mode_metadata(ctx).context("Failed to read metadata")?;
    let repo = &*ctx.repo.get()?;

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
    let new_workspace_tree = {
        let workspace_ref = gix::refs::FullName::try_from(WORKSPACE_BRANCH_REF)?;
        let commit_id = outcome.reference_target(workspace_ref.as_ref())?;
        but_core::Commit::from_id(commit_id.attach(outcome.repo()))?
            .tree_id_or_auto_resolution()?
            .detach()
    };
    let worktree = uncommitted_changes_on_workspace(
        outcome.repo(),
        get_uncommitted_changes(repo)?,
        workspace_commit.tree_id()?.detach(),
        new_workspace_tree,
    )?;
    let edit_index = repo.open_index()?;
    but_core::worktree::overwrite_worktree(outcome.repo(), worktree, None)?;
    if let Err(err) = outcome.materialize_without_checkout() {
        restore_edit_worktree(repo, tree_id, edit_index)
            .context("Failed to restore the edit-mode worktree after saving the rebase failed")?;
        return Err(err);
    }
    ctx.invalidate_workspace_cache()?;
    point_head_at_workspace(repo, "save edit mode")?;
    but_core::worktree::reset_index(repo, repo.head_tree_id()?.detach())?;

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

    let conflicts = conflict_presence(repo, metadata.commit_oid)?;

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
