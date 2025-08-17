use bstr::BString;
use but_graph::VirtualBranchesTomlMetadata;
use std::collections::BTreeSet;

/// A way to determine what should be included in the snapshot when calling [create_tree()](function::create_tree).
#[derive(Debug, Clone)]
pub struct State {
    /// The result of a previous worktree changes call, but [the one **without** renames](but_core::diff::worktree_changes_no_renames()).
    ///
    /// It contains detailed information about the complete set of possible changes to become part of the worktree.
    pub changes: but_core::WorktreeChanges,
    /// Repository-relative and slash-separated paths that match any change in the  [`changes`](State::changes) field.
    /// It is *not* error if there is no match, as there can be snapshots without working tree changes, but with other changes.
    /// It's up to the caller to check for that via [`Outcome::is_empty()`].
    pub selection: BTreeSet<BString>,
    /// If `true`, store the current `HEAD` reference, i.e. its target, as well as the targets of all refs it's pointing to by symbolic link.
    pub head: bool,
}

/// Contains all state that the snapshot contains.
#[derive(Debug, Copy, Clone)]
pub struct Outcome {
    /// The snapshot itself, with all the subtrees available that are also listed in this structure.
    pub snapshot_tree: gix::ObjectId,
    /// For good measure, the input `HEAD^{tree}` that is used as the basis to learn about worktree changes.
    pub head_tree: gix::ObjectId,
    /// The `head_tree`  with the selected worktree changes applied, suitable for being stored in a commit,
    /// or `None` if there was no change in the worktree.
    pub worktree: Option<gix::ObjectId>,
    /// The tree representing the current changed index, without conflicts, or `None` if there was no change to the index.
    pub index: Option<gix::ObjectId>,
    /// A tree with files in a custom storage format to allow keeping conflicting blobs reachable, along with detailed conflict information
    /// to allow restoring the conflict entries in the index.
    pub index_conflicts: Option<gix::ObjectId>,
    /// The tree representing the reference targets of all references within the *workspace*.
    pub workspace_references: Option<gix::ObjectId>,
    /// The tree representing the reference targets of all references reachable from `HEAD`, so typically `HEAD` itself, and the
    /// target object of the reference it is pointing to.
    pub head_references: Option<gix::ObjectId>,
    /// The tree representing the metadata of all references within the *workspace*.
    pub metadata: Option<gix::ObjectId>,
}

impl Outcome {
    /// Return `true` if the snapshot contains no information whatsoever, which is equivalent to being an empty tree.
    pub fn is_empty(&self) -> bool {
        self.snapshot_tree.is_empty_tree()
    }
}

/// A utility to more easily use *no* workspace or metadata.
pub fn no_workspace_and_meta() -> Option<(
    &'static but_graph::projection::Workspace<'static>,
    &'static VirtualBranchesTomlMetadata,
)> {
    None
}

pub(super) mod function {
    use super::{Outcome, State};
    use crate::{DiffSpec, commit_engine};
    use anyhow::{Context, bail};
    use bstr::{BString, ByteSlice};
    use but_core::{ChangeState, RefMetadata};
    use gix::diff::index::Change;
    use gix::object::tree::EntryKind;
    use gix::status::plumbing::index_as_worktree::EntryStatus;
    use std::collections::BTreeSet;
    use tracing::instrument;

