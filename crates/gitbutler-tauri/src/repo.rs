pub mod commands {
    use crate::error::{Error, UnmarkedError};
    use anyhow::Result;
    use gitbutler_branch_actions::{hooks, RemoteBranchFile};
    use gitbutler_command_context::CommandContext;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_repo::hooks::{HookResult, MessageHookResult};
    use gitbutler_repo::{FileInfo, RepoCommands};
    use gitbutler_settings::AppSettingsWithDiskSync;
    use gitbutler_stack::BranchOwnershipClaims;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use tauri::State;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn git_get_local_config(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
        key: &str,
    ) -> Result<Option<String>, Error> {
        let project = projects.get(id)?;
        Ok(project.get_local_config(key)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn git_set_local_config(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        let project = projects.get(id)?;
        project.set_local_config(key, value).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn check_signing_settings(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
    ) -> Result<bool, Error> {
        let project = projects.get(id)?;
        project.check_signing_settings().map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument]
    pub fn git_clone_repository(
        repository_url: &str,
        target_dir: &Path,
    ) -> Result<(), UnmarkedError> {
        let should_interrupt = AtomicBool::new(false);

        gix::prepare_clone(repository_url, target_dir)?
            .fetch_then_checkout(gix::progress::Discard, &should_interrupt)
            .map(|(checkout, _outcome)| checkout)?
            .main_worktree(gix::progress::Discard, &should_interrupt)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings))]
    pub fn get_uncommited_files(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        id: ProjectId,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(id)?;

        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        Ok(gitbutler_branch_actions::get_uncommited_files(&ctx)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects))]
    pub fn get_commit_file(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        relative_path: &Path,
        commit_id: String,
    ) -> Result<FileInfo, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(commit_id.as_ref()).map_err(anyhow::Error::from)?;
        Ok(project.read_file_from_commit(commit_oid, relative_path)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects))]
    pub fn get_workspace_file(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        relative_path: &Path,
    ) -> Result<FileInfo, Error> {
        let project = projects.get(project_id)?;
        Ok(project.read_file_from_workspace(relative_path)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings))]
    pub fn pre_commit_hook(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        ownership: BranchOwnershipClaims,
    ) -> Result<HookResult, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        Ok(hooks::pre_commit(&ctx, &ownership)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings))]
    pub fn post_commit_hook(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
    ) -> Result<HookResult, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        Ok(gitbutler_repo::hooks::post_commit(&ctx)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings))]
    pub fn message_hook(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        message: String,
    ) -> Result<MessageHookResult, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        Ok(gitbutler_repo::hooks::commit_msg(&ctx, message)?)
    }
}
