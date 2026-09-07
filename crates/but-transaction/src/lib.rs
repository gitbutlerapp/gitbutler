use std::collections::BTreeMap;

use anyhow::Context as _;
use bstr::{BStr, BString, ByteSlice as _, ByteVec};
use but_api::WorkspaceState;
use but_core::{
    DiffSpec, DryRun,
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
use but_workspace::commit::{
    ChangeSource, MoveChangesOutcome, SquashCommitsOutcome,
    squash_commits::MessageCombinationStrategy,
};
use gix::{
    ObjectId,
    prelude::ObjectIdExt as _,
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
/// Database changes share one SQLite transaction and become visible only on commit.
/// Completed Git reference and checkout changes are restored on late failures, including a
/// rejected database commit. Recovery refuses to overwrite detected independent ref/index
/// edits and reports failures to safely restore files. Native partial I/O failures remain
/// best-effort; this is not a cross-resource or crash-atomic transaction.
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
pub fn with_transaction<F, T>(
    ctx: &mut Context,
    snapshot_details: SnapshotDetails,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, '_>) -> anyhow::Result<T>,
    T: TransactionOutcome,
{
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    with_transaction_with_perm(ctx, perm, snapshot_details, dry_run, f)
}

/// Like [`with_transaction`] but allows the caller to provide the lock.
pub fn with_transaction_with_perm<F, T>(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    snapshot_details: SnapshotDetails,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, '_>) -> anyhow::Result<T>,
    T: TransactionOutcome,
{
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        snapshot_details,
        perm.read_permission(),
        dry_run,
    );

    let (should_rollback, outcome) = with_transaction_with_perm_only(ctx, perm, dry_run, f)?;

    if !should_rollback && let Some(snapshot) = maybe_oplog_entry {
        snapshot.commit(ctx, perm)?;
    }

    Ok(outcome)
}

pub fn with_transaction_with_perm_only<F, T>(
    ctx: &mut Context,
    perm: &mut RepoExclusive,
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<(bool, T::Outcome)>
where
    F: FnOnce(Transaction<'_, '_, '_>) -> anyhow::Result<T>,
    T: TransactionOutcome,
{
    let result = (|| {
        let context_lines = ctx.settings.context_lines;
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
        let mut sql_transaction = db.immediate_transaction()?;
        let worktree_names = ws
            .graph
            .worktree_tips
            .iter()
            .map(|tip| tip.name.clone())
            .collect::<Vec<_>>();
        let editor = Editor::create(&mut ws, &repo, sql_transaction.connection_mut())?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            commit_mappings: CommitMappings::default(),
            pending_created_independent_refs: Vec::new(),
            pending_ref_changes: PendingRefChanges::default(),
            pending_checkout: None,
            context_lines,
            materialize_without_checkout: MaterializeWithoutCheckout::Either,
        };
        let callback_outcome = match f(Transaction { inner: &mut inner }) {
            Ok(outcome) => outcome,
            Err(err) => {
                return Err(inner.pending_ref_changes.rollback_error(&repo, err));
            }
        };
        let Inner {
            mut rebase,
            commit_mappings: _,
            pending_created_independent_refs,
            mut pending_ref_changes,
            pending_checkout,
            context_lines: _,
            materialize_without_checkout,
        } = inner;
        let rebase = rebase.take().expect("rebase is always Some(_)");
        let should_rollback = callback_outcome.should_rollback();
        let workspace = if should_rollback {
            drop(rebase);
            Ok(None)
        } else {
            (|| {
                if matches!(dry_run, DryRun::No) {
                    pending_ref_changes.capture_checkouts(&repo, &worktree_names)?;
                }
                let workspace = workspace_state_from_rebase(
                    rebase,
                    &repo,
                    pending_created_independent_refs,
                    FinalizeOptions {
                        checkout: pending_checkout,
                        dry_run,
                        materialize_without_checkout: matches!(
                            materialize_without_checkout,
                            MaterializeWithoutCheckout::Yes
                        ),
                    },
                    &mut pending_ref_changes.committed,
                    &mut pending_ref_changes.checkouts,
                )
                .map(Some);
                let heads = pending_ref_changes.record_materialized_heads();
                match (workspace, heads) {
                    (Err(err), Err(head_error)) => Err(err.context(format!(
                        "Could not record materialized HEAD: {head_error:#}"
                    ))),
                    (Err(err), _) | (_, Err(err)) => Err(err),
                    (Ok(workspace), Ok(())) => Ok(workspace),
                }
            })()
        };
        let workspace = match workspace {
            Ok(workspace) => workspace,
            Err(err) => {
                return Err(pending_ref_changes.rollback_error(&repo, err));
            }
        };
        if should_rollback || dry_run.into() {
            pending_ref_changes.rollback(&repo)?;
            sql_transaction.rollback()?;
        } else if let Err(err) = sql_transaction.commit() {
            return Err(pending_ref_changes.rollback_error(&repo, err.into()));
        }
        Ok((should_rollback, callback_outcome.into_outcome(workspace)))
    })();
    // Release the editor/database borrows before discarding a projection of rolled-back state.
    if (result.is_err() || dry_run.into() || matches!(&result, Ok((true, _))))
        && let Err(cache_error) = ctx.invalidate_workspace_cache()
    {
        return match result {
            Err(err) => Err(err.context(format!(
                "Could not invalidate workspace cache: {cache_error:#}"
            ))),
            Ok(_) => Err(cache_error),
        };
    }
    result
}

/// A workspace transaction that allows changing multiple operations and having them all succeed or
/// fail together.
///
/// See [`with_transaction`] for more details.
pub struct Transaction<'inner, 'rebase, 'conn> {
    // Store a mutable reference so the callback for `with_transaction` can get an owned
    // `Transaction`. It needs to be owned to verify statically that `Transaction::rollback` is
    // only called once.
    inner: &'inner mut Inner<'rebase, 'conn>,
}

struct Inner<'rebase, 'conn> {
    // an Option so we can "take" the rebase, convert it into an editor, perform another rebase,
    // and put the result back.
    rebase: Option<SuccessfulRebase<'rebase, 'rebase, 'conn>>,
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

impl<'rebase, 'conn> Transaction<'_, 'rebase, 'conn> {
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
                                if refname == target_local_branch ||
                                    refname == "refs/heads/gitbutler/target")
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
            .rebase
            .as_mut()
            .expect("rebase is always Some(_)")
            .repo_and_db_mut()
            .1
            .meta_mut()?
            .remove(ref_name)?;
        Ok(())
    }

    /// Restack `source_branch` on top of `target_branch` within the transaction's workspace.
    ///
    /// Branch moves inside transactions currently require a managed workspace. The ad-hoc path
    /// returns [`Outcome::new_tip`] and [`Outcome::branch_stack_order`] for its caller to apply.
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

        self.set_workspace_metadata(ws_meta)?;

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

        self.set_workspace_metadata(ws_meta)?;

        Ok(())
    }

    fn set_workspace_metadata(
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

        let (_, db) = self
            .inner
            .rebase
            .as_mut()
            .expect("rebase is always Some(_)")
            .repo_and_db_mut();
        db.meta_mut()?.set_workspace(ref_name.as_ref(), &ws_meta)?;

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
                position: but_workspace::branch::create_reference::Position::Below,
            }) => self.rebase(|editor, _| {
                // Metadata ordering can make a projected segment empty before the editor's
                // topology changes. Resolve its boundary from the steps we will actually edit.
                let mut cursor = editor.select_reference(ref_name.as_ref())?;
                let target = editor.target_selector();
                let mut oldest_commit_id = None;
                while let Some((parent, _)) = editor
                    .direct_parents(cursor)?
                    .into_iter()
                    .min_by_key(|(_, order)| *order)
                {
                    if Some(parent) == target {
                        break;
                    }
                    match editor.lookup_step(parent)? {
                        Step::Pick(pick) => oldest_commit_id = Some(pick.id),
                        Step::Reference { .. } => break,
                        Step::None => {}
                    }
                    cursor = parent;
                }
                use but_workspace::branch::create_reference::{Anchor, Position};
                let anchor = if oldest_commit_id.is_some() {
                    Anchor::AtSegment {
                        ref_name,
                        position: Position::Below,
                    }
                } else {
                    Anchor::AtReference {
                        ref_name,
                        position: Position::Below,
                    }
                };
                Ok((
                    (Some(anchor), oldest_commit_id),
                    MaterializeWithoutCheckout::Either,
                    editor.rebase()?,
                ))
            })?,
            anchor => (anchor, None),
        };
        let repo = self.repo().clone();
        let rebase = self
            .inner
            .rebase
            .as_mut()
            .expect("rebase is always Some(_)");
        let (_, db) = rebase.repo_and_db_mut();
        // Dependent anchors need normalized stack membership. Independent branches are applied
        // after materialization, preserving parent order while initializing workspace metadata.
        if !creates_independent_branch
            && let Some(workspace_meta) = workspace.metadata_from_projection()?
        {
            db.meta_mut()?.set_workspace(
                workspace
                    .ref_name()
                    .context("managed workspace has a ref")?,
                &workspace_meta,
            )?;
        }
        but_workspace::branch::create_reference_with_ref_edits(
            ref_name,
            anchor.clone(),
            &repo,
            &workspace,
            db,
            new_stack_id,
            order,
            &mut self.inner.pending_ref_changes.committed,
        )?;
        if creates_independent_branch {
            self.inner
                .pending_created_independent_refs
                .push(PendingCreatedIndependentRef {
                    name: ref_name.to_owned(),
                    order,
                });
        }

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
            Editor<'rebase, 'rebase, 'conn>,
            &CommitMappings,
        ) -> anyhow::Result<(
            T,
            MaterializeWithoutCheckout,
            SuccessfulRebase<'rebase, 'rebase, 'conn>,
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

        self.request_materialization(materialize_without_checkout)?;

        self.inner.commit_mappings = CommitMappings(new_rebase.history.commit_mappings());
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
    committed: Vec<RefEdit>,
    checkouts: Vec<CheckoutSnapshot>,
}

