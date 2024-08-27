pub mod commands {
    use anyhow::{Context, Result};
    use gitbutler_branch_actions::{RemoteBranchFile, VirtualBranchActions};
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_repo::RepoCommands;
    use gix::progress::Discard;
    use std::path::Path;
    use std::sync::atomic::AtomicBool;
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;

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
    pub fn git_clone_repository(repository_url: &str, target_dir: &Path) -> Result<(), Error> {
        let url =
            gix::url::parse(repository_url.into()).context("Failed to parse repository URL")?;
        let should_interrupt = AtomicBool::new(false);
        let mut prepared_clone =
            gix::prepare_clone(url, target_dir).context("Failed to prepare clone")?;
        let (mut prepared_checkout, _) = prepared_clone
            .fetch_then_checkout(Discard, &should_interrupt)
            .context("Failed to fetch")?;
        let should_interrupt = AtomicBool::new(false);
        prepared_checkout
            .main_worktree(Discard, &should_interrupt)
            .context("Failed to checkout main worktree")?;
        Ok(())
    }

    #[tauri::command(async)]
    pub fn get_uncommited_files(
        projects: State<'_, projects::Controller>,
        id: ProjectId,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(id)?;

        Ok(VirtualBranchActions.get_uncommited_files(&project)?)
    }
}
