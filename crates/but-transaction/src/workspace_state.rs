use nonempty::NonEmpty;
use tracing::warn;

use super::WorkspaceState;
use std::collections::{BTreeMap, HashMap};

use but_core::{ChangeId, DryRun, sync::RepoExclusive};
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use but_rebase::graph_rebase::{MaterializeOutcome, SuccessfulRebase};
use but_workspace::ref_info::SegmentIdentity;

impl WorkspaceState {
    /// Map each projected segment that can be told apart to whether its commits contain conflicts.
    #[cfg(not(feature = "graph-workspace"))]
    pub fn conflicts_by_segment(&self) -> HashMap<SegmentIdentity, bool> {
        self.head_info
            .lanes()
            .flat_map(but_workspace::ref_info::Lane::identified_segments)
            .filter_map(|(identity, segment)| {
                Some((
                    identity?,
                    segment.commits.iter().any(|commit| commit.has_conflicts),
                ))
            })
            .collect()
    }

    /// Map each projected local reference to whether its commits contain conflicts.
    ///
    /// The graph projection carries no worktrees, so detached worktrees are absent.
    #[cfg(feature = "graph-workspace")]
    pub fn conflicts_by_segment(&self) -> HashMap<SegmentIdentity, bool> {
        use but_workspace::ui::workspace::DetailedGraphRowData;

        self.graph_workspace
            .stacks
            .iter()
            .flat_map(|stack| {
                stack.reference_segments.iter().filter_map(|segment| {
                    let DetailedGraphRowData::Reference(reference) =
                        &stack.rows.get(segment.reference_idx)?.data
                    else {
                        return None;
                    };
                    let has_conflicts = segment.row_idxs.iter().any(|&row_idx| {
                        matches!(
                            stack.rows.get(row_idx).map(|row| &row.data),
                            Some(DetailedGraphRowData::Commit(commit)) if commit.has_conflicts
                        )
                    });
                    let ref_name = gix::refs::FullName::try_from(bstr::BString::from(
                        reference.ref_name.full_name_bytes.to_vec(),
                    ))
                    .ok()?;
                    Some((SegmentIdentity::Branch(ref_name), has_conflicts))
                })
            })
            .collect()
    }

    /// Build a [`WorkspaceState`] from an already-prepared overlayed graph.
    ///
    /// Use this when the caller already has a graph describing the workspace after the
    /// intended operation, regardless of whether that graph came from a preview, a
    /// materialized rebase, or another graph-producing workflow. The caller is
    /// responsible for supplying the matching `replaced_commits` map for that graph.
    ///
    /// `db` supplies the workspace metadata and cached forge information.
    ///
    /// This is the most direct constructor in this module and is the right choice when
    /// there is no need to inspect or materialize a [`SuccessfulRebase`].
    #[allow(clippy::too_many_arguments)]
    fn from_workspace_with_prs(
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        conflicted_commits: Vec<(gix::ObjectId, ChangeId)>,
        prs_by_head: &HashMap<String, but_forge::ReviewAssociation>,
        db: &mut but_db::DbHandle,
        conflicted_checkouts: Vec<but_rebase::graph_rebase::ConflictedCheckout>,
    ) -> anyhow::Result<WorkspaceState> {
        #[cfg(not(feature = "graph-workspace"))]
        {
            let _ = db;
            let mut head_info = but_workspace::graph_to_ref_info(
                workspace,
                repo,
                but_workspace::ref_info::Options {
                    project_meta: workspace.graph.project_meta.clone(),
                    traversal: but_graph::init::Options::limited(),
                    expensive_commit_info: true,
                    ..Default::default()
                },
            )?;

            // Same pass the `head_info` read command runs, so mutation
            // responses (which Lite renders directly) carry the same
            // associations: open reviews from the forge cache for active
            // branches, durable stored identity for integrated ones.
            head_info.apply_forge_review_associations(prs_by_head);

            Ok(WorkspaceState {
                replaced_commits,
                conflicted_commits,
                head_info,
                conflicted_checkouts,
            })
        }
        #[cfg(feature = "graph-workspace")]
        {
            // The graph_workspace projection needs its own equivalent enrichment;
            // that is out of scope here.
            let _ = prs_by_head;
            let mut workspace = workspace.clone();
            let graph_workspace =
                but_workspace::workspace::detailed_graph_workspace(&mut workspace, repo, db)?;

            Ok(WorkspaceState {
                replaced_commits,
                conflicted_commits,
                graph_workspace: graph_workspace.into(),
                conflicted_checkouts,
            })
        }
    }

