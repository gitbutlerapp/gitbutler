use std::collections::BTreeMap;

use anyhow::Context as _;
use bstr::BStr;
use but_core::{
    ChangeId, DiffSpec, DryRun, RefMetadata, commit::CommitIdentifiers, sync::RepoExclusive,
    tree::create_tree::RejectionReason,
};
use but_ctx::Context;
use but_oplog::legacy::SnapshotDetails;
use but_rebase::graph_rebase::{
    CommitMappings, Editor, GraphEditorOptions, Step, SuccessfulRebase,
    mutate::{InsertSide, RelativeTo},
};
#[cfg(not(feature = "graph-workspace"))]
use but_workspace::RefInfo;
use but_workspace::branch::create_reference::Anchor;
use but_workspace::commit::{
    ChangeSource, MoveChangesOutcome, SquashCommitsOutcome, UncommitChangesFailure,
    UncommitChangesFromCommitsOutcome, UncommitChangesSource,
    squash_commits::MessageCombinationStrategy,
};
use gix::{
    ObjectId,
    refs::{FullName, FullNameRef},
};

#[cfg(test)]
mod tests;

pub mod workspace_state;

/// This describes the post-operation workspace view that mutations should
/// return regardless of whether they executed for real or as a dry-run.
#[derive(Debug, Clone)]
pub struct WorkspaceState {
    /// Commits that were replaced by the operation. Maps `old_id -> new_id`.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
    pub conflicted_commits: Vec<(gix::ObjectId, ChangeId)>,
    /// The workspace presented for the frontend. See [`RefInfo`] for more
    /// detail.
    #[cfg(not(feature = "graph-workspace"))]
    pub head_info: RefInfo,
    /// The workspace presented for the frontend, as the rendered graph
    /// projection. See
    /// [`DetailedGraphWorkspace`](but_workspace::ui::workspace::DetailedGraphWorkspace)
    /// for more detail.
    #[cfg(feature = "graph-workspace")]
    pub graph_workspace: but_workspace::ui::workspace::DetailedGraphWorkspace,
    /// The checkouts whose uncommitted changes now carry conflict markers.
    pub conflicted_checkouts: Vec<but_rebase::graph_rebase::ConflictedCheckout>,
}

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
    with_transaction_options(
        ctx,
        meta,
        perm,
        TransactionOptions {
            snapshot_details: Some(snapshot_details),
            dry_run,
            ..Default::default()
        },
        f,
    )
}

/// Like [`with_transaction`] but allows the caller to provide the lock, and records
/// `snapshot_refs` as [`TransactionOptions::snapshot_refs`] does.
pub fn with_transaction_with_perm<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    perm: &mut RepoExclusive,
    snapshot_details: SnapshotDetails,
    snapshot_refs: &[&FullNameRef],
    dry_run: DryRun,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, M>) -> anyhow::Result<T>,
    M: RefMetadata,
    T: TransactionOutcome,
{
    with_transaction_options(
        ctx,
        meta,
        perm,
        TransactionOptions {
            snapshot_details: Some(snapshot_details),
            snapshot_refs,
            dry_run,
            ..Default::default()
        },
        f,
    )
}

/// How [`with_transaction_options()`] runs a transaction.
#[derive(Default)]
pub struct TransactionOptions<'a> {
    /// The oplog entry a committed transaction records, or `None` to record none.
    ///
    /// Recording it is best-effort: a transaction that landed succeeds even when its oplog entry
    /// can't be written, so it can't be undone.
    pub snapshot_details: Option<SnapshotDetails>,
    /// References the snapshot also preserves, so undo restores each or, if it did not exist
    /// yet, deletes it. See [`but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm`].
    pub snapshot_refs: &'a [&'a FullNameRef],
    pub dry_run: DryRun,
    /// References outside the workspace that the transaction may rewrite, along with what they
    /// reach, as it may what `HEAD` reaches. The snapshot preserves them as it does
    /// [`Self::snapshot_refs`].
    pub extra_mutable_refs: Vec<FullName>,
    /// References outside the workspace that the transaction sees but leaves as they are.
    pub extra_immutable_refs: Vec<FullName>,
}

