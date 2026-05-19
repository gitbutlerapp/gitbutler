use std::collections::{BTreeSet, VecDeque};

use anyhow::bail;
use bstr::{BStr, BString, ByteSlice, ByteVec};
use but_error::bail_precondition;
use gix::{
    diff::{
        rewrites::tracker::{Change, ChangeKind},
        tree::visit,
    },
    merge::tree::TreatAsUnresolved,
    prelude::ObjectIdExt,
};

use crate::{
    TreeStatus,
    ext::ObjectStorageExt,
    snapshot,
    worktree::checkout::{Outcome, UncommitedWorktreeChanges},
};

/// Preserve any uncommitted worktree changes that would be affected by a checkout from one tree to another.
///
/// If the checkout would touch paths with local worktree changes, the *relevant* worktree changes are first
/// captured in a snapshot tree based on `source_tree_id`. That snapshot is then resolved onto
/// `destination_tree_id`; depending on `uncommitted_changes`, unresolved conflicts either abort the
/// checkout or are kept in the snapshot while the checkout proceeds.
///
/// Returns `None` if there are no checkout changes, or if no local worktree changes need to be preserved.
/// Otherwise returns the snapshot tree ID and, if resolving the snapshot changed the checkout target, the
/// replacement destination tree ID to check out, the one that contains the non-conflicting worktree changes.
///
/// # Parameters
///
/// * `files_to_checkout` - Repo-relative paths, with their change kinds, that the checkout from
///   `source_tree_id` to `destination_tree_id` intends to add, modify, or delete.
/// * `repo` - Repository used to inspect the current worktree, read the current `HEAD` tree, create
///   snapshot trees, and persist any in-memory objects needed by the adjusted destination tree if one is written.
///   No changes will be observable if there is no intersecting worktree changes.
/// * `source_tree_id` - The tree expected to match the repository's current `HEAD`. It is used as the
///   base for the snapshot of local worktree changes, and is verified against the actual `HEAD` before
///   preserving changes.
/// * `destination_tree_id` - The tree the checkout originally intends to write into the worktree. If
///   preserved worktree changes can be cleanly reapplied, they are resolved onto this tree and may
///   produce a replacement destination tree in the return value.
/// * `checkout_opts` - The checkout builder that the caller will later pass to `git2` for the actual
///   checkout. This function only adds explicit pathspecs for deleted local worktree paths that the
///   checkout may need to recreate after the caller removes checkout deletions from disk:
///
///   - the deleted worktree path intersects a checkout deletion, so the pathspec constrains `git2`
///     instead of letting an empty pathspec checkout everything;
///   - `destination_tree_id` still has a file at that path, so checkout should restore it from the
///     destination tree;
///   - `destination_tree_id` has a tree at that path and preserved worktree content exists underneath
///     it, so checkout should materialize the destination subtree.
///
///   If none of those cases adds a pathspec for a pure-deletion checkout, this function adds one
///   checkout-deletion path as a guardrail so `git2` does not interpret an empty pathspec as a request
///   to checkout the whole destination tree.
///
///   Normal added or modified checkout paths are still added by the caller.
/// * `uncommitted_changes` - Policy for preserved worktree changes that do not apply cleanly to
///   `destination_tree_id`: either abort before checkout, or keep the conflicting content in the
///   snapshot while allowing the checkout to overwrite the worktree.
pub fn merge_worktree_changes_into_destination_or_keep_snapshot(
    files_to_checkout: &[(ChangeKind, BString)],
    repo: &gix::Repository,
    source_tree_id: gix::ObjectId,
    destination_tree_id: gix::ObjectId,
    checkout_opts: &mut git2::build::CheckoutBuilder,
    uncommitted_changes: UncommitedWorktreeChanges,
    merge_base_override: Option<gix::ObjectId>,
) -> anyhow::Result<Option<(gix::ObjectId, Option<gix::ObjectId>)>> {
    if files_to_checkout.is_empty() {
        return Ok(None);
    };
    let worktree_changes = crate::diff::worktree_changes_no_renames(repo)?;
    if !worktree_changes.changes.is_empty() || !worktree_changes.ignored_changes.is_empty() {
        let actual_head_tree_id = repo.head_tree_id_or_empty()?;
        if actual_head_tree_id != source_tree_id {
            bail!(
                "Specified HEAD {source_tree_id} didn't match actual HEAD^{{tree}} {actual_head_tree_id}"
            )
        }
        let mut checkout_deletions_lut = super::tree::Lut::default();
        let mut checkout_writes_lut = super::tree::Lut::default();
        for (kind, path) in files_to_checkout {
            if matches!(kind, ChangeKind::Deletion) {
                checkout_deletions_lut.track_file(path.as_ref());
            } else {
                checkout_writes_lut.track_file(path.as_ref());
            }
        }
        let checkout_writes_tree_entries = !checkout_writes_lut.nodes_is_empty();
        let destination_tree = checkout_writes_tree_entries
            .then(|| destination_tree_id.attach(repo).object()?.peel_to_tree())
            .transpose()?;

        let mut change_lut = super::tree::Lut::default();
        for wt_change in &worktree_changes.changes {
            match wt_change.status {
                TreeStatus::Deletion { .. } => {}
                TreeStatus::Addition { .. } | TreeStatus::Modification { .. } => {
                    // It's not about the actual values, just to have a lookup for overlapping paths.
                    change_lut.track_file(wt_change.path.as_ref());
                }
                TreeStatus::Rename { .. } => {
                    unreachable!("rename tracking was disabled")
                }
            }
        }
        // Pick up conflicts as well, let's ignore nothing for the selection.
        for ignored in &worktree_changes.ignored_changes {
            change_lut.track_file(ignored.path.as_ref());
        }

        let mut added_deleted_worktree_pathspec = false;
        for wt_change in &worktree_changes.changes {
            match wt_change.status {
                TreeStatus::Deletion { .. } => {
                    let mut checkout_deletion_intersections = BTreeSet::new();
                    checkout_deletions_lut.get_intersecting(
                        wt_change.path.as_ref(),
                        &mut checkout_deletion_intersections,
                    );
                    let mut checkout_write_intersections = BTreeSet::new();
                    checkout_writes_lut.get_intersecting(
                        wt_change.path.as_ref(),
                        &mut checkout_write_intersections,
                    );

                    // Look up the destination entry kind only when at least one
                    // intersection set is non-empty — otherwise we already know
                    // no pathspec will be added.
                    let destination_entry_kind = if !checkout_deletion_intersections.is_empty()
                        || !checkout_write_intersections.is_empty()
                    {
                        destination_tree
                            .as_ref()
                            .map(|tree| {
                                tree.lookup_entry(
                                    super::tree::to_components(wt_change.path.as_ref())
                                        .map(ToOwned::to_owned),
                                )
                                .map(|entry| entry.map(|entry| entry.mode().kind()))
                            })
                            .transpose()?
                            .flatten()
                    } else {
                        None
                    };

                    // Only consider restoring a worktree-deleted file from the destination
                    // tree when the checkout would actually write to that path (or a path
                    // underneath it). Without this guard, *every* uncommitted deletion whose
                    // file still exists in the destination tree would be unconditionally
                    // restored, even if the checkout doesn't touch that path at all.
                    let destination_needs_checkout = if checkout_write_intersections.is_empty() {
                        false
                    } else {
                        match destination_entry_kind {
                            Some(gix::object::tree::EntryKind::Tree) => {
                                let mut worktree_content_intersections = BTreeSet::new();
                                change_lut.get_intersecting(
                                    wt_change.path.as_ref(),
                                    &mut worktree_content_intersections,
                                );
                                !worktree_content_intersections.is_empty()
                            }
                            Some(_) => true,
                            None => false,
                        }
                    };

                    // Add an explicit pathspec for deleted worktree paths when it either
                    // constrains a checkout deletion or restores a file from the destination.
                    //
                    // When the destination entry is a tree (directory), we must NOT add the
                    // pathspec: `git2` with `disable_pathspec_match` treats it as a literal
                    // blob path, which can trigger a "null OID" error when it encounters a
                    // tree instead of a blob. The checkout of files *under* that directory
                    // is already covered by the non-deletion pathspecs in `safe_checkout`.
                    let is_destination_tree = matches!(
                        destination_entry_kind,
                        Some(gix::object::tree::EntryKind::Tree)
                    );
                    if (!checkout_deletion_intersections.is_empty() && !is_destination_tree)
                        || destination_needs_checkout
                    {
                        checkout_opts.path(wt_change.path.as_bytes());
                        added_deleted_worktree_pathspec = true;
                    }
                }
                TreeStatus::Addition { .. } | TreeStatus::Modification { .. } => {}
                TreeStatus::Rename { .. } => {
                    unreachable!("rename tracking was disabled")
                }
            }
        }
        if !added_deleted_worktree_pathspec
            && files_to_checkout
                .iter()
                .all(|(kind, _)| matches!(kind, ChangeKind::Deletion))
            && let Some((_, path)) = files_to_checkout.first()
        {
            // Keep pure-deletion checkouts from reaching `git2` with an empty pathspec, which
            // would apply the destination tree broadly and restore unrelated worktree deletions.
            checkout_opts.path(path.as_bytes());
        }

        let mut selection_of_changes_checkout_would_affect = BTreeSet::new();
        for file_to_be_modified in files_to_checkout
            .iter()
            // Deleted files shouldn't be in the snapshot as they won't ever be in our way.
            // .filter(|(kind, _)| !matches!(kind, ChangeKind::Deletion))
            .map(|(_, p)| p)
        {
            change_lut.get_intersecting(
                file_to_be_modified.as_ref(),
                &mut selection_of_changes_checkout_would_affect,
            );
        }

        if !selection_of_changes_checkout_would_affect.is_empty() {
            let mut repo_in_memory = repo.clone().with_object_memory();
            repo_in_memory
                .config_snapshot_mut()
                .set_value(&gix::config::tree::Merge::RENORMALIZE, "true")?;

            let out = crate::snapshot::create_tree(
                source_tree_id.attach(&repo_in_memory),
                snapshot::create_tree::State {
                    changes: worktree_changes,
                    selection: selection_of_changes_checkout_would_affect,
                    head: false,
                },
            )?;

            if !out.is_empty() {
                let resolve = crate::snapshot::resolve_tree(
                    out.snapshot_tree.attach(&repo_in_memory),
                    destination_tree_id,
                    snapshot::resolve_tree::Options {
                        worktree_cherry_pick: None,
                        merge_base_override,
                    },
                )?;
                let new_destination_id =
                    if let Some(mut worktree_cherry_pick) = resolve.worktree_cherry_pick {
                        // re-apply snapshot of just what we need and see if they apply cleanly.
                        match uncommitted_changes {
                            UncommitedWorktreeChanges::KeepAndAbortOnConflict => {
                                let unresolved = TreatAsUnresolved::git();
                                if worktree_cherry_pick.has_unresolved_conflicts(unresolved) {
                                    let mut paths = worktree_cherry_pick
                                        .conflicts
                                        .iter()
                                        .filter(|c| c.is_unresolved(unresolved))
                                        .map(|c| format!("{:?}", c.ours.location()))
                                        .collect::<Vec<_>>();
                                    paths.sort();
                                    paths.dedup();
                                    bail_precondition!(
                                        "Uncommitted files would be overwritten by checkout: {}",
                                        paths.join(", ")
                                    );
                                }
                            }
                            UncommitedWorktreeChanges::KeepConflictingInSnapshotAndOverwrite => {}
                        }
                        let res = Some(worktree_cherry_pick.tree.write()?.detach());
                        if let Some(memory) = repo_in_memory.objects.take_object_memory() {
                            memory.persist(repo_in_memory)?;
                        }
                        res
                    } else {
                        None
                    };
                return Ok(Some((out.snapshot_tree, new_destination_id)));
                // TODO: deal with index, but to do that it needs to be merged with destination tree!
            }
        }
    }
    Ok(None)
}