impl PendingRefChanges {
    fn remove_eagerly_created_ref(
        &mut self,
        repo: &gix::Repository,
        ref_name: &FullNameRef,
    ) -> anyhow::Result<()> {
        let Some((None, Some(target))) = self.ref_changes().remove(ref_name) else {
            return Ok(());
        };
        self.committed.extend(repo.edit_reference(RefEdit::delete(
            ref_name.to_owned(),
            PreviousValue::MustExistAndMatch(target),
        ))?);
        Ok(())
    }

    fn ref_changes(&self) -> BTreeMap<FullName, (Option<Target>, Option<Target>)> {
        use gix::refs::transaction::{Change, RefLog};
        let mut changes = BTreeMap::new();
        for edit in &self.committed {
            let expected = match &edit.change {
                Change::Update { log, .. } if log.mode == RefLog::Only => continue,
                Change::Delete {
                    log: RefLog::Only, ..
                } => continue,
                Change::Update { expected, .. } | Change::Delete { expected, .. } => expected,
            };
            // Native transactions replace this constraint only if the ref actually existed.
            // ExistingMustMatch can remain on a successful creation of an absent reference.
            let previous = match expected {
                PreviousValue::MustExistAndMatch(target) => Some(target.clone()),
                _ => None,
            };
            let new = edit.change.new_value().map(Into::into);
            changes
                .entry(edit.name.clone())
                .and_modify(|(original, current)| {
                    // An independently changed ref breaks our chain. Only undo the newest
                    // continuous suffix, retaining the intervening writer's value.
                    if *current != previous {
                        *original = previous.clone();
                    }
                    *current = new.clone();
                })
                .or_insert((previous, new));
        }
        changes
    }

