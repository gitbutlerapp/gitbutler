use std::{cmp::Ordering, io::Read, path::PathBuf};

use anyhow::{Context as _, bail};
use bstr::{BStr, BString, ByteSlice, ByteVec};
use gix::{
    dir::{entry, walk::EmissionMode},
    filter::plumbing::pipeline::convert::ToGitOutcome,
    object::tree::EntryKind,
    status,
    status::{
        index_worktree,
        index_worktree::RewriteSource,
        plumbing::index_as_worktree::{self, EntryStatus},
        tree_index::TrackRenames,
    },
};
use tracing::instrument;

use crate::{
    ChangeState, IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, ModeFlags, TreeChange,
    TreeStatus, UnifiedPatch, WorktreeChanges,
};

/// Identify where a [`TreeChange`] is from.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq)]
enum Origin {
    /// The change was detected when doing a diff between a tree (`HEAD^{tree}`) and an index (`.git/index`).
    TreeIndex,
    /// The change was detected when doing a diff between an index (`.git/index`) and a worktree (working tree, working copy or current checkout).
    IndexWorktree,
}

/// Return [`WorktreeChanges`] that live in the worktree of `repo` that changed and thus can become part of a commit.
///
/// It's equivalent to a `git status` which is "boiled down" into all the changes that one would have to add into `HEAD^{tree}`
/// to get a commit with a tree equal to the current worktree.
#[instrument(skip(repo), err(Debug))]
pub fn worktree_changes(repo: &gix::Repository) -> anyhow::Result<WorktreeChanges> {
    worktree_changes_inner(repo, RenameTracking::Always)
}

/// Just like [`worktree_changes()`], but don't do any rename tracking for performance.
#[instrument(skip(repo), err(Debug))]
pub fn worktree_changes_no_renames(repo: &gix::Repository) -> anyhow::Result<WorktreeChanges> {
    worktree_changes_inner(repo, RenameTracking::Disabled)
}

enum RenameTracking {
    Always,
    Disabled,
}