    /// Build a [`WorkspaceState`] from an already-prepared overlayed graph.
    ///
    /// This is the API-facing constructor for callers that already hold the
    /// workspace cache DB. It derives PR associations from the forge review
    /// cache before projecting the workspace state.
    ///
    /// It reports no `conflicted_checkouts`, which is only true of a workspace that was never
    /// checked out. Use [`Self::from_materialized`] after a materialize with checkout.
    pub fn from_workspace_with_db(
        workspace: &but_graph::Workspace,
        repo: &gix::Repository,
        replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
        conflicted_commits: Vec<(gix::ObjectId, ChangeId)>,
        db: &mut but_db::DbHandle,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(db, repo)?;
        Self::from_workspace_with_prs(
            workspace,
            repo,
            replaced_commits,
            conflicted_commits,
            &prs_by_head,
            db,
            Vec::new(),
        )
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// Use this when the caller needs to report the post-rebase workspace layout before
    /// writing the rebase result back to the repository, such as dry-run flows or
    /// operations that intentionally preview the outcome first and materialize later.
    fn from_rebase_preview_with_prs(
        rebase: &mut SuccessfulRebase<'_, '_>,
        prs_by_head: &HashMap<String, but_forge::ReviewAssociation>,
    ) -> anyhow::Result<WorkspaceState> {
        let workspace = rebase.overlayed_graph()?.into_workspace()?;
        let replaced_commits = rebase.history.commit_mappings();
        let conflicted_commits = rebase.history.conflicted_commits.clone();
        let (repo, db) = rebase.repo_and_db_mut();
        Self::from_workspace_with_prs(
            &workspace,
            repo,
            replaced_commits,
            conflicted_commits,
            prs_by_head,
            db,
            Vec::new(),
        )
    }

    /// Build a preview [`WorkspaceState`] from a successful rebase without materializing it.
    ///
    /// This is the API-facing preview constructor: it reads PR associations from
    /// the forge review cache through the database handle the rebase carries,
    /// before projecting the preview state.
    pub fn from_rebase_preview(
        rebase: &mut SuccessfulRebase<'_, '_>,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(rebase.db(), rebase.repo())?;
        Self::from_rebase_preview_with_prs(rebase, &prs_by_head)
    }

    /// Build a [`WorkspaceState`] from a materialized rebase.
    ///
    /// This takes the whole [`MaterializeOutcome`] rather than its parts so that everything the
    /// projection reports — the workspace, the commit mappings, the PR associations, and which
    /// checkouts conflicted — is read from one place. A caller cannot forget to pass one on,
    /// which is the failure mode [`Self::from_workspace_with_db`] invites for materialized work.
    pub fn from_materialized(
        materialized: MaterializeOutcome<'_, '_>,
        repo: &gix::Repository,
    ) -> anyhow::Result<WorkspaceState> {
        let prs_by_head = but_forge::review_associations_by_head(materialized.db, repo)?;
        Self::from_workspace_with_prs(
            materialized.workspace,
            repo,
            materialized.history.commit_mappings(),
            materialized.history.conflicted_commits,
            &prs_by_head,
            materialized.db,
            materialized.conflicted_checkouts,
        )
    }

    /// Build a [`WorkspaceState`] from a successful rebase, materializing it when needed.
    ///
    /// Use this as the default entry point when an operation ends with a [`SuccessfulRebase`] and
    /// the API should return the resulting workspace state. When `dry_run` is `true`, it projects
    /// the overlayed graph so the caller sees the outcome without changing the repository.
    /// Otherwise it materializes the rebase, then reports the workspace state together with the
    /// final commit-replacement mappings returned by the materialized history.
    ///
    /// PR associations come from the forge review cache, read through the database handle the
    /// rebase carries.
    pub fn from_successful_rebase(
        rebase: SuccessfulRebase<'_, '_>,
        repo: &gix::Repository,
        dry_run: DryRun,
    ) -> anyhow::Result<WorkspaceState> {
        if dry_run.into() {
            let mut rebase = rebase;
            let prs_by_head = but_forge::review_associations_by_head(rebase.db(), rebase.repo())?;
            return Self::from_rebase_preview_with_prs(&mut rebase, &prs_by_head);
        }

        Self::from_materialized(rebase.materialize(Default::default())?, repo)
    }
}

/// Result of restoring the previously applied branches into the workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceRecreateResult {
    /// Workspace state after recreation, or the unchanged state for a no-op.
    pub workspace: WorkspaceState,
    /// Previously applied stack heads that could not be reapplied due to conflicts.
    pub conflicting_stacks: Vec<gix::refs::FullName>,
    /// Whether the repository was already on a managed workspace and nothing was changed.
    pub already_on_workspace: bool,
}

