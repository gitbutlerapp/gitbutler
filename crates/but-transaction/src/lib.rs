use std::collections::BTreeMap;

use anyhow::Context as _;
use bstr::{BStr, BString, ByteVec};
use but_api::WorkspaceState;
use but_core::{
    DiffSpec, DryRun, RefMetadata,
    commit::CommitIdentifiers,
    ref_metadata,
    sync::RepoExclusive,
    tree::create_tree::RejectionReason,
    update_head_reference,
    worktree::{checkout, safe_checkout_from_head},
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Step, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::branch::create_reference::Anchor;
use but_workspace::commit::{
    ChangeSource, MoveChangesOutcome, SquashCommitsOutcome,
    squash_commits::MessageCombinationStrategy,
};
use gix::{
    ObjectId,
    refs::{
        FullName, FullNameRef, Target,
        transaction::{PreviousValue, RefEdit},
    },
};

#[cfg(test)]
mod tests;

/// Run a workspace transaction.
///
/// This allows chaining multiple operations and having them all succeed or fail together.
///
/// Note this isn't fully ACID compliant database transactions but rather a "best effort" version
/// using our in-memory repositories and rebases. Its scope is the rebase and the refs and metadata
/// it writes; project-database rows are *not* part of it, so anything an operation writes there
/// stands whether the transaction commits or rolls back.
///
/// # Committing
///
/// The transaction will be committed if:
///
/// - The callback doesn't return an error.
/// - The success value is either `()` or [`DynamicOutcome::Commit`].
///
/// Use [`DynamicOutcome::Rollback`] to conditionally rollback the transaction without returning
/// an error.
///
/// Use [`Transaction::rollback`] to rollback unconditionally without returning an error.
///
/// When the transaction is committed a single oplog entry with `snapshot_details` will be created.
/// This enables a single `but undo` to undo the whole transaction.
///
/// # Commit mapping
///
/// The transaction will automatically map between source commits and rebased commits in the
/// in-memory repository.
///
/// For example this means commits can be squashed like this:
///
/// ```ignore
/// tx.squash_commits([source_one], target)?;
/// tx.squash_commits([source_two], target)?;
/// tx.squash_commits([source_three], target)?;
/// ```
///
/// The SHA for `target` will change after the first squash which would normally require looking up
/// the new SHA to perform the second squash. `Transaction` does this automatically so callers can
/// continue using the source commits.
///
/// Commits can still manually be mapped using [`Transaction::get_mapped_commit`] if necessary.
pub fn with_transaction<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    snapshot_details: SnapshotDetails,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, M>) -> anyhow::Result<T>,
    M: RefMetadata,
    T: TransactionOutcome,
{
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    with_transaction_with_perm(ctx, meta, perm, snapshot_details, dry_run, f)
}

/// Like [`with_transaction`] but allows the caller to provide the lock.
pub fn with_transaction_with_perm<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    perm: &mut RepoExclusive,
    snapshot_details: SnapshotDetails,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, M>) -> anyhow::Result<T>,
    M: RefMetadata,
    T: TransactionOutcome,
{
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        snapshot_details,
        perm.read_permission(),
        dry_run,
    );

    let (should_rollback, outcome) = with_transaction_with_perm_only(ctx, meta, perm, dry_run, f)?;

    if !should_rollback && let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm)?;
    }

    Ok(outcome)
}

