use std::collections::HashSet;

use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_core::{DiffSpec, sync::RepoExclusive, tree::create_tree::RejectionReason};
use but_hunk_assignment::HunkAssignmentRequest;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{
    GraphExt, LookupStep as _,
    mutate::{InsertSide, RelativeTo},
};
use tracing::instrument;

use crate::json;

/// Outcome after creating a commit
pub struct CommitCreateResult {
    /// If the commit was successfully created. This should only be none if all the DiffSpecs were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Any specs that failed to be committed.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
}

/// Outcome after moving changes between commits.
pub struct MoveChangesResult {
    /// Commits that have been mapped from one thing to another.
    pub replaced_commits: Vec<(gix::ObjectId, gix::ObjectId)>,
}

/// Rewords a commit
///
/// Returns the ID of the newly renamed commit
#[but_api]
#[instrument(err(Debug))]
pub fn commit_reword_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<gix::ObjectId> {
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let editor = ws.graph.to_editor(&repo)?;

    let (outcome, edited_commit_selector) =
        but_workspace::commit::reword(editor, commit_id, message.as_bstr())?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(edited_commit_selector)?;

    Ok(id)
}

/// Rewords a commit, but without updating the oplog.
///
/// Returns the ID of the newly renamed commit
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn commit_reword(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<gix::ObjectId> {
    // NOTE: since this is optional by nature, the same would be true if snapshotting/undo would be disabled via `ctx` app settings, for instance.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::UpdateCommitMessage),
    )
    .ok();

    let res = commit_reword_only(ctx, commit_id, message);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}

/// UI types for commit insertion
pub mod ui {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase", tag = "type", content = "subject")]
    /// Specifies a location, usually used to either have something inserted
    /// relative to it, or for the selected object to actually be replaced.
    pub enum RelativeTo {
        /// Relative to a commit
        #[serde(with = "but_serde::object_id")]
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        Commit(gix::ObjectId),
        /// Relative to a reference
        #[serde(with = "but_serde::fullname_lossy")]
        #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
        Reference(gix::refs::FullName),
    }

    impl From<but_rebase::graph_rebase::mutate::RelativeTo<'_>> for RelativeTo {
        fn from(value: but_rebase::graph_rebase::mutate::RelativeTo) -> Self {
            match value {
                but_rebase::graph_rebase::mutate::RelativeTo::Commit(c) => Self::Commit(c),
                but_rebase::graph_rebase::mutate::RelativeTo::Reference(r) => {
                    Self::Reference(r.into())
                }
            }
        }
    }

    impl<'a> From<&'a RelativeTo> for but_rebase::graph_rebase::mutate::RelativeTo<'a> {
        fn from(value: &'a RelativeTo) -> Self {
            match value {
                RelativeTo::Commit(c) => Self::Commit(*c),
                RelativeTo::Reference(r) => Self::Reference(r.as_ref()),
            }
        }
    }
}

/// Inserts a blank commit relative to either a commit or a reference
///
/// Returns the ID of the newly created blank commit
#[but_api(json::HexHash)]
#[instrument(err(Debug))]
pub fn commit_insert_blank_only(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<gix::ObjectId> {
    let mut guard = ctx.exclusive_worktree_access();
    commit_insert_blank_only_impl(ctx, relative_to, side, guard.write_permission())
}

/// Implementation of inserting a blank commit relative to either a commit or a reference
///
/// Returns the ID of the newly created blank commit
pub(crate) fn commit_insert_blank_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
    perm: &mut RepoExclusive,
) -> anyhow::Result<gix::ObjectId> {
    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = ws.graph.to_editor(&repo)?;

    let relative_to: RelativeTo = (&relative_to).into();

    let (outcome, blank_commit_selector) =
        but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(blank_commit_selector)?;

    // Play it safe and refresh the workspace - who knows how the context is used after this.
    ws.refresh_from_head(&repo, &meta)?;

    Ok(id)
}

/// Inserts a blank commit relative to either a commit or a reference, with oplog support
///
/// Returns the ID of the newly created blank commit
#[but_api(napi, json::HexHash)]
#[instrument(err(Debug))]
pub fn commit_insert_blank(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<gix::ObjectId> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_insert_blank_only_impl(ctx, relative_to, side, guard.write_permission());
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}

/// Creates and inserts a commit relative to either a commit or a reference.
#[but_api(json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create_only(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        context_lines,
        guard.write_permission(),
    )
}

/// Creates and inserts a commit relative to either a commit or a reference.
pub(crate) fn commit_create_only_impl(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = ws.graph.to_editor(&repo)?;
    let relative_to: RelativeTo = (&relative_to).into();

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

    let new_commit = match commit_selector {
        Some(commit_selector) => {
            let materialized = rebase.materialize()?;
            Some(materialized.lookup_pick(commit_selector)?)
        }
        None => None,
    };

    ws.refresh_from_head(&repo, &meta)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
    })
}

