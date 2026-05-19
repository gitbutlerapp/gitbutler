use but_api_macros::but_api;
use but_core::{
    sync::{RepoExclusive, RepoShared},
    ui::TreeChange,
};
use but_ctx::Context;
use but_hunk_assignment::{HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use gix::prelude::ObjectIdExt;
use tracing::instrument;

boolean_enums::gen_boolean_enum!(pub serde ComputeLineStats);

use but_core::diff::CommitDetails;

/// JSON types
// TODO: add schemars
pub mod json {
    use but_core::diff::LineStats;
    use serde::Serialize;

    /// The JSON sibling of [but_core::diff::CommitDetails].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct CommitDetails {
        /// The commit itself.
        // TODO: make this our own json structure - this one is GUI specific and isn't great
        pub commit: but_workspace::ui::Commit,
        /// The changes
        pub changes: Vec<but_core::ui::TreeChange>,
        /// The stats of the changes.
        // TODO: adapt the frontend to be more specific as well.
        #[serde(rename = "stats")]
        pub line_stats: Option<LineStats>,
        /// Conflicting entries in `commit` as stored in the conflict commit metadata.
        pub conflict_entries: Option<but_core::commit::ConflictEntries>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(CommitDetails);

    impl From<but_core::diff::CommitDetails> for CommitDetails {
        fn from(value: but_core::diff::CommitDetails) -> Self {
            let but_core::diff::CommitDetails {
                commit,
                diff_with_first_parent,
                line_stats,
                conflict_entries,
            } = value;

            CommitDetails {
                commit: commit.into(),
                changes: diff_with_first_parent.into_iter().map(Into::into).collect(),
                line_stats,
                conflict_entries,
            }
        }
    }
}

/// Computes the tree diff for `commit_id` against its first parent and
/// optionally calculates `line_stats`.
///
/// For lower-level implementation details, see
/// [`but_core::diff::CommitDetails::from_commit_id()`].
#[but_api(json::CommitDetails)]
#[instrument(err(Debug))]
pub fn commit_details(
    ctx: &Context,
    commit_id: gix::ObjectId,
    line_stats: ComputeLineStats,
) -> anyhow::Result<CommitDetails> {
    let repo = ctx.repo.get()?;
    CommitDetails::from_commit_id(commit_id.attach(&repo), line_stats.into())
}

/// Computes commit details for `commit_id` with line statistics enabled.
///
/// This exists for callers that always want line statistics without passing
/// `line_stats` explicitly.
#[but_api(napi, json::CommitDetails)]
#[instrument(err(Debug))]
pub fn commit_details_with_line_stats(
    ctx: &Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<CommitDetails> {
    commit_details(ctx, commit_id, ComputeLineStats::Yes)
}

/// Produces a unified patch for `change`.
///
/// `change` must not be a type change or a submodule change. For lower-level
/// implementation details, see [`but_core::TreeChange::unified_patch()`].
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn tree_change_diffs(
    ctx: &Context,
    change: TreeChange,
) -> anyhow::Result<Option<but_core::UnifiedPatch>> {
    let change: but_core::TreeChange = change.into();
    let repo = ctx.repo.get()?;
    change.unified_patch(&repo, ctx.settings.context_lines)
}

/// See [`changes_in_worktree_with_perm()`].
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn changes_in_worktree(ctx: &Context) -> anyhow::Result<WorktreeChanges> {
    let guard = ctx.shared_worktree_access();
    changes_in_worktree_with_perm(ctx, guard.read_permission())
}

/// This UI-version of [`but_core::diff::worktree_changes()`] simplifies the `git status` information for display in
/// the user interface as it is right now. From here, it's always possible to add more information as the need arises.
///
/// ### Notable Transformations
/// * There is no notion of an index (`.git/index`) - all changes seem to have happened in the worktree.
/// * Modifications that were made to the index will be ignored *only if* there is a worktree modification to the same file.
/// * conflicts are ignored
///
/// All ignored status changes are also provided so they can be displayed separately.
///
/// For lower-level implementation details, see
/// [`but_core::diff::worktree_changes()`],
/// [`but_hunk_assignment::assignments_with_fallback()`], and
/// [`but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir()`].
#[but_api(napi)]
#[instrument(skip_all, err(Debug))]
pub fn changes_in_worktree_with_perm(
    ctx: &Context,
    perm: &RepoShared,
) -> anyhow::Result<WorktreeChanges> {
    let context_lines = ctx.settings.context_lines;

    let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(perm)?;

    let changes = but_core::diff::worktree_changes(&repo)?;

    let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        &repo,
        &ws,
        Some(changes.changes.clone()),
    );
    let mut trans = db.immediate_transaction()?;

    let (assignments, assignments_error) = {
        but_hunk_assignment::assignments_with_fallback(
            trans.hunk_assignments_mut()?,
            &repo,
            &ws,
            Some(changes.changes.clone()),
            context_lines,
        )?
    };

    trans.commit()?;
    drop((repo, ws, db));

    Ok(WorktreeChanges {
        worktree_changes: changes.into(),
        assignments,
        assignments_error: assignments_error.map(|err| serde_error::Error::new(&*err)),
        dependencies: dependencies.as_ref().ok().cloned(),
        dependencies_error: dependencies
            .as_ref()
            .err()
            .map(|err| serde_error::Error::new(&**err)),
    })
}

/// Persists `assignments` for the current workspace without creating an oplog
/// entry.
///
/// This acquires exclusive worktree access from `ctx` before writing
/// assignments.
///
/// See [`assign_hunk_only_with_perm()`] for details.
#[but_api]
#[instrument(skip_all, err(Debug))]
pub fn assign_hunk_only(
    ctx: &mut Context,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    assign_hunk_only_with_perm(ctx, assignments, guard.write_permission())
}

/// Persists `assignments` under caller-held exclusive repository access without
/// creating an oplog entry.
///
/// For lower-level implementation details, see
/// [`but_hunk_assignment::assign()`].
pub fn assign_hunk_only_with_perm(
    ctx: &mut Context,
    assignments: Vec<HunkAssignmentRequest>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    but_hunk_assignment::assign(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        assignments,
        context_lines,
    )?;
    Ok(())
}

/// Persists `assignments` for the current workspace and records an oplog
/// snapshot on success.
///
/// This acquires exclusive worktree access from `ctx` before writing
/// assignments.
///
/// See [`assign_hunk_with_perm()`] for details.
#[but_api(napi)]
#[instrument(skip_all, err(Debug))]
pub fn assign_hunk(
    ctx: &mut Context,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    assign_hunk_with_perm(ctx, assignments, guard.write_permission())
}

/// Persists `assignments` under caller-held exclusive repository access and
/// records an oplog snapshot on success.
///
/// It behaves like [`assign_hunk_only_with_perm()`], but first prepares a
/// best-effort `MoveHunk` oplog snapshot and commits the snapshot only if the
/// assignment succeeds.
pub fn assign_hunk_with_perm(
    ctx: &mut Context,
    assignments: Vec<HunkAssignmentRequest>,
    perm: &mut RepoExclusive,
) -> anyhow::Result<()> {
    // this oplog entry is currently a noop (i.e. restoring it does nothing) but we do wanna
    // support it in the future so leaving it here for consistency
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MoveHunk),
        perm.read_permission(),
        but_core::DryRun::No,
    );

    let res = assign_hunk_only_with_perm(ctx, assignments, perm);
    if let Some(snapshot) = maybe_oplog_entry
        && res.is_ok()
    {
        snapshot.commit(ctx, perm).ok();
    }
    res
}