/// Recreate an existing workspace by applying all previously applied branches, under
/// caller-held exclusive access, recording one undo snapshot.
///
/// This incorporates changes made in single-branch mode. The current branch is included
/// unless it is the target or its local tracking branch. Conflicting stacks remain unapplied
/// and are returned as partial success. Already being on the workspace is a no-op, and then
/// no snapshot is prepared.
///
/// The workspace reference must already exist. This does not initialize a new workspace.
pub fn workspace_recreate_with_perm(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceRecreateResult> {
    workspace_recreate(ctx, perm, true)
}

/// Recreate the workspace as in [`workspace_recreate_with_perm()`], without recording undo.
pub fn workspace_recreate_with_perm_only(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
) -> anyhow::Result<WorkspaceRecreateResult> {
    workspace_recreate(ctx, perm, false)
}

fn workspace_recreate(
    ctx: &mut but_ctx::Context,
    perm: &mut RepoExclusive,
    record_snapshot: bool,
) -> anyhow::Result<WorkspaceRecreateResult> {
    let already_on_workspace = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        match &ws.kind {
            but_graph::workspace::WorkspaceKind::Managed { .. }
            | but_graph::workspace::WorkspaceKind::ManagedMissingWorkspaceCommit { .. } => true,
            but_graph::workspace::WorkspaceKind::AdHoc => {
                anyhow::ensure!(
                    repo.try_find_reference(but_core::WORKSPACE_REF_NAME)?
                        .is_some(),
                    "Cannot recreate a workspace without an existing workspace reference"
                );
                false
            }
        }
    };

    let maybe_oplog_entry = if already_on_workspace || !record_snapshot {
        None
    } else {
        but_oplog::UnmaterializedOplogSnapshot::from_details_with_perm(
            ctx,
            SnapshotDetails::new(OperationKind::SwitchToWorkspace),
            perm.read_permission(),
            DryRun::No,
        )
    };

    let conflicting_stacks = if already_on_workspace {
        Vec::new()
    } else {
        let (repo, mut ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;

        // The branches of each saved stack that still exist, tip first.
        let previously_applied_stacks: Vec<NonEmpty<gix::refs::FullName>> = {
            let workspace_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
            let workspace_meta = db.meta().workspace(workspace_ref.as_ref())?;
            let mut existing_stacks = Vec::new();
            for stack in workspace_meta.stacks(but_core::ref_metadata::StackKind::Applied) {
                // A deleted tip must not hide surviving lower branches in the saved stack.
                // Only absence is recoverable; propagate reference lookup errors.
                let mut existing_branches = Vec::new();
                for branch in stack.branches.iter().filter(|branch| !branch.archived) {
                    if repo.try_find_reference(branch.ref_name.as_ref())?.is_some() {
                        existing_branches.push(branch.ref_name.clone());
                    }
                }
                existing_stacks.extend(NonEmpty::from_vec(existing_branches));
            }
            existing_stacks
        };

        // Reapply the saved stacks in metadata order. If it fails due to conflict or overlap,
        // remove the offending stack and try again.
        let mut stacks = previously_applied_stacks;
        let mut conflicting_stacks = Vec::new();
        loop {
            let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &repo, &mut db)?;
            let (index, err) = match switch_to_workspace_with_stacks(editor, &stacks)? {
                Ok(rebase) => {
                    rebase.materialize(Default::default())?;
                    break;
                }
                Err(failed_stack) => failed_stack,
            };
            let stack = stacks.remove(index);
            if err
                .downcast_ref::<but_rebase::merge::ConflictErrorContext>()
                .is_some()
            {
                conflicting_stacks.push(stack.head);
            } else if err
                .downcast_ref::<but_workspace::branch::apply::OverlapErrorContext>()
                .is_some()
            {
                // A stack that shares commits with another one is usually already in the
                // workspace through it.
                warn!("Not reapplying an overlapping stack: {err:#}");
            } else {
                return Err(err);
            }
        }
        drop((repo, db));
        ctx.update_workspace_cache(ws);

        conflicting_stacks
    };

    if let Some(snapshot) = maybe_oplog_entry {
        _ = snapshot.commit(ctx, perm);
    }

    if !already_on_workspace {
        ctx.reload_repo_and_invalidate_workspace(perm)?;
    }
    let (repo, ws, mut db) = ctx.workspace_mut_and_db_mut_with_perm(perm)?;
    let workspace = WorkspaceState::from_workspace_with_db(
        &ws,
        &repo,
        Default::default(),
        Vec::new(),
        &mut db,
    )?;
    Ok(WorkspaceRecreateResult {
        workspace,
        conflicting_stacks,
        already_on_workspace,
    })
}

