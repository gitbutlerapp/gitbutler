use crate::{
    IgnoredWorktreeChange, IgnoredWorktreeTreeChangeStatus, ModeFlags, TreeChange, UnifiedDiff,
    WorktreeChanges,
};
use anyhow::Context;
use bstr::{BString, ByteSlice};
use gix::dir::entry;
use gix::dir::walk::EmissionMode;
use gix::object::tree::EntryKind;
use gix::status;
use gix::status::index_worktree;
use gix::status::index_worktree::RewriteSource;
use gix::status::plumbing::index_as_worktree::{self, EntryStatus};
use gix::status::tree_index::TrackRenames;
use serde::{Deserialize, Serialize};

/// Identify where a [`TreeChange`] is from.
#[derive(Debug, Clone, Copy, Ord, PartialOrd, Eq, PartialEq, Serialize)]
enum Origin {
    /// The change was detected when doing a diff between a tree (`HEAD^{tree}`) and an index (`.git/index`).
    TreeIndex,
    /// The change was detected when doing a diff between an index (`.git/index`) and a worktree (working tree, working copy or current checkout).
    IndexWorktree,
}

/// Specifically defines a [`TreeChange`].
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "subject")]
pub enum TreeStatus {
    /// Something was added or scheduled to be added.
    Addition {
        /// The current state of what was added or will be added
        state: ChangeState,
        /// If `true`, this is a future addition from an untracked file, a file that wasn't yet added to the index (`.git/index`).
        #[serde(rename = "isUntracked")]
        is_untracked: bool,
    },
    /// Something was deleted.
    Deletion {
        /// The that Git stored before the deletion.
        #[serde(rename = "previousState")]
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
        /// The that Git stored before the modification.
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
        /// Derived information based on the mode of both states.
        flags: Option<ModeFlags>,
    },
    /// An entry was renamed from `previous_path` to its current location.
    ///
    /// Note that this may include a content change, as well as a change of the executable bit.
    Rename {
        /// The path relative to the repository at which the entry was previously located.
        #[serde(rename = "previousPath", with = "gitbutler_serde::bstring_lossy")]
        previous_path: BString,
        /// The that Git stored before the modification.
        #[serde(rename = "previousState")]
        previous_state: ChangeState,
        /// The current state, i.e. the modification itself.
        state: ChangeState,
        /// Derived information based on the mode of both states.
        flags: Option<ModeFlags>,
    },
}

/// Something that fully identifies the state of a [`TreeChange`].
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ChangeState {
    /// The content of the committable.
    ///
    /// If [`null`](gix::ObjectId::is_null), the current state isn't known which can happen
    /// if this state is living in the worktree and has never been hashed.
    #[serde(with = "gitbutler_serde::object_id")]
    pub id: gix::ObjectId,
    /// The kind of the committable.
    pub kind: EntryKind,
}

