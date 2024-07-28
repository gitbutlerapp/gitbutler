pub mod commands {
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_repo::RepoCommands;
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
}