/// Creates and inserts a commit relative to either a commit or a reference, with oplog support.
#[but_api(napi, json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_create(
    ctx: &mut but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
    changes: Vec<DiffSpec>,
    message: String,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::CreateCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_create_only_impl(
        ctx,
        relative_to,
        side,
        changes,
        message,
        context_lines,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}

/// Amends an existing commit with selected changes.
#[but_api(json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let mut guard = ctx.exclusive_worktree_access();
    commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        context_lines,
        guard.write_permission(),
    )
}

/// Amends an existing commit with selected changes.
pub(crate) fn commit_amend_only_impl(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
    context_lines: u32,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CommitCreateResult> {
    let meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(perm)?;
    let editor = ws.graph.to_editor(&repo)?;

    let but_workspace::commit::CommitAmendOutcome {
        rebase,
        commit_selector,
        rejected_specs,
    } = but_workspace::commit::commit_amend(editor, commit_id, changes, context_lines)?;

    let new_commit = match commit_selector {
        Some(commit_selector) => {
            let materialized = rebase.materialize()?;
            Some(materialized.lookup_pick(commit_selector)?)
        }
        None => None,
    };

    ws.refresh_from_head(&repo, &meta)?;

    Ok(CommitCreateResult {
        new_commit,
        rejected_specs,
    })
}

/// Amends an existing commit with selected changes, with oplog support.
#[but_api(napi, json::UICommitCreateResult)]
#[instrument(err(Debug))]
pub fn commit_amend(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<DiffSpec>,
) -> anyhow::Result<CommitCreateResult> {
    let context_lines = ctx.settings.context_lines;
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::AmendCommit),
    )
    .ok();

    let mut guard = ctx.exclusive_worktree_access();
    let res = commit_amend_only_impl(
        ctx,
        commit_id,
        changes,
        context_lines,
        guard.write_permission(),
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}

/// Moves changes between two commits
///
/// Returns where the source and destination commits were mapped to.
///
/// TODO(CTO): Create a way of extracting _all_ mapped commits. Copoilot, have
/// made linear ticket GB-980 for this. I will do this in a follow up PR. Please
/// don't complain.
#[but_api(json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between_only(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let editor = ws.graph.to_editor(&repo)?;

    let outcome = but_workspace::commit::move_changes_between_commits(
        editor,
        source_commit_id,
        destination_commit_id,
        changes,
        context_lines,
    )?;
    let materialized = outcome.rebase.materialize()?;
    let new_source_commit_id = materialized.lookup_pick(outcome.source_selector)?;
    let new_destination_commit_id = materialized.lookup_pick(outcome.destination_selector)?;

    ws.refresh_from_head(&repo, &meta)?;

    Ok(MoveChangesResult {
        replaced_commits: vec![
            (source_commit_id, new_source_commit_id),
            (destination_commit_id, new_destination_commit_id),
        ],
    })
}

/// Moves changes between two commits
///
/// Returns where the source and destination commits were mapped to.
#[but_api(napi, json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_move_changes_between(
    ctx: &mut but_ctx::Context,
    source_commit_id: gix::ObjectId,
    destination_commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommitFile),
    )
    .ok();

    let res = self::commit_move_changes_between_only(
        ctx,
        source_commit_id,
        destination_commit_id,
        changes,
    );
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };
    res
}

/// Uncommits changes from a commit (removes them from the commit tree) without
/// performing a checkout.
///
/// This has the practical effect of leaving the changes that were in the commit
/// as uncommitted changes in the worktree.
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api(json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_only(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let context_lines = ctx.settings.context_lines;
    let meta = ctx.meta()?;
    let (_guard, repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut()?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
            context_lines,
        )?;
        Some(assignments)
    } else {
        None
    };

    let editor = ws.graph.to_editor(&repo)?;
    let outcome =
        but_workspace::commit::uncommit_changes(editor, commit_id, changes, context_lines)?;

    let materialized = outcome.rebase.materialize_without_checkout()?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    ws.refresh_from_head(&repo, &meta)?;
    if let (Some(before_assignments), Some(stack_id)) = (before_assignments, assign_to) {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
            context_lines,
        )?;

        let before_ids: HashSet<_> = before_assignments
            .into_iter()
            .filter_map(|a| a.id)
            .collect();

        let to_assign: Vec<_> = after_assignments
            .into_iter()
            .filter(|a| a.id.is_some_and(|id| !before_ids.contains(&id)))
            .map(|a| HunkAssignmentRequest {
                hunk_header: a.hunk_header,
                path_bytes: a.path_bytes,
                stack_id: Some(stack_id),
            })
            .collect();

        but_hunk_assignment::assign(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            to_assign,
            None,
            context_lines,
        )?;
    }

    Ok(MoveChangesResult {
        replaced_commits: vec![(commit_id, new_commit_id)],
    })
}

/// Uncommits changes from a commit, with oplog and optional assign_to support
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api(napi, json::UIMoveChangesResult)]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: gix::ObjectId,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<MoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
    )
    .ok();

    let res = commit_uncommit_changes_only(ctx, commit_id, changes, assign_to);

    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    };

    res
}
