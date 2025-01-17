use crate::WorktreeChange;
use anyhow::Context;
use bstr::BString;
use gix::dir::entry;
use gix::dir::walk::EmissionMode;
use gix::object::tree::EntryKind;
use gix::status;
use gix::status::index_worktree;
use gix::status::index_worktree::RewriteSource;
use gix::status::plumbing::index_as_worktree::{self, EntryStatus};
use gix::status::tree_index::TrackRenames;
use serde::Serialize;

/// Identify where a [`WorktreeChange`] is from.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize)]
pub enum Origin {
    /// The change was detected when doing a diff between a tree (`HEAD^{tree}`) and an index (`.git/index`).
    TreeIndex,
    /// The change was detected when doing a diff between an index (`.git/index`) and a worktree (working tree, working copy or current checkout).
    IndexWorktree,
}

/// Specifically defines a [`WorktreeChange`].
#[derive(Debug, Clone)]
pub enum Status {
    /// The *index entry* is in a conflicting state, which means the *worktree* can be in one of many states,
    /// but none of which is the one the user might desire as they didn't specify it yet.
    Conflict(gix::status::plumbing::index_as_worktree::Conflict),
    /// A file that was never tracked by Git.
    Untracked {
        /// The kind of file if it was tracked, with unknown content.
        state: ChangeState,
    },
    /// Something was added or scheduled to be added.
    Addition {
        /// Where the addition was registered.
        ///
        /// * If [`Origin::IndexWorktree`], then `state` is the current state on disk as it may be added to the index.
        /// * If [`Origin::TreeIndex`], then `state` is what has been added to the index and what should be in the next commit.
        origin: Origin,
        /// The current state of what was added or will be added
        state: ChangeState,
    },
    /// Something was deleted.
    Deletion {
        /// Where the deletion was registered.
        ///
        /// * If [`Origin::IndexWorktree`], then `previous_state` is what was recorded in the index, and the working tree file was deleted.
        /// * If [`Origin::TreeIndex`], then `previous_state` is what was recorded in `HEAD^{tree}` and the entry from the index was deleted.
        origin: Origin,
        /// The that Git stored before the deletion.
        previous_state: ChangeState,
    },
    /// A tracked entry was modified, which might mean:
    ///
    /// * the content change, i.e. a file was changed
    /// * the type changed, a file is now a symlink or something else
    /// * the executable bit changed, so a file is now executable, or isn't anymore.
    ///
    /// This change may sit in the index if `origin` is [`Origin::TreeIndex`] or in the worktree if `origin` is [`Origin::IndexWorktree`].
    ///
    /// Note that a modification may be applied in both `origin`s, along with other possible combinations of *two* status changes to the same path.
    Modification {
        /// Where the modification was registered.
        origin: Origin,
        /// The that Git stored before the modification.
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
    },
    /// An entry was renamed from `previous_path` to its current location.
    ///
    /// Note that this may include a content change, as well as a change of the executable bit.
    Rename {
        /// Where the modification was registered.
        origin: Origin,
        /// The path relative to the repository at which the entry was previously located.
        previous_path: BString,
        /// The that Git stored before the modification.
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
    },
}

impl Status {
    /// Return the [origin](Origin) of this instance, or extrapolate it if it's not explicitly set.
    pub fn origin(&self) -> Origin {
        match self {
            Status::Conflict(_) | Status::Untracked { .. } => Origin::IndexWorktree,
            Status::Addition { origin, .. }
            | Status::Deletion { origin, .. }
            | Status::Modification { origin, .. }
            | Status::Rename { origin, .. } => *origin,
        }
    }
}

/// Something that fully identifies the state of a [`WorktreeChange`].
#[derive(Debug, Clone, Copy)]
pub struct ChangeState {
    /// The content of the committable.
    ///
    /// If [`null`](gix::ObjectId::is_null), the current state isn't known which can happen
    /// if this state is living in the worktree and has never been hashed.
    pub id: gix::ObjectId,
    /// The kind of the committable.
    pub kind: gix::object::tree::EntryKind,
}

