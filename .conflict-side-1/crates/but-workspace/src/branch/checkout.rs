/// What to do when uncommitted changes are in the way of files that will be affected by the checkout, and that
/// don't re-apply cleanly on top of the new worktree commit.
#[derive(Default, Debug, Copy, Clone)]
pub enum UncommitedWorktreeChanges {
    /// Do not alter anything if local worktree changes conflict with the incoming one, but abort the operation instead.
    #[default]
    KeepAndAbortOnConflict,
    /// Place the files that would be altered, AND at least one conflicts when brought back, into a snapshot based
    /// on the current `HEAD`, and overwrite them.
    /// Note that uncommitted changes that aren't affected will just be left as is.
    KeepConflictingInSnapshotAndOverwrite,
}

/// Options for use in [super::safe_checkout()].
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// How to deal with uncommitted changes.
    pub uncommitted_changes: UncommitedWorktreeChanges,
}

/// The successful outcome of [super::safe_checkout()] operation.
#[derive(Clone)]
pub struct Outcome {
    /// The tree of the snapshot which stores the worktree changes that have been overwritten as part of the checkout,
    /// based on the `current_head_tree_id` from which it was created.
    pub snapshot_tree: Option<gix::ObjectId>,
    /// If `new_head_id` was a commit, these are the ref-edits returned after performing the transaction.
    pub head_update: Option<Vec<gix::refs::transaction::RefEdit>>,
    /// The number of files that were deleted turn the current worktree into the desired one.
    /// Note that this only counts files, not directories.
    pub num_deleted_files: usize,
    /// The number of files that were added or modified turn the current worktree into the desired one.
    /// Note that this only counts files, not directories.
    pub num_added_or_updated_files: usize,
}

