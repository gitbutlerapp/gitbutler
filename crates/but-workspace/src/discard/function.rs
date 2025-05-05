use std::os::unix::fs::PermissionsExt;

use crate::{
    commit_engine::{DiffSpec, HunkHeader, apply_hunks, index::apply_lhs_to_rhs},
    discard::{
        DiscardSpec,
        hunk::{HunkSubstraction, subtract_hunks},
    },
};
use anyhow::Context;
use bstr::ByteSlice;
use but_core::{ChangeState, TreeStatus};
use gix::status::index_worktree;

use super::file::checkout_repo_worktree;

/// Same as create_index_without_changes, but specifically for the worktree.
///
/// The index will be written to the repository if any changes are made to it.
pub fn discard_workspace_changes(
    repository: &gix::Repository,
    changes: impl IntoIterator<Item = DiscardSpec>,
    context_lines: u32,
) -> anyhow::Result<Vec<DiscardSpec>> {
    let (tree, dropped) =
        create_tree_without_diff(repository, ChangesSource::Worktree, changes, context_lines)?;

    update_wd_to_tree(repository, tree)?;
    let tree_index = repository.index_from_tree(&tree)?;
    let mut real_index =
        match repository.open_index() {
            Ok(index) => Ok(index),
            Err(err) => match err {
                gix::worktree::open_index::Error::IndexFile(gix::index::file::init::Error::Io(
                    ..,
                )) => Ok(repository
                    .index_from_tree(&gix::ObjectId::empty_tree(gix::hash::Kind::Sha1))?),
                err => Err(err),
            },
        }?;

    apply_lhs_to_rhs(
        repository.workdir().context("non-bare repository")?,
        &tree_index,
        &mut real_index,
    )?;

    real_index.write(Default::default())?;

    Ok(dropped)
}

