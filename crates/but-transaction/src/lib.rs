use std::collections::BTreeMap;

use bstr::BStr;
use but_api::WorkspaceState;
use but_core::{DiffSpec, DryRun, RefMetadata, tree::create_tree::RejectionReason};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_rebase::graph_rebase::{
    Editor, LookupStep as _, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};
use but_workspace::commit::{SquashCommitsOutcome, squash_commits::MessageCombinationStrategy};
use gix::{ObjectId, refs::FullNameRef};

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

    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        snapshot_details,
        perm.read_permission(),
        dry_run,
    );

    let context_lines = ctx.settings.context_lines;
    let (repo, mut ws, _db) = ctx.workspace_mut_and_db_with_perm(perm)?;

    let editor = Editor::create(&mut ws, meta, &repo)?;
    let rebase = editor.rebase()?;

    let mut inner = Inner {
        rebase: Some(rebase),
        commit_mappings: CommitMappings::default(),
        context_lines,
    };

    let callback_outcome = {
        let tx = Transaction { inner: &mut inner };
        f(tx)?
    };

    let rebase = inner.rebase.take().expect("rebase is always Some(_)");
    let should_rollback = callback_outcome.should_rollback();

    let outcome = callback_outcome.maybe_commit(&repo, rebase, dry_run)?;

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
            let new_commit = rebase.lookup_pick(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn reword_commit(&mut self, commit: ObjectId, message: &BStr) -> anyhow::Result<ObjectId> {
        self.rebase(|editor, commit_mappings| {
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

    pub fn remove_reference(&mut self, ref_name: &FullNameRef) -> anyhow::Result<()> {
        self.rebase(|mut editor, _| {
            let selector = editor.select_reference(ref_name)?;
            editor.replace(selector, but_rebase::graph_rebase::Step::None)?;
            let rebase = editor.rebase()?;
            Ok(((), rebase))
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

    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome>;
}

impl TransactionOutcome for () {
    type Outcome = WorkspaceState;

    fn should_rollback(&self) -> bool {
        false
    }

    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome> {
        WorkspaceState::from_successful_rebase(rebase, repo, dry_run)
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

    fn maybe_commit<M: RefMetadata>(
        self,
        _repo: &gix::Repository,
        _rebase: SuccessfulRebase<'_, '_, M>,
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

    fn maybe_commit<M: RefMetadata>(
        self,
        repo: &gix::Repository,
        rebase: SuccessfulRebase<'_, '_, M>,
        dry_run: DryRun,
    ) -> anyhow::Result<Self::Outcome> {
        match self {
            DynamicOutcome::Commit(value) => {
                let workspace = WorkspaceState::from_successful_rebase(rebase, repo, dry_run)?;
                Ok(DynamicOutcome::Commit((value, workspace)))
            }
            DynamicOutcome::Rollback(value) => Ok(DynamicOutcome::Rollback(value)),
        }
    }
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