    fn capture_checkouts(
        &mut self,
        repo: &gix::Repository,
        worktree_names: &[BString],
    ) -> anyhow::Result<()> {
        self.checkouts
            .push(CheckoutSnapshot::capture(repo.clone())?);
        if !worktree_names.is_empty() {
            let proxies = repo.worktrees()?;
            for name in worktree_names {
                let proxy = proxies
                    .iter()
                    .find(|proxy| proxy.id() == name.as_bstr())
                    .with_context(|| format!("Visible worktree {name} no longer exists"))?;
                self.checkouts
                    .push(CheckoutSnapshot::capture(proxy.clone().into_repo()?)?);
            }
        }
        Ok(())
    }

    fn record_materialized_heads(&mut self) -> anyhow::Result<()> {
        for checkout in &mut self.checkouts {
            if checkout.target.is_some() {
                checkout.materialized_head = Some((
                    checkout.repo.head_name()?,
                    checkout.repo.head()?.id().map(|id| id.detach()),
                ));
            }
        }
        Ok(())
    }

    fn rollback(&mut self, repo: &gix::Repository) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        // Each completed checkout recorded its target before refs could fail. Use that
        // known base to retain independently added worktree changes during restoration.
        for checkout in self.checkouts.drain(..).rev() {
            if let Err(err) = checkout.restore() {
                errors.push(format!("{err:#}"));
            }
        }
        for (name, (previous, current)) in self.ref_changes() {
            if previous == current {
                continue;
            }
            let expected = match current {
                Some(target) => PreviousValue::MustExistAndMatch(target),
                None => PreviousValue::MustNotExist,
            };
            let edit = match previous {
                Some(target) => {
                    RefEdit::update(name.clone(), target, expected, "rollback transaction")
                }
                None => RefEdit::delete(name.clone(), expected),
            };
            if let Err(err) = repo.edit_reference(edit) {
                errors.push(format!("Could not restore {name}: {err}"));
            }
        }
        self.committed.clear();
        anyhow::ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }

    fn rollback_error(&mut self, repo: &gix::Repository, err: anyhow::Error) -> anyhow::Error {
        match self.rollback(repo) {
            Ok(()) => err,
            Err(recovery) => err.context(format!("Git transaction recovery failed: {recovery:#}")),
        }
    }
}

