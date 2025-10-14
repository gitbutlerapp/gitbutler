use crate::error::Error;
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use but_worktrees::list::ListWorktreeOutcome;
use but_worktrees::new::NewWorktreeOutcome;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn worktree_new(
    project_id: ProjectId,
    reference: gix::refs::PartialName,
) -> Result<NewWorktreeOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let guard = project.exclusive_worktree_access();
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    but_worktrees::new::worktree_new(&mut ctx, guard.read_permission(), reference.as_ref())
        .map_err(Into::into)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn worktree_list(project_id: ProjectId) -> Result<ListWorktreeOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let guard = project.exclusive_worktree_access();
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    but_worktrees::list::worktree_list(&mut ctx, guard.read_permission()).map_err(Into::into)
}
