pub mod commands {
    use anyhow::Context;
    use std::path;

    use gitbutler_core::error;
    use gitbutler_core::error::Code;
    use gitbutler_core::projects::{self, controller::Controller, ProjectId};
    use tauri::Manager;
    use tracing::instrument;

    use crate::error::Error;
    use crate::watcher::Watchers;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn update_project(
        handle: tauri::AppHandle,
        project: projects::UpdateRequest,
    ) -> Result<projects::Project, Error> {
        handle
            .state::<Controller>()
            .update(&project)
            .await
            .map_err(Error::from_error_with_context)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn add_project(
        handle: tauri::AppHandle,
        path: &path::Path,
    ) -> Result<projects::Project, Error> {
        handle
            .state::<Controller>()
            .add(path)
            .map_err(Error::from_error_with_context)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_project(
        handle: tauri::AppHandle,
        id: &str,
    ) -> Result<projects::Project, Error> {
        let id = id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<Controller>()
            .get(&id)
            .map_err(Error::from_error_with_context)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
        handle.state::<Controller>().list().map_err(Into::into)
    }

    /// This trigger is the GUI telling us that the project with `id` is now displayed.
    ///
    /// We use it to start watching for filesystem events.
    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn set_project_active(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
        let id: ProjectId = id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        let project = handle
            .state::<Controller>()
            .get(&id)
            .context("project not found")?;
        Ok(handle.state::<Watchers>().watch(&project)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
        let id = id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        handle
            .state::<Controller>()
            .delete(&id)
            .await
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_get_local_config(
        handle: tauri::AppHandle,
        id: &str,
        key: &str,
    ) -> Result<Option<String>, Error> {
        let id = id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        Ok(handle
            .state::<Controller>()
            .get_local_config(&id, key)
            .context(Code::Projects)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn git_set_local_config(
        handle: tauri::AppHandle,
        id: &str,
        key: &str,
        value: &str,
    ) -> Result<(), Error> {
        let id = id.parse().context(error::Context::new_static(
            Code::Validation,
            "Malformed project id",
        ))?;
        Ok(handle
            .state::<Controller>()
            .set_local_config(&id, key, value)
            .context(Code::Projects)?)
    }
}
