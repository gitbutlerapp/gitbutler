// Re-export for use in other crates
use anyhow::Result;
use but_api_macros::api_cmd_tauri;
use but_ctx::Context;
pub use but_worktrees::integrate::WorktreeIntegrationStatus as IntegrationStatus;
use but_worktrees::{
    WorktreeId, destroy::DestroyWorktreeOutcome, integrate::WorktreeIntegrationStatus,
    list::ListWorktreeOutcome, new::NewWorktreeOutcome,
};
use gitbutler_project::ProjectId;
use tracing::instrument;

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_new(
    project_id: ProjectId,
    reference: gix::refs::FullName,
) -> Result<NewWorktreeOutcome> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::new::worktree_new(&mut ctx, guard.read_permission(), reference.as_ref())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_list(project_id: ProjectId) -> Result<ListWorktreeOutcome> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::list::worktree_list(&mut ctx, guard.read_permission())
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_integration_status(
    project_id: ProjectId,
    id: WorktreeId,
    target: gix::refs::FullName,
) -> Result<WorktreeIntegrationStatus> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let guard = ctx.exclusive_worktree_access();

    but_worktrees::integrate::worktree_integration_status(
        &mut ctx,
        guard.read_permission(),
        &id,
        target.as_ref(),
    )
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_integrate(
    project_id: ProjectId,
    id: WorktreeId,
    target: gix::refs::FullName,
) -> Result<()> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut guard = ctx.exclusive_worktree_access();

    but_worktrees::integrate::worktree_integrate(
        &mut ctx,
        guard.write_permission(),
        &id,
        target.as_ref(),
    )
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_id(
    project_id: ProjectId,
    id: WorktreeId,
) -> Result<DestroyWorktreeOutcome> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut guard = ctx.exclusive_worktree_access();

    but_worktrees::destroy::worktree_destroy_by_id(&mut ctx, guard.write_permission(), &id)
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn worktree_destroy_by_reference(
    project_id: ProjectId,
    reference: gix::refs::FullName,
) -> Result<DestroyWorktreeOutcome> {
    let project = gitbutler_project::get(project_id)?;
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut guard = ctx.exclusive_worktree_access();

    but_worktrees::destroy::worktree_destroy_by_reference(
        &mut ctx,
        guard.write_permission(),
        reference.as_ref(),
    )
}
