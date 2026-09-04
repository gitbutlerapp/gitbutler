use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use anyhow::Context as _;
use bstr::BStr;
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
use but_rebase::graph_rebase::{Editor, RebasedEditor, anchor::Anchor, mutate::InsertSide};
use but_workspace::commit::{
    ChangeSource, MoveChangesOutcome, SquashCommitsOutcome,
    squash_commits::MessageCombinationStrategy,
};
use gix::{
    ObjectId,
    refs::{
        FullName, FullNameRef, Target,
        transaction::{Change, PreviousValue, RefEdit},
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

        let editor = Editor::for_workspace(&ws, meta, &repo)?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            workspace: ws.clone(),
            commit_mappings: CommitMappings::default(),
            pending_metadata_updates: Vec::new(),
            pending_created_independent_refs: Vec::new(),
            pending_ref_changes: PendingRefChanges::default(),
            pending_checkout: None,
            context_lines,
            materialize_without_checkout: MaterializeWithoutCheckout::Either,
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
            workspace: _,
            commit_mappings: _,
            pending_metadata_updates,
            pending_created_independent_refs,
            mut pending_ref_changes,
            pending_checkout,
            context_lines: _,
            materialize_without_checkout,
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
                &mut ws,
                &mut db,
                pending_metadata_updates,
                pending_created_independent_refs,
                FinalizeOptions {
                    checkout: pending_checkout,
                    dry_run,
                    materialize_without_checkout: matches!(
                        materialize_without_checkout,
                        MaterializeWithoutCheckout::Yes
                    ),
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
    rebase: Option<RebasedEditor<'rebase, M>>,
    // The workspace the transaction started from, used as the base for overlay previews.
    workspace: but_graph::Workspace,
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<PendingCreatedIndependentRef>,
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
    // How to materialize the final rebase outcome unfortunately depends on which operations we
    // perform. Most operations need `materialize` but uncommitting needs
    // `materialize_without_checkout`. `Either` means no operation has demanded one yet.
    //
    // Mixing different kinds of materialize requests results in an error.
    materialize_without_checkout: MaterializeWithoutCheckout,
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
            let SquashCommitsOutcome { rebase, commit } = but_workspace::commit::squash_commits(
                editor,
                subjects
                    .into_iter()
                    .map(|commit| commit_mappings.map(commit))
                    .collect(),
                commit_mappings.map(target),
                how_to_combine_messages,
            )?;
            let new_commit = rebase.identifiers_of(commit)?;
            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
        })
    }

    pub fn reword_commit(
        &mut self,
        commit: ObjectId,
        message: &BStr,
    ) -> anyhow::Result<CommitIdentifiers> {
        self.rebase(|editor, commit_mappings| {
            let commit = editor.select_commit(commit_mappings.map(commit))?;
            let (rebase, edited_commit_handle) =
                but_workspace::commit::reword(editor, commit, message)?;
            let new_commit = rebase.identifiers_of(edited_commit_handle)?;
            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
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
            Ok(((), MaterializeWithoutCheckout::No, rebase))
        })
    }

    pub fn uncommit_commits(
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
            Ok(((), MaterializeWithoutCheckout::Yes, rebase))
        })
    }

    pub fn discard_changes_from_commit(
        &mut self,
        source: gix::ObjectId,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<CommitIdentifiers> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let but_workspace::commit::UncommitChangesOutcome { rebase, commit } = {
                let source = editor.select_commit(commit_mappings.map(source))?;
                but_workspace::commit::uncommit_changes(editor, source, changes, context_lines)?
            };

            let new_commit = rebase.identifiers_of(commit)?;
            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
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

        resolve_checkout_target(self.repo(), branch)?;

        self.request_materialization(MaterializeWithoutCheckout::No)?;
        self.inner.pending_checkout = Some(branch.to_owned());
        Ok(())
    }

    pub fn remove_reference(&mut self, ref_name: &FullNameRef) -> anyhow::Result<()> {
        let workspace = self.inner.workspace.clone();
        self.rebase(|mut editor, _| {
            let handle = editor.select_reference(ref_name)?;
            // Removing a reference lets its dependents heal onto whatever lies below it. When
            // the only dependent is the workspace commit and the reference rests on
            // integrated history (an empty lane on the base), healing would keep the base as a
            // merge parent — so that parent entry goes, not heals: the lane is gone.
            let children = editor.direct_children(Anchor::Held(handle.into()))?;
            if let [(ws_commit, _)] = children[..]
                && let Some(ws_index) = ws_commit.as_commit()
                && editor.direct_parents(Anchor::Held(ws_commit))?.len() > 1
                && but_graph::workspace::commit::is_managed_workspace_by_message(
                    editor.commit_of(ws_index)?.message.as_ref(),
                )
                && rests_on_integrated_history(&editor, &workspace, handle.into())?
            {
                editor.detach(Anchor::Held(ws_commit), Anchor::Held(handle.into()))?;
            }
            editor.remove_reference(handle)?;
            let rebase = editor.rebase()?;
            Ok(((), MaterializeWithoutCheckout::Either, rebase))
        })?;
        let repo = self.repo().clone();
        self.inner
            .pending_ref_changes
            .remove_eagerly_created_ref(&repo, ref_name)?;
        // Cancel metadata this transaction recorded for the name: an eagerly created ref
        // leaves no delete edit behind, so nothing else would stop the pending write —
        // branch updates drop, recorded workspace listings lose the segment. Refs that
        // existed on disk need no entry here — materialize retires the metadata of every
        // name its ref transaction deletes.
        self.inner
            .pending_metadata_updates
            .retain_mut(|update| match update {
                PendingMetadataUpdate::Branch(branch) => branch.as_ref() != ref_name,
                PendingMetadataUpdate::Workspace(workspace) => {
                    workspace.value.remove_segment(ref_name);
                    true
                }
                PendingMetadataUpdate::BranchStackOrder(branches) => {
                    branches.retain(|branch| branch.as_ref() != ref_name);
                    !branches.is_empty()
                }
            });
        Ok(())
    }

    /// Restack `source_branch` on top of `target_branch` within the transaction's workspace.
    ///
    /// Transactions operate on managed workspaces only. The ad-hoc (single-branch) move path is the
    /// one that populates [`Outcome::new_tip`] and [`Outcome::branch_stack_order`] for the caller to
    /// apply, and `RecordingMetadata` can't persist branch stack order anyway, so we bail if either
    /// field is ever set rather than silently dropping a metadata reorder or a required checkout.
    ///
    /// [`Outcome::new_tip`]: but_workspace::branch::move_branch::Outcome::new_tip
    /// [`Outcome::branch_stack_order`]: but_workspace::branch::move_branch::Outcome::branch_stack_order
    pub fn stack_branch_on(
        &mut self,
        source_branch: &FullNameRef,
        target_branch: &FullNameRef,
    ) -> anyhow::Result<()> {
        let workspace = self.inner.workspace.clone();
        let (ws_meta, new_tip, branch_stack_order) = self.rebase(|editor, _| {
            let outcome = but_workspace::branch::move_branch(
                editor,
                &workspace,
                source_branch,
                target_branch,
            )?;
            Ok((
                (outcome.ws_meta, outcome.new_tip, outcome.branch_stack_order),
                MaterializeWithoutCheckout::No,
                outcome.rebase,
            ))
        })?;

        anyhow::ensure!(
            new_tip.is_none() && branch_stack_order.is_none(),
            "Ad-hoc (single-branch) branch moves are not supported inside transactions"
        );

        self.record_workspace_metadata_update(ws_meta)?;

        Ok(())
    }

    pub fn tear_off_branch(&mut self, source_branch: &FullNameRef) -> anyhow::Result<()> {
        let workspace = self.inner.workspace.clone();
        let ws_meta = self.rebase(|editor, _| {
            let outcome =
                but_workspace::branch::tear_off_branch(editor, &workspace, source_branch, None)?;
            Ok((
                outcome.ws_meta,
                MaterializeWithoutCheckout::No,
                outcome.rebase,
            ))
        })?;

        self.record_workspace_metadata_update(ws_meta)?;

        Ok(())
    }

    fn record_workspace_metadata_update(
        &mut self,
        ws_meta: Option<ref_metadata::Workspace>,
    ) -> anyhow::Result<()> {
        let Some(ws_meta) = ws_meta else {
            return Ok(());
        };

        let workspace = but_workspace::workspace::overlayed_workspace(
            &self.inner.workspace,
            self.inner
                .rebase
                .as_ref()
                .expect("rebase is always Some(_)"),
        )?;

        let ref_name = workspace
            .ref_name()
            .context("workspace metadata update requires workspace ref")?
            .to_owned();

        self.inner
            .pending_metadata_updates
            .push(PendingMetadataUpdate::Workspace(RecordingMetadataHandle {
                name: ref_name,
                value: ws_meta,
                is_default: false,
            }));

        Ok(())
    }

    pub fn create_reference<'name>(
        &mut self,
        ref_name: &FullNameRef,
        anchor: impl Into<Option<but_workspace::branch::create_reference::Anchor<'name>>>,
        new_stack_id: impl FnOnce(&FullNameRef) -> ref_metadata::StackId,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<()> {
        let anchor = anchor.into();
        let order = order.into();
        let creates_independent_branch = anchor.is_none();
        let previous = self
            .repo()
            .try_find_reference(ref_name)?
            .map(|reference| reference.target().into());

        let workspace = but_workspace::workspace::overlayed_workspace(
            &self.inner.workspace,
            self.inner
                .rebase
                .as_ref()
                .expect("rebase is always Some(_)"),
        )?;
        let (anchor, anchor_segment_oldest_commit_id) = match anchor {
            Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                ref_name,
                position,
            }) => {
                let (_, segment) = workspace.try_find_branch(ref_name.as_ref())?;
                if matches!(
                    position,
                    but_workspace::branch::create_reference::Position::Below
                ) && segment.tip().is_none()
                {
                    (
                        Some(
                            but_workspace::branch::create_reference::Anchor::AtReference {
                                ref_name,
                                position,
                            },
                        ),
                        None,
                    )
                } else {
                    let oldest_commit_id = match segment.commits.last().copied() {
                        Some(id) => id,
                        // `segment` came from the derivation above, so its resting commit
                        // must come from there too. Falling back to the display answered the same
                        // question from a pruned view, which would let display policy decide where
                        // a reference is anchored.
                        None => workspace
                            .try_branch_resting_commit_id(ref_name.as_ref())
                            .with_context(|| {
                                format!(
                                    "Cannot position reference below unborn segment '{}'",
                                    ref_name.shorten()
                                )
                            })?,
                    };
                    (
                        Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                            ref_name,
                            position,
                        }),
                        Some(oldest_commit_id),
                    )
                }
            }
            anchor => (anchor, None),
        };
        let repo = self.repo().clone();
        let branch_stack_orders = self
            .inner
            .pending_metadata_updates
            .iter()
            .filter_map(|update| match update {
                PendingMetadataUpdate::Workspace(_) | PendingMetadataUpdate::Branch(_) => None,
                PendingMetadataUpdate::BranchStackOrder(branches) => Some(branches.clone()),
            })
            .collect();
        let rebase = self
            .inner
            .rebase
            .as_mut()
            .expect("rebase is always Some(_)");
        let (_, persisted_meta) = rebase.repo_and_meta_mut();
        let mut meta = RecordingMetadata {
            persisted_meta,
            workspace_name: workspace.ref_name().map(ToOwned::to_owned),
            workspace: workspace.metadata_from_projection()?,
            branch_stack_orders,
            updates: Vec::new(),
        };

        but_workspace::branch::create_reference(
            ref_name,
            anchor.clone(),
            &repo,
            &workspace,
            &mut meta,
            new_stack_id,
            order,
        )?;

        self.inner
            .pending_metadata_updates
            .append(&mut meta.updates);
        if creates_independent_branch {
            self.inner
                .pending_created_independent_refs
                .push(PendingCreatedIndependentRef {
                    name: ref_name.to_owned(),
                    order,
                });
        }
        self.inner
            .pending_ref_changes
            .record_eager_create(ref_name, previous);

        self.rebase(|mut editor, _| {
            if editor.try_select_reference(ref_name).is_some() {
                return Ok(((), MaterializeWithoutCheckout::No, editor.rebase()?));
            }

            let target_id = editor
                .repo()
                .find_reference(ref_name)?
                .peel_to_id()?
                .detach();
            let reference = ref_name.to_owned();

            match anchor {
                Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                    commit_id,
                    position: but_workspace::branch::create_reference::Position::Below,
                }) => {
                    editor.insert_reference(
                        editor.select_commit(commit_id)?,
                        reference,
                        InsertSide::Below,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                    ref_name: anchor_ref,
                    position: but_workspace::branch::create_reference::Position::Above,
                }) => {
                    editor.insert_reference(
                        editor.select_reference(anchor_ref.as_ref())?,
                        reference,
                        InsertSide::Above,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                    position: but_workspace::branch::create_reference::Position::Below,
                    ..
                }) => {
                    let anchor_oldest_commit = anchor_segment_oldest_commit_id
                        .expect("AtSegment anchor always has oldest commit resolved");
                    editor.insert_reference(
                        editor.select_commit(anchor_oldest_commit)?,
                        reference,
                        InsertSide::Below,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtReference {
                    ref_name: anchor_ref,
                    position,
                }) => {
                    let side = match position {
                        but_workspace::branch::create_reference::Position::Above => {
                            InsertSide::Above
                        }
                        but_workspace::branch::create_reference::Position::Below => {
                            InsertSide::Below
                        }
                    };
                    editor.insert_reference(
                        editor.select_reference(anchor_ref.as_ref())?,
                        reference,
                        side,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                    position: but_workspace::branch::create_reference::Position::Above,
                    ..
                }) => {
                    let target = editor.select_commit(target_id)?;
                    let reference = editor.add_reference(reference)?;
                    editor.insert_parent(reference, target, 0)?;
                }
                None => {
                    let target = editor.select_commit(target_id)?;
                    let reference = editor.add_reference(reference)?;
                    editor.insert_parent(reference, target, 0)?;
                    // A new independent branch is a new lane: hang it off the workspace commit
                    // at the requested position so the merge's parent order — which is the
                    // lane order — follows the metadata order it was created with.
                    if workspace.kind().has_managed_commit()
                        && let Some(ws_tip) = workspace.tip_commit_id()
                    {
                        let ws_commit = editor.select_commit(ws_tip)?;
                        editor.insert_parent(ws_commit, reference, order.unwrap_or(usize::MAX))?;
                    }
                }
            }
            Ok(((), MaterializeWithoutCheckout::No, editor.rebase()?))
        })
    }

    /// `source` selects the checkout `changes` were read from, and hence the one whose
    /// merge base is overridden so it doesn't reintroduce them as uncommitted changes.
    /// A [`ChangeSource::Worktree`] source reads `HEAD^{tree}` from that checkout on disk,
    /// so it must describe the pre-commit state - in practice, run this before any other
    /// operation that could change what the worktree is based on.
    pub fn create_commit(
        &mut self,
        relative_to: Anchor,
        side: InsertSide,
        changes: Vec<DiffSpec>,
        message: String,
        source: ChangeSource<'_>,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                Anchor::Commit(object_id) => Anchor::Commit(commit_mappings.map(object_id)),
                other => other,
            };

            let but_workspace::commit::CommitCreateOutcome {
                rebase,
                commit,
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

            let new_commit = commit
                .map(|commit| rebase.identifiers_of(commit))
                .transpose()?;

            Ok((
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                MaterializeWithoutCheckout::No,
                rebase,
            ))
        })
    }

    pub fn insert_blank_commit(
        &mut self,
        relative_to: Anchor,
        side: InsertSide,
    ) -> anyhow::Result<CommitIdentifiers> {
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                Anchor::Commit(object_id) => Anchor::Commit(commit_mappings.map(object_id)),
                other => other,
            };

            let (rebase, blank_commit_handle) =
                but_workspace::commit::insert_blank_commit(editor, relative_to, side)?;
            let new_commit = rebase.identifiers_of(blank_commit_handle)?;

            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
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
        relative_to: Anchor,
        side: InsertSide,
        order_commits_by_parentage: bool,
    ) -> anyhow::Result<Vec<CommitIdentifiers>> {
        self.rebase(|editor, commit_mappings| {
            let source_commit_ids = source_commit_ids
                .into_iter()
                .map(|commit| commit_mappings.map(commit))
                .collect::<Vec<_>>();
            let relative_to = match relative_to {
                Anchor::Commit(object_id) => Anchor::Commit(commit_mappings.map(object_id)),
                other => other,
            };

            let source_commit_ids = if order_commits_by_parentage {
                let commits = source_commit_ids
                    .iter()
                    .map(|id| editor.select_commit(*id))
                    .collect::<anyhow::Result<Vec<_>>>()?;
                editor
                    .order_by_parentage(commits)?
                    .into_iter()
                    .map(|commit| editor.id_of(commit))
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
                .map(|handle| rebase.identifiers_of(handle))
                .collect::<anyhow::Result<Vec<_>>>()?;

            Ok((new_commits, MaterializeWithoutCheckout::No, rebase))
        })
    }

    pub fn move_commits(
        &mut self,
        subject_commit_ids: impl IntoIterator<Item = ObjectId>,
        relative_to: Anchor,
        side: InsertSide,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings| {
            let subject_commit_ids = subject_commit_ids
                .into_iter()
                .map(|commit| commit_mappings.map(commit));
            let relative_to = match relative_to {
                Anchor::Commit(object_id) => Anchor::Commit(commit_mappings.map(object_id)),
                other => other,
            };

            let rebase =
                but_workspace::commit::move_commits(editor, subject_commit_ids, relative_to, side)?;

            Ok(((), MaterializeWithoutCheckout::No, rebase))
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
                commit,
                rejected_specs,
            } = {
                let target = editor.select_commit(commit_mappings.map(target))?;
                but_workspace::commit::commit_amend(editor, target, changes, context_lines, source)?
            };

            let new_commit = commit
                .map(|commit| rebase.identifiers_of(commit))
                .transpose()?;

            Ok((
                IntermediateCommitCreateResult {
                    new_commit,
                    rejected_specs,
                },
                MaterializeWithoutCheckout::No,
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
            let source = editor.select_commit(commit_mappings.map(source))?;
            let target = editor.select_commit(commit_mappings.map(target))?;

            let MoveChangesOutcome {
                rebase,
                destination,
                ..
            } = but_workspace::commit::move_changes_between_commits(
                editor,
                source,
                target,
                changes,
                context_lines,
            )?;

            let new_commit = rebase
                .identifiers_of(destination)
                .context("failed to find rebased commit")?;

            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
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
            Editor<'rebase, M>,
            &CommitMappings,
        )
            -> anyhow::Result<(T, MaterializeWithoutCheckout, RebasedEditor<'rebase, M>)>,
    {
        let editor = self
            .inner
            .rebase
            .take()
            .expect("rebase is always Some(_)")
            .into_editor();
        let (outcome, materialize_without_checkout, new_rebase) =
            f(editor, &self.inner.commit_mappings)?;

        self.request_materialization(materialize_without_checkout)?;

        self.inner.commit_mappings = CommitMappings(new_rebase.commit_mappings());
        self.inner.rebase = Some(new_rebase);
        Ok(outcome)
    }

    fn request_materialization(
        &mut self,
        requested: MaterializeWithoutCheckout,
    ) -> anyhow::Result<()> {
        match (self.inner.materialize_without_checkout, requested) {
            (_, MaterializeWithoutCheckout::Either) => {}
            (MaterializeWithoutCheckout::Either, requested) => {
                self.inner.materialize_without_checkout = requested;
            }
            (demanded, requested) => anyhow::ensure!(
                demanded == requested,
                "cannot mix operations that require `materialize` and `materialize_without_checkout`"
            ),
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializeWithoutCheckout {
    Yes,
    No,
    Either,
}

struct FinalizeOptions {
    checkout: Option<FullName>,
    dry_run: DryRun,
    materialize_without_checkout: bool,
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
                repo.edit_references([RefEdit {
                    name,
                    change: Change::Update {
                        log: Default::default(),
                        expected: PreviousValue::Any,
                        new: target,
                    },
                    deref: false,
                }])?;
            }
            None => {
                if repo.try_find_reference(name.as_ref())?.is_some() {
                    repo.edit_references([RefEdit {
                        name,
                        change: Change::Delete {
                            log: gix::refs::transaction::RefLog::AndReference,
                            expected: PreviousValue::MustExist,
                        },
                        deref: false,
                    }])?;
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

#[derive(Debug)]
struct PendingCreatedIndependentRef {
    name: FullName,
    order: Option<usize>,
}

#[derive(Clone)]
enum PendingMetadataUpdate {
    Workspace(RecordingMetadataHandle<ref_metadata::Workspace>),
    Branch(RecordingMetadataHandle<ref_metadata::Branch>),
    BranchStackOrder(Vec<FullName>),
}

struct RecordingMetadata<'meta, M: RefMetadata> {
    persisted_meta: &'meta M,
    workspace_name: Option<FullName>,
    workspace: Option<ref_metadata::Workspace>,
    branch_stack_orders: Vec<Vec<FullName>>,
    updates: Vec<PendingMetadataUpdate>,
}

#[derive(Clone)]
struct RecordingMetadataHandle<T> {
    name: FullName,
    value: T,
    is_default: bool,
}

impl<T> Deref for RecordingMetadataHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for RecordingMetadataHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> AsRef<FullNameRef> for RecordingMetadataHandle<T> {
    fn as_ref(&self) -> &FullNameRef {
        self.name.as_ref()
    }
}

impl<T> ref_metadata::ValueInfo for RecordingMetadataHandle<T> {
    fn is_default(&self) -> bool {
        self.is_default
    }
}

impl<M: RefMetadata> RefMetadata for RecordingMetadata<'_, M> {
    type Handle<T> = RecordingMetadataHandle<T>;

    fn iter(&self) -> impl Iterator<Item = anyhow::Result<(FullName, Box<dyn std::any::Any>)>> {
        std::iter::empty()
    }

    fn workspace(
        &self,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<Self::Handle<ref_metadata::Workspace>> {
        let value = self
            .workspace_name
            .as_ref()
            .filter(|name| name.as_ref() == ref_name)
            .and_then(|_| self.workspace.clone());
        let is_default = value.is_none();
        Ok(RecordingMetadataHandle {
            name: ref_name.to_owned(),
            value: value.unwrap_or_default(),
            is_default,
        })
    }

    fn branch(&self, ref_name: &FullNameRef) -> anyhow::Result<Self::Handle<ref_metadata::Branch>> {
        Ok(RecordingMetadataHandle {
            name: ref_name.to_owned(),
            value: ref_metadata::Branch::default(),
            is_default: true,
        })
    }

    fn set_workspace(
        &mut self,
        value: &Self::Handle<ref_metadata::Workspace>,
    ) -> anyhow::Result<()> {
        self.updates
            .push(PendingMetadataUpdate::Workspace(RecordingMetadataHandle {
                name: value.name.clone(),
                value: value.value.clone(),
                is_default: value.is_default,
            }));
        Ok(())
    }

    fn set_branch(&mut self, value: &Self::Handle<ref_metadata::Branch>) -> anyhow::Result<()> {
        self.updates
            .push(PendingMetadataUpdate::Branch(RecordingMetadataHandle {
                name: value.name.clone(),
                value: value.value.clone(),
                is_default: value.is_default,
            }));
        Ok(())
    }

    fn branch_stack_order(&self, ref_name: &FullNameRef) -> anyhow::Result<Option<Vec<FullName>>> {
        let pending_order = self
            .updates
            .iter()
            .rev()
            .filter_map(|update| match update {
                PendingMetadataUpdate::Workspace(_) | PendingMetadataUpdate::Branch(_) => None,
                PendingMetadataUpdate::BranchStackOrder(branches) => Some(branches),
            })
            .chain(self.branch_stack_orders.iter().rev())
            .find(|branches| branches.iter().any(|branch| branch.as_ref() == ref_name));

        match pending_order {
            Some(branches) => Ok(Some(branches.clone())),
            None => self.persisted_meta.branch_stack_order(ref_name),
        }
    }

    fn set_branch_stack_order(&mut self, branches: &[FullName]) -> anyhow::Result<()> {
        self.updates
            .push(PendingMetadataUpdate::BranchStackOrder(branches.to_vec()));
        Ok(())
    }

    fn can_persist_branch_stack_order(&self) -> bool {
        self.persisted_meta.can_persist_branch_stack_order()
    }

    fn remove(&mut self, _ref_name: &FullNameRef) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn rename(
        &mut self,
        _old_ref_name: &FullNameRef,
        _new_ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        // Renames aren't part of the recorded transaction surface (like `remove`, which is handled
        // out-of-band via `Transaction::remove_reference`); nothing to record here.
        Ok(())
    }
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

#[allow(clippy::too_many_arguments)]
fn workspace_state_from_rebase<M: RefMetadata>(
    rebase: RebasedEditor<'_, M>,
    repo: &gix::Repository,
    workspace: &mut but_graph::Workspace,
    db: &mut but_db::DbHandle,
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<PendingCreatedIndependentRef>,
    options: FinalizeOptions,
) -> anyhow::Result<WorkspaceState> {
    let FinalizeOptions {
        checkout: pending_checkout,
        dry_run,
        materialize_without_checkout,
    } = options;
    if dry_run.into() {
        let Some(branch) = pending_checkout else {
            return WorkspaceState::from_successful_rebase_with_db(
                workspace, rebase, repo, dry_run, db,
            );
        };
        let target = rebase
            .reference_target(branch.as_ref())
            .or_else(|_| resolve_checkout_target(rebase.repo(), branch.as_ref()))?;
        let replaced_commits = rebase.commit_mappings();
        let overlay = rebase.overlay_with(Some((target, branch)), None)?;
        let preview = workspace.preview_from_commit_graph(
            rebase.commit_graph().clone(),
            rebase.repo(),
            rebase.meta(),
            overlay,
        )?;
        let mut rebase = rebase;
        let (repo, meta) = rebase.repo_and_meta_mut();
        return WorkspaceState::from_workspace_with_db(&preview, meta, repo, replaced_commits, db);
    }

    let commit_mappings = rebase.commit_mappings();
    let (graph, meta, checkout_conflict_occurred) = if materialize_without_checkout {
        let (graph, meta) = rebase.materialize_without_checkout()?;
        (graph, meta, false)
    } else {
        let materialized = rebase.materialize_with_outcome()?;
        (
            materialized.commit_graph,
            materialized.meta,
            materialized.checkout_conflict_occurred,
        )
    };
    workspace.refresh_from_commit_graph(graph, repo, &*meta, db)?;
    for branch in pending_created_independent_refs {
        if workspace.find_branch(branch.name.as_ref()).is_some() {
            continue;
        }
        let outcome = but_workspace::branch::apply(
            branch.name.as_ref(),
            workspace,
            repo,
            meta,
            but_workspace::branch::apply::Options {
                order: branch.order,
                ..Default::default()
            },
        )?;
        *workspace = outcome.workspace;
    }
    for update in pending_metadata_updates {
        match update {
            PendingMetadataUpdate::Workspace(workspace) => {
                let mut handle = meta.workspace(workspace.as_ref())?;
                *handle = workspace.value;
                meta.set_workspace(&handle)?;
            }
            PendingMetadataUpdate::Branch(branch) => {
                let mut handle = meta.branch(branch.as_ref())?;
                *handle = branch.value;
                meta.set_branch(&handle)?;
            }
            PendingMetadataUpdate::BranchStackOrder(branches) => {
                meta.set_branch_stack_order(&branches)?;
            }
        }
    }
    if let Some(branch) = pending_checkout {
        checkout_reference(repo, branch.as_ref())?;
        let project_meta = workspace.project_meta().clone();
        workspace.refresh_from_head(repo, &*meta, project_meta, db)?;
    }

    WorkspaceState::from_workspace_with_db_and_checkout(
        workspace,
        meta,
        repo,
        commit_mappings,
        db,
        checkout_conflict_occurred,
    )
}

/// Whether `entry` sits on integrated history: following its single-parent chain down through
/// references reaches a commit the walk flagged integrated (the target or below).
fn rests_on_integrated_history<M: RefMetadata>(
    editor: &but_rebase::graph_rebase::Editor<'_, M>,
    workspace: &but_graph::Workspace,
    mut entry: but_rebase::graph_rebase::EditorIndex,
) -> anyhow::Result<bool> {
    loop {
        if let Some(commit) = entry.as_commit() {
            let id = editor.id_of(commit)?;
            return Ok(workspace
                .commit_graph()
                .node(id)
                .is_some_and(|n| n.flags.contains(but_graph::CommitFlags::Integrated)));
        }
        let parents = editor.direct_parents(Anchor::Held(entry))?;
        let [(parent, _)] = parents[..] else {
            return Ok(false);
        };
        entry = parent;
    }
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