#[derive(Default)]
pub struct Delegate {
    path_deque: VecDeque<BString>,
    path: BString,
    // Repo-relative slash separated paths that need to be altered during checkout.
    pub changed_files: Vec<(ChangeKind, BString)>,
}

impl Delegate {
    fn pop_element(&mut self) {
        if let Some(pos) = self.path.rfind_byte(b'/') {
            self.path.resize(pos, 0);
        } else {
            self.path.clear();
        }
    }

    fn push_element(&mut self, name: &BStr) {
        if name.is_empty() {
            return;
        }
        if !self.path.is_empty() {
            self.path.push(b'/');
        }
        self.path.push_str(name);
    }
}

impl gix::diff::tree::Visit for Delegate {
    fn pop_front_tracked_path_and_set_current(&mut self) {
        self.path = self
            .path_deque
            .pop_front()
            .expect("every parent is set only once");
    }

    fn push_back_tracked_path_component(&mut self, component: &BStr) {
        self.push_element(component);
        self.path_deque.push_back(self.path.clone());
    }

    fn push_path_component(&mut self, component: &BStr) {
        self.push_element(component);
    }

    fn pop_path_component(&mut self) {
        self.pop_element();
    }

    fn visit(&mut self, change: visit::Change) -> visit::Action {
        if change.entry_mode().is_no_tree() {
            self.changed_files.push((change.kind(), self.path.clone()));
        }
        std::ops::ControlFlow::Continue(())
    }
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Outcome {
            snapshot_tree,
            head_update,
            num_deleted_files,
            num_added_or_updated_files,
        } = self;
        f.debug_struct("Outcome")
            .field("snapshot_tree", snapshot_tree)
            .field("num_deleted_files", num_deleted_files)
            .field("num_added_or_updated_files", num_added_or_updated_files)
            .field(
                "head_update",
                &match head_update {
                    None => "None".to_string(),
                    Some(edits) => edits
                        .last()
                        .map(|edit| {
                            format!("Update {} to {:?}", edit.name, edit.change.new_value())
                        })
                        .unwrap_or_default(),
                },
            )
            .finish()
    }
}
