pub mod commands {
    use anyhow::Context;
    use std::path;

    use gitbutler_project::ProjectId;
    use gitbutler_project::{self as projects, Controller};
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;
    use crate::window::WindowState;

    #[tauri::command(async)]
    #[instrument(skip(controller), err(Debug))]
    pub async fn update_project(
        controller: State<'_, Controller>,
        project: projects::UpdateRequest,
    ) -> Result<projects::Project, Error> {
        Ok(controller.update(&project).await?)
    }

    #[tauri::command(async)]
    #[instrument(skip(controller), err(Debug))]
    pub async fn add_project(
        controller: State<'_, Controller>,
        path: &path::Path,
    ) -> Result<projects::Project, Error> {
        Ok(controller.add(path)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(controller), err(Debug))]
    pub async fn get_project(
        controller: State<'_, Controller>,
        id: ProjectId,
    ) -> Result<projects::Project, Error> {
        Ok(controller.get(id)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(controller), err(Debug))]
    pub async fn list_projects(
        controller: State<'_, Controller>,
    ) -> Result<Vec<projects::Project>, Error> {
        controller.list().map_err(Into::into)
    }

    /// This trigger is the GUI telling us that the project with `id` is now displayed.
    ///
    /// We use it to start watching for filesystem events.
    #[tauri::command(async)]
    #[instrument(skip(controller, watchers), err(Debug))]
    pub async fn set_project_active(
        controller: State<'_, Controller>,
        watchers: State<'_, WindowState>,
        id: ProjectId,
    ) -> Result<(), Error> {
        let project = controller.get(id).context("project not found")?;
        Ok(watchers.watch(&project)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(controller), err(Debug))]
    pub async fn delete_project(
        controller: State<'_, Controller>,
        id: ProjectId,
    ) -> Result<(), Error> {
        controller.delete(id).await.map_err(Into::into)
    }
}
