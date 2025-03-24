use crate::discard::{DiscardSpec, file};
use anyhow::Context;
use bstr::ByteSlice;
use but_core::{ChangeState, TreeStatus};

/// Discard the given `changes` in the worktree of `repo`. If a change could not be matched with an actual worktree change, for
/// instance due to a race, that's not an error, instead it will be returned in the result Vec.
/// The returned Vec is typically empty, meaning that all `changes` could be discarded.
///
/// Discarding a change is really more of an 'undo' of a change as it will restore the previous state to the desired extent - Git
/// doesn't have a notion of this.
///
/// Each of the `changes` will be matched against actual worktree changes to make this operation as safe as possible, after all, it
/// discards changes without recovery.
///
/// In practice, this is like a selective 'inverse-checkout', as such it must have a lot of the capabilities of checkout, but focussed
/// on just a couple of paths, and with special handling for renamed files, something that `checkout` can't naturally handle
/// as it's only dealing with single file-paths.
pub fn discard_workspace_changes(
    repo: &gix::Repository,
    changes: impl IntoIterator<Item = DiscardSpec>,
) -> anyhow::Result<Vec<DiscardSpec>> {
    let wt_changes = but_core::diff::worktree_changes(repo)?;
    let mut dropped = Vec::new();
    let mut index = repo.index_or_empty()?.into_owned_or_cloned();
    let mut initial_entries_len = index.entries().len();
    let (mut pipeline, _) = repo.filter_pipeline(Some(repo.empty_tree().id))?;
    let head_tree = repo.head_tree_id_or_empty()?.object()?.into_tree();

    let mut path_check = gix::status::plumbing::SymlinkCheck::new(
        repo.workdir().context("non-bare repository")?.into(),
    );
    for spec in changes {
        let Some(wt_change) = wt_changes.changes.iter().find(|c| {
            c.path == spec.path
                && c.previous_path() == spec.previous_path.as_ref().map(|p| p.as_bstr())
        }) else {
            dropped.push(spec);
            continue;
        };

        if spec.hunk_headers.is_empty() {
            match wt_change.status {
                TreeStatus::Addition { is_untracked, .. } => {
                    std::fs::remove_file(
                        path_check
                            .verified_path(&gix::path::from_bstr(wt_change.path.as_bstr()))?,
                    )?;
                    if !is_untracked {
                        file::index::mark_entry_for_deletion(
                            &mut index,
                            wt_change.path.as_bstr(),
                            initial_entries_len,
                        );
                    }
                    if let Some(entry) =
                        head_tree.lookup_entry(wt_change.path.split(|b| *b == b'/'))?
                    {
                        file::restore_state_to_worktree(
                            &mut pipeline,
                            &mut index,
                            wt_change.path.as_bstr(),
                            ChangeState {
                                id: entry.object_id(),
                                kind: entry.mode().into(),
                            },
                            file::RestoreMode::Deleted,
                            &mut path_check,
                            &mut initial_entries_len,
                        )?
                    }
                }
                TreeStatus::Deletion { previous_state } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        wt_change.path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Deleted,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                }
                TreeStatus::Modification { previous_state, .. } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        wt_change.path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Update,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                }
                TreeStatus::Rename {
                    ref previous_path,
                    previous_state,
                    ..
                } => {
                    file::restore_state_to_worktree(
                        &mut pipeline,
                        &mut index,
                        previous_path.as_bstr(),
                        previous_state,
                        file::RestoreMode::Deleted,
                        &mut path_check,
                        &mut initial_entries_len,
                    )?;
                    file::purge_and_restore_from_head_tree(
                        &mut index,
                        wt_change.path.as_bstr(),
                        &mut path_check,
                        initial_entries_len,
                    )?;
                }
            }
        } else {
            todo!("hunk-based undo")
        }
    }

    let has_removals_or_updates = index.entries().iter().any(|e| {
        e.flags
            .intersects(gix::index::entry::Flags::REMOVE | gix::index::entry::Flags::UPDATE)
    });
    if has_removals_or_updates {
        index.remove_tree();
        index.remove_resolve_undo();
        // Always sort, we currently don't keep track of wether this is truly required
        // and checking the amount of entries isn't safe in light of conflicts (that may get removed).
        index.sort_entries();
        index.write(Default::default())?;
    }
    Ok(dropped)
}