#[derive(Debug)]
struct CheckoutSnapshot {
    repo: gix::Repository,
    worktree: ObjectId,
    index: Option<Vec<u8>>,
    target: Option<ObjectId>,
    materialized_index: Option<Option<Vec<u8>>>,
    materialized_head: Option<(Option<FullName>, Option<ObjectId>)>,
}

impl CheckoutSnapshot {
    fn capture(repo: gix::Repository) -> anyhow::Result<Self> {
        let index = read_index_bytes(&repo)?;
        let head_tree = repo.head_tree_id_or_empty()?.detach();
        let changes = but_core::diff::worktree_changes_no_renames(&repo)?;
        let mut selection = changes
            .changes
            .iter()
            .map(|change| change.path.clone())
            .collect::<std::collections::BTreeSet<_>>();
        selection.extend(
            changes
                .index_changes
                .iter()
                .map(|change| change.location().to_owned()),
        );
        selection.extend(changes.index_conflicts.iter().map(|(path, _)| path.clone()));
        let snapshot = but_core::snapshot::create_tree(
            head_tree.attach(&repo),
            but_core::snapshot::create_tree::State {
                changes,
                selection,
                head: false,
            },
        )?;
        Ok(Self {
            repo,
            worktree: snapshot.worktree.unwrap_or(head_tree),
            index,
            target: None,
            materialized_index: None,
            materialized_head: None,
        })
    }

    fn record_checkout(&mut self, target: ObjectId) -> anyhow::Result<()> {
        self.target = Some(target);
        self.materialized_index = None;
        self.materialized_index = Some(read_index_bytes(&self.repo)?);
        Ok(())
    }

    fn restore(self) -> anyhow::Result<()> {
        use std::io::Write;
        let Some(target) = self.target else {
            return Ok(());
        };
        let materialized_index = self
            .materialized_index
            .context("Could not capture the completed checkout index for safe recovery")?;
        let materialized_head = self
            .materialized_head
            .context("Could not capture HEAD after materialization for safe recovery")?;
        anyhow::ensure!(
            (
                self.repo.head_name()?,
                self.repo.head()?.id().map(|id| id.detach())
            ) == materialized_head,
            "HEAD changed independently in {}; leaving its worktree intact",
            self.repo.git_dir().display()
        );
        let acquire_index = || {
            gix::lock::File::acquire_to_update_resource(
                self.repo.index_path(),
                gix::lock::acquire::Fail::Immediately,
                None,
            )
        };
        let index_lock = acquire_index()?;
        anyhow::ensure!(
            read_index_bytes(&self.repo)? == materialized_index,
            "Index changed independently in {}; leaving its staging intact",
            self.repo.git_dir().display()
        );
        drop(index_lock);
        safe_checkout_from_head(
            self.worktree,
            &self.repo,
            checkout::Options {
                skip_head_update: true,
                merge_base_override: Some(target),
                ..Default::default()
            },
        )
        .with_context(|| {
            format!(
                "Could not restore worktree {}",
                self.repo.git_dir().display()
            )
        })?;
        // Checkout has its own index locking. Compare its output again under the lock used
        // for restoring the original bytes, including staging, stat data, and extensions.
        let checked_out_index = read_index_bytes(&self.repo)?;
        let mut index_lock = acquire_index()?;
        anyhow::ensure!(
            read_index_bytes(&self.repo)? == checked_out_index,
            "Index changed independently during recovery in {}",
            self.repo.git_dir().display()
        );
        if let Some(index) = self.index {
            index_lock.write_all(&index)?;
            index_lock.commit()?;
        } else if checked_out_index.is_some() {
            std::fs::remove_file(self.repo.index_path())?;
        }
        Ok(())
    }
}