    /// Create a tree that represents the snapshot for the given `selection`, whereas the basis for these changes
    /// is the `head_tree_id` *(i.e. the tree to which `HEAD` is ultimately pointing to)* -
    /// make this an empty tree if the `HEAD` is unborn.
    /// It's valid to have no changes to write, which is when the snapshot won't contain any worktree information
    /// to the point where it may be entirely [empty](Outcome::is_empty()).
    ///
    /// If `workspace_and_meta` is not `None`, the workspace and metadata to store in the snapshot.
    /// We will only store reference positions, and assume that their commits are safely stored in the reflog to not
    /// be garbage collected. Metadata is only stored for the references that are included in the `workspace`.
    ///
    /// Note that objects will be written into the repository behind `head_tree_id` unless it's configured
    /// to keep everything in memory.
    ///
    /// ### Snapshot Tree Format
    ///
    /// There are the following top-level trees, with their own sub-formats which aren't specified here.
    /// However, it's notable that they have to be implemented so that they remain compatible to prior versions
    /// of the tree.
    ///
    /// Note that all top-level entries are optional, and only present if there is a snapshot to store.
    ///
    /// * `HEAD`
    ///     - the tree to which `HEAD` was pointing at the time the snapshot was created.
    ///     - this is relevant when re-applying the worktree-changes and when recreating the `index`.
    ///     - only set if it is needed to restore some of the snapshot state.
    /// * `worktree`
    ///     - the tree of `HEAD + uncommitted files`. Technically this means that now possibly untracked files are known to Git,
    ///       even though it might be that the respective objects aren't written to disk yet.
    ///     - Note that this tree may contain files with conflict markers as it will pick up the conflicting state visible on disk.
    /// * `index`
    ///     - A representation of the non-conflicting and changed portions of the index, without its meta-data.
    ///     - may be empty if only conflicts exist.
    /// * `index-conflicts`
    ///     - `<entry-path>/[1,2,3]` - the blobs at their respective stages.
    #[instrument(skip(changes, _workspace_and_meta), err(Debug))]
    pub fn create_tree(
        head_tree_id: gix::Id<'_>,
        State {
            changes,
            selection,
            head: _,
        }: State,
        _workspace_and_meta: Option<(&but_graph::projection::Workspace, &impl RefMetadata)>,
    ) -> anyhow::Result<Outcome> {
        // Assure this is a tree early.
        let head_tree = head_tree_id.object()?.into_tree();
        let repo = head_tree_id.repo;
        let mut changes_to_apply: Vec<_> = changes
            .changes
            .iter()
            .filter(|c| selection.contains(&c.path))
            .map(|c| Ok(DiffSpec::from(c)))
            .collect();
        changes_to_apply.extend(
            changes
                .ignored_changes
                .iter()
                .filter_map(|c| match &c.status_item {
                    Some(gix::status::Item::IndexWorktree(
                        gix::status::index_worktree::Item::Modification {
                            status: EntryStatus::Conflict { .. },
                            rela_path,
                            ..
                        },
                    )) => Some(rela_path),
                    _ => None,
                })
                .filter(|rela_path| selection.contains(rela_path.as_bstr()))
                .map(|rela_path| {
                    // Create a pretend-addition to pick up conflicted paths as well.
                    Ok(DiffSpec::from(but_core::TreeChange {
                        path: rela_path.to_owned(),
                        status: but_core::TreeStatus::Addition {
                            state: ChangeState {
                                id: repo.object_hash().null(),
                                // This field isn't relevant when entries are read from disk.
                                kind: EntryKind::Tree,
                            },
                            is_untracked: true,
                        },
                        status_item: None,
                    }))
                }),
        );

        let (new_tree, base_tree) = commit_engine::tree::apply_worktree_changes(
            head_tree_id.into(),
            repo,
            &mut changes_to_apply,
            0, /* context lines don't matter */
        )?;

        let rejected = changes_to_apply
            .into_iter()
            .filter_map(Result::err)
            .collect::<Vec<_>>();
        if !rejected.is_empty() {
            bail!(
                "It should be impossible to fail to apply changes that are in the tree that was provided as HEAD^{{tree}} - {rejected:?}"
            )
        }

        let mut edit = repo.empty_tree().edit()?;

        let worktree = (new_tree != base_tree).then_some(new_tree.detach());
        let mut needs_head = false;
        if let Some(worktree) = worktree {
            edit.upsert("worktree", EntryKind::Tree, worktree)?;
            needs_head = true;
        }

        let (index, index_conflicts) = snapshot_index(&mut edit, head_tree, changes, selection)?
            .inspect(|(index, index_conflicts)| {
                needs_head |= index_conflicts.is_some() && index.is_none();
            })
            .unwrap_or_default();

        if needs_head {
            edit.upsert("HEAD", EntryKind::Tree, head_tree_id)?;
        }

        Ok(Outcome {
            snapshot_tree: edit.write()?.into(),
            head_tree: head_tree_id.detach(),
            worktree,
            index,
            index_conflicts,
            workspace_references: None,
            head_references: None,
            metadata: None,
        })
    }

