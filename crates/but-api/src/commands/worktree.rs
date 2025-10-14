use crate::error::Error;
use but_api_macros::api_cmd;
use but_settings::AppSettings;
use but_worktrees::new::NewWorktreeOutcome;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use tracing::instrument;

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn worktree_new(project_id: ProjectId, reference: String) -> Result<NewWorktreeOutcome, Error> {
    let project = gitbutler_project::get(project_id)?;
    let guard = project.exclusive_worktree_access();
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    but_worktrees::new::worktree_new(&mut ctx, guard.read_permission(), &reference)
        .map_err(Into::into)
}