pub fn with_transaction_with_perm_only<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    perm: &mut RepoExclusive,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<(bool, T::Outcome)>
where
    F: FnOnce(Transaction<'_, '_, M>) -> anyhow::Result<T>,
    M: RefMetadata,
    T: TransactionOutcome,
{
    let (should_rollback, outcome) = {
        let context_lines = ctx.settings.context_lines;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;

        let editor = Editor::create(&mut ws, meta, &repo, &mut db)?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            commit_mappings: CommitMappings::default(),
            pending_metadata_removals: Vec::new(),
            pending_branch_metadata: Vec::new(),
            pending_ref_changes: PendingRefChanges::default(),
            pending_checkout: None,
            context_lines,
        };

        let callback_outcome = {
            let tx = Transaction { inner: &mut inner };
            f(tx)
        };

        let callback_outcome = match callback_outcome {
            Ok(outcome) => outcome,
            Err(err) => {
                inner.pending_ref_changes.rollback(&repo)?;
                return Err(err);
            }
        };

        let Inner {
            mut rebase,
            commit_mappings: _,
            pending_metadata_removals,
            pending_branch_metadata,
            mut pending_ref_changes,
            pending_checkout,
            context_lines: _,
        } = inner;
        let rebase = rebase.take().expect("rebase is always Some(_)");

        let should_rollback = callback_outcome.should_rollback();
        // A rolled-back transaction never materializes, so it has no workspace to report.
        let workspace = if should_rollback {
            Ok(None)
        } else {
            workspace_state_from_rebase(
                rebase,
                &repo,
                pending_metadata_removals,
                pending_branch_metadata,
                FinalizeOptions {
                    checkout: pending_checkout,
                    dry_run,
                },
            )
            .map(Some)
        };

        let workspace = match workspace {
            Ok(workspace) => workspace,
            Err(err) => {
                pending_ref_changes.rollback(&repo)?;
                return Err(err);
            }
        };
        let outcome = callback_outcome.into_outcome(workspace);

        if should_rollback || dry_run.into() {
            pending_ref_changes.rollback(&repo)?;
        }

        (should_rollback, outcome)
    };

    Ok((should_rollback, outcome))
}

/// A workspace transaction that allows changing multiple operations and having them all succeed or
/// fail together.
///
/// See [`with_transaction`] for more details.
pub struct Transaction<'inner, 'rebase, M>
where
    M: RefMetadata,
{
    // Store a mutable reference so the callback for `with_transaction` can get an owned
    // `Transaction`. It needs to be owned to verify statically that `Transaction::rollback` is
    // only called once.
    inner: &'inner mut Inner<'rebase, M>,
}

struct Inner<'rebase, M>
where
    M: RefMetadata,
{
    // an Option so we can "take" the rebase, convert it into an editor, perform another rebase,
    // and put the result back.
    rebase: Option<SuccessfulRebase<'rebase, 'rebase, M>>,
    pending_metadata_removals: Vec<FullName>,
    pending_branch_metadata: Vec<PendingBranchMetadata>,
    pending_ref_changes: PendingRefChanges,
    // A checkout cannot happen until the in-memory rebase and its references are materialized.
    pending_checkout: Option<FullName>,
    // Commits given to `squash_commits`, `reword_commit`, etc are allowed to be the original
    // commits from live repo. This is used to map those to the rebased in-memory commits.
    //
    // Doing this mapping automatically makes the API simpler for the callers because they don't
    // need to map commits after each operation.
    commit_mappings: CommitMappings,
    context_lines: u32,
}

