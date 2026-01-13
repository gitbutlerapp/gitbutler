use std::collections::HashSet;

use crate::json;
use bstr::{BString, ByteSlice};
use but_api_macros::but_api;
use but_graph::Graph;
use but_hunk_assignment::HunkAssignmentRequest;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{GraphExt, LookupStep as _, mutate::InsertSide};
use but_workspace::commit::{move_changes_between_commits, uncommit_changes};
use tracing::instrument;

/// Rewords a commit
///
/// Returns the ID of the newly renamed commit
#[but_api]
#[instrument(err(Debug))]
pub fn commit_reword_only(
    ctx: &but_ctx::Context,
    commit_id: gix::ObjectId,
    message: BString,
) -> anyhow::Result<gix::ObjectId> {
    let guard = ctx.exclusive_worktree_access();
    let (_, graph) = ctx.graph_and_read_only_meta_from_head(guard.read_permission())?;
    let repo = ctx.repo.get()?;
    let editor = graph.to_editor(&repo)?;

    let (outcome, edited_commit_selector) =
        but_workspace::commit::reword(editor, commit_id, message.as_bstr())?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(edited_commit_selector)?;

    Ok(id)
}

/// Rewords a commit, but without updating the oplog.
///
/// Returns the ID of the newly renamed commit
#[but_api]
#[instrument(err(Debug))]
pub fn commit_reword(
    ctx: &but_ctx::Context,
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
        snapshot.commit(ctx).ok();
    };
    res
}

/// UI types for commit insertion
pub mod ui {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase", tag = "type", content = "subject")]
    /// Specifies where to insert a blank commit
    pub enum RelativeTo {
        /// Relative to a commit
        #[serde(with = "but_serde::object_id")]
        Commit(gix::ObjectId),
        /// Relative to a reference
        #[serde(with = "but_serde::fullname_lossy")]
        Reference(gix::refs::FullName),
    }

    impl From<but_workspace::commit::insert_blank_commit::RelativeTo<'_>> for RelativeTo {
        fn from(value: but_workspace::commit::insert_blank_commit::RelativeTo) -> Self {
            match value {
                but_workspace::commit::insert_blank_commit::RelativeTo::Commit(c) => {
                    Self::Commit(c)
                }
                but_workspace::commit::insert_blank_commit::RelativeTo::Reference(r) => {
                    Self::Reference(r.into())
                }
            }
        }
    }

    impl<'a> From<&'a RelativeTo> for but_workspace::commit::insert_blank_commit::RelativeTo<'a> {
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
    ctx: &but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<gix::ObjectId> {
    let guard = ctx.exclusive_worktree_access();
    let (_, graph) = ctx.graph_and_read_only_meta_from_head(guard.read_permission())?;
    let repo = ctx.repo.get()?;
    commit_insert_blank_only_impl(&graph, &repo, relative_to, side)
}