/// Return a list of [`WorktreeChange`] that live in the worktree of `repo` that changed and thus can become part of a commit.
/// Note that the changes are returned by path, and such that [tree-index](Origin::TreeIndex) changes are happening *after*
/// [index-worktree](Origin::IndexWorktree) changes.
///
/// ### Important: possibly non-unique paths
///
/// We return entries of different [kinds](Origin), and a path could be changed in the worktree,
/// but it could also have been staged with a different change in the index.
///
/// The [`Origin`] determines which diff was performed to learn about the [`WorktreeChange`].
pub fn changes(repo: &gix::Repository) -> anyhow::Result<Vec<WorktreeChange>> {
    let rewrites = Default::default(); /* standard Git rewrite handling for everything */
    let status_changes = repo
        .status(gix::progress::Discard)?
        .tree_index_track_renames(TrackRenames::Given(rewrites))
        .index_worktree_rewrites(rewrites)
        // Learn about submodule changes, but only do the cheap checks, showing only what we could commit.
        .index_worktree_submodules(gix::status::Submodule::Given {
            ignore: gix::submodule::config::Ignore::Dirty,
            check_dirty: true,
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

    let mut out = Vec::new();
    for change in status_changes {
        let change = change?;
        let change = match change {
            status::Item::TreeIndex(gix::diff::index::Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            }) => WorktreeChange {
                status: Status::Deletion {
                    origin: Origin::TreeIndex,
                    previous_state: ChangeState {
                        id: id.into_owned(),
                        kind: into_tree_entry_kind(entry_mode)?,
                    },
                },
                path: location.into_owned(),
            },
            status::Item::TreeIndex(gix::diff::index::Change::Addition {
                location,
                entry_mode,
                id,
                ..
            }) => WorktreeChange {
                path: location.into_owned(),
                status: Status::Addition {
                    origin: Origin::TreeIndex,
                    state: ChangeState {
                        id: id.into_owned(),
                        kind: into_tree_entry_kind(entry_mode)?,
                    },
                },
            },
            status::Item::TreeIndex(gix::diff::index::Change::Modification {
                location,
                previous_entry_mode,
                entry_mode,
                previous_id,
                id,
                ..
            }) => WorktreeChange {
                path: location.into_owned(),
                status: Status::Modification {
                    origin: Origin::TreeIndex,
                    previous_state: ChangeState {
                        id: previous_id.into_owned(),
                        kind: into_tree_entry_kind(previous_entry_mode)?,
                    },
                    state: ChangeState {
                        id: id.into_owned(),
                        kind: into_tree_entry_kind(entry_mode)?,
                    },
                },
            },
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::Change(index_as_worktree::Change::Removed),
                ..
            }) => WorktreeChange {
                path: rela_path,
                status: Status::Deletion {
                    origin: Origin::IndexWorktree,
                    previous_state: ChangeState {
                        id: entry.id,
                        kind: into_tree_entry_kind(entry.mode)?,
                    },
                },
            },
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::Change(index_as_worktree::Change::Type { worktree_mode }),
                ..
            }) => WorktreeChange {
                path: rela_path,
                status: Status::Modification {
                    origin: Origin::IndexWorktree,
                    previous_state: ChangeState {
                        id: entry.id,
                        kind: into_tree_entry_kind(entry.mode)?,
                    },
                    state: ChangeState {
                        // actual state unclear, type changed to something potentially unhashable
                        id: repo.object_hash().null(),
                        kind: into_tree_entry_kind(worktree_mode)?,
                    },
                },
            },
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
                WorktreeChange {
                    path: rela_path,
                    status: Status::Modification {
                        origin: Origin::IndexWorktree,
                        previous_state: ChangeState { id: entry.id, kind },
                        state: ChangeState {
                            id: repo.object_hash().null(),
                            kind: if executable_bit_changed {
                                if kind == gix::object::tree::EntryKind::BlobExecutable {
                                    gix::object::tree::EntryKind::Blob
                                } else {
                                    gix::object::tree::EntryKind::BlobExecutable
                                }
                            } else {
                                kind
                            },
                        },
                    },
                }
            }
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status: EntryStatus::IntentToAdd,
                ..
            }) => WorktreeChange {
                path: rela_path,
                // Because `IntentToAdd` stores an empty blob in the index, it's exactly the same diff-result
                // as if the whole file was added to the index.
                status: Status::Addition {
                    origin: Origin::IndexWorktree,
                    state: ChangeState {
                        id: repo.object_hash().null(), /* hash unclear for working tree file */
                        kind: into_tree_entry_kind(entry.mode)?,
                    },
                },
            },
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
            }) => WorktreeChange {
                path: rela_path,
                status: Status::Untracked {
                    state: match disk_kind_to_entry_kind(disk_kind, index_kind)? {
                        None => continue,
                        Some(kind) => ChangeState {
                            id: repo.object_hash().null(),
                            kind,
                        },
                    },
                },
            },
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                entry,
                status:
                    EntryStatus::Change(index_as_worktree::Change::SubmoduleModification(change)),
                ..
            }) => {
                let Some(checked_out_head_id) = change.checked_out_head_id else {
                    continue;
                };
                WorktreeChange {
                    path: rela_path,
                    status: Status::Modification {
                        origin: Origin::IndexWorktree,
                        previous_state: ChangeState {
                            id: entry.id,
                            kind: into_tree_entry_kind(entry.mode)?,
                        },
                        state: ChangeState {
                            id: checked_out_head_id,
                            kind: into_tree_entry_kind(entry.mode)?,
                        },
                    },
                }
            }
            status::Item::IndexWorktree(index_worktree::Item::Rewrite {
                source,
                dirwalk_entry,
                dirwalk_entry_id,
                ..
            }) => WorktreeChange {
                path: dirwalk_entry.rela_path,
                status: Status::Rename {
                    origin: Origin::IndexWorktree,
                    previous_path: source.rela_path().into(),
                    previous_state: match source {
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
                            )? {
                                None => continue,
                                Some(kind) => kind,
                            },
                        },
                    },
                    state: ChangeState {
                        id: dirwalk_entry_id,
                        kind: match disk_kind_to_entry_kind(
                            dirwalk_entry.disk_kind,
                            dirwalk_entry.index_kind,
                        )? {
                            None => continue,
                            Some(kind) => kind,
                        },
                    },
                },
            },
            status::Item::TreeIndex(gix::diff::index::Change::Rewrite {
                source_location,
                location,
                source_entry_mode,
                source_id,
                entry_mode,
                id,
                ..
            }) => WorktreeChange {
                path: location.into_owned(),
                status: Status::Rename {
                    origin: Origin::TreeIndex,
                    previous_path: source_location.into_owned(),
                    previous_state: ChangeState {
                        id: source_id.into_owned(),
                        kind: into_tree_entry_kind(source_entry_mode)?,
                    },
                    state: ChangeState {
                        id: id.into_owned(),
                        kind: into_tree_entry_kind(entry_mode)?,
                    },
                },
            },
            status::Item::IndexWorktree(index_worktree::Item::Modification {
                rela_path,
                status: EntryStatus::Conflict(conflict),
                ..
            }) => WorktreeChange {
                path: rela_path,
                status: Status::Conflict(conflict),
            },

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
        out.push(change);
    }

    out.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.status.origin().cmp(&b.status.origin()).reverse())
    });
    Ok(out)
}

fn into_tree_entry_kind(
    mode: gix::index::entry::Mode,
) -> anyhow::Result<gix::object::tree::EntryKind> {
    Ok(mode
        .to_tree_entry_mode()
        .with_context(|| format!("Entry contained invalid entry mode: {mode:?}"))?
        .kind())
}

/// Most importantly, this function allows to skip over untrackable entries, like named pipes, sockets and character devices, just like Git.
fn disk_kind_to_entry_kind(
    disk_kind: Option<gix::dir::entry::Kind>,
    index_kind: Option<gix::dir::entry::Kind>,
) -> anyhow::Result<Option<gix::object::tree::EntryKind>> {
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
            entry::Kind::File => EntryKind::Blob,
            entry::Kind::Symlink => EntryKind::Link,
        },
    ))
}
