use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};

use bstr::BStr;
use but_api::WorkspaceState;
use but_core::{
    DiffSpec, DryRun, RefMetadata, ref_metadata, sync::RepoExclusive,
    tree::create_tree::RejectionReason,
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, Step, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::commit::{
    MoveChangesOutcome, SquashCommitsOutcome, squash_commits::MessageCombinationStrategy,
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
/// using our in-memory repositories and rebases.
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

        let db_tx = db.transaction()?;

        let editor = Editor::create(&mut ws, meta, &repo)?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            db_tx,
            commit_mappings: CommitMappings::default(),
            pending_metadata_removals: Vec::new(),
            pending_metadata_updates: Vec::new(),
            pending_created_independent_refs: Vec::new(),
            pending_ref_changes: PendingRefChanges::default(),
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
            db_tx,
            commit_mappings: _,
            pending_metadata_removals,
            pending_metadata_updates,
            pending_created_independent_refs,
            mut pending_ref_changes,
            context_lines: _,
        } = inner;
        let rebase = rebase.take().expect("rebase is always Some(_)");

        let should_rollback = callback_outcome.should_rollback();
        let outcome = callback_outcome.maybe_commit(
            &repo,
            rebase,
            db_tx,
            pending_metadata_removals,
            pending_metadata_updates,
            pending_created_independent_refs,
            dry_run,
        );

        let outcome = match outcome {
            Ok(outcome) => outcome,
            Err(err) => {
                pending_ref_changes.rollback(&repo)?;
                return Err(err);
            }
        };

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
    db_tx: but_db::Transaction<'rebase>,
    pending_metadata_removals: Vec<FullName>,
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<FullName>,
    pending_ref_changes: PendingRefChanges,
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
    ) -> anyhow::Result<ObjectId> {
        self.rebase(|editor, commit_mappings, _| {
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
            let new_commit = rebase.lookup_pick(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn reword_commit(&mut self, commit: ObjectId, message: &BStr) -> anyhow::Result<ObjectId> {
        self.rebase(|editor, commit_mappings, _| {
            let (rebase, edited_commit_selector) =
                but_workspace::commit::reword(editor, commit_mappings.map(commit), message)?;
            let new_commit = rebase.lookup_pick(edited_commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn discard_commits(
        &mut self,
        subjects: impl IntoIterator<Item = gix::ObjectId>,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, commit_mappings, _| {
            let rebase = but_workspace::commit::discard_commits(
                editor,
                subjects
                    .into_iter()
                    .map(|commit| commit_mappings.map(commit)),
            )?;
            Ok(((), rebase))
        })
    }

    pub fn remove_reference(&mut self, ref_name: &FullNameRef) -> anyhow::Result<()> {
        self.rebase(|mut editor, _, _| {
            let selector = editor.select_reference(ref_name)?;
            editor.replace(selector, but_rebase::graph_rebase::Step::None)?;
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

    pub fn create_reference<'name>(
        &mut self,
        ref_name: &FullNameRef,
        anchor: impl Into<Option<but_workspace::branch::create_reference::Anchor<'name>>>,
        new_stack_id: impl FnOnce(&FullNameRef) -> ref_metadata::StackId,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<()> {
        let anchor = anchor.into();
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
        let anchor_segment_oldest_commit_id = match &anchor {
            Some(but_workspace::branch::create_reference::Anchor::AtSegment {
                ref_name, ..
            }) => {
                let (_, segment) =
                    workspace.try_find_segment_and_stack_by_refname(ref_name.as_ref())?;
                Some(
                    segment
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
                        })?,
                )
            }
            _ => None,
        };
        let mut meta = RecordingMetadata {
            workspace_name: workspace.ref_name().map(ToOwned::to_owned),
            workspace: workspace.metadata.clone(),
            updates: Vec::new(),
        };

        but_workspace::branch::create_reference(
            ref_name,
            anchor.clone(),
            self.repo(),
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
                .push(ref_name.to_owned());
        }
        self.inner
            .pending_ref_changes
            .record_eager_create(ref_name, previous);

        self.rebase(|mut editor, _, _| {
            if editor.try_select_reference(ref_name).is_some() {
                return Ok(((), editor.rebase()?));
            }

            let target_id = editor
                .repo()
                .find_reference(ref_name)?
                .peel_to_id()?
                .detach();
            let reference = Step::Reference {
                refname: ref_name.to_owned(),
            };

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
            Ok(((), editor.rebase()?))
        })
    }

    pub fn create_commit(
        &mut self,
        relative_to: RelativeTo,
        side: InsertSide,
        changes: Vec<DiffSpec>,
        message: String,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings, _| {
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
            )?;

            let new_commit = commit_selector
                .map(|commit_selector| rebase.lookup_pick(commit_selector))
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
    ) -> anyhow::Result<gix::ObjectId> {
        self.rebase(|editor, commit_mappings, _| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => RelativeTo::Commit(commit_mappings.map(object_id)),
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let (rebase, blank_commit_selector) =
                but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;
            let new_commit = rebase.lookup_pick(blank_commit_selector)?;

            Ok((new_commit, rebase))
        })
    }

    pub fn amend_commit(
        &mut self,
        target: ObjectId,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings, _| {
            let but_workspace::commit::CommitAmendOutcome {
                rebase,
                commit_selector,
                rejected_specs,
            } = but_workspace::commit::commit_amend(
                editor,
                commit_mappings.map(target),
                changes,
                context_lines,
            )?;

            let new_commit = commit_selector
                .map(|commit_selector| rebase.lookup_pick(commit_selector))
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
    ) -> anyhow::Result<()> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings, _| {
            let source = commit_mappings.map(source);
            let target = commit_mappings.map(target);

            let MoveChangesOutcome { rebase, .. } =
                but_workspace::commit::move_changes_between_commits(
                    editor,
                    source,
                    target,
                    changes,
                    context_lines,
                )?;

            Ok(((), rebase))
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
            &mut but_db::Transaction<'rebase>,
        ) -> anyhow::Result<(T, SuccessfulRebase<'rebase, 'rebase, M>)>,
    {
        let editor = self
            .inner
            .rebase
            .take()
            .expect("rebase is always Some(_)")
            .into_editor();
        let (outcome, new_rebase) = f(editor, &self.inner.commit_mappings, &mut self.inner.db_tx)?;
        self.inner.commit_mappings = CommitMappings(new_rebase.history.commit_mappings());
        self.inner.rebase = Some(new_rebase);
        Ok(outcome)
    }
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

#[derive(Clone)]
enum PendingMetadataUpdate {
    Workspace(RecordingMetadataHandle<ref_metadata::Workspace>),
    Branch(RecordingMetadataHandle<ref_metadata::Branch>),
}

#[derive(Default)]
struct RecordingMetadata {
    workspace_name: Option<FullName>,
    workspace: Option<ref_metadata::Workspace>,
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

impl RefMetadata for RecordingMetadata {
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

    fn remove(&mut self, _ref_name: &FullNameRef) -> anyhow::Result<bool> {
        Ok(false)
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
}

pub trait TransactionOutcome: sealed::Sealed {
    type Outcome;

    fn should_rollback(&self) -> bool;

    #[expect(private_interfaces, clippy::too_many_arguments)]
    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        db_tx: but_db::Transaction<'_>,
        pending_metadata_removals: Vec<FullName>,
        pending_metadata_updates: Vec<PendingMetadataUpdate>,
        pending_created_independent_refs: Vec<FullName>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome>;
}

impl TransactionOutcome for () {
    type Outcome = WorkspaceState;

    fn should_rollback(&self) -> bool {
        false
    }

    #[expect(private_interfaces)]
    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        db_tx: but_db::Transaction<'_>,
        pending_metadata_removals: Vec<FullName>,
        pending_metadata_updates: Vec<PendingMetadataUpdate>,
        pending_created_independent_refs: Vec<FullName>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome> {
        let ws = workspace_state_from_rebase(
            rebase,
            repo,
            pending_metadata_removals,
            pending_metadata_updates,
            pending_created_independent_refs,
            dry_run,
        )?;
        if dry_run == DryRun::No {
            db_tx.commit()?;
        }
        Ok(ws)
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

    #[expect(private_interfaces)]
    fn maybe_commit<M: RefMetadata>(
        self,
        _repo: &gix::Repository,
        _rebase: SuccessfulRebase<'_, '_, M>,
        _db_tx: but_db::Transaction<'_>,
        _pending_metadata_removals: Vec<FullName>,
        _pending_metadata_updates: Vec<PendingMetadataUpdate>,
        _pending_created_independent_refs: Vec<FullName>,
        _dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome> {
        Ok(self.0)
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

    #[expect(private_interfaces)]
    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        db_tx: but_db::Transaction<'_>,
        pending_metadata_removals: Vec<FullName>,
        pending_metadata_updates: Vec<PendingMetadataUpdate>,
        pending_created_independent_refs: Vec<FullName>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome> {
        match self {
            DynamicOutcome::Commit(value) => {
                let workspace = workspace_state_from_rebase(
                    rebase,
                    repo,
                    pending_metadata_removals,
                    pending_metadata_updates,
                    pending_created_independent_refs,
                    dry_run,
                )?;
                if dry_run == DryRun::No {
                    db_tx.commit()?;
                }
                Ok(DynamicOutcome::Commit((value, workspace)))
            }
            DynamicOutcome::Rollback(value) => Ok(DynamicOutcome::Rollback(value)),
        }
    }
}

fn workspace_state_from_rebase<M: RefMetadata>(
    rebase: SuccessfulRebase<'_, '_, M>,
    repo: &gix::Repository,
    pending_metadata_removals: Vec<FullName>,
    pending_metadata_updates: Vec<PendingMetadataUpdate>,
    pending_created_independent_refs: Vec<FullName>,
    dry_run: DryRun,
) -> anyhow::Result<WorkspaceState> {
    if dry_run.into() {
        return WorkspaceState::from_successful_rebase(rebase, repo, dry_run);
    }

    let materialized = rebase.materialize()?;
    for branch in pending_created_independent_refs {
        if materialized
            .workspace
            .find_segment_and_stack_by_refname(branch.as_ref())
            .is_some()
        {
            continue;
        }
        let outcome = but_workspace::branch::apply(
            branch.as_ref(),
            materialized.workspace,
            repo,
            materialized.meta,
            Default::default(),
        )?;
        *materialized.workspace = outcome.workspace.into_owned();
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
        }
    }
    for ref_name in pending_metadata_removals {
        materialized.meta.remove(ref_name.as_ref())?;
    }

    WorkspaceState::from_materialized_rebase(materialized, repo)
}

/// Intermediate outcome after creating a commit.
///
/// It is intermediate in the sense that the commit hasn't been materialized yet and only exists
/// in-memory.
pub struct IntermediateCommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
}
