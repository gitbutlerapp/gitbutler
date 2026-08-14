use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use anyhow::Context as _;
use bstr::{BStr, BString, ByteVec};
use but_api::WorkspaceState;
use but_core::{
    DiffSpec, DryRun, RefMetadata, commit::CommitIdentifiers, ref_metadata, sync::RepoExclusive,
    tree::create_tree::RejectionReason,
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Step, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::commit::{
    ChangeSource, MoveChangesOutcome, SquashCommitsOutcome,
    squash_commits::MessageCombinationStrategy,
};
use gix::{
    ObjectId,
    refs::{
        FullName, FullNameRef,
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

    let (should_rollback, outcome) = {
        let context_lines = ctx.settings.context_lines;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;

        let editor = Editor::create(&mut ws, meta, &repo, &mut db)?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            commit_mappings: CommitMappings::default(),
            pending_metadata_removals: Vec::new(),
            pending_metadata_updates: Vec::new(),
            pending_created_independent_refs: Vec::new(),
            pending_ref_changes: PendingRefChanges::default(),
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
            commit_mappings: _,
            pending_metadata_removals,
            pending_metadata_updates,
            pending_created_independent_refs,
            mut pending_ref_changes,
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
                pending_metadata_removals,
                pending_metadata_updates,
                pending_created_independent_refs,
                dry_run,
                matches!(
                    materialize_without_checkout,
                    MaterializeWithoutCheckout::Yes
                ),
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

    if !should_rollback && let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm)?;
    }

    Ok(outcome)
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
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<PendingCreatedIndependentRef>,
    pending_ref_changes: PendingRefChanges,
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
            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
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
            Ok((new_commit, MaterializeWithoutCheckout::No, rebase))
        })
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
                                if refname.as_bstr() == target_local_branch ||
                                    refname.as_bstr() == b"refs/heads/gitbutler/target")
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

            editor.replace(ref_selector, but_rebase::graph_rebase::Step::None)?;
            if let Some(must_disconnect_child) = must_disconnect_child {
                editor.remove_edges(must_disconnect_child, ref_selector)?;
            }
            let rebase = editor.rebase()?;
            Ok(((), MaterializeWithoutCheckout::Either, rebase))
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
        let (ws_meta, new_tip, branch_stack_order) = self.rebase(|editor, _| {
            let outcome = but_workspace::branch::move_branch(editor, source_branch, target_branch)?;
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
        let ws_meta = self.rebase(|editor, _| {
            let outcome = but_workspace::branch::tear_off_branch(editor, source_branch, None)?;
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

        let workspace = self
            .inner
            .rebase
            .as_ref()
            .expect("rebase is always Some(_)")
            .overlayed_graph()?
            .into_workspace()?;

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

        let graph = self
            .inner
            .rebase
            .as_ref()
            .expect("rebase is always Some(_)")
            .overlayed_graph()?;
        let workspace = graph.into_workspace()?;
        let (anchor, anchor_segment_oldest_commit_id) = match anchor {
            Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                ref_name,
                position,
            }) => {
                let (_, segment) =
                    workspace.try_find_segment_and_stack_by_refname(ref_name.as_ref())?;
                if matches!(
                    position,
                    but_workspace::branch::create_reference::Position::Below
                ) && segment.commits.is_empty()
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
                    let oldest_commit_id = segment
                        .commits
                        .last()
                        .map(|commit| commit.id)
                        .or_else(|| {
                            workspace
                                .tip_commit_by_segment_id(segment.id)
                                .map(|commit| commit.id)
                        })
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "Cannot position reference below unborn segment '{}'",
                                ref_name.shorten()
                            )
                        })?;
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
            let reference = Step::new_reference(ref_name.to_owned());

            match anchor {
                Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                    commit_id,
                    position: but_workspace::branch::create_reference::Position::Below,
                }) => {
                    editor.insert(
                        editor.select_commit(commit_id)?,
                        reference,
                        InsertSide::Below,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                    ref_name: anchor_ref,
                    position: but_workspace::branch::create_reference::Position::Above,
                }) => {
                    editor.insert(
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
                    editor.insert(
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
                    editor.insert(
                        editor.select_reference(anchor_ref.as_ref())?,
                        reference,
                        side,
                    )?;
                }
                Some(but_workspace::branch::create_reference::Anchor::AtCommit {
                    position: but_workspace::branch::create_reference::Position::Above,
                    ..
                })
                | None => {
                    let target = editor.select_commit(target_id)?;
                    let reference = editor.add_step(reference)?;
                    editor.add_edge(reference, target, 0)?;
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
                MaterializeWithoutCheckout::No,
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

            Ok((new_commits, MaterializeWithoutCheckout::No, rebase))
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
            Editor<'rebase, 'rebase, M>,
            &CommitMappings,
        ) -> anyhow::Result<(
            T,
            MaterializeWithoutCheckout,
            SuccessfulRebase<'rebase, 'rebase, M>,
        )>,
    {
        let editor = self
            .inner
            .rebase
            .take()
            .expect("rebase is always Some(_)")
            .into_editor();
        let (outcome, materialize_without_checkout, new_rebase) =
            f(editor, &self.inner.commit_mappings)?;

        match (
            self.inner.materialize_without_checkout,
            materialize_without_checkout,
        ) {
            (_, MaterializeWithoutCheckout::Either) => {}
            (MaterializeWithoutCheckout::Either, requested) => {
                self.inner.materialize_without_checkout = requested;
            }
            (demanded, requested) => anyhow::ensure!(
                demanded == requested,
                "cannot mix operations that require `materialize` and `materialize_without_checkout`"
            ),
        }

        self.inner.commit_mappings = CommitMappings(new_rebase.history.commit_mappings());
        self.inner.rebase = Some(new_rebase);
        Ok(outcome)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterializeWithoutCheckout {
    Yes,
    No,
    Either,
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

fn workspace_state_from_rebase<M: RefMetadata>(
    rebase: SuccessfulRebase<'_, '_, M>,
    repo: &gix::Repository,
    pending_metadata_removals: Vec<FullName>,
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<PendingCreatedIndependentRef>,
    dry_run: DryRun,
    materialize_without_checkout: bool,
) -> anyhow::Result<WorkspaceState> {
    if dry_run.into() {
        return WorkspaceState::from_successful_rebase(rebase, repo, dry_run);
    }

    let materialized = if materialize_without_checkout {
        rebase.materialize_without_checkout()?
    } else {
        rebase.materialize(Default::default())?
    };
    for branch in pending_created_independent_refs {
        if materialized
            .workspace
            .find_segment_and_stack_by_refname(branch.name.as_ref())
            .is_some()
        {
            continue;
        }
        let outcome = but_workspace::branch::apply(
            branch.name.as_ref(),
            materialized.workspace.clone(),
            repo,
            materialized.meta,
            but_workspace::branch::apply::Options {
                order: branch.order,
                ..Default::default()
            },
        )?;
        *materialized.workspace = outcome.workspace;
    }
    for update in pending_metadata_updates {
        match update {
            PendingMetadataUpdate::Workspace(workspace) => {
                let mut handle = materialized.meta.workspace(workspace.as_ref())?;
                *handle = workspace.value;
                materialized.meta.set_workspace(&handle)?;
            }
            PendingMetadataUpdate::Branch(branch) => {
                let mut handle = materialized.meta.branch(branch.as_ref())?;
                *handle = branch.value;
                materialized.meta.set_branch(&handle)?;
            }
            PendingMetadataUpdate::BranchStackOrder(branches) => {
                materialized.meta.set_branch_stack_order(&branches)?;
            }
        }
    }
    for ref_name in pending_metadata_removals {
        materialized.meta.remove(ref_name.as_ref())?;
    }

    WorkspaceState::from_materialized(materialized, repo)
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