pub(crate) mod function {
    use super::{Options, Outcome, UncommitedWorktreeChanges};
    use crate::snapshot;
    use crate::snapshot::create_tree::no_workspace_and_meta;
    use anyhow::bail;
    use bstr::{BStr, BString, ByteSlice, ByteVec};
    use but_core::TreeStatus;
    use gitbutler_oxidize::ObjectIdExt;
    use gix::diff::rewrites::tracker::{Change as _, ChangeKind};
    use gix::diff::tree::visit;
    use gix::index::entry::Stage;
    use gix::object::tree::EntryKind;
    use gix::objs::TreeRefIter;
    use gix::prelude::ObjectIdExt as _;
    use gix::refs::Target;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    use std::collections::{BTreeSet, VecDeque};

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
                                format!(
                                    "Update {} to {:?}",
                                    edit.name.as_bstr(),
                                    edit.change.new_value()
                                )
                            })
                            .unwrap_or_default(),
                    },
                )
                .finish()
        }
    }

    /// Given the `current_head_id^{tree}` for the tree that matches what `HEAD` points to, perform all file operations necessary
    /// to turn the *worktree* of `repo` into `new_head_id^{tree}`. Note that the current *worktree* is assumed to be at the state of
    /// `current_head_tree_id` along with arbitrary uncommitted user changes.
    ///
    /// Note that we don't care if the worktree actually matches the `new_head_id^{tree}`, we only care about the operations from
    /// `current_head_id^{tree}` to be performed, and if there are none, we will do nothing.
    ///
    /// If `new_head_id` is a commit, we will also set `HEAD` (or the ref it points to if symbolic) to the `new_head_id`.
    /// We will also update the `.git/index` to match the `new_head_id^{tree}`.
    /// Note that the value for [`UncommitedWorktreeChanges`] is critical to determine what happens if a change would be overwritten.
    ///
    /// We will always handle changes in the worktree safely to avoid loss of uncommited information. This also means that deletions
    /// never cause us to conflict. Conflicted files that would be checked out will cause an error.
    ///
    /// #### Note: No rename tracking
    ///
    /// To keep it simpler, we don't do rename tracking, so deletions and additions are always treated separately.
    /// If this changes, then the source sid of a rename could also cause conflicts, maybe? It's a bit unclear what it would mean
    /// in practice, but I guess that we bring deleted files back instead of conflicting.
    pub fn safe_checkout(
        current_head_id: gix::ObjectId,
        new_head_id: gix::ObjectId,
        repo: &gix::Repository,
        Options {
            uncommitted_changes,
        }: Options,
    ) -> anyhow::Result<Outcome> {
        let source_tree = current_head_id.attach(repo).object()?.peel_to_tree()?;
        let new_object = new_head_id.attach(repo).object()?;
        let destination_tree = new_object.clone().peel_to_tree()?;

        let mut delegate = Delegate::default();
        gix::diff::tree(
            TreeRefIter::from_bytes(&source_tree.data),
            TreeRefIter::from_bytes(&destination_tree.data),
            &mut gix::diff::tree::State::default(),
            repo,
            &mut delegate,
        )?;

        let mut opts = git2::build::CheckoutBuilder::new();
        let snapshot_tree = if !delegate.changed_files.is_empty() {
            let changes = but_core::diff::worktree_changes_no_renames(repo)?;
            if !changes.changes.is_empty() {
                let actual_head_tree_id = repo.head_tree_id_or_empty()?;
                if actual_head_tree_id != source_tree.id {
                    bail!(
                        "Specified HEAD {source} didn't match actual HEAD^{{tree}} {actual_head_tree_id}",
                        source = source_tree.id
                    )
                }
                // Figure out which added or modified files are actually touched. Deletions we ignore, and allow
                // these files to be recreated during checkout even if they were part in a rename
                // (we don't do rename tracking here)
                let mut change_lut = repo.empty_tree().edit()?;
                for change in &changes.changes {
                    match change.status {
                        TreeStatus::Deletion { .. } => {
                            // additive snapshots only so checkout can write onto deleted files
                            // (and has to, to restore more)
                            opts.path(change.path.as_bytes());
                        }
                        TreeStatus::Addition { .. } | TreeStatus::Modification { .. } => {
                            // It's not about the actual values, just to have a lookup for overlapping paths.
                            change_lut.upsert(
                                &change.path,
                                EntryKind::Blob,
                                repo.object_hash().empty_blob(),
                            )?;
                        }
                        TreeStatus::Rename { .. } => {
                            unreachable!("rename tracking was disabled")
                        }
                    }
                }

                let selection_of_changes_checkout_would_affect = BTreeSet::new();
                // TODO: find uncommitted that would be overwritten.
                for _file_to_be_modified in &delegate.changed_files {
                    // selection.extend(change_lut.extend_leafs(&file_to_be_modified))
                }

                if !selection_of_changes_checkout_would_affect.is_empty() {
                    let repo_in_memory = repo.clone().with_object_memory();
                    let _out = crate::snapshot::create_tree(
                        source_tree.id.attach(&repo_in_memory),
                        snapshot::create_tree::State {
                            changes,
                            selection: selection_of_changes_checkout_would_affect,
                            head: false,
                        },
                        no_workspace_and_meta(),
                    )?;

                    match uncommitted_changes {
                        UncommitedWorktreeChanges::KeepAndAbortOnConflict => {}
                        UncommitedWorktreeChanges::KeepConflictingInSnapshotAndOverwrite => {}
                    }
                    todo!("deal with snapshot")
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Finally, perform the actual checkout
        // TODO(gix): use unconditional `gix` checkout implementation as pre-cursor to the real deal (not needed here).
        //            All it has to do is to be able to apply the target changes to any working tree, while using filters,
        //            and while doing it symlink-safe.
        if !delegate.changed_files.is_empty() {
            let git2_repo = git2::Repository::open(repo.git_dir())?;
            let destination_tree = git2_repo
                .find_tree(destination_tree.id.to_git2())?
                .into_object();
            let index = repo.index()?;
            let mut conflicting = Vec::new();
            for (_kind, path_to_alter) in &delegate.changed_files {
                if index
                    .entry_by_path(path_to_alter.as_bstr())
                    .is_some_and(|e| e.stage() != Stage::Unconflicted)
                {
                    conflicting.push(path_to_alter.as_bstr());
                }
                opts.path(path_to_alter.as_bytes());
            }

            if !conflicting.is_empty() {
                bail!(
                    "Refusing to overwrite conflicting paths: {}",
                    conflicting
                        .into_iter()
                        .map(|rela_path| format!("'{rela_path}'"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
            }

            git2_repo.checkout_tree(
                &destination_tree,
                Some(opts.force().disable_pathspec_match(true)),
            )?;
        }

        let head_update = if new_object.kind.is_commit() {
            let needs_update = repo
                .head()?
                .id()
                .is_none_or(|actual_head_id| actual_head_id != new_head_id);
            if needs_update {
                let edits = repo.edit_reference(RefEdit {
                    change: Change::Update {
                        log: LogChange {
                            mode: RefLog::AndReference,
                            force_create_reflog: false,
                            message: gix::reference::log::message(
                                "safe checkout",
                                "GitButler".into(),
                                new_object.into_commit().parent_ids().count(),
                            ),
                        },
                        // We play it loose here, as we assume a repository lock so we won't interfere with ourselves.
                        // Git itself enforces no lock either, so we rely on basic locking ref-locking here. Good enough.
                        expected: PreviousValue::Any,
                        new: Target::Object(new_head_id),
                    },
                    name: "HEAD".try_into().expect("root refs are always valid"),
                    deref: true,
                })?;
                Some(edits)
            } else {
                None
            }
        } else {
            None
        };

        let num_deleted_files = delegate
            .changed_files
            .iter()
            .filter(|(kind, _)| matches!(kind, ChangeKind::Deletion))
            .count();
        Ok(Outcome {
            snapshot_tree,
            head_update,
            num_deleted_files,
            num_added_or_updated_files: delegate.changed_files.len() - num_deleted_files,
        })
    }

    #[derive(Default)]
    struct Delegate {
        path_deque: VecDeque<BString>,
        path: BString,
        // Repo-relative slash separated paths that need to be altered during checkout.
        changed_files: Vec<(ChangeKind, BString)>,
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
}