/// Put a workspace commit on top of `HEAD` and check it out, then apply `stacks`, each given
/// tip first, to it.
///
/// If a stack can't be applied, its index in `stacks` is returned with the error, which carries a
/// [`but_rebase::merge::ConflictErrorContext`] if it conflicts with the stacks before it, and a
/// [`but_workspace::branch::apply::OverlapErrorContext`] if it shares commits with them.
fn switch_to_workspace_with_stacks<'ws, 'meta>(
    editor: but_rebase::graph_rebase::Editor<'ws, 'meta>,
    stacks: &[NonEmpty<gix::refs::FullName>],
) -> anyhow::Result<
    Result<but_rebase::graph_rebase::SuccessfulRebase<'ws, 'meta>, (usize, anyhow::Error)>,
> {
    let (mut rebase, _) = but_workspace::editor_workspace::switch_to_workspace(editor)?;
    for (order, stack) in stacks.iter().enumerate() {
        rebase = match but_workspace::branch::apply_stack(
            rebase.into_editor(),
            stack.head.as_ref(),
            order,
        ) {
            Ok(outcome) => outcome.rebase,
            Err(err) => return Ok(Err((order, err))),
        };
    }
    let mut editor = rebase.into_editor();
    if let Some(target) = editor.target_selector() {
        for NonEmpty {
            head: tip,
            tail: lower_branches,
        } in stacks
        {
            for lower_branch in lower_branches {
                stack_lower_branch(&mut editor, target, tip.as_ref(), lower_branch.as_ref())?;
            }
        }
    }
    Ok(Ok(editor.rebase()?))
}

/// If `lower_branch` points to a commit in the lane of `tip` and is not already in a lane, move it
/// into the lane.
fn stack_lower_branch(
    editor: &mut but_rebase::graph_rebase::Editor<'_, '_>,
    target: but_rebase::graph_rebase::Selector,
    tip: &gix::refs::FullNameRef,
    lower_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let Some(lower) = editor.try_select_reference(lower_branch) else {
        return Ok(());
    };
    // Must not already be in a lane.
    if !editor.direct_children(lower)?.is_empty() {
        return Ok(());
    }
    let [(commit, _)] = editor.direct_parents(lower)?[..] else {
        return Ok(());
    };
    // Find the other end of the edge that we must break (the child's commit).
    let mut step = editor.select_reference(tip)?;
    let child_of_commit = loop {
        if step == target {
            return Ok(());
        }
        let Some((parent, _)) = editor
            .direct_parents(step)?
            .into_iter()
            .min_by_key(|(_, order)| *order)
        else {
            return Ok(());
        };
        if parent == commit {
            break step;
        }
        step = parent;
    };
    for order in editor.remove_edges(child_of_commit, commit)? {
        editor.add_edge(child_of_commit, lower, order)?;
    }
    Ok(())
}
