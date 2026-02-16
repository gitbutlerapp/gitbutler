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

// TODO: generate this with an improved `api_cmd` macro.
/// Just like [apply_only()], but will create an oplog entry as well on success.
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
#[but_api(TreeChanges)]
#[instrument(err(Debug))]
pub fn branch_diff(ctx: &Context, branch: String) -> anyhow::Result<TreeChanges> {
    let (_guard, _, ws, _) = ctx.workspace_and_db()?;
    let repo = ctx.repo.get()?;
    let reference = repo.find_reference(&branch)?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, reference.name())
}
