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
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::new::worktree_new(ctx, guard.read_permission(), reference.as_ref())
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_list(ctx: &mut but_ctx::Context) -> Result<ListWorktreeOutcome> {
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::list::worktree_list(ctx, guard.read_permission())
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_integration_status(
    ctx: &mut but_ctx::Context,
    id: WorktreeId,
    target: gix::refs::FullName,
) -> Result<WorktreeIntegrationStatus> {
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::integrate::worktree_integration_status(
        ctx,
        guard.read_permission(),
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

    but_worktrees::integrate::worktree_integrate(
        ctx,
        guard.write_permission(),
        &id,
        target.as_ref(),
    )
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_id(
    ctx: &mut but_ctx::Context,
    id: WorktreeId,
) -> Result<DestroyWorktreeOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    but_worktrees::destroy::worktree_destroy_by_id(ctx, guard.write_permission(), &id)
}

#[but_api]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_reference(
    ctx: &mut but_ctx::Context,
    reference: gix::refs::FullName,
) -> Result<DestroyWorktreeOutcome> {
    let mut guard = ctx.exclusive_worktree_access();

    but_worktrees::destroy::worktree_destroy_by_reference(
        ctx,
        guard.write_permission(),
        reference.as_ref(),
    )
}