fn worktree_changes_inner(
    repo: &gix::Repository,
    renames: RenameTracking,
) -> anyhow::Result<WorktreeChanges> {
    let (tree_index_rewrites, worktree_rewrites) = match renames {
        RenameTracking::Always => {
            let rewrites = gix::diff::Rewrites::default(); /* standard Git rewrite handling for everything */
            debug_assert!(
                rewrites.copies.is_none(),
                "TODO: copy tracking needs specific support wherever 'previous_path()' is called."
            );
            (TrackRenames::Given(rewrites), Some(rewrites))
        }
        RenameTracking::Disabled => (TrackRenames::Disabled, None),
    };
    let has_submodule_ignore_configuration = repo.modules()?.is_some_and(|modules| {
        modules
            .names()
            .any(|name| modules.ignore(name).ok().flatten().is_some())
    });
    let status_changes = repo
        .status(gix::progress::Discard)?
        .tree_index_track_renames(tree_index_rewrites)
        .index_worktree_rewrites(worktree_rewrites)
        // Learn about submodule changes, but only do the cheap checks, showing only what we could commit.
        .index_worktree_submodules(if has_submodule_ignore_configuration {
            gix::status::Submodule::AsConfigured { check_dirty: true }
        } else {
            gix::status::Submodule::Given {
                ignore: gix::submodule::config::Ignore::Dirty,
                check_dirty: true,
            }
        })
        .index_worktree_options_mut(|opts| {
            if let Some(opts) = opts.dirwalk_options.as_mut() {
                opts.set_emit_ignored(None)
                    .set_emit_pruned(false)
                    .set_emit_tracked(false)
                    // Don't collapse directories of untracked files, we need each match
                    // (relevant if we had a pathspec), so that we can do rename tracking
                    // on a per-file basis.
                    .set_emit_untracked(EmissionMode::Matching)
                    .set_emit_collapsed(None);
            }
        })
        .into_iter(None)?;

    let work_dir = repo.workdir().context("need non-bare repository")?;
    let mut tmp = Vec::new();
    let mut ignored_changes = Vec::new();
    let mut index_conflicts = Vec::new();
    let mut index_changes = Vec::new();
    for change in status_changes {
        let change = change?;
        let change = match change {
            status::Item::TreeIndex(gix::diff::index::Change::Deletion {
                location,
                index,
                id,
                entry_mode,
            }) => {
                let res = (
                    Origin::TreeIndex,
                    TreeChange {
                        status: TreeStatus::Deletion {
                            previous_state: ChangeState {
                                id: id.clone().into_owned(),
                                kind: into_tree_entry_kind(entry_mode)?,
                            },
                        },
                        path: location.clone().into_owned(),
                    },
                );
                index_changes.push(gix::diff::index::Change::Deletion {
                    location,
                    index,
                    id,
                    entry_mode,
                });
                res
            }
            status::Item::TreeIndex(gix::diff::index::Change::Addition {
                location,
                index,
                entry_mode,
                id,
            }) => {
                let res = (
                    Origin::TreeIndex,
                    TreeChange {
                        path: location.clone().into_owned(),
                        status: TreeStatus::Addition {
                            is_untracked: false,
                            state: ChangeState {
                                id: id.clone().into_owned(),
                                kind: into_tree_entry_kind(entry_mode)?,
                            },
                        },
                    },
                );
                index_changes.push(gix::diff::index::ChangeRef::Addition {
                    location,
                    index,
                    entry_mode,
                    id,
                });
                res
            }
            status::Item::TreeIndex(gix::diff::index::Change::Modification {
                location,
                previous_index,
                previous_entry_mode,
                index,
                entry_mode,
                previous_id,
                id,
            }) => {
                let previous_state = ChangeState {
                    id: previous_id.clone().into_owned(),
                    kind: into_tree_entry_kind(previous_entry_mode)?,
                };
                let state = ChangeState {
                    id: id.clone().into_owned(),
                    kind: into_tree_entry_kind(entry_mode)?,
                };
                let res = (
                    Origin::TreeIndex,
                    TreeChange {
                        path: location.clone().into_owned(),
                        status: TreeStatus::Modification {
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                );
                index_changes.push(gix::diff::index::Change::Modification {
                    location,
                    previous_index,
                    previous_entry_mode,
                    previous_id,
                    index,
                    entry_mode,
                    id,
                });
                res
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::Change(index_as_worktree::Change::Removed),
                ..
            }) => (
                Origin::IndexWorktree,
                TreeChange {
                    path: rela_path,
                    status: TreeStatus::Deletion {
                        previous_state: ChangeState {
                            id: entry.id,
                            kind: into_tree_entry_kind(entry.mode)?,
                        },
                    },
                },
            ),
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::Change(index_as_worktree::Change::Type { worktree_mode }),
                ..
            }) => {
                let previous_state = ChangeState {
                    id: entry.id,
                    kind: into_tree_entry_kind(entry.mode)?,
                };
                let state = ChangeState {
                    // actual state unclear, type changed to something potentially unhashable
                    id: repo.object_hash().null(),
                    kind: into_tree_entry_kind(worktree_mode)?,
                };
                (
                    Origin::IndexWorktree,
                    TreeChange {
                        path: rela_path,
                        status: TreeStatus::Modification {
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                )
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status:
                    EntryStatus::Change(index_as_worktree::Change::Modification {
                        executable_bit_changed,
                        ..
                    }),
                ..
            }) => {
                let kind = into_tree_entry_kind(entry.mode)?;
                let previous_state = ChangeState { id: entry.id, kind };
                let state = ChangeState {
                    id: repo.object_hash().null(),
                    kind: if executable_bit_changed {
                        if kind == EntryKind::BlobExecutable {
                            EntryKind::Blob
                        } else {
                            EntryKind::BlobExecutable
                        }
                    } else {
                        kind
                    },
                };
                (
                    Origin::IndexWorktree,
                    TreeChange {
                        path: rela_path,
                        status: TreeStatus::Modification {
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                )
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::IntentToAdd,
                ..
            }) => (
                Origin::IndexWorktree,
                TreeChange {
                    path: rela_path,
                    // Because `IntentToAdd` stores an empty blob in the index, it's exactly the same diff-result
                    // as if the whole file was added to the index.
                    status: TreeStatus::Addition {
                        state: ChangeState {
                            id: repo.object_hash().null(), /* hash unclear for working tree file */
                            kind: into_tree_entry_kind(entry.mode)?,
                        },
                        is_untracked: false,
                    },
                },
            ),
            status::Item::IndexWorktree(index_worktree::Item::DirectoryContents {
                entry:
                    gix::dir::Entry {
                        rela_path,
                        disk_kind,
                        index_kind,
                        status: gix::dir::entry::Status::Untracked,
                        ..
                    },
                ..
            }) => {
                let kind = disk_kind_to_entry_kind(
                    disk_kind,
                    index_kind,
                    work_dir.join(gix::path::from_bstr(rela_path.as_bstr())),
                )?;
                (
                    Origin::IndexWorktree,
                    TreeChange {
                        path: rela_path,
                        status: TreeStatus::Addition {
                            state: match kind {
                                None => continue,
                                Some(kind) => ChangeState {
                                    id: repo.object_hash().null(),
                                    kind,
                                },
                            },
                            is_untracked: true,
                        },
                    },
                )
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status:
                    EntryStatus::Change(index_as_worktree::Change::SubmoduleModification(
                        submodule_change,
                    )),
                ..
            }) => {
                let Some(checked_out_head_id) = submodule_change.checked_out_head_id else {
                    continue;
                };
                // We can arrive here if the user configures to `ignore = none`, and there are
                // only worktree changes.
                // As we can't do anything with that unless submodules become first-class citizens,
                // we ignore this case for now.
                if entry.id == checked_out_head_id {
                    continue;
                }
                let previous_state = ChangeState {
                    id: entry.id,
                    kind: into_tree_entry_kind(entry.mode)?,
                };
                let state = ChangeState {
                    id: checked_out_head_id,
                    kind: into_tree_entry_kind(entry.mode)?,
                };
                (
                    Origin::IndexWorktree,
                    TreeChange {
                        path: rela_path,
                        status: TreeStatus::Modification {
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                )
            }
            status::Item::IndexWorktree(index_worktree::Item::Rewrite {
                source,
                dirwalk_entry,
                // This ID is usually null, but might be set if used for comparisons.
                // However, this wouldn't mean the object exists.
                dirwalk_entry_id: _,
                ..
            }) => {
                let previous_path: BString = source.rela_path().into();
                let previous_state = match source {
                    RewriteSource::RewriteFromIndex { source_entry, .. } => ChangeState {
                        id: source_entry.id,
                        kind: into_tree_entry_kind(source_entry.mode)?,
                    },
                    RewriteSource::CopyFromDirectoryEntry {
                        source_dirwalk_entry,
                        source_dirwalk_entry_id,
                        ..
                    } => ChangeState {
                        id: source_dirwalk_entry_id,
                        kind: match disk_kind_to_entry_kind(
                            source_dirwalk_entry.disk_kind,
                            source_dirwalk_entry.index_kind,
                            work_dir.join(gix::path::from_bstr(previous_path.as_bstr())),
                        )? {
                            None => continue,
                            Some(kind) => kind,
                        },
                    },
                };
                let state = ChangeState {
                    // Use the worktree version
                    id: repo.object_hash().null(),
                    kind: match disk_kind_to_entry_kind(
                        dirwalk_entry.disk_kind,
                        dirwalk_entry.index_kind,
                        work_dir.join(gix::path::from_bstr(dirwalk_entry.rela_path.as_bstr())),
                    )? {
                        None => continue,
                        Some(kind) => kind,
                    },
                };
                (
                    Origin::IndexWorktree,
                    TreeChange {
                        path: dirwalk_entry.rela_path,
                        status: TreeStatus::Rename {
                            previous_path,
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                )
            }
            status::Item::TreeIndex(gix::diff::index::Change::Rewrite {
                source_location,
                location,
                source_entry_mode,
                source_id,
                entry_mode,
                id,
                ..
            }) => {
                let previous_state = ChangeState {
                    id: source_id.into_owned(),
                    kind: into_tree_entry_kind(source_entry_mode)?,
                };
                let state = ChangeState {
                    id: id.into_owned(),
                    kind: into_tree_entry_kind(entry_mode)?,
                };
                (
                    Origin::TreeIndex,
                    TreeChange {
                        path: location.into_owned(),
                        status: TreeStatus::Rename {
                            previous_path: source_location.into_owned(),
                            previous_state,
                            state,
                            flags: ModeFlags::calculate(&previous_state, &state),
                        },
                    },
                )
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                status: EntryStatus::Conflict { entries, .. },
                ..
            }) => {
                index_conflicts.push((rela_path.clone(), entries));
                ignored_changes.push(IgnoredWorktreeChange {
                    path: rela_path,
                    status: IgnoredWorktreeTreeChangeStatus::Conflict,
                });
                continue;
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
            ) => {
                unreachable!(
                    "we never return these as the status iteration is configured accordingly"
                )
            }
        };
        tmp.push(change);
    }

    tmp.sort_by(|(a_origin, a), (b_origin, b)| {
        cmp_prefer_overlapping(a, b).then(a_origin.cmp(b_origin).reverse())
    });

    let mut last_change = None::<&TreeChange>;
    let mut changes = Vec::<TreeChange>::with_capacity(tmp.len());
    let (mut filter, index) = repo.filter_pipeline(None)?;
    let mut path_check = gix::status::plumbing::SymlinkCheck::new(
        repo.workdir().map(ToOwned::to_owned).context("non-bare")?,
    );
    for (_origin, change) in tmp {
        // At this point we know that the current `change` is the tree/index variant
        // of a prior change between index/worktree.
        if last_change
            .as_ref()
            .is_some_and(|last_change| cmp_prefer_overlapping(last_change, &change).is_eq())
        {
            last_change = None;
            // This is usually two modifications, but it's also possible that
            // This one is a rename. In that case, we want the rename, combined
            // with the current state pointing to the worktree,
            // which we expect to be a modification
            let index_wt_change = changes
                .pop()
                .expect("the reason we are here is the previous change");
            let change_path = change.path.clone();
            let tree_index_change = change;
            let status = match merge_changes(
                tree_index_change,
                index_wt_change,
                &mut filter,
                &index,
                &mut path_check,
            )? {
                [None, None] => IgnoredWorktreeTreeChangeStatus::TreeIndexWorktreeChangeIneffective,
                [Some(merged), None] | [None, Some(merged)] => {
                    changes.push(merged);
                    IgnoredWorktreeTreeChangeStatus::TreeIndex
                }
                [Some(first), Some(second)] => {
                    ignored_changes.push(IgnoredWorktreeChange {
                        path: first.path.clone(),
                        status: IgnoredWorktreeTreeChangeStatus::TreeIndex,
                    });
                    changes.push(first);
                    ignored_changes.push(IgnoredWorktreeChange {
                        path: second.path.clone(),
                        status: IgnoredWorktreeTreeChangeStatus::TreeIndex,
                    });
                    changes.push(second);
                    continue;
                }
            };
            ignored_changes.push(IgnoredWorktreeChange {
                path: change_path,
                status,
            });
            continue;
        }
        changes.push(change);
        last_change = changes.last();
    }

    Ok(WorktreeChanges {
        changes,
        ignored_changes,
        index_changes,
        index_conflicts,
    })
}

fn cmp_prefer_overlapping(a: &TreeChange, b: &TreeChange) -> Ordering {
    if a.path == b.path
        || a.previous_path() == Some(b.path.as_bstr())
        || Some(a.path.as_bstr()) == b.previous_path()
    {
        Ordering::Equal
    } else {
        a.path.cmp(&b.path)
    }
}

/// Merge changes from tree/index into changes of `index_wt` and assure the merged result isn't a no-op,
/// which is when `[None, None]` is returned. Otherwise, `[Some(_), None]` or `[Some(_), Some(_)]` are returned.
/// Note that this case is more expensive as we have to hash the worktree version to check for a no-op.
/// `diff_filter` is used to obtain hashes of worktree content.
fn merge_changes(
    mut tree_index: TreeChange,
    mut index_wt: TreeChange,
    filter: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
) -> anyhow::Result<[Option<TreeChange>; 2]> {
    fn single(change: TreeChange) -> [Option<TreeChange>; 2] {
        [Some(change), None]
    }
    let merged = match (&mut tree_index.status, &mut index_wt.status) {
        (TreeStatus::Modification { .. }, TreeStatus::Addition { .. })
        | (TreeStatus::Deletion { .. }, TreeStatus::Deletion { .. })
        | (TreeStatus::Deletion { .. }, TreeStatus::Rename { .. })
        | (TreeStatus::Deletion { .. }, TreeStatus::Modification { .. })
        | (
            TreeStatus::Deletion { .. },
            TreeStatus::Addition {
                is_untracked: false,
                ..
            },
        )
        | (TreeStatus::Addition { .. }, TreeStatus::Addition { .. }) => {
            bail!(
                "BUG: entered unreachable code with tree_index_change = {:?} and index_wt_change = {:?}",
                tree_index.status.kind(),
                index_wt.status.kind()
            );
        }
        (
            TreeStatus::Addition {
                is_untracked,
                state,
            },
            TreeStatus::Modification {
                state: state_wt, ..
            },
        ) => {
            *is_untracked = true;
            *state = *state_wt;
            return Ok(single(tree_index));
        }
        (TreeStatus::Addition { .. }, TreeStatus::Deletion { .. }) => {
            // keep the most recent known state, which is from the index.
            return Ok(single(index_wt));
        }
        (
            TreeStatus::Addition { state, .. },
            TreeStatus::Rename {
                previous_state: ps_wt,
                ..
            },
        ) => {
            // This is conflicting actually, and a little bit unclear what committing this will do.
            // Pretend the added file (in index) is the one that was deleted, hence the rename.
            *ps_wt = *state;
            // Can't be no-op as this is a rename
            return Ok(single(index_wt));
        }
        (
            TreeStatus::Deletion { previous_state, .. },
            TreeStatus::Addition {
                is_untracked: true,
                state,
            },
        ) => {
            index_wt.status = TreeStatus::Modification {
                previous_state: *previous_state,
                state: *state,
                flags: None,
            };
            index_wt
        }
        (
            TreeStatus::Modification { previous_state, .. },
            TreeStatus::Modification {
                previous_state: ps_wt,
                ..
            },
        ) => {
            *ps_wt = *previous_state;
            index_wt
        }
        (TreeStatus::Modification { .. }, TreeStatus::Deletion { .. }) => {
            return Ok(single(index_wt));
        }
        (
            TreeStatus::Modification { previous_state, .. },
            TreeStatus::Rename {
                previous_state: ps_wt,
                ..
            },
        ) => {
            *ps_wt = *previous_state;
            index_wt
        }
        (
            TreeStatus::Rename {
                state: state_index, ..
            },
            TreeStatus::Modification {
                state: state_wt, ..
            },
        ) => {
            *state_index = *state_wt;
            return Ok(single(tree_index));
        }
        (
            TreeStatus::Rename {
                previous_path,
                previous_state,
                ..
            },
            TreeStatus::Rename {
                previous_path: pp_wt,
                previous_state: ps_wt,
                ..
            },
        ) => {
            // The worktree-rename is dominating, but we can combine both
            // so there is the indexed version as source, and the one in the worktree
            // as destination.
            *pp_wt = std::mem::take(previous_path);
            *ps_wt = *previous_state;
            return Ok(single(index_wt));
        }
        (
            TreeStatus::Rename {
                previous_path,
                previous_state,
                ..
            },
            TreeStatus::Deletion { .. },
        ) => {
            // Destination is deleted as well, so what's left is a deletion of the source.
            return Ok(single(TreeChange {
                path: std::mem::take(previous_path),
                status: TreeStatus::Deletion {
                    previous_state: *previous_state,
                },
            }));
        }
        (
            TreeStatus::Rename {
                state: state_index, ..
            },
            TreeStatus::Addition {
                state: state_wt, ..
            },
        ) => {
            return Ok([
                Some(TreeChange {
                    path: tree_index.path,
                    status: TreeStatus::Addition {
                        state: *state_index,
                        // It's untracked as we know the destination isn't in the tree yet.
                        // It's just in the index, which to us doesn't exist.
                        is_untracked: true,
                    },
                }),
                Some(TreeChange {
                    path: index_wt.path,
                    status: TreeStatus::Addition {
                        state: *state_wt,
                        // In theory, this should be considered tracked even though it's object file isn't
                        // in the index (but in the tree) as this is the source of the rename.
                        // However, doing so would trip up diffing code that uses this flag to know where to
                        // read the initial state of a file from.
                        is_untracked: true,
                    },
                }),
            ]);
        }
    };

    let current = id_or_hash_from_worktree(
        merged.status.state(),
        merged.path.as_bstr(),
        filter,
        index,
        path_check,
    )?;
    let (prev_state, prev_path) = merged
        .status
        .previous_state_and_path()
        .map(|(a, b)| (Some(a), b))
        .unwrap_or((None, None));
    let previous = id_or_hash_from_worktree(
        prev_state,
        prev_path.unwrap_or(merged.path.as_bstr()),
        filter,
        index,
        path_check,
    )?;
    Ok(if current == previous {
        [None, None]
    } else {
        single(merged)
    })
}

// TODO(gix): worktree-status already can do hashing and to-git conversions while dealing with links, but it's not exposed.
//            Make this easier in Gix.
/// Produces hashes for comparing states, or produce a hash from what's on disk if no hash is available.
/// Note that null-hash will be returned if no directory entry is available.
fn id_or_hash_from_worktree(
    change: Option<ChangeState>,
    rela_path: &BStr,
    filter: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
) -> anyhow::Result<gix::ObjectId> {
    let Some(change) = change else {
        return Ok(index.object_hash().null());
    };
    if !change.id.is_null() {
        return Ok(change.id);
    }

    let path = path_check.verified_path_allow_nonexisting(rela_path)?;
    let md = path.symlink_metadata()?;
    let repo = filter.repo;
    let id = if md.is_file() {
        let to_git = filter.convert_to_git(
            std::fs::File::open(path)?,
            // TODO(gix): definitely use `ToCompoents` here to avoid these conversions.
            //            Whatever you do, it's never right, so must be abstract.
            &gix::path::try_from_bstr(rela_path)?,
            index,
        )?;
        match to_git {
            ToGitOutcome::Unchanged(mut stream) => gix::objs::compute_stream_hash(
                repo.object_hash(),
                gix::object::Kind::Blob,
                &mut stream,
                md.len(),
                &mut gix::progress::Discard,
                &gix::interrupt::IS_INTERRUPTED,
            )?,
            ToGitOutcome::Process(mut stream) => {
                let mut buf = repo.empty_reusable_buffer();
                stream.read_to_end(&mut buf)?;
                gix::objs::compute_hash(repo.object_hash(), gix::object::Kind::Blob, &buf)?
            }
            ToGitOutcome::Buffer(buf) => {
                gix::objs::compute_hash(repo.object_hash(), gix::object::Kind::Blob, buf)?
            }
        }
    } else if md.is_symlink() {
        let bytes = gix::path::os_string_into_bstring(std::fs::read_link(path)?.into())?;
        gix::objs::compute_hash(repo.object_hash(), gix::object::Kind::Blob, &bytes)?
    } else {
        bail!("Cannot hash directory entries that aren't files or symlinks");
    };
    Ok(id)
}

fn into_tree_entry_kind(mode: gix::index::entry::Mode) -> anyhow::Result<EntryKind> {
    Ok(mode
        .to_tree_entry_mode()
        .with_context(|| format!("Entry contained invalid entry mode: {mode:?}"))?
        .kind())
}

/// Most importantly, this function allows to skip over untrackable entries, like named pipes, sockets and character devices, just like Git.
/// `path` is needed for now while we have to stat the file again to learn about the executable bits.
// TODO: remove `path` and provide the stat information or at least executable info with `gitoxide` - it has that info.
fn disk_kind_to_entry_kind(
    disk_kind: Option<gix::dir::entry::Kind>,
    index_kind: Option<gix::dir::entry::Kind>,
    path: PathBuf,
) -> anyhow::Result<Option<EntryKind>> {
    Ok(Some(
        match disk_kind
            .or(index_kind)
            .context("Didn't have any type information for untracked item")?
        {
            entry::Kind::Repository => EntryKind::Commit,
            entry::Kind::Directory => {
                unreachable!("BUG: we use 'matching' so there are no directories")
            }
            entry::Kind::Untrackable => return Ok(None),
            entry::Kind::File => {
                let md = path.symlink_metadata()?;
                if gix::fs::is_executable(&md) {
                    EntryKind::BlobExecutable
                } else {
                    EntryKind::Blob
                }
            }
            entry::Kind::Symlink => EntryKind::Link,
        },
    ))
}

/// Unified diffs
impl TreeChange {
    /// Like [`Self::unified_patch()`], but also provides the file header for diffs like this:
    ///
    /// ```patch
    /// --- a/README.md
    /// +++ b/README.md
    /// @@ -1,5 +1,5 @@
    /// -This is old text.
    /// +This is new text.
    ///  More content...
    /// ```
    ///
    /// Return `None` if this change cannot produce a diff, typically because a submodule is involved.
    ///
    /// Warning: we return binary-to-text conversions as patches, so these diffs aren't usable for actual patching,
    /// as they also remove the information about such filter, and it's unclear to the caller if they ran at all.
    // TODO: also add mode information, but it's not super trivial to decide on a good format.
    pub fn unified_diff(
        &self,
        repo: &gix::Repository,
        context_lines: u32,
    ) -> anyhow::Result<Option<BString>> {
        fn prefixed_path_line(prefix: &str, path: &BString, out: &mut BString) {
            out.push_str(prefix);
            out.push_str(path);
            out.push(b'\n');
        }

        let mut out = BString::default();
        match &self.status {
            TreeStatus::Addition { .. } => {
                out.push_str("--- /dev/null\n");
                prefixed_path_line("+++ b/", &self.path, &mut out);
            }
            TreeStatus::Deletion { .. } => {
                prefixed_path_line("+++ a/", &self.path, &mut out);
                out.push_str("--- /dev/null\n");
            }
            TreeStatus::Modification { .. } => {
                prefixed_path_line("--- a/", &self.path, &mut out);
                prefixed_path_line("+++ b/", &self.path, &mut out);
            }
            TreeStatus::Rename { previous_path, .. } => {
                out.push_str("rename from ");
                out.push_str(previous_path);
                out.push(b'\n');

                out.push_str("rename to ");
                out.push_str(&self.path);
                out.push(b'\n');
            }
        }
        match self.unified_patch(repo, context_lines)? {
            Some(UnifiedPatch::Patch { hunks, .. }) => {
                for hunk in hunks {
                    out.push_str(&hunk.diff);
                }
            }
            None => {}
            _ => return Ok(None),
        }
        Ok(Some(out))
    }

    /// Obtain a unified diff by comparing the previous and current state of this change, using `repo` to retrieve objects or
    /// for obtaining a working tree to read files from disk.
    /// Note that the mount of lines of context around each hunk are currently hardcoded to `3` as it *might* be relevant for creating
    /// commits later.
    /// Return `None` if this change cannot produce a diff, typically because a submodule is involved.
    /// Note that this format only contains hunk-headers and the patches themselves, not the file header.
    ///
    /// ### Example
    ///
    /// ```diff
    /// @@ -1,5 +1,5 @@
    /// -This is old text.
    /// +This is new text.
    ///  More content...
    /// ```
    ///
    /// Note that the file header is missing, so the following is *not* present.
    /// ```
    /// --- a/README.md
    /// +++ b/README.md
    /// ```
    pub fn unified_patch(
        &self,
        repo: &gix::Repository,
        context_lines: u32,
    ) -> anyhow::Result<Option<UnifiedPatch>> {
        let mut diff_filter = crate::unified_diff::filter_from_state(
            repo,
            self.status.state(),
            UnifiedPatch::CONVERSION_MODE,
        )?;
        self.unified_patch_with_filter(repo, context_lines, &mut diff_filter)
    }

    /// Like [`Self::unified_patch()`], but uses `diff_filter` to control the content used for the diff.
    pub fn unified_patch_with_filter(
        &self,
        repo: &gix::Repository,
        context_lines: u32,
        diff_filter: &mut gix::diff::blob::Platform,
    ) -> anyhow::Result<Option<UnifiedPatch>> {
        match &self.status {
            TreeStatus::Deletion { previous_state } => UnifiedPatch::compute_with_filter(
                repo,
                self.path.as_bstr(),
                None,
                None,
                *previous_state,
                context_lines,
                diff_filter,
            ),
            TreeStatus::Addition {
                state,
                is_untracked: _,
            } => UnifiedPatch::compute_with_filter(
                repo,
                self.path.as_bstr(),
                None,
                *state,
                None,
                context_lines,
                diff_filter,
            ),
            TreeStatus::Modification {
                state,
                previous_state,
                flags: _,
            } => UnifiedPatch::compute_with_filter(
                repo,
                self.path.as_bstr(),
                None,
                *state,
                *previous_state,
                context_lines,
                diff_filter,
            ),
            TreeStatus::Rename {
                previous_path,
                previous_state,
                state,
                flags: _,
            } => UnifiedPatch::compute_with_filter(
                repo,
                self.path.as_bstr(),
                Some(previous_path.as_bstr()),
                *state,
                *previous_state,
                context_lines,
                diff_filter,
            ),
        }
    }
}

impl std::fmt::Debug for WorktreeChanges {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorktreeChanges")
            .field("changes", &self.changes)
            .field("ignored_changes", &self.ignored_changes)
            .finish()
    }
}