/// Return [`WorktreeChanges`] that live in the worktree of `repo` that changed and thus can become part of a commit.
pub fn changes(repo: &gix::Repository) -> anyhow::Result<WorktreeChanges> {
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

    let mut tmp = Vec::new();
    let mut ignored_changes = Vec::new();
    for change in status_changes {
        let change = change?;
        let change = match change {
            status::Item::TreeIndex(gix::diff::index::Change::Deletion {
                location,
                id,
                entry_mode,
                ..
            }) => (
                Origin::TreeIndex,
                TreeChange {
                    status: TreeStatus::Deletion {
                        previous_state: ChangeState {
                            id: id.into_owned(),
                            kind: into_tree_entry_kind(entry_mode)?,
                        },
                    },
                    path: location.into_owned(),
                },
            ),
            status::Item::TreeIndex(gix::diff::index::Change::Addition {
                location,
                entry_mode,
                id,
                ..
            }) => (
                Origin::TreeIndex,
                TreeChange {
                    path: location.into_owned(),
                    status: TreeStatus::Addition {
                        is_untracked: false,
                        state: ChangeState {
                            id: id.into_owned(),
                            kind: into_tree_entry_kind(entry_mode)?,
                        },
                    },
                },
            ),
            status::Item::TreeIndex(gix::diff::index::Change::Modification {
                location,
                previous_entry_mode,
                entry_mode,
                previous_id,
                id,
                ..
            }) => {
                let previous_state = ChangeState {
                    id: previous_id.into_owned(),
                    kind: into_tree_entry_kind(previous_entry_mode)?,
                };
                let state = ChangeState {
                    id: id.into_owned(),
                    kind: into_tree_entry_kind(entry_mode)?,
                };
                (
                    Origin::TreeIndex,
                    TreeChange {
                        path: location.into_owned(),
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
            }) => (
                Origin::IndexWorktree,
                TreeChange {
                    path: rela_path,
                    status: TreeStatus::Addition {
                        state: match disk_kind_to_entry_kind(disk_kind, index_kind)? {
                            None => continue,
                            Some(kind) => ChangeState {
                                id: repo.object_hash().null(),
                                kind,
                            },
                        },
                        is_untracked: true,
                    },
                },
            ),
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
                dirwalk_entry_id,
                ..
            }) => {
                let previous_path = source.rela_path().into();
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
                        )? {
                            None => continue,
                            Some(kind) => kind,
                        },
                    },
                };
                let state = ChangeState {
                    id: dirwalk_entry_id,
                    kind: match disk_kind_to_entry_kind(
                        dirwalk_entry.disk_kind,
                        dirwalk_entry.index_kind,
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
                status: EntryStatus::Conflict(_conflict),
                ..
            }) => {
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
        a.path.cmp(&b.path).then(a_origin.cmp(b_origin).reverse())
    });

    let mut last_path = None;
    let mut changes = Vec::with_capacity(tmp.len());
    for (_origin, change) in tmp {
        if last_path.as_ref() == Some(&change.path) {
            ignored_changes.push(IgnoredWorktreeChange {
                path: change.path,
                status: IgnoredWorktreeTreeChangeStatus::TreeIndex,
            });
            continue;
        }
        last_path = Some(change.path.clone());
        changes.push(change);
    }

    Ok(WorktreeChanges {
        changes,
        ignored_changes,
    })
}

fn into_tree_entry_kind(mode: gix::index::entry::Mode) -> anyhow::Result<EntryKind> {
    Ok(mode
        .to_tree_entry_mode()
        .with_context(|| format!("Entry contained invalid entry mode: {mode:?}"))?
        .kind())
}

/// Most importantly, this function allows to skip over untrackable entries, like named pipes, sockets and character devices, just like Git.
fn disk_kind_to_entry_kind(
    disk_kind: Option<gix::dir::entry::Kind>,
    index_kind: Option<gix::dir::entry::Kind>,
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
            entry::Kind::File => EntryKind::Blob,
            entry::Kind::Symlink => EntryKind::Link,
        },
    ))
}

impl TreeChange {
    /// Obtain a unified diff by comparing the previous and current state of this change, using `repo` to retrieve objects or
    /// for obtaining a working tree to read files from disk.
    /// Note that the mount of lines of context around each hunk are currently hardcoded to `3` as it *might* be relevant for creating
    /// commits later.
    pub fn unified_diff(&self, repo: &gix::Repository) -> anyhow::Result<UnifiedDiff> {
        const CONTEXT_LINES: u32 = 3;
        match &self.status {
            TreeStatus::Deletion { previous_state } => UnifiedDiff::compute(
                repo,
                self.path.as_bstr(),
                None,
                None,
                *previous_state,
                CONTEXT_LINES,
            ),
            TreeStatus::Addition {
                state,
                is_untracked: _,
            } => UnifiedDiff::compute(repo, self.path.as_bstr(), None, *state, None, CONTEXT_LINES),
            TreeStatus::Modification {
                state,
                previous_state,
                flags: _,
            } => UnifiedDiff::compute(
                repo,
                self.path.as_bstr(),
                None,
                *state,
                *previous_state,
                CONTEXT_LINES,
            ),
            TreeStatus::Rename {
                previous_path,
                previous_state,
                state,
                flags: _,
            } => UnifiedDiff::compute(
                repo,
                self.path.as_bstr(),
                Some(previous_path.as_bstr()),
                *state,
                *previous_state,
                CONTEXT_LINES,
            ),
        }
    }
}
