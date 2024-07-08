pub mod commands {
    use crate::error::Error;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_repo::RepoCommands;
    use tauri::Manager;
    use tracing::instrument;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_get_local_config(
        handle: tauri::AppHandle,
        id: ProjectId,
        key: &str,
    ) -> Result<Option<String>, Error> {
        let project = handle.state::<projects::Controller>().get(id)?;
        Ok(project.get_local_config(key)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_set_local_config(
        handle: tauri::AppHandle,
        id: ProjectId,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(id)?;
        project.set_local_config(key, value).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn check_signing_settings(
        handle: tauri::AppHandle,
        id: ProjectId,
    ) -> Result<bool, Error> {
        let project = handle.state::<projects::Controller>().get(id)?;
        project.check_signing_settings().map_err(Into::into)
    }
}