fn update_wd_to_tree(
    repository: &gix::Repository,
    source_tree: gix::ObjectId,
) -> anyhow::Result<()> {
    let source_tree = repository.find_tree(source_tree)?;
    let wd_tree = create_wd_tree(repository, 0)?;
    let wt_changes = but_core::diff::tree_changes(repository, Some(wd_tree), source_tree.id)?;

    let mut path_check = gix::status::plumbing::SymlinkCheck::new(
        repository.workdir().context("non-bare repository")?.into(),
    );

    for change in wt_changes.0 {
        match &change.status {
            TreeStatus::Deletion { .. } => {
                // Work tree has the file but the source tree doesn't.
                std::fs::remove_file(path_check.verified_path(&change.path)?)?;
            }
            TreeStatus::Addition { .. } => {
                let entry = source_tree
                    .lookup_entry(change.path.clone().split_str("/"))?
                    .context("path must exist")?;
                // Work tree doesn't have the file but the source tree does.
                write_entry(
                    change.path.as_bstr(),
                    &entry,
                    &mut path_check,
                    WriteKind::Addition,
                )?;
            }
            TreeStatus::Modification { .. } => {
                let entry = source_tree
                    .lookup_entry(change.path.clone().split_str("/"))?
                    .context("path must exist")?;
                // Work tree doesn't have the file but the source tree does.
                write_entry(
                    change.path.as_bstr(),
                    &entry,
                    &mut path_check,
                    WriteKind::Modification,
                )?;
            }
            TreeStatus::Rename { previous_path, .. } => {
                let entry = source_tree
                    .lookup_entry(change.path.clone().split_str("/"))?
                    .context("path must exist")?;
                // Work tree has the file under `previous_path`, but the source tree wants it under `path`.
                let previous_path = path_check.verified_path(previous_path)?;
                if std::path::Path::new(&previous_path).is_dir() {
                    std::fs::remove_dir_all(previous_path)?;
                } else {
                    std::fs::remove_file(previous_path)?;
                }
                write_entry(
                    change.path.as_bstr(),
                    &entry,
                    &mut path_check,
                    WriteKind::Addition,
                )?;
            }
        }
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
enum WriteKind {
    Addition,
    Modification,
}

fn write_entry(
    relative_path: &bstr::BStr,
    entry: &gix::object::tree::Entry<'_>,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    write_kind: WriteKind,
) -> anyhow::Result<()> {
    match entry.mode().kind() {
        gix::objs::tree::EntryKind::Tree => {
            unreachable!(
                "The tree changes produced from the diff will always be a file-like entry"
            );
        }
        gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
            let mut blob = entry.object()?.into_blob();
            let path = path_check.verified_path(relative_path)?;
            prepare_path(path)?;
            std::fs::write(path, blob.take_data())?;
            #[cfg(unix)]
            {
                if entry.mode().kind() == gix::objs::tree::EntryKind::BlobExecutable {
                    let mut permissions = std::fs::metadata(path)?.permissions();
                    // Set the executable bit
                    permissions.set_mode(permissions.mode() | 0o111);
                    std::fs::set_permissions(path, permissions)?;
                } else {
                    let mut permissions = std::fs::metadata(path)?.permissions();
                    // Unset the executable bit
                    permissions.set_mode(permissions.mode() & !0o111);
                    std::fs::set_permissions(path, permissions)?;
                }
            }
        }
        gix::objs::tree::EntryKind::Link => {
            let blob = entry.object()?.into_blob();
            let link_target = gix::path::from_bstr(blob.data.as_bstr());
            let path = path_check.verified_path(relative_path)?;
            prepare_path(path)?;
            gix::fs::symlink::create(&link_target, path)?;
        }
        gix::objs::tree::EntryKind::Commit => match write_kind {
            WriteKind::Modification => {
                let path = path_check.verified_path(relative_path)?;
                let out = std::process::Command::from(
                    gix::command::prepare(format!(
                        "git reset --hard {id} && git clean -fxd",
                        id = entry.id()
                    ))
                    .with_shell(),
                )
                .current_dir(path)
                .output()?;
                if !out.status.success() {
                    anyhow::bail!(
                        "Could not reset submodule at '{sm_dir}' to commit {id}: {err}",
                        sm_dir = path.display(),
                        id = entry.id(),
                        err = out.stderr.as_bstr()
                    );
                }
            }
            WriteKind::Addition => {
                let sm_repo = entry
                    .repo
                    .submodules()?
                    .into_iter()
                    .flatten()
                    .find_map(|sm| {
                        let is_active = sm.is_active().ok()?;
                        is_active.then(|| -> anyhow::Result<_> {
                            Ok(
                                if sm
                                    .path()
                                    .ok()
                                    .is_some_and(|sm_path| sm_path == relative_path)
                                {
                                    sm.open()?
                                } else {
                                    None
                                },
                            )
                        })
                    })
                    .transpose()?
                    .flatten();
                match sm_repo {
                    None => {
                        // A directory is what git creates with `git restore` even if the thing to restore is a submodule.
                        // We are trying to be better than that if we find a submodule, hoping that this is what users expect.
                        // We do that as baseline as there is no need to fail here.
                    }
                    Some(repo) => {
                        // We will only restore the submodule if there is a local clone already available, to avoid any network
                        // activity that would likely happen during an actual clone.
                        // Thus, all we have to do is to check out the submodule.
                        // TODO(gix): find a way to deal with nested submodules - they should also be checked out which
                        //            isn't done by `gitoxide`, but probably should be an option there.

                        let wt_root = path_check.inner.root().to_owned();
                        checkout_repo_worktree(&wt_root, repo)?;
                    }
                }
                let path = path_check.verified_path(relative_path)?;
                std::fs::create_dir(path).or_else(|err| {
                    if err.kind() == std::io::ErrorKind::AlreadyExists {
                        Ok(())
                    } else {
                        Err(err)
                    }
                })?;
            }
        },
    };

    Ok(())
}

fn prepare_path(path: &std::path::Path) -> anyhow::Result<()> {
    let parent = path.parent().context("paths will always have a parent")?;
    if std::fs::exists(parent)? {
        if !std::path::Path::new(&parent).is_dir() {
            std::fs::remove_file(parent)?;
            std::fs::create_dir_all(parent)?;
        }
    } else {
        std::fs::create_dir_all(parent)?;
    }
    if std::fs::exists(path)? {
        if std::path::Path::new(&path).is_dir() {
            std::fs::remove_dir_all(path)?;
        } else {
            std::fs::remove_file(path)?;
        }
    }
    Ok(())
}

pub enum ChangesSource {
    Worktree,
    Commit {
        id: gix::ObjectId,
    },
    Tree {
        after_id: gix::ObjectId,
        before_id: gix::ObjectId,
    },
}

impl ChangesSource {
    fn before<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Worktree => {
                Ok(repository.find_tree(repository.head_tree_id_or_empty()?)?)
            }
            ChangesSource::Commit { id } => {
                let commit = repository.find_commit(*id)?;
                let parent_id = commit.parent_ids().next().context("no parent")?;
                let parent = repository.find_commit(parent_id)?;
                Ok(parent.tree()?)
            }
            ChangesSource::Tree { before_id, .. } => Ok(repository.find_tree(*before_id)?),
        }
    }

    fn after<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Worktree => {
                let wd_tree = create_wd_tree(repository, 0)?;
                Ok(repository.find_tree(wd_tree)?)
            }
            ChangesSource::Commit { id } => Ok(repository.find_commit(*id)?.tree()?),
            ChangesSource::Tree { after_id, .. } => Ok(repository.find_tree(*after_id)?),
        }
    }
}