/// Run a transaction as [`with_transaction`] does, under the caller's lock, as `options` say.
///
/// Operations only see the workspace and what `options` add to it. See
/// [`but_graph::init::Options::extra_refs`] for how those references join it.
pub fn with_transaction_options<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    perm: &mut RepoExclusive,
    options: TransactionOptions<'_>,
    f: F,
) -> anyhow::Result<T::Outcome>
where
    F: FnOnce(Transaction<'_, '_, M>) -> anyhow::Result<T>,
    M: RefMetadata,
    T: TransactionOutcome,
{
    let TransactionOptions {
        snapshot_details,
        snapshot_refs,
        dry_run,
        extra_mutable_refs,
        extra_immutable_refs,
    } = options;
    let maybe_oplog_entry = snapshot_details.and_then(|snapshot_details| {
        let snapshot_refs: Vec<_> = (snapshot_refs.iter().copied())
            .chain(extra_mutable_refs.iter().map(|name| name.as_ref()))
            .collect();
        but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
            ctx,
            snapshot_details,
            &snapshot_refs,
            perm.read_permission(),
            dry_run,
        )
    });

    let (should_rollback, outcome) = run_transaction(
        ctx,
        meta,
        perm,
        dry_run,
        extra_mutable_refs,
        extra_immutable_refs,
        f,
    )?;

    if !should_rollback
        && let Some(snapshot) = maybe_oplog_entry
        && let Err(err) = snapshot.commit(ctx, perm)
    {
        tracing::warn!(?err, "couldn't record the transaction in the oplog");
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
    run_transaction(ctx, meta, perm, dry_run, Vec::new(), Vec::new(), f)
}

