use but_api_macros::but_api;
use but_core::{ui::TreeChanges, worktree::checkout::UncommitedWorktreeChanges};
use but_ctx::Context;
use but_oplog::legacy::{OperationKind, SnapshotDetails, Trailer};
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
};
use tracing::instrument;

/// Apply `existing_branch` to the workspace in the repository that `ctx` refers to, or create the workspace with default name.
pub fn apply_only(
    ctx: &but_ctx::Context,
    existing_branch: &gix::refs::FullNameRef,
) -> anyhow::Result<but_workspace::branch::apply::Outcome<'static>> {
    let mut guard = ctx.exclusive_worktree_access();
    let (repo, mut meta, graph) =
        ctx.graph_and_meta_mut_and_repo_from_head(guard.write_permission())?;
    let ws = graph.to_workspace()?;
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
    Ok(out)
}

// TODO: generate this with an improved `api_cmd` macro.
/// Just like [apply_only()], but will create an oplog entry as well on success.
pub fn apply(
    ctx: &but_ctx::Context,
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
        snapshot.commit(ctx).ok();
    }
    res
}

/// Gets the changes for a given branch.
#[but_api(TreeChanges)]
#[instrument(err(Debug))]
pub fn branch_diff(ctx: &Context, branch: String) -> anyhow::Result<TreeChanges> {
    let guard = ctx.shared_worktree_access();
    let (repo, _meta, graph) =
        ctx.graph_and_meta_and_repo_from_head(ctx.repo.get()?.clone(), guard.read_permission())?;
    let reference = repo.find_reference(&branch)?;
    let ws = graph.to_workspace()?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, reference.name())
}
