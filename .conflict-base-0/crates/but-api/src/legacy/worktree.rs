// Re-export for use in other crates
use anyhow::Result;
use but_api_macros::but_api;
pub use but_worktrees::integrate::WorktreeIntegrationStatus as IntegrationStatus;
use but_worktrees::{
    WorktreeId, destroy::DestroyWorktreeOutcome, integrate::WorktreeIntegrationStatus,
    list::ListWorktreeOutcome, new::NewWorktreeOutcome,
};
use tracing::instrument;

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_new(
    ctx: &mut but_ctx::Context,
    reference: gix::refs::FullName,
) -> Result<NewWorktreeOutcome> {
    let guard = ctx.shared_worktree_access();
    let (repo, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;

    but_worktrees::new::worktree_new(&repo, &ws, &ctx.project_data_dir(), reference.as_ref())
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_list(ctx: &mut but_ctx::Context) -> Result<ListWorktreeOutcome> {
    let _guard = ctx.shared_worktree_access();
    let repo = ctx.repo.get()?;

    but_worktrees::list::worktree_list(&repo)
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_integration_status(
    ctx: &mut but_ctx::Context,
    id: WorktreeId,
    target: gix::refs::FullName,
) -> Result<WorktreeIntegrationStatus> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;

    but_worktrees::integrate::worktree_integration_status(
        &repo,
        &mut ws,
        &mut meta,
        &id,
        target.as_ref(),
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_integrate(
    ctx: &mut but_ctx::Context,
    id: WorktreeId,
    target: gix::refs::FullName,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;

    but_worktrees::integrate::worktree_integrate(&repo, &mut ws, &mut meta, &id, target.as_ref())
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_id(
    ctx: &mut but_ctx::Context,
    id: WorktreeId,
) -> Result<DestroyWorktreeOutcome> {
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;

    but_worktrees::destroy::worktree_destroy_by_id(&repo, &id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_reference(
    ctx: &mut but_ctx::Context,
    reference: gix::refs::FullName,
) -> Result<DestroyWorktreeOutcome> {
    let _guard = ctx.exclusive_worktree_access();
    let repo = ctx.repo.get()?;

    but_worktrees::destroy::worktree_destroy_by_reference(&repo, reference.as_ref())
}
