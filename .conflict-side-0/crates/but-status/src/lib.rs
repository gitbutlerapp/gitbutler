use gix::status::index_worktree;

/// Creates a tree containing the uncommited changes in the project.
/// This includes files in the index that are considered conflicted.
///
/// TODO: This is a copy of `create_wd_tree` from the old world. Ideally we
///       should share between the old and new worlds to prevent duplication between
///       these.
pub fn create_wd_tree(
    repo: &gix::Repository,
    untracked_limit_in_bytes: u64,
) -> anyhow::Result<gix::ObjectId> {
    use std::collections::HashSet;

    use bstr::ByteSlice;
    use gix::{
        bstr::BStr,
        status,
        status::plumbing::index_as_worktree::{Change, EntryStatus},
    };

    let (mut pipeline, index) = repo.filter_pipeline(None)?;
    let mut added_worktree_file = |rela_path: &BStr,
                                   head_tree_editor: &mut gix::object::tree::Editor<'_>|
     -> anyhow::Result<bool> {
        let Some((id, kind, md)) = pipeline.worktree_file_to_object(rela_path, &index)? else {
            head_tree_editor.remove(rela_path)?;
            return Ok(false);
        };
        if untracked_limit_in_bytes != 0 && md.len() > untracked_limit_in_bytes {
            return Ok(false);
        }
        head_tree_editor.upsert(rela_path, kind, id)?;
        Ok(true)
    };
    let head_tree = repo.head_tree_id_or_empty()?;
    let mut head_tree_editor = repo.edit_tree(head_tree)?;
    let status_changes = get_status(repo)?;

    let mut worktreepaths_changed = HashSet::new();
    // We have to apply untracked items last, but don't have ordering here so impose it ourselves.
    let mut untracked_items = Vec::new();
    for change in status_changes {
        match change {
            status::Item::TreeIndex(gix::diff::index::Change::Deletion { location, .. }) => {
                // These changes play second fiddle - they are overwritten by worktree-changes,
                // or we assure we don't overwrite, as we may arrive out of order.
                if !worktreepaths_changed.contains(location.as_bstr()) {
                    head_tree_editor.remove(location.as_ref())?;
                }
            }
            status::Item::TreeIndex(
                gix::diff::index::Change::Addition {
                    location,
                    entry_mode,
                    id,
                    ..
                }
                | gix::diff::index::Change::Modification {
                    location,
                    entry_mode,
                    id,
                    ..
                },
            ) => {
                if let Some(entry_mode) = entry_mode
                    .to_tree_entry_mode()
                    // These changes play second fiddle - they are overwritten by worktree-changes,
                    // or we assure we don't overwrite, as we may arrive out of order.
                    .filter(|_| !worktreepaths_changed.contains(location.as_bstr()))
                {
                    head_tree_editor.upsert(location.as_ref(), entry_mode.kind(), id.as_ref())?;
                }
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                status: EntryStatus::Change(Change::Removed),
                ..
            }) => {
                head_tree_editor.remove(rela_path.as_bstr())?;
                worktreepaths_changed.insert(rela_path);
            }
            // modified, conflicted, or untracked files are unconditionally added as blob.
            // Note that this implementation will re-read the whole blob even on type-change
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                status:
                    EntryStatus::Change(Change::Type { .. } | Change::Modification { .. })
                    | EntryStatus::Conflict { .. }
                    | EntryStatus::IntentToAdd,
                ..
            }) => {
                if added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)? {
                    worktreepaths_changed.insert(rela_path);
                }
            }
            status::Item::IndexWorktree(index_worktree::Item::DirectoryContents {
                entry:
                    gix::dir::Entry {
                        rela_path,
                        status: gix::dir::entry::Status::Untracked,
                        ..
                    },
                ..
            }) => {
                untracked_items.push(rela_path);
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                status: EntryStatus::Change(Change::SubmoduleModification(change)),
                ..
            }) => {
                if let Some(possibly_changed_head_commit) = change.checked_out_head_id {
                    head_tree_editor.upsert(
                        rela_path.as_bstr(),
                        gix::object::tree::EntryKind::Commit,
                        possibly_changed_head_commit,
                    )?;
                    worktreepaths_changed.insert(rela_path);
                }
            }
            status::Item::IndexWorktree(index_worktree::Item::Rewrite { .. })
            | status::Item::TreeIndex(gix::diff::index::Change::Rewrite { .. }) => {
                unreachable!("disabled")
            }
            status::Item::IndexWorktree(
                index_worktree::Item::Modification {
                    status: EntryStatus::NeedsUpdate(_),
                    ..
                }
                | index_worktree::Item::DirectoryContents {
                    entry:
                        gix::dir::Entry {
                            status:
                                gix::dir::entry::Status::Tracked
                                | gix::dir::entry::Status::Pruned
                                | gix::dir::entry::Status::Ignored(_),
                            ..
                        },
                    ..
                },
            ) => {}
        }
    }

    for rela_path in untracked_items {
        added_worktree_file(rela_path.as_ref(), &mut head_tree_editor)?;
    }

    let tree_oid = head_tree_editor.write()?;
    Ok(tree_oid.detach())
}

/// Gets the status of a given repository.
pub fn get_status(repo: &gix::Repository) -> anyhow::Result<Vec<gix::status::Item>> {
    use gix::{dir::walk::EmissionMode, status::tree_index::TrackRenames};

    let status_changes = repo
        .status(gix::progress::Discard)?
        .tree_index_track_renames(TrackRenames::Disabled)
        .index_worktree_rewrites(None)
        .index_worktree_submodules(gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::Dirty,
            check_dirty: true,
        })
        .index_worktree_options_mut(|opts| {
            if let Some(opts) = opts.dirwalk_options.as_mut() {
                opts.set_emit_ignored(None)
                    .set_emit_pruned(false)
                    .set_emit_tracked(false)
                    .set_emit_untracked(EmissionMode::Matching)
                    .set_emit_collapsed(None);
            }
        })
        .into_iter(None)?
        .filter_map(|change| change.ok())
        .collect::<Vec<_>>();

    Ok(status_changes)
}