impl<'rebase, M> Transaction<'_, 'rebase, M>
where
    M: RefMetadata,
{
    /// Rollback the transaction, without returning an error.
    ///
    /// If the transaction needs to be rolled back conditionally use [`DynamicOutcome::Rollback`].
    // TODO(david): not sure if we actually need this
    pub fn rollback<T>(self, outcome: T) -> Rollback<T> {
        Rollback(outcome)
    }

    pub fn squash_commits(
        &mut self,
        subjects: impl IntoIterator<Item = ObjectId>,
        target: ObjectId,
        how_to_combine_messages: MessageCombinationStrategy,
    ) -> anyhow::Result<CommitIdentifiers> {
        self.rebase(|editor, commit_mappings| {
            let SquashCommitsOutcome {
                rebase,
                commit_selector,
            } = but_workspace::commit::squash_commits(
                editor,
                subjects
                    .into_iter()
                    .map(|commit| commit_mappings.map(commit))
                    .collect(),
                commit_mappings.map(target),
                how_to_combine_messages,
            )?;
            let new_commit = rebase.lookup_commit(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn reword_commit(
        &mut self,
        commit: ObjectId,
        message: &BStr,
    ) -> anyhow::Result<CommitIdentifiers> {
        self.rebase(|editor, commit_mappings| {
            let (rebase, edited_commit_selector) =
                but_workspace::commit::reword(editor, commit_mappings.map(commit), message)?;
            let new_commit = rebase.lookup_commit(edited_commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn discard_commits(
        &mut self,
        subjects: impl IntoIterator<Item = gix::ObjectId>,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let rebase = but_workspace::commit::discard_commits(
                editor,
                subjects
                    .into_iter()
                    .map(|commit| commit_mappings.map(commit)),
            )?;
            Ok(((), rebase))
        })
    }

    pub fn uncommit_commits(
        &mut self,
        subjects: impl IntoIterator<Item = gix::ObjectId>,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let mut rebase = but_workspace::commit::discard_commits(
                editor,
                subjects
                    .into_iter()
                    .map(|commit| commit_mappings.map(commit)),
            )?;
            rebase.keep_worktrees_unchanged()?;
            Ok(((), rebase))
        })
    }

    pub fn discard_changes_from_commit(
        &mut self,
        source: gix::ObjectId,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<CommitIdentifiers> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let but_workspace::commit::UncommitChangesOutcome {
                rebase,
                commit_selector,
            } = but_workspace::commit::uncommit_changes(
                editor,
                commit_mappings.map(source),
                changes,
                context_lines,
            )?;

            let new_commit = rebase.lookup_commit(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    /// Check out `branch` when the transaction commits.
    ///
    /// The checkout is deferred until all in-memory commits and reference changes have been
    /// materialized. Consequently, operations after this call still observe the checkout from
    /// before the transaction. Calling this more than once replaces the previously requested final
    /// checkout.
    pub fn checkout(&mut self, branch: &FullNameRef) -> anyhow::Result<()> {
        anyhow::ensure!(
            branch.category() == Some(gix::refs::Category::LocalBranch),
            "Can only check out local branches under refs/heads, got '{}'",
            branch.as_bstr()
        );

        let rebase = self
            .inner
            .rebase
            .as_ref()
            .expect("rebase is always Some(_)");
        rebase
            .reference_target(branch)
            .or_else(|_| resolve_checkout_target(rebase.repo(), branch))?;

        self.inner.pending_checkout = Some(branch.to_owned());
        Ok(())
    }

    pub fn remove_reference(&mut self, ref_name: &FullNameRef) -> anyhow::Result<()> {
        self.rebase(|mut editor, _| {
            let ref_selector = editor.select_reference(ref_name)?;

            let must_disconnect_child = 'must_disconnect: {
                let Some(target_selector) = editor.target_selector() else {
                    break 'must_disconnect None;
                };

                // Only one child, which must be the workspace commit. The
                // workspace commit must also have more than one parent (if
                // not the workspace commit would end up with no parents, which
                // is bad).
                let child_selectors = editor.direct_children(ref_selector)?;
                let [(child_selector, _)] = child_selectors[..] else {
                    break 'must_disconnect None;
                };
                if !matches!(editor.lookup_step(child_selector)?, Step::Pick(..)) {
                    break 'must_disconnect None;
                }
                let (_, child_commit) = editor.find_selectable_commit(child_selector)?;
                if !but_graph::workspace::commit::is_managed_workspace_by_message(
                    child_commit.message.as_ref(),
                ) {
                    break 'must_disconnect None;
                }
                if editor.direct_parents(child_selector)?.len() == 1 {
                    break 'must_disconnect None;
                }

                // All ancestors up to the target commit must be Step::None or
                // the local branch corresponding to the target ref.
                let mut ancestor_selectors: Vec<_> = editor
                    .direct_parents(ref_selector)?
                    .into_iter()
                    .map(|(selector, _)| selector)
                    .collect();
                let target_local_branch = editor.target_ref().map(|r| {
                    let bstr = r.as_bstr();
                    if let Some(shortname) = bstr.rsplit(|&c| c == b'/').next() {
                        let mut target_ref = BString::new(b"refs/heads/".to_vec());
                        target_ref.push_str(shortname);
                        target_ref
                    } else {
                        bstr.to_owned()
                    }
                });
                while let Some(ancestor_selector) = ancestor_selectors.pop() {
                    if ancestor_selector == target_selector {
                        // OK, do nothing
                    } else {
                        let step = editor.lookup_step(ancestor_selector)?;
                        let mut ok_to_skip = matches!(step, Step::None);
                        if !ok_to_skip
                            && let Some(ref target_local_branch) = target_local_branch
                            && matches!(step, Step::Reference { ref refname, .. }
                                if refname == target_local_branch)
                        {
                            ok_to_skip = true;
                        }
                        if ok_to_skip {
                            ancestor_selectors.extend(
                                editor
                                    .direct_parents(ancestor_selector)?
                                    .into_iter()
                                    .map(|(selector, _)| selector),
                            );
                        } else {
                            break 'must_disconnect None;
                        }
                    }
                }
                Some(child_selector)
            };

            editor.replace_with_none(ref_selector)?;
            if let Some(must_disconnect_child) = must_disconnect_child {
                editor.remove_edges(must_disconnect_child, ref_selector)?;
            }
            let rebase = editor.rebase()?;
            Ok(((), rebase))
        })?;
        let repo = self.repo().clone();
        self.inner
            .pending_ref_changes
            .remove_eagerly_created_ref(&repo, ref_name)?;
        self.inner
            .pending_metadata_removals
            .push(ref_name.to_owned());
        Ok(())
    }

    /// Move `source_branch` to `side` of `relative_to` within the transaction's workspace.
    ///
    /// In single-branch mode, `HEAD` follows the top of the reordered stack when materialized.
    pub fn move_branch(
        &mut self,
        source_branch: &FullNameRef,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };
            let rebase =
                but_workspace::branch::move_branch(editor, source_branch, relative_to, side)?;
            Ok(((), rebase))
        })
    }

    pub fn tear_off_branch(&mut self, source_branch: &FullNameRef) -> anyhow::Result<()> {
        self.rebase(|editor, _| {
            let rebase = but_workspace::branch::tear_off_branch(editor, source_branch)?;
            Ok(((), rebase))
        })?;

        Ok(())
    }

    /// Create a new local branch at a commit already present in the editor.
    ///
    /// Unlike [`Transaction::create_reference`], this does not add the branch to workspace
    /// metadata or require it to appear in the current checkout's projection. It can be used
    /// with [`Transaction::checkout`] to prepare an independent branch before switching to it.
    /// The reference is available to subsequent operations and is removed on rollback or dry run.
    pub fn create_reference_at_commit(
        &mut self,
        ref_name: &FullNameRef,
        commit_id: ObjectId,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            ref_name.category() == Some(gix::refs::Category::LocalBranch),
            "Can only create local branches under refs/heads"
        );
        let commit_id = self.inner.commit_mappings.map(commit_id);
        self.repo().reference(
            ref_name,
            commit_id,
            gix::refs::transaction::PreviousValue::MustNotExist,
            format!("create {ref_name}"),
        )?;
        self.inner
            .pending_ref_changes
            .record_eager_create(ref_name, None);
        // Preserve GitButler-created branch identity, including when it later becomes empty,
        // without recording a stack relationship with the branch owning the base commit.
        let mut branch = ref_metadata::Branch::default();
        branch.update_times(true);
        self.inner
            .pending_branch_metadata
            .push(PendingBranchMetadata {
                name: ref_name.to_owned(),
                value: branch,
            });

        self.rebase(|mut editor, _| {
            let target = editor.select_commit(commit_id)?;
            let reference = editor.add_step(Step::new_reference(ref_name.to_owned()))?;
            editor.add_edge(reference, target, 0)?;
            Ok(((), editor.rebase()?))
        })
    }

    pub fn create_reference<'name>(
        &mut self,
        ref_name: &FullNameRef,
        anchor: impl Into<Option<Anchor<'name>>>,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<()> {
        let anchor = anchor.into();
        let order = order.into();
        self.rebase(|editor, commit_mappings| {
            let anchor = anchor.map(|anchor| match anchor {
                Anchor::AtCommit {
                    commit_id,
                    position,
                } => Anchor::AtCommit {
                    commit_id: commit_mappings.map(commit_id),
                    position,
                },
                anchor => anchor,
            });
            let rebase = but_workspace::branch::create_reference(editor, ref_name, anchor, order)?;
            Ok(((), rebase))
        })
    }

    /// Name the anonymous segment whose top commit is `top` as `ref_name`, as seen from
    /// `checkout`, attaching `checkout` to it if it is detached at `top`.
    ///
    /// See [`but_workspace::branch::name_segment()`].
    pub fn name_segment(
        &mut self,
        checkout: &but_graph::WorktreeKind,
        top: ObjectId,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let rebase = but_workspace::branch::name_segment(
                editor,
                checkout,
                commit_mappings.map(top),
                ref_name,
            )?;
            Ok(((), rebase))
        })
    }

    /// `source` selects the checkout `changes` were read from, and hence the one whose
    /// merge base is overridden so it doesn't reintroduce them as uncommitted changes.
    /// A [`ChangeSource::Worktree`] source reads `HEAD^{tree}` from that checkout on disk,
    /// so it must describe the pre-commit state - in practice, run this before any other
    /// operation that could change what the worktree is based on.
    pub fn create_commit(
        &mut self,
        relative_to: RelativeTo,
        side: InsertSide,
        changes: Vec<DiffSpec>,
        message: String,
        source: ChangeSource<'_>,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let but_workspace::commit::CommitCreateOutcome {
                rebase,
                commit_selector,
                rejected_specs,
            } = but_workspace::commit::commit_create(
                editor,
                changes,
                relative_to,
                side,
                &message,
                context_lines,
                source,
            )?;

            let new_commit = commit_selector
                .map(|commit_selector| rebase.lookup_commit(commit_selector))
                .transpose()?;

            Ok((
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                rebase,
            ))
        })
    }

    pub fn insert_blank_commit(
        &mut self,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<CommitIdentifiers> {
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let (rebase, blank_commit_selector) =
                but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;
            let new_commit = rebase.lookup_commit(blank_commit_selector)?;

            Ok((new_commit, rebase))
        })
    }

    /// Cherry-pick commits into the transaction's workspace graph.
    ///
    /// Source and target commit IDs are automatically mapped through changes made earlier in the
    /// transaction. The returned identifiers refer to the newly created commits and can be passed
    /// to subsequent transaction operations.
    ///
    /// If `order_commits_by_parentage` is true then all commits must be in the workspace.
    pub fn cherry_pick_commits(
        &mut self,
        source_commit_ids: impl IntoIterator<Item = ObjectId>,
        relative_to: RelativeTo,
        side: InsertSide,
        order_commits_by_parentage: bool,
    ) -> anyhow::Result<Vec<CommitIdentifiers>> {
        self.rebase(|editor, commit_mappings| {
            let source_commit_ids = source_commit_ids
                .into_iter()
                .map(|commit| commit_mappings.map(commit))
                .collect::<Vec<_>>();
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let source_commit_ids = if order_commits_by_parentage {
                editor
                    .order_commit_selectors_by_parentage(source_commit_ids)?
                    .into_iter()
                    .map(|selector| -> anyhow::Result<_> {
                        let (_, commit) = editor.find_selectable_commit(selector)?;
                        Ok(commit.id)
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?
            } else {
                source_commit_ids
            };

            let (rebase, inserted_selectors) = but_workspace::commit::cherry_pick_commits(
                editor,
                source_commit_ids,
                relative_to,
                side,
            )?;
            let new_commits = inserted_selectors
                .into_iter()
                .map(|selector| rebase.lookup_commit(selector))
                .collect::<anyhow::Result<Vec<_>>>()?;

            Ok((new_commits, rebase))
        })
    }

    pub fn move_commits(
        &mut self,
        subject_commit_ids: impl IntoIterator<Item = ObjectId>,
        relative_to: RelativeTo,
        side: InsertSide,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let subject_commit_ids = subject_commit_ids
                .into_iter()
                .map(|commit| commit_mappings.map(commit));
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let rebase =
                but_workspace::commit::move_commits(editor, subject_commit_ids, relative_to, side)?;

            Ok(((), rebase))
        })
    }

    /// `source` selects the checkout `changes` were read from, see [`Self::create_commit()`].
    /// With a [`ChangeSource::Worktree`] source, `target` may live anywhere in the editor
    /// graph, but that checkout must still describe the pre-amend state.
    pub fn amend_commit(
        &mut self,
        target: ObjectId,
        changes: Vec<DiffSpec>,
        source: ChangeSource<'_>,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings| {
            let but_workspace::commit::CommitAmendOutcome {
                rebase,
                commit_selector,
                rejected_specs,
            } = but_workspace::commit::commit_amend(
                editor,
                commit_mappings.map(target),
                changes,
                context_lines,
                source,
            )?;

            let new_commit = commit_selector
                .map(|commit_selector| rebase.lookup_commit(commit_selector))
                .transpose()?;

            Ok((
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                rebase,
            ))
        })
    }

    pub fn move_committed_changes_between(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        changes: Vec<but_core::DiffSpec>,
    ) -> anyhow::Result<CommitIdentifiers> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings| {
            let source = commit_mappings.map(source);
            let target = commit_mappings.map(target);

            let MoveChangesOutcome {
                rebase,
                destination_selector,
                ..
            } = but_workspace::commit::move_changes_between_commits(
                editor,
                source,
                target,
                changes,
                context_lines,
            )?;

            let new_commit = rebase
                .lookup_commit(destination_selector)
                .context("failed to find rebased commit")?;

            Ok((new_commit, rebase))
        })
    }

    /// Look up a commit that has been rewritten as part of a rebase.
    ///
    /// In most cases this shouldn't be necessary. See [`with_transaction`] for more details.
    pub fn get_mapped_commit(&self, original_commit: ObjectId) -> Option<ObjectId> {
        self.inner.commit_mappings.try_map(original_commit)
    }

    /// Returns the in-memory repository that backs this transaction.
    pub fn repo(&self) -> &gix::Repository {
        self.inner
            .rebase
            .as_ref()
            .expect("rebase is always Some(_)")
            .repo()
    }

    pub fn context_lines(&self) -> u32 {
        self.inner.context_lines
    }

    fn rebase<F, T>(&mut self, f: F) -> anyhow::Result<T>
    where
        F: FnOnce(
            Editor<'rebase, 'rebase, M>,
            &CommitMappings,
        ) -> anyhow::Result<(T, SuccessfulRebase<'rebase, 'rebase, M>)>,
    {
        let editor = self
            .inner
            .rebase
            .take()
            .expect("rebase is always Some(_)")
            .into_editor();
        let (outcome, new_rebase) = f(editor, &self.inner.commit_mappings)?;

        self.inner.commit_mappings = CommitMappings(new_rebase.history.commit_mappings());
        self.inner.rebase = Some(new_rebase);
        Ok(outcome)
    }
}

