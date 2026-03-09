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

/// JSON transport types for branch APIs.
pub mod json {
    use serde::Serialize;

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

    impl<'a> From<but_workspace::branch::apply::Outcome<'a>> for ApplyOutcome {
        fn from(value: but_workspace::branch::apply::Outcome<'a>) -> Self {
            ApplyOutcome {
                workspace_changed: value.workspace_changed(),
                applied_branches: value.applied_branches.into_iter().map(Into::into).collect(),
                workspace_ref_created: value.workspace_ref_created,
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
#[but_api(json::ApplyOutcome)]
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
#[but_api]
#[instrument(err(Debug))]
pub fn move_branch(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
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
fn move_branch_impl(
    ctx: &mut but_ctx::Context,
    subject_branch: &gix::refs::FullNameRef,
    target_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<()> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, mut workspace, _) = ctx.workspace_mut_and_db()?;
    let editor = workspace.graph.to_editor(&repo)?;
    let but_workspace::branch::move_branch::Outcome { rebase, ws_meta } =
        but_workspace::branch::move_branch(&workspace, editor, subject_branch, target_branch)?;

    rebase.materialize()?;
    if let Some((ws_meta, ref_name)) = ws_meta.zip(workspace.ref_name()) {
        let mut md = meta.workspace(ref_name)?;
        *md = ws_meta;
        meta.set_workspace(&md)?;
    }
    workspace.refresh_from_head(&repo, &meta)?;

    Ok(())
}
