use gitbutler_project::Project;

pub mod commands {
    use anyhow::Context;
    use std::path;

    use gitbutler_project::ProjectId;
    use gitbutler_project::{self as projects, Controller};
    use tauri::{State, Window};
    use tracing::instrument;

    use crate::error::Error;
    use crate::projects::ProjectForFrontend;
    use crate::{window, WindowState};

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
    #[instrument(skip(controller, window_state), err(Debug))]
    pub async fn list_projects(
        window_state: State<'_, WindowState>,
        controller: State<'_, Controller>,
    ) -> Result<Vec<ProjectForFrontend>, Error> {
        let open_projects = window_state.open_projects();
        controller.list().map_err(Into::into).map(|projects| {
            projects
                .into_iter()
                .map(|project| ProjectForFrontend {
                    is_open: open_projects.contains(&project.id),
                    inner: project,
                })
                .collect()
        })
    }

    /// This trigger is the GUI telling us that the project with `id` is now displayed.
    ///
    /// We use it to start watching for filesystem events.
    #[tauri::command(async)]
    #[instrument(skip(controller, window_state, window), err(Debug))]
    pub async fn set_project_active(
        controller: State<'_, Controller>,
        window_state: State<'_, WindowState>,
        window: Window,
        id: ProjectId,
    ) -> Result<(), Error> {
        let project = controller.get(id).context("project not found")?;
        Ok(window_state.set_project_to_window(window.label(), &project)?)
    }

    /// Open the project with the given ID in a new Window, or focus an existing one.
    ///
    /// Note that this command is blocking the main thread just to prevent the chance for races
    /// without haveing to lock explicitly.
    #[tauri::command]
    #[instrument(skip(handle), err(Debug))]
    pub async fn open_project_in_window(
        handle: tauri::AppHandle,
        id: ProjectId,
    ) -> Result<(), Error> {
        let label = std::time::UNIX_EPOCH
            .elapsed()
            .or_else(|_| std::time::UNIX_EPOCH.duration_since(std::time::SystemTime::now()))
            .map(|d| d.as_millis().to_string())
            .context("didn't manage to get any time-based unique ID")?;
        window::create(&handle, &label, format!("{id}/board")).map_err(anyhow::Error::from)?;
        Ok(())
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

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}