fn read_index_bytes(repo: &gix::Repository) -> anyhow::Result<Option<Vec<u8>>> {
    match std::fs::read(repo.index_path()) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[derive(Debug)]
struct PendingCreatedIndependentRef {
    name: FullName,
    order: Option<usize>,
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

fn workspace_state_from_rebase(
    rebase: SuccessfulRebase<'_, '_, '_>,
    repo: &gix::Repository,
    pending_created_independent_refs: Vec<PendingCreatedIndependentRef>,
    options: FinalizeOptions,
    committed_ref_edits: &mut Vec<RefEdit>,
    checkouts: &mut [CheckoutSnapshot],
) -> anyhow::Result<WorkspaceState> {
    let FinalizeOptions {
        checkout: pending_checkout,
        dry_run,
        materialize_without_checkout,
    } = options;
    if dry_run.into() {
        let Some(branch) = pending_checkout else {
            return WorkspaceState::from_successful_rebase(rebase, repo, dry_run);
        };
        let target = rebase
            .reference_target(branch.as_ref())
            .or_else(|_| resolve_checkout_target(rebase.repo(), branch.as_ref()))?;
        let replaced_commits = rebase.history.commit_mappings();
        let workspace = rebase
            .overlayed_graph_with_workspace_overrides(Some((target, branch)), None)?
            .into_workspace()?;
        let mut rebase = rebase;
        let (repo, db) = rebase.repo_and_db_mut();
        return WorkspaceState::from_workspace_with_db(
            &workspace,
            repo,
            replaced_commits,
            db.reborrow(),
        );
    }

    let mut on_checkout = |repo: &gix::Repository, target| {
        checkouts
            .iter_mut()
            .find(|checkout| checkout.repo.index_path() == repo.index_path())
            .context("BUG: each materialized worktree has a recovery snapshot")?
            .record_checkout(target)
    };
    let mut materialized = rebase.materialize_with_changes(
        but_rebase::graph_rebase::materialize::MaterializeOptions {
            without_checkout: materialize_without_checkout,
        },
        committed_ref_edits,
        &mut on_checkout,
    )?;
    for branch in pending_created_independent_refs {
        if materialized
            .workspace
            .find_segment_and_stack_by_refname(branch.name.as_ref())
            .is_some()
        {
            continue;
        }
        let outcome = but_workspace::branch::apply_with_changes(
            branch.name.as_ref(),
            materialized.workspace.clone(),
            repo,
            &mut materialized.db,
            but_workspace::branch::apply::Options {
                order: branch.order,
                ..Default::default()
            },
            committed_ref_edits,
            &mut on_checkout,
        )?;
        *materialized.workspace = outcome.workspace;
    }
    if let Some(branch) = pending_checkout {
        checkout_reference(repo, branch.as_ref(), committed_ref_edits, &mut on_checkout)?;
        let project_meta = materialized.workspace.graph.project_meta.clone();
        materialized
            .workspace
            .refresh_from_head(repo, project_meta, &mut materialized.db)?;
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

fn checkout_reference(
    repo: &gix::Repository,
    reference_name: &FullNameRef,
    committed_ref_edits: &mut Vec<RefEdit>,
    on_checkout: &mut impl FnMut(&gix::Repository, ObjectId) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
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
    on_checkout(repo, target)?;
    committed_ref_edits.extend(
        update_head_reference(
            repo,
            Target::Symbolic(reference_name.to_owned()),
            false,
            "checkout",
            reference_name.as_bstr(),
            target_commit.parent_ids().count(),
        )
        .with_context(|| format!("Could not update HEAD to '{}'", reference_name.as_bstr()))?,
    );
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