/// Discard the given `changes` in either the work tree or an arbitrary commit or tree. If a change could not be matched with an
/// actual worktree change, for instance due to a race, that's not an error, instead it will be returned in the result Vec, along
/// with all hunks that couldn't be matched.
///
/// The returned Vec is typically empty, meaning that all `changes` could be discarded.
///
/// `context_lines` is the amount of context lines we should assume when obtaining hunks of worktree changes to match against
/// the ones we have specified in the hunks contained within `changes`.
///
/// Discarding a change is really more of an 'undo' of a change as it will restore the previous state to the desired extent - Git
/// doesn't have a notion of this on a whole-file basis.
///
/// Each of the `changes` will be matched against actual worktree changes to make this operation as safe as possible, after all, it
/// discards changes without recovery.
///
/// In practice, this is like a selective 'inverse-checkout', as such it must have a lot of the capabilities of checkout, but focussed
/// on just a couple of paths, and with special handling for renamed files, something that `checkout` can't naturally handle
/// as it's only dealing with single file-paths.
///
/// ### Hunk-based discarding
///
/// When an instance in `changes` contains hunks, these are the hunks to be discarded. If they match a whole hunk in the worktree changes,
/// it will be discarded entirely, simply by not applying it.
///
/// ### Sub-Hunk discarding
///
/// It's possible to specify ranges of hunks to discard. To do that, they need an *anchor*. The *anchor* is the pair of
/// `(line_number, line_count)` that should not be changed, paired with the *other* pair with the new `(line_number, line_count)`
/// to discard.
///
/// For instance, when there is a single patch `-1,10 +1,10` and we want to bring back the removed 5th line *and* the added 5th line,
/// we'd specify *just* two selections, one in the old via `-5,1 +1,10` and one in the new via `-1,10 +5,1`.
/// This works because internally, it will always match the hunks (and sub-hunks) with their respective pairs obtained through a
/// worktree status.
pub fn create_tree_without_diff(
    repository: &gix::Repository,
    changes_source: ChangesSource,
    changes_to_discard: impl IntoIterator<Item = DiscardSpec>,
    context_lines: u32,
) -> anyhow::Result<(gix::ObjectId, Vec<DiscardSpec>)> {
    let mut dropped = Vec::new();

    let before = changes_source.before(repository)?;
    let after = changes_source.after(repository)?;

    let mut builder = repository.edit_tree(after.id())?;

    for change in changes_to_discard {
        let before_path = change
            .previous_path
            .clone()
            .unwrap_or_else(|| change.path.clone());
        let before_entry = before.lookup_entry(before_path.clone().split_str("/"))?;

        let Some(after_entry) = after.lookup_entry(change.path.clone().split_str("/"))? else {
            let Some(before_entry) = before_entry else {
                // If there is no before entry and no after entry, then
                // something has gone wrong.
                dropped.push(change);
                continue;
            };

            if change.hunk_headers.is_empty() {
                // If there is no after_change, then it must have been deleted.
                // Therefore, we can just add it again.
                builder.upsert(
                    change.path.as_bstr(),
                    before_entry.mode().kind(),
                    before_entry.object_id(),
                )?;
                continue;
            } else {
                anyhow::bail!(
                    "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
                );
            }
        };

        match after_entry.mode().kind() {
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                let after_blob = after_entry.object()?.into_blob();
                if change.hunk_headers.is_empty() {
                    revert_file_to_before_state(&before_entry, &mut builder, &change)?;
                } else {
                    let Some(before_entry) = before_entry else {
                        anyhow::bail!(
                            "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
                        );
                    };

                    let diff = but_core::UnifiedDiff::compute(
                        repository,
                        change.path.as_bstr(),
                        Some(before_path.as_bstr()),
                        ChangeState {
                            id: after_entry.id().detach(),
                            kind: after_entry.mode().kind(),
                        },
                        ChangeState {
                            id: before_entry.id().detach(),
                            kind: before_entry.mode().kind(),
                        },
                        context_lines,
                    )?;

                    let but_core::UnifiedDiff::Patch {
                        hunks: diff_hunks, ..
                    } = diff
                    else {
                        anyhow::bail!("expected a patch");
                    };

                    let mut good_hunk_headers = vec![];
                    let mut bad_hunk_headers = vec![];

                    for hunk in &change.hunk_headers {
                        if diff_hunks
                            .iter()
                            .any(|diff_hunk| HunkHeader::from(diff_hunk.clone()).contains(*hunk))
                        {
                            good_hunk_headers.push(*hunk);
                        } else {
                            bad_hunk_headers.push(*hunk);
                        }
                    }

                    if !bad_hunk_headers.is_empty() {
                        dropped.push(DiscardSpec::from(DiffSpec {
                            previous_path: change.previous_path.clone(),
                            path: change.path.clone(),
                            hunk_headers: bad_hunk_headers,
                        }));
                    }

                    // TODO: Validate that the hunks coorespond with actual changes?
                    let before_blob = before_entry.object()?.into_blob();

                    let new_hunks = new_hunks_after_removals(
                        diff_hunks.into_iter().map(Into::into).collect(),
                        good_hunk_headers,
                    )?;
                    let new_after_contents = apply_hunks(
                        before_blob.data.as_bstr(),
                        after_blob.data.as_bstr(),
                        &new_hunks,
                    )?;
                    let mode = if new_after_contents == before_blob.data {
                        before_entry.mode().kind()
                    } else {
                        after_entry.mode().kind()
                    };
                    let new_after_contents = repository.write_blob(&new_after_contents)?;

                    // Keep the mode of the after state. We _should_ at some
                    // point introduce the mode specifically as part of the
                    // DiscardSpec, but for now, we can just use the after state.
                    builder.upsert(change.path.as_bstr(), mode, new_after_contents)?;
                }
            }
            _ => {
                revert_file_to_before_state(&before_entry, &mut builder, &change)?;
            }
        }
    }

    let final_tree = builder.write()?;
    Ok((final_tree.detach(), dropped))
}