/// Implementation of inserting a blank commit relative to either a commit or a reference
///
/// Returns the ID of the newly created blank commit
pub fn commit_insert_blank_only_impl(
    graph: &Graph,
    repo: &gix::Repository,
    relative_to: ui::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<gix::ObjectId> {
    let editor = graph.to_editor(repo)?;

    let relative_to = (&relative_to).into();

    let (outcome, blank_commit_selector) =
        but_workspace::commit::insert_blank_commit(editor, side, relative_to)?;

    let outcome = outcome.materialize()?;
    let id = outcome.lookup_pick(blank_commit_selector)?;

    Ok(id)
}

/// Inserts a blank commit relative to either a commit or a reference, with oplog support
///
/// Returns the ID of the newly created blank commit
#[but_api(json::HexHash)]
#[instrument(err(Debug))]
pub fn commit_insert_blank(
    ctx: &but_ctx::Context,
    relative_to: ui::RelativeTo,
    side: InsertSide,
) -> anyhow::Result<gix::ObjectId> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::InsertBlankCommit),
    )
    .ok();

    let res = commit_insert_blank_only(ctx, relative_to, side);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx).ok();
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
#[but_api]
#[instrument(err(Debug))]
pub fn commit_move_changes_between_only(
    ctx: &but_ctx::Context,
    source_commit_id: json::HexHash,
    destination_commit_id: json::HexHash,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<json::UIMoveChangesResult> {
    let guard = ctx.exclusive_worktree_access();
    let (_, graph) = ctx.graph_and_read_only_meta_from_head(guard.read_permission())?;
    let repo = ctx.repo.get()?;
    let editor = graph.to_editor(&repo)?;

    let outcome = move_changes_between_commits(
        editor,
        source_commit_id.into(),
        destination_commit_id.into(),
        changes,
        3,
    )?;
    let materialized = outcome.rebase.materialize()?;
    let new_source_commit_id = materialized.lookup_pick(outcome.source_selector)?;
    let new_destination_commit_id = materialized.lookup_pick(outcome.destination_selector)?;

    Ok(json::UIMoveChangesResult {
        replaced_commits: vec![
            (source_commit_id, new_source_commit_id.into()),
            (destination_commit_id, new_destination_commit_id.into()),
        ],
    })
}

/// Moves changes between two commits
///
/// Returns where the source and destination commits were mapped to.
#[but_api]
#[instrument(err(Debug))]
pub fn commit_move_changes_between(
    ctx: &but_ctx::Context,
    source_commit_id: json::HexHash,
    destination_commit_id: json::HexHash,
    changes: Vec<but_core::DiffSpec>,
) -> anyhow::Result<json::UIMoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::MoveCommitFile),
    )
    .ok();

    let res =
        commit_move_changes_between_only(ctx, source_commit_id, destination_commit_id, changes);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx).ok();
    };
    res
}

/// Uncommits changes from a commit (removes them from the commit tree) without
/// performing a checkout.
///
/// This has the practical effect of leaving the
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes_only(
    ctx: &mut but_ctx::Context,
    commit_id: json::HexHash,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<json::UIMoveChangesResult> {
    let guard = ctx.exclusive_worktree_access();
    let (_, graph) = ctx.graph_and_read_only_meta_from_head(guard.read_permission())?;
    // We need to pass a mutable ctx below, so we can't hold a reference to the repo
    let repo = ctx.repo.get()?.clone();
    let editor = graph.to_editor(&repo)?;

    let (_, workspace) = ctx.workspace_and_read_only_meta_from_head(guard.read_permission())?;

    let before_assignments = if assign_to.is_some() {
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            ctx,
            &repo,
            &workspace,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
        )?;
        Some(assignments)
    } else {
        None
    };

    let outcome = uncommit_changes(editor, commit_id.into(), changes, 3)?;

    let materialized = outcome.rebase.materialize_without_checkout()?;
    let new_commit_id = materialized.lookup_pick(outcome.commit_selector)?;

    let workspace = ctx
        .workspace_and_read_only_meta_from_head(guard.read_permission())
        .map(|x| x.1);
    if let (Some(before_assignments), Some(stack_id), Ok(workspace)) =
        (before_assignments, assign_to, workspace)
    {
        let (after_assignments, _) = but_hunk_assignment::assignments_with_fallback(
            ctx,
            &repo,
            &workspace,
            false,
            None::<Vec<but_core::TreeChange>>,
            None,
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

        but_hunk_assignment::assign(ctx, &repo, &workspace, to_assign, None)?;
    }

    Ok(json::UIMoveChangesResult {
        replaced_commits: vec![(commit_id, new_commit_id.into())],
    })
}

/// Uncommits changes from a commit, with oplog and optional assign_to support
///
/// If `assign_to` is provided, the newly uncommitted changes will be assigned
/// to the specified stack.
#[but_api]
#[instrument(err(Debug))]
pub fn commit_uncommit_changes(
    ctx: &mut but_ctx::Context,
    commit_id: json::HexHash,
    changes: Vec<but_core::DiffSpec>,
    assign_to: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<json::UIMoveChangesResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::DiscardChanges),
    )
    .ok();

    let res = commit_uncommit_changes_only(ctx, commit_id, changes.clone(), assign_to);

    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        snapshot.commit(ctx).ok();
    };

    res
}
