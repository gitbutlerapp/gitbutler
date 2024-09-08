use gitbutler_project::Project;

pub mod commands {
    use std::path;

    use anyhow::Context;
    use gitbutler_fs::list_files;
    use gitbutler_project::{self as projects, Controller, ProjectId};
    use tauri::{State, Window};
    use tracing::instrument;

    use crate::{error::Error, projects::ProjectForFrontend, window, WindowState};

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn update_project(
        projects: State<'_, Controller>,
        project: projects::UpdateRequest,
    ) -> Result<projects::Project, Error> {
        Ok(projects.update(&project)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn add_project(
        projects: State<'_, Controller>,
        path: &path::Path,
    ) -> Result<projects::Project, Error> {
        Ok(projects.add(path)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn get_project(
        projects: State<'_, Controller>,
        id: ProjectId,
        no_validation: Option<bool>,
    ) -> Result<projects::Project, Error> {
        if no_validation.unwrap_or(false) {
            Ok(projects.get_raw(id)?)
        } else {
            Ok(projects.get_validated(id)?)
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, window_state), err(Debug))]
    pub fn list_projects(
        window_state: State<'_, WindowState>,
        projects: State<'_, Controller>,
    ) -> Result<Vec<ProjectForFrontend>, Error> {
        let open_projects = window_state.open_projects();
        projects.list().map_err(Into::into).map(|projects| {
            projects
                .into_iter()
                .map(|project| ProjectForFrontend {
                    is_open: open_projects.contains(&project.id),
                    inner: project,
                })
                .collect()
        })
    }

    #[tauri::command(async)]
    // NOTE: Do I need this instrument macro?
    pub fn get_available_pull_request_templates(
        path: &path::Path,
    ) -> Result<Vec<path::PathBuf>, Error> {
        let walked_paths = list_files(path, &[&path])?;
        println!("WalkedPaths: {:#?}", walked_paths);

        let mut available_paths = Vec::new();
        for entry in walked_paths {
            let path = entry.as_path();
            let path_str = path.to_string_lossy();
            if path_str == "PULL_REQUEST_TEMPLATE.md"
                || path_str == "pull_request_template.md"
                || path_str.contains("PULL_REQUEST_TEMPLATE/")
            {
                available_paths.push(path.to_path_buf());
            }
        }

        Ok(available_paths)
    }

    /// This trigger is the GUI telling us that the project with `id` is now displayed.
    ///
    /// We use it to start watching for filesystem events.
    #[tauri::command(async)]
    #[instrument(skip(projects, window_state, window), err(Debug))]
    pub fn set_project_active(
        projects: State<'_, Controller>,
        window_state: State<'_, WindowState>,
        window: Window,
        id: ProjectId,
    ) -> Result<(), Error> {
        let project = projects.get_validated(id).context("project not found")?;
        Ok(window_state.set_project_to_window(window.label(), &project)?)
    }

    /// Open the project with the given ID in a new Window, or focus an existing one.
    ///
    /// Note that this command is blocking the main thread just to prevent the chance for races
    /// without haveing to lock explicitly.
    #[tauri::command]
    #[instrument(skip(handle), err(Debug))]
    pub fn open_project_in_window(handle: tauri::AppHandle, id: ProjectId) -> Result<(), Error> {
        let label = std::time::UNIX_EPOCH
            .elapsed()
            .or_else(|_| std::time::UNIX_EPOCH.duration_since(std::time::SystemTime::now()))
            .map(|d| d.as_millis().to_string())
            .context("didn't manage to get any time-based unique ID")?;
        window::create(&handle, &label, format!("{id}/board")).map_err(anyhow::Error::from)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn delete_project(projects: State<'_, Controller>, id: ProjectId) -> Result<(), Error> {
        projects.delete(id).map_err(Into::into)
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}
