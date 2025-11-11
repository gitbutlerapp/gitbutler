use std::collections::{BTreeSet, VecDeque};

use anyhow::bail;
use bstr::{BStr, BString, ByteSlice, ByteVec};
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

pub fn merge_worktree_changes_into_destination_or_keep_snapshot(
    changed_files: &[(ChangeKind, BString)],
    repo: &gix::Repository,
    source_tree_id: gix::ObjectId,
    destination_tree_id: gix::ObjectId,
    checkout_opts: &mut git2::build::CheckoutBuilder,
    uncommitted_changes: UncommitedWorktreeChanges,
) -> anyhow::Result<Option<(gix::ObjectId, Option<gix::ObjectId>)>> {
    if changed_files.is_empty() {
        return Ok(None);
    };
    let changes = crate::diff::worktree_changes_no_renames(repo)?;
    if !changes.changes.is_empty() || !changes.ignored_changes.is_empty() {
        let actual_head_tree_id = repo.head_tree_id_or_empty()?;
        if actual_head_tree_id != source_tree_id {
            bail!(
                "Specified HEAD {source} didn't match actual HEAD^{{tree}} {actual_head_tree_id}",
                source = source_tree_id
            )
        }
        // Figure out which added or modified files are actually touched. Deletions we ignore, and allow
        // these files to be recreated during checkout even if they were part in a rename
        // (we don't do rename tracking here)
        let mut change_lut = super::tree::Lut::default();
        for change in &changes.changes {
            match change.status {
                TreeStatus::Deletion { .. } => {
                    // additive snapshots only so checkout can write onto deleted files
                    // (and has to, to restore more)
                    checkout_opts.path(change.path.as_bytes());
                }
                TreeStatus::Addition { .. } | TreeStatus::Modification { .. } => {
                    // It's not about the actual values, just to have a lookup for overlapping paths.
                    change_lut.track_file(change.path.as_ref());
                }
                TreeStatus::Rename { .. } => {
                    unreachable!("rename tracking was disabled")
                }
            }
        }
        // Pick up conflicts as well, let's ignore nothing for the selection.
        for ignored in &changes.ignored_changes {
            change_lut.track_file(ignored.path.as_ref());
        }

        let mut selection_of_changes_checkout_would_affect = BTreeSet::new();
        for file_to_be_modified in changed_files
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
            let out = crate::snapshot::create_tree(
                source_tree_id.attach(&repo_in_memory),
                snapshot::create_tree::State {
                    changes,
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
                                    bail!(
                                        "Worktree changes would be overwritten by checkout: {}",
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
        visit::Action::Continue
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