fn run_transaction<M, F, T>(
    ctx: &mut Context,
    meta: &mut M,
    perm: &mut RepoExclusive,
    dry_run: DryRun,
    extra_mutable_refs: Vec<FullName>,
    extra_immutable_refs: Vec<FullName>,
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

        if !extra_mutable_refs.is_empty() || !extra_immutable_refs.is_empty() {
            ws.graph.options.extra_refs = (extra_mutable_refs.iter())
                .chain(&extra_immutable_refs)
                .cloned()
                .collect();
            let project_meta = ws.graph.project_meta.clone();
            ws.refresh_from_head(&repo, &*meta, project_meta, &mut db)?;
        }
        let editor = Editor::create_with_opts(
            &mut ws,
            meta,
            &repo,
            &mut db,
            &GraphEditorOptions {
                extra_mutable_refs,
                ..Default::default()
            },
        )?;
        let rebase = editor.rebase()?;

        let mut inner = Inner {
            rebase: Some(rebase),
            commit_mappings: CommitMappings::default(),
            context_lines,
        };

        let callback_outcome = f(Transaction { inner: &mut inner })?;
        let rebase = inner.rebase.take().expect("rebase is always Some(_)");

        let should_rollback = callback_outcome.should_rollback();
        // A rolled-back transaction never materializes, so it has no workspace to report.
        let workspace = if should_rollback {
            None
        } else {
            Some(WorkspaceState::from_successful_rebase(
                rebase, &repo, dry_run,
            )?)
        };
        let outcome = callback_outcome.into_outcome(workspace);

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
                    .map(|commit| commit_mappings.current(commit))
                    .collect::<anyhow::Result<_>>()?,
                commit_mappings.current(target)?,
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
                but_workspace::commit::reword(editor, commit_mappings.current(commit)?, message)?;
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
                    .map(|commit| commit_mappings.current(commit))
                    .collect::<anyhow::Result<Vec<_>>>()?,
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
                    .map(|commit| commit_mappings.current(commit))
                    .collect::<anyhow::Result<Vec<_>>>()?,
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
                commit_mappings.current(source)?,
                changes,
                context_lines,
            )?;

            let new_commit = rebase.lookup_commit(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    pub fn uncommit_changes_from_commit(
        &mut self,
        source: gix::ObjectId,
        changes: Vec<DiffSpec>,
    ) -> anyhow::Result<CommitIdentifiers> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let but_workspace::commit::UncommitChangesOutcome {
                mut rebase,
                commit_selector,
            } = but_workspace::commit::uncommit_changes(
                editor,
                commit_mappings.current(source)?,
                changes,
                context_lines,
            )?;
            rebase.keep_worktrees_unchanged()?;

            let new_commit = rebase.lookup_commit(commit_selector)?;
            Ok((new_commit, rebase))
        })
    }

    /// Uncommit what it can of `sources`, returning those it couldn't. See
    /// [`but_workspace::commit::uncommit_changes_from_commits()`].
    pub fn uncommit_changes_from_commits(
        &mut self,
        sources: impl IntoIterator<Item = UncommitChangesSource>,
    ) -> anyhow::Result<Vec<UncommitChangesFailure>> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let sources = sources
                .into_iter()
                .map(|source| {
                    anyhow::Ok(UncommitChangesSource {
                        commit_id: commit_mappings.current(source.commit_id)?,
                        changes: source.changes,
                    })
                })
                .collect::<anyhow::Result<Vec<_>>>()?;
            let UncommitChangesFromCommitsOutcome {
                mut rebase,
                failures,
            } = but_workspace::commit::uncommit_changes_from_commits(
                editor,
                sources,
                context_lines,
            )?;
            rebase.keep_worktrees_unchanged()?;
            Ok((failures, rebase))
        })
    }

    /// Check out `branch`, which operations after this call see as `HEAD`.
    ///
    /// Unless `HEAD` is on it already, the transaction fails without changing anything if that
    /// would overwrite uncommitted changes, or check out a conflicted commit. See
    /// [`but_workspace::editor_workspace::switch_to_branch()`].
    pub fn checkout(&mut self, branch: &FullNameRef) -> anyhow::Result<()> {
        anyhow::ensure!(
            branch.category() == Some(gix::refs::Category::LocalBranch),
            "Can only check out local branches under refs/heads, got '{}'",
            branch.as_bstr()
        );
        self.rebase(|editor, _| {
            let rebase = but_workspace::editor_workspace::switch_to_branch(editor, branch)?;
            Ok(((), rebase))
        })
    }

    /// Remove the local branch `ref_name` as [`but_workspace::branch::drop_reference()`] does.
    /// The branch, its metadata and its `branch.<name>` Git configuration are deleted when the
    /// transaction commits. A branch outside the workspace needs to be one of the transaction's
    /// [extra mutable refs](TransactionOptions::extra_mutable_refs).
    pub fn remove_reference(&mut self, ref_name: &FullNameRef) -> anyhow::Result<()> {
        self.rebase(|editor, _| {
            let rebase = but_workspace::branch::drop_reference(editor, ref_name)?;
            Ok(((), rebase))
        })
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
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };
            let rebase =
                but_workspace::branch::move_branch(editor, source_branch, relative_to, side)?;
            Ok(((), rebase))
        })
    }

    /// Take `source_branch` out of its stack into a stack of its own, placed among the others
    /// at `order`, or after them all, as [`but_workspace::branch::tear_off_branch()`] says.
    pub fn tear_off_branch(
        &mut self,
        source_branch: &FullNameRef,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<()> {
        self.rebase(|editor, _| {
            let rebase = but_workspace::branch::tear_off_branch(editor, source_branch, order)?;
            Ok(((), rebase))
        })
    }

    /// Create a new local branch at a commit already present in the editor.
    ///
    /// Unlike [`Transaction::create_reference`], this does not add the branch to workspace
    /// metadata or require it to appear in the current checkout's projection. It can be used
    /// with [`Transaction::checkout`] to prepare an independent branch before switching to it.
    /// Like [`Transaction::create_reference`], it refuses a name that's taken locally or by a
    /// remote's branch, or that nests with another branch's.
    pub fn create_reference_at_commit(
        &mut self,
        ref_name: &FullNameRef,
        commit_id: ObjectId,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            ref_name.category() == Some(gix::refs::Category::LocalBranch),
            "Can only create local branches under refs/heads"
        );
        self.rebase(|mut editor, commit_mappings| {
            but_workspace::branch::ensure_name_is_available(&editor, ref_name)?;
            let target = editor.select_commit(commit_mappings.current(commit_id)?)?;
            let reference = editor.add_step(Step::new_reference(ref_name.to_owned()))?;
            editor.add_edge(reference, target, 0)?;
            Ok(((), editor.rebase()?))
        })
    }

    /// Create `ref_name` at `anchor`, or as a new stack at `order` without one, as
    /// [`but_workspace::branch::create_reference()`] does.
    pub fn create_reference<'name>(
        &mut self,
        ref_name: &FullNameRef,
        anchor: impl Into<Option<Anchor<'name>>>,
        order: impl Into<Option<usize>>,
    ) -> anyhow::Result<()> {
        let anchor = anchor.into();
        let order = order.into();
        self.rebase(|editor, commit_mappings| {
            let anchor = anchor
                .map(|anchor| {
                    anyhow::Ok(match anchor {
                        Anchor::AtCommit {
                            commit_id,
                            position,
                        } => Anchor::AtCommit {
                            commit_id: commit_mappings.current(commit_id)?,
                            position,
                        },
                        anchor => anchor,
                    })
                })
                .transpose()?;
            let rebase = but_workspace::branch::create_reference(editor, ref_name, anchor, order)?;
            Ok(((), rebase))
        })
    }

    /// Create `ref_name`, or a unique canned name without one, as a branch of its own, first
    /// among the stacks, and return its name. With `switch`, it starts at the target and is
    /// checked out instead, unless the target is what's checked out.
    pub fn create_unstacked_reference(
        &mut self,
        ref_name: Option<FullName>,
        switch: bool,
    ) -> anyhow::Result<FullName> {
        let ref_name = match ref_name {
            Some(ref_name) => ref_name,
            None => but_core::branch::unique_canned_refname(self.repo())?,
        };
        if switch {
            let project_meta = but_core::ref_metadata::ProjectMeta::resolve(self.repo())?;
            let checked_out = self.repo().head_name()?;
            if let Some(target_ref) = &project_meta.target_ref
                && checked_out.as_ref().map(|name| name.as_ref())
                    != Some(target_ref.as_local_full_name_ref())
            {
                let target = project_meta.resolve_target_commit(self.repo())?;
                self.create_reference_at_commit(ref_name.as_ref(), target)?;
                self.checkout(ref_name.as_ref())?;
                return Ok(ref_name);
            }
        }
        self.create_reference(ref_name.as_ref(), None, Some(0))?;
        Ok(ref_name)
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
                commit_mappings.current(top)?,
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
        conflicts: but_workspace::commit::Conflicts,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.inner.context_lines;
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
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
                conflicts,
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
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
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
                .map(|commit| commit_mappings.current(commit))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
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
                .map(|commit| commit_mappings.current(commit))
                .collect::<anyhow::Result<Vec<_>>>()?;
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
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
        conflicts: but_workspace::commit::Conflicts,
    ) -> anyhow::Result<IntermediateCommitCreateResult> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings| {
            let but_workspace::commit::CommitAmendOutcome {
                rebase,
                commit_selector,
                rejected_specs,
            } = but_workspace::commit::commit_amend(
                editor,
                commit_mappings.current(target)?,
                changes,
                context_lines,
                source,
                conflicts,
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

    /// Split `changes` off `source` into a new commit placed on `side` of `relative_to`,
    /// returning the new commit.
    pub fn split_commit(
        &mut self,
        source: ObjectId,
        relative_to: RelativeTo,
        side: InsertSide,
        changes: Vec<but_core::DiffSpec>,
    ) -> anyhow::Result<CommitIdentifiers> {
        let context_lines = self.context_lines();
        self.rebase(|editor, commit_mappings| {
            let relative_to = match relative_to {
                RelativeTo::Commit(object_id) => {
                    RelativeTo::Commit(commit_mappings.current(object_id)?)
                }
                RelativeTo::Reference(full_name) => RelativeTo::Reference(full_name),
            };

            let MoveChangesOutcome {
                rebase,
                destination_selector,
                ..
            } = but_workspace::commit::split_commit(
                editor,
                commit_mappings.current(source)?,
                relative_to,
                side,
                changes,
                context_lines,
            )?;

            let new_commit = rebase
                .lookup_commit(destination_selector)
                .context("failed to find the split off commit")?;

            Ok((new_commit, rebase))
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
            let source = commit_mappings.current(source)?;
            let target = commit_mappings.current(target)?;

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

    /// The commit `original_commit` is now, after the rewrites made so far: itself if it
    /// wasn't rewritten. Fails if it was rewritten into several commits.
    ///
    /// In most cases this shouldn't be necessary. See [`with_transaction`] for more details.
    pub fn get_mapped_commit(&self, original_commit: ObjectId) -> anyhow::Result<ObjectId> {
        self.inner.commit_mappings.current(original_commit)
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

        self.inner.commit_mappings = new_rebase.history.commit_mappings();
        self.inner.rebase = Some(new_rebase);
        Ok(outcome)
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

/// Intermediate outcome after creating a commit.
///
/// It is intermediate in the sense that the commit hasn't been materialized yet and only exists
/// in-memory.
pub struct IntermediateCommitCreateResult {
    /// This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<CommitIdentifiers>,
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
}