struct FinalizeOptions {
    checkout: Option<FullName>,
    dry_run: DryRun,
}

#[derive(Debug, Default)]
struct PendingRefChanges {
    eagerly_created_refs: Vec<EagerlyCreatedRef>,
}

impl PendingRefChanges {
    fn record_eager_create(&mut self, ref_name: &FullNameRef, previous: Option<gix::refs::Target>) {
        self.eagerly_created_refs.push(EagerlyCreatedRef {
            name: ref_name.to_owned(),
            previous,
        });
    }

    fn remove_eagerly_created_ref(
        &mut self,
        repo: &gix::Repository,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        if let Some(created_ref_index) = self.eagerly_created_refs.iter().position(|created_ref| {
            created_ref.name.as_ref() == ref_name && created_ref.previous.is_none()
        }) {
            let created_ref = self.eagerly_created_refs.remove(created_ref_index);
            Self::restore_one(repo, created_ref)?;
        }
        Ok(())
    }

    fn rollback(&mut self, repo: &gix::Repository) -> anyhow::Result<()> {
        for created_ref in self.eagerly_created_refs.drain(..).rev() {
            Self::restore_one(repo, created_ref)?;
        }
        Ok(())
    }

    fn restore_one(repo: &gix::Repository, created_ref: EagerlyCreatedRef) -> anyhow::Result<()> {
        let EagerlyCreatedRef { name, previous } = created_ref;
        match previous {
            Some(target) => {
                repo.edit_references([RefEdit::update(name, target, PreviousValue::Any, "")])?;
            }
            None => {
                if repo.try_find_reference(name.as_ref())?.is_some() {
                    repo.edit_references([RefEdit::delete(name, PreviousValue::MustExist)])?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug)]
struct EagerlyCreatedRef {
    name: FullName,
    previous: Option<gix::refs::Target>,
}

struct PendingBranchMetadata {
    name: FullName,
    value: ref_metadata::Branch,
}

#[derive(Debug, Default)]
struct CommitMappings(BTreeMap<gix::ObjectId, gix::ObjectId>);

impl CommitMappings {
    fn map(&self, commit: ObjectId) -> ObjectId {
        self.try_map(commit).unwrap_or(commit)
    }

    fn try_map(&self, commit: ObjectId) -> Option<ObjectId> {
        self.0.get(&commit).copied()
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for () {}
    impl<T> Sealed for super::Rollback<T> {}
    impl<T, K> Sealed for super::DynamicOutcome<T, K> {}
    impl<T> Sealed for super::Commit<T> {}
}

pub trait TransactionOutcome: sealed::Sealed {
    type Outcome;

    fn should_rollback(&self) -> bool;

    /// Package the callback's value together with the workspace the transaction produced.
    ///
    /// `workspace` is `Some` exactly when [`Self::should_rollback`] returned `false`; a
    /// rolled-back transaction is never materialized and so has no workspace to report.
    fn into_outcome(self, workspace: Option<WorkspaceState>) -> Self::Outcome;
}

/// The workspace state that [`TransactionOutcome::into_outcome`] is handed whenever the
/// transaction commits.
fn committed_workspace(workspace: Option<WorkspaceState>) -> WorkspaceState {
    workspace.expect("a committed transaction always materializes a workspace")
}

impl TransactionOutcome for () {
    type Outcome = WorkspaceState;

    fn should_rollback(&self) -> bool {
        false
    }

    fn into_outcome(self, workspace: Option<WorkspaceState>) -> Self::Outcome {
        committed_workspace(workspace)
    }
}

/// Statically roll back the current transaction.
#[must_use = "`Rollback` must be returned from `with_transaction` for the transaction to be rolled back"]
pub struct Rollback<T>(T);

impl<T> TransactionOutcome for Rollback<T> {
    type Outcome = T;

    fn should_rollback(&self) -> bool {
        true
    }

    fn into_outcome(self, _workspace: Option<WorkspaceState>) -> Self::Outcome {
        self.0
    }
}

/// Always commit the transaction.
#[must_use]
pub struct Commit<T>(pub T);

impl<T> TransactionOutcome for Commit<T> {
    type Outcome = (T, WorkspaceState);

    fn should_rollback(&self) -> bool {
        false
    }

    fn into_outcome(self, workspace: Option<WorkspaceState>) -> Self::Outcome {
        (self.0, committed_workspace(workspace))
    }
}

/// Dynamically either commit or roll back the current transaction.
#[must_use = "`DynamicOutcome` must be returned from `with_transaction` otherwise the transaction will be committed"]
pub enum DynamicOutcome<T, K> {
    Commit(T),
    Rollback(K),
}

impl<T, K> TransactionOutcome for DynamicOutcome<T, K> {
    type Outcome = DynamicOutcome<(T, WorkspaceState), K>;

    fn should_rollback(&self) -> bool {
        matches!(self, Self::Rollback(_))
    }

    fn into_outcome(self, workspace: Option<WorkspaceState>) -> Self::Outcome {
        match self {
            DynamicOutcome::Commit(value) => {
                DynamicOutcome::Commit((value, committed_workspace(workspace)))
            }
            DynamicOutcome::Rollback(value) => DynamicOutcome::Rollback(value),
        }
    }
}

fn workspace_state_from_rebase<M: RefMetadata>(
    rebase: SuccessfulRebase<'_, '_, M>,
    repo: &gix::Repository,
    pending_metadata_removals: Vec<FullName>,
    pending_branch_metadata: Vec<PendingBranchMetadata>,
    options: FinalizeOptions,
) -> anyhow::Result<WorkspaceState> {
    let FinalizeOptions {
        checkout: pending_checkout,
        dry_run,
    } = options;
    if dry_run.into() {
        let Some(branch) = pending_checkout else {
            return WorkspaceState::from_successful_rebase(rebase, repo, dry_run);
        };
        let target = rebase
            .reference_target(branch.as_ref())
            .or_else(|_| resolve_checkout_target(rebase.repo(), branch.as_ref()))?;
        let replaced_commits = rebase.history.commit_mappings();
        let conflicted_commits = rebase.history.conflicted_commits.clone();
        let workspace = rebase
            .overlayed_graph_with_workspace_overrides(Some((target, branch)), None)?
            .into_workspace()?;
        let mut rebase = rebase;
        let (repo, meta, db) = rebase.repo_meta_and_db_mut();
        return WorkspaceState::from_workspace_with_db(
            &workspace,
            meta,
            repo,
            replaced_commits,
            conflicted_commits,
            db,
        );
    }

    let materialized = rebase.materialize(Default::default())?;
    for branch in pending_branch_metadata {
        let mut handle = materialized.meta.branch(branch.name.as_ref())?;
        *handle = branch.value;
        materialized.meta.set_branch(&handle)?;
    }
    for ref_name in pending_metadata_removals {
        materialized.meta.remove(ref_name.as_ref())?;
    }
    if let Some(branch) = pending_checkout {
        checkout_reference(repo, branch.as_ref())?;
        let project_meta = materialized.workspace.graph.project_meta.clone();
        materialized.workspace.refresh_from_head(
            repo,
            &*materialized.meta,
            project_meta,
            &mut *materialized.db,
        )?;
    }

    WorkspaceState::from_materialized(materialized, repo)
}

fn resolve_checkout_target(
    repo: &gix::Repository,
    reference_name: &FullNameRef,
) -> anyhow::Result<ObjectId> {
    let mut reference = repo
        .find_reference(reference_name)
        .with_context(|| format!("Could not find ref '{}'", reference_name.as_bstr()))?;
    let target = reference
        .peel_to_id()
        .with_context(|| format!("Could not resolve ref '{}'", reference_name.as_bstr()))?
        .detach();
    repo.find_commit(target).with_context(|| {
        format!(
            "Ref '{}' does not point to a commit",
            reference_name.as_bstr()
        )
    })?;
    Ok(target)
}

fn checkout_reference(repo: &gix::Repository, reference_name: &FullNameRef) -> anyhow::Result<()> {
    let current_head = repo
        .head_id()
        .context("Cannot check out a branch while HEAD is unborn")?
        .detach();
    let target = resolve_checkout_target(repo, reference_name)?;
    let target_commit = repo.find_commit(target)?;

    safe_checkout_from_head(
        target,
        repo,
        checkout::Options {
            skip_head_update: true,
            ..Default::default()
        },
    )
    .with_context(|| {
        format!(
            "Could not safely check out '{}' from {current_head} to {target}",
            reference_name.as_bstr()
        )
    })?;
    update_head_reference(
        repo,
        Target::Symbolic(reference_name.to_owned()),
        false,
        "checkout",
        reference_name.as_bstr(),
        target_commit.parent_ids().count(),
    )
    .with_context(|| format!("Could not update HEAD to '{}'", reference_name.as_bstr()))?;
    Ok(())
}

/// Intermediate outcome after creating a commit.
///
/// It is intermediate in the sense that the commit hasn't been materialized yet and only exists
/// in-memory.
pub struct IntermediateCommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<CommitIdentifiers>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
}