fn new_hunks_after_removals(
    change_hunks: Vec<HunkHeader>,
    mut removal_hunks: Vec<HunkHeader>,
) -> anyhow::Result<Vec<HunkHeader>> {
    // If a removal hunk matches completly then we can drop it entirely.
    let hunks_to_keep: Vec<HunkHeader> = change_hunks
        .into_iter()
        .filter(|hunk| {
            match removal_hunks
                .iter()
                .enumerate()
                .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
            {
                None => true,
                Some(idx_to_remove) => {
                    removal_hunks.remove(idx_to_remove);
                    false
                }
            }
        })
        .collect();

    // TODO(perf): instead of brute-force searching, assure hunks_to_discard are sorted and speed up the search that way.
    let mut hunks_to_keep_with_splits = Vec::new();
    for hunk_to_split in hunks_to_keep {
        let mut subtractions = Vec::new();
        removal_hunks.retain(|sub_hunk_to_discard| {
            if sub_hunk_to_discard.old_range() == hunk_to_split.old_range() {
                subtractions.push(HunkSubstraction::New(sub_hunk_to_discard.new_range()));
                false
            } else if sub_hunk_to_discard.new_range() == hunk_to_split.new_range() {
                subtractions.push(HunkSubstraction::Old(sub_hunk_to_discard.old_range()));
                false
            } else {
                true
            }
        });
        if subtractions.is_empty() {
            hunks_to_keep_with_splits.push(hunk_to_split);
        } else {
            let hunk_with_subtractions = subtract_hunks(hunk_to_split, subtractions)?;
            hunks_to_keep_with_splits.extend(hunk_with_subtractions);
        }
    }
    Ok(hunks_to_keep_with_splits)
}

fn revert_file_to_before_state(
    before_entry: &Option<gix::object::tree::Entry<'_>>,
    builder: &mut gix::object::tree::Editor<'_>,
    change: &DiscardSpec,
) -> Result<(), anyhow::Error> {
    // If there are no hunk headers, then we want to revert the
    // whole file to the state it was in before tree.
    if let Some(before_entry) = before_entry {
        builder.remove(change.path.as_bstr())?;
        builder.upsert(
            change
                .previous_path
                .clone()
                .unwrap_or(change.path.clone())
                .as_bstr(),
            before_entry.mode().kind(),
            before_entry.object_id(),
        )?;
    } else {
        builder.remove(change.path.as_bstr())?;
    }
    Ok(())
}

/// Creates a tree containing the uncommited changes in the project.
/// This includes files in the index that are considered conflicted.
fn create_wd_tree(
    repo: &gix::Repository,
    untracked_limit_in_bytes: u64,
) -> anyhow::Result<gix::ObjectId> {
    use bstr::ByteSlice;
    use gix::bstr::BStr;
    use gix::dir::walk::EmissionMode;
    use gix::status;
    use gix::status::plumbing::index_as_worktree::{Change, EntryStatus};
    use gix::status::tree_index::TrackRenames;
    use std::collections::HashSet;

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
        .into_iter(None)?;

    let mut worktreepaths_changed = HashSet::new();
    // We have to apply untracked items last, but don't have ordering here so impose it ourselves.
    let mut untracked_items = Vec::new();
    for change in status_changes {
        let change = change?;
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
                    | EntryStatus::Conflict(_)
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
