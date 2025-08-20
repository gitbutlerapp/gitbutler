use anyhow::{bail, Context};
use but_api::commands::projects::{
    self, AddProjectParams, DeleteProjectParams, GetProjectParams, UpdateProjectParams,
};
use but_api::error::Error;
use gitbutler_command_context::CommandContext;
use gitbutler_project::{Project, ProjectId};
use gix::bstr::ByteSlice;
use std::collections::BTreeSet;
use std::path;
use std::path::{Path, PathBuf};
use tauri::{State, Window};
use tracing::instrument;

use crate::window::state::ProjectAccessMode;
use crate::{window, WindowState};

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_project(
    app: State<'_, but_api::App>,
    project: gitbutler_project::UpdateRequest,
) -> Result<gitbutler_project::Project, Error> {
    projects::update_project(&app, UpdateProjectParams { project })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn add_project(
    app: State<'_, but_api::App>,
    path: &path::Path,
) -> Result<gitbutler_project::Project, Error> {
    projects::add_project(
        &app,
        AddProjectParams {
            path: path.to_path_buf(),
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn add_project_with_trust(
    app: State<'_, but_api::App>,
    path: &path::Path,
) -> Result<gitbutler_project::Project, Error> {
    projects::add_project_with_trust(
        &app,
        AddProjectParams {
            path: path.to_path_buf(),
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_project(
    app: State<'_, but_api::App>,
    project_id: ProjectId,
    no_validation: Option<bool>,
) -> Result<gitbutler_project::Project, Error> {
    projects::get_project(
        &app,
        GetProjectParams {
            project_id,
            no_validation,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(window_state), err(Debug))]
pub fn list_projects(
    window_state: State<'_, WindowState>,
) -> Result<Vec<ProjectForFrontend>, Error> {
    let open_projects = window_state.open_projects();
    gitbutler_project::assure_app_can_startup_or_fix_it(gitbutler_project::list())
        .map_err(Into::into)
        .map(|projects| {
            projects
                .into_iter()
                .map(|project| ProjectForFrontend {
                    is_open: open_projects.contains(&project.id),
                    inner: project,
                })
                .collect()
        })
}

/// Additional information to help the user interface communicate what happened with the project.
#[derive(Debug, serde::Serialize)]
pub struct ProjectInfo {
    /// `true` if the window is the first one to open the project.
    is_exclusive: bool,
    /// The display version of the error that communicates what went wrong while opening the database.
    db_error: Option<String>,
    /// Provide information about the project just opened.
    headsup: Option<String>,
}

/// This trigger is the GUI telling us that the project with `id` is now displayed.
/// Return `true` if the project is opened exclusively, i.e. there is no other Window looking at it.
///
/// We use it to start watching for filesystem events.
#[tauri::command(async)]
#[instrument(skip(window_state, window, app), err(Debug), ret)]
pub fn set_project_active(
    window_state: State<'_, WindowState>,
    app: tauri::State<'_, but_api::App>,
    window: Window,
    id: ProjectId,
) -> Result<Option<ProjectInfo>, Error> {
    let project = match gitbutler_project::get_validated(id).ok() {
        Some(project) => project,
        None => {
            tracing::warn!("Project with ID {id} not found, cannot set it active");
            return Ok(None);
        }
    };
    let ctx = &mut CommandContext::open(&project, app.app_settings.get()?.clone())?;
    let mode = window_state.set_project_to_window(
        window.label(),
        &project,
        (*app.app_settings).clone(),
        ctx,
    )?;
    let db_error = assure_database_valid(project.gb_dir())?;
    let filter_error = warn_about_filters_and_git_lfs(ctx.gix_repo_local_only()?)?;
    for err in [&db_error, &filter_error] {
        if let Some(err) = &err {
            tracing::error!("{err}");
        }
    }
    let is_exclusive = match mode {
        ProjectAccessMode::First => true,
        ProjectAccessMode::Shared => false,
    };
    Ok(Some(ProjectInfo {
        is_exclusive,
        db_error,
        headsup: filter_error,
    }))
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
    window::create(&handle, &label, id.to_string()).map_err(anyhow::Error::from)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn delete_project(app: State<'_, but_api::App>, project_id: ProjectId) -> Result<(), Error> {
    projects::delete_project(&app, DeleteProjectParams { project_id })
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct ProjectForFrontend {
    #[serde(flatten)]
    pub inner: Project,
    /// Tell if the project is known to be open in a Window in the frontend.
    pub is_open: bool,
}

/// Fatal errors are returned as error, fixed errors for tracing will be `Some(err)`
fn assure_database_valid(gb_dir: PathBuf) -> anyhow::Result<Option<String>> {
    if let Err(err) = but_db::DbHandle::new_in_directory(&gb_dir) {
        let db_path = but_db::DbHandle::db_file_path(&gb_dir);
        let db_filename = db_path.file_name().unwrap();
        let max_attempts = 255;
        for round in 1..max_attempts {
            let backup_path = gb_dir.join(format!(
                "{db_name}.maybe-broken-{round:02}",
                db_name = Path::new(db_filename).display()
            ));
            if backup_path.is_file() {
                continue;
            }

            if let Err(err) = std::fs::rename(&db_path, &backup_path) {
                bail!(
                    "Failed to rename {} to {} - application may fail to startup: {err}",
                    db_path.display(),
                    backup_path.display()
                );
            }

            return Ok(Some(format!(
                "Could not open db file at '{}'.\nIt was moved to {} for recovery. \n\nError was: {err}",
                db_path.display(),
                backup_path.display()
            )));
        }
        bail!("Database file at '{db_path} has {max_attempts} corrupted copies - giving up, application probably won't work", db_path = db_path.display());
    }
    Ok(None)
}

/// Return an error message that
fn warn_about_filters_and_git_lfs(repo: gix::Repository) -> anyhow::Result<Option<String>> {
    let index = repo.index_or_empty()?;
    let mut cache = repo.attributes_only(
        &index,
        gix::worktree::stack::state::attributes::Source::WorktreeThenIdMapping,
    )?;
    let mut attrs = cache.selected_attribute_matches(Some("filter"));
    let mut all_filters = BTreeSet::<String>::new();
    let mut files_with_filter = Vec::new();
    for entry in index.entries() {
        let cache_entry = cache.at_entry(entry.path(&index), None)?;
        if cache_entry.matching_attributes(&mut attrs) {
            let mut added = false;
            all_filters.extend(attrs.iter().filter_map(|attr| {
                attr.assignment.state.as_bstr().map(|s| {
                    if !added {
                        files_with_filter.push(entry.path(&index).to_str_lossy());
                        added = true;
                    }
                    s.to_string()
                })
            }));
        }
    }

    if all_filters.is_empty() {
        return Ok(None);
    }

    let has_lfs = all_filters.contains("lfs");
    let mut msg = format!(
        "Worktree filter(s) detected: {comma_separated}\n\
Filters will silently not be applied during workspace operations to the files listed below.\n\
Ensure these aren't touched by GitButler or avoid using it in this repository.",
        comma_separated = Vec::from_iter(all_filters).join(", ")
    );
    if has_lfs {
        msg.push_str(
            r#"

`git lfs pull --include="*" to restore git-lfs files.` can be used to restore git-lfs files after GitButler touched them."#,
        );
    }
    let max_files = 10;
    msg.push_str("\n\n");
    msg.push_str(&files_with_filter[..files_with_filter.len().min(max_files)].join("\n"));
    if files_with_filter.len() > max_files {
        msg.push_str(&format!(
            "\n[and {} more]",
            files_with_filter.len() - max_files
        ));
    }
    Ok(Some(msg))
}
