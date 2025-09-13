/// What to do when uncommitted changes are in the way of files that will be affected by the checkout.
#[derive(Default, Debug, Copy, Clone)]
pub enum UncommitedWorktreeChanges {
    /// Do not alter anything, but abort the operation instead.
    #[default]
    KeepAndAbort,
    /// Place the files that would be altered into a snapshot, based on the current worktree, and overwrite them.
    /// Note that uncommitted changes that aren't affected will just be left as is.
    PutInSnapshotAndAlter,
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
    /// The number of files that were added, deleted or modified turn the current worktree into the desired one.
    /// Note that this only counts files, not directories.
    pub num_changed_files: usize,
}

pub(crate) mod function {
    use super::{Options, Outcome, UncommitedWorktreeChanges};
    use bstr::{BStr, BString, ByteSlice, ByteVec};
    use gix::diff::tree::visit;
    use gix::objs::TreeRefIter;
    use gix::prelude::ObjectIdExt;
    use gix::refs::Target;
    use gix::refs::transaction::{Change, LogChange, PreviousValue, RefEdit, RefLog};
    use std::collections::VecDeque;

    impl std::fmt::Debug for Outcome {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            let Outcome {
                snapshot_tree,
                head_update,
                num_changed_files,
            } = self;
            f.debug_struct("Outcome")
                .field("snapshot_tree", snapshot_tree)
                .field("num_changed_files", num_changed_files)
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
    /// never cause us to conflict.
    // TODO(gix): use `gitoxide` for the checkout to support filters, and do it much more quickly due to the option of multi-threading.
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
            &repo,
            &mut delegate,
        )?;

        // TODO: find uncommitted that would be overwritten.
        match uncommitted_changes {
            UncommitedWorktreeChanges::KeepAndAbort => {}
            UncommitedWorktreeChanges::PutInSnapshotAndAlter => {}
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

        Ok(Outcome {
            snapshot_tree: None,
            head_update,
            num_changed_files: delegate.num_changed_files,
        })
    }

    #[derive(Default)]
    struct Delegate {
        path_deque: VecDeque<BString>,
        path: BString,
        num_changed_files: usize,
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
            self.num_changed_files += usize::from(change.entry_mode().is_no_tree());
            visit::Action::Continue
        }
    }
}
