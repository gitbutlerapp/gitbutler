use std::collections::BTreeMap;

use but_api_macros::but_api;
use but_core::{RefMetadata, ui::TreeChanges, worktree::checkout::UncommitedWorktreeChanges};
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_rebase::graph_rebase::GraphExt;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
};
use tracing::instrument;

/// Outcome after moving a branch.
pub struct MoveBranchResult {
    /// Commits that were replaced while transplanting a branch.
    pub replaced_commits: BTreeMap<gix::ObjectId, gix::ObjectId>,
}

/// JSON transport types for branch APIs.
pub mod json {
    use serde::Serialize;

    use crate::{branch::MoveBranchResult, json::HexHash};

    /// JSON sibling of [`but_workspace::branch::apply::Outcome`].
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct ApplyOutcome {
        /// Whether the workspace changed while applying the branch.
        pub workspace_changed: bool,
        /// The branches that were actually applied.
        pub applied_branches: Vec<crate::json::FullRefName>,
        /// Whether the workspace reference had to be created.
        pub workspace_ref_created: bool,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(ApplyOutcome);

    impl<'a> From<but_workspace::branch::apply::Outcome<'a>> for ApplyOutcome {
        fn from(value: but_workspace::branch::apply::Outcome<'a>) -> Self {
            let workspace_changed = value.workspace_changed();
            let but_workspace::branch::apply::Outcome {
                workspace: _,
                applied_branches,
                workspace_ref_created,
                workspace_merge: _,
                conflicting_stack_ids: _,
            } = value;

            ApplyOutcome {
                workspace_changed,
                applied_branches: applied_branches.into_iter().map(Into::into).collect(),
                workspace_ref_created,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    /// UI type for moving a branch.
    pub struct UIMoveBranchResult {
        /// Commits that have been replaced after transplanting a branch.
        /// Maps `oldId → newId`.
        #[cfg_attr(
            feature = "export-schema",
            schemars(with = "std::collections::BTreeMap<String, String>")
        )]
        pub replaced_commits: std::collections::BTreeMap<HexHash, HexHash>,
    }
    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(UIMoveBranchResult);

    impl From<MoveBranchResult> for UIMoveBranchResult {
        fn from(value: MoveBranchResult) -> Self {
            Self {
                replaced_commits: value
                    .replaced_commits
                    .into_iter()
                    .map(|(old, new)| (old.into(), new.into()))
                    .collect(),
            }
        }
    }
}

/// Apply `existing_branch` to the workspace in the repository that `ctx` refers to, or create the workspace with default name.
pub fn apply_only(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
    let out = but_workspace::branch::apply(
        existing_branch,
        &ws,
        &repo,
        &mut meta,
        // NOTE: Options can later be passed as parameter, or we have a separate function for that.
        //       Showing them off here while leaving defaults.
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::default(),
            on_workspace_conflict: OnWorkspaceMergeConflict::default(),
            workspace_reference_naming: WorkspaceReferenceNaming::default(),
            uncommitted_changes: UncommitedWorktreeChanges::default(),
            order: None,
            new_stack_id: None,
        },
    )?
    .into_owned();

    *ws = out.workspace.clone().into_owned();
    Ok(out)
}

/// Just like [apply_only()], but will create an oplog entry as well on success.
#[but_api(napi, json::ApplyOutcome)]
#[instrument(err(Debug))]
pub fn apply(
    ctx: &mut but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
    // NOTE: since this is optional by nature, the same would be true if snapshotting/undo would be disabled via `ctx` app settings, for instance.
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::CreateBranch).with_trailers(vec![Trailer {
            key: "name".into(),
            value: existing_branch.to_string(),
        }]),
    )
    .ok();

    let res = apply_only(ctx, existing_branch);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| res.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    }
    res
}

/// Gets the changes for a given branch.
#[but_api(napi, TreeChanges)]
#[instrument(err(Debug))]
pub fn branch_diff(ctx: &Context, branch: String) -> anyhow::Result<TreeChanges> {
    let (_guard, repo, ws, _) = ctx.workspace_and_db()?;
    let branch = repo.find_reference(&branch)?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, branch.name())
}

/// Move a branch on top of another
#[but_api(napi, json::UIMoveBranchResult)]
#[instrument(err(Debug))]
pub fn move_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<MoveBranchResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::MoveBranch),
    )
    .ok();

    let move_branch_result = move_branch_impl(ctx, subject_branch, target_branch);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| move_branch_result.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    }
    move_branch_result
}

/// Move the branch, updating the workspace and the metadata.
///
/// `subject_branch` - The branch to move.
///
/// `target_branch` - The branch to move `subject_branch` on top of.
fn move_branch_impl(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<MoveBranchResult> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut workspace, _) = ctx.workspace_mut_and_db()?;
    let editor = workspace.graph.to_editor(&repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(editor, &workspace, subject_branch, target_branch)?;

    let materialization = rebase.materialize()?;
    if let Some((ws_meta, ref_name)) = ws_meta.zip(workspace.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    workspace.refresh_from_head(&repo, &meta)?;

    Ok(MoveBranchResult {
        replaced_commits: materialization.history.commit_mappings(),
    })
}

/// Take a branch out of a stack
///
/// `subject_branch` - The branch to take out of its stack, and create a new one out of.
#[but_api(napi, json::UIMoveBranchResult)]
#[instrument(err(Debug))]
pub fn tear_off_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<MoveBranchResult> {
    let maybe_oplog_entry = but_oplog::UnmaterializedOplogSnapshot::from_details(
        ctx,
        SnapshotDetails::new(OperationKind::TearOffBranch),
    )
    .ok();

    let move_branch_result = tear_off_branch_impl(ctx, subject_branch);
    if let Some(snapshot) = maybe_oplog_entry.filter(|_| move_branch_result.is_ok()) {
        let mut guard = ctx.exclusive_worktree_access();
        snapshot.commit(ctx, guard.write_permission()).ok();
    }
    move_branch_result
}

/// Move the branch, updating the workspace and the metadata.
fn tear_off_branch_impl(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<MoveBranchResult> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut workspace, _) = ctx.workspace_mut_and_db()?;
    let editor = workspace.graph.to_editor(&repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::tear_off_branch(editor, &workspace, subject_branch, None)?;

    let materialization = rebase.materialize()?;
    if let Some((ws_meta, ref_name)) = ws_meta.zip(workspace.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    workspace.refresh_from_head(&repo, &meta)?;

    Ok(MoveBranchResult {
        replaced_commits: materialization.history.commit_mappings(),
    })
}