    /// `snapshot_tree` is the tree into which our `index` and `index-conflicts` trees are written. These will also be returned
    /// if they were written.
    ///
    /// `base_tree_id` is the tree from which a clean index can be created, and which we will edit to incorporate the
    /// non-conflicting index changes.
    fn snapshot_index(
        snapshot_tree: &mut gix::object::tree::Editor,
        base_tree: gix::Tree,
        changes: but_core::WorktreeChanges,
        selection: BTreeSet<BString>,
    ) -> anyhow::Result<Option<(Option<gix::ObjectId>, Option<gix::ObjectId>)>> {
        let mut conflicts = Vec::new();
        let changes: Vec<_> = changes
            .changes
            .into_iter()
            .filter_map(|c| c.status_item)
            .chain(
                changes
                    .ignored_changes
                    .into_iter()
                    .filter_map(|c| c.status_item),
            )
            .filter_map(|item| match item {
                gix::status::Item::IndexWorktree(
                    gix::status::index_worktree::Item::Modification {
                        status: EntryStatus::Conflict { entries, .. },
                        rela_path,
                        ..
                    },
                ) => {
                    conflicts.push((rela_path, entries));
                    None
                }
                gix::status::Item::TreeIndex(c) => Some(c),
                _ => None,
            })
            .filter(|c| selection.iter().any(|path| path == c.location()))
            .collect();

        if changes.is_empty() && conflicts.is_empty() {
            return Ok(None);
        }

        let mut base_tree_edit = base_tree.edit()?;
        for change in changes {
            match change {
                Change::Deletion { location, .. } => {
                    base_tree_edit.remove(location.as_bstr())?;
                }
                Change::Addition {
                    location,
                    entry_mode,
                    id,
                    ..
                }
                | Change::Modification {
                    location,
                    entry_mode,
                    id,
                    ..
                } => {
                    base_tree_edit.upsert(
                        location.as_bstr(),
                        entry_mode
                            .to_tree_entry_mode()
                            .with_context(|| format!("Could not convert the index entry {entry_mode:?} at '{location}' into a tree entry kind"))?
                            .kind(),
                        id.into_owned(),
                    )?;
                }
                Change::Rewrite { .. } => {
                    unreachable!("BUG: this must have been deactivated")
                }
            }
        }

        let index = base_tree_edit.write()?;
        let index = (index != base_tree.id).then_some(index.detach());
        if let Some(index) = index {
            snapshot_tree.upsert("index", EntryKind::Tree, index)?;
        }

        let index_conflicts = if conflicts.is_empty() {
            None
        } else {
            let mut root = snapshot_tree.cursor_at("index-conflicts")?;
            for (rela_path, conflict_entries) in conflicts {
                for (stage, entry) in conflict_entries
                    .into_iter()
                    .enumerate()
                    .filter_map(|(idx, e)| e.map(|e| (idx + 1, e)))
                {
                    root.upsert(
                        format!("{rela_path}/{stage}"),
                        entry
                            .mode
                            .to_tree_entry_mode()
                            .with_context(|| {
                                format!(
                                    "Could not convert the index entry {entry_mode:?} \
                            at '{location}' into a tree entry kind",
                                    entry_mode = entry.mode,
                                    location = rela_path
                                )
                            })?
                            .kind(),
                        entry.id,
                    )?;
                }
            }
            root.write()?.detach().into()
        };

        Ok(Some((index, index_conflicts)))
    }
}
