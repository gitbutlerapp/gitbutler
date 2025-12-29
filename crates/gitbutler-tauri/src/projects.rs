use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, bail};
use but_api::json;
use but_ctx::Context;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_project::ProjectId;
use gix::bstr::ByteSlice;
use tauri::{State, Window};
use tracing::instrument;

use crate::{WindowState, window, window::state::ProjectAccessMode};

#[tauri::command(async)]
#[instrument(skip(window_state), err(Debug))]
pub fn list_projects(
    window_state: State<'_, WindowState>,
) -> Result<Vec<but_api::legacy::projects::ProjectForFrontend>, json::Error> {
    let open_projects = window_state.open_projects();
    but_api::legacy::projects::list_projects(open_projects).map_err(Into::into)
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
#[instrument(skip(window_state, window, app_settings_sync), err(Debug), ret)]
pub fn set_project_active(
    window_state: State<'_, WindowState>,
    app_settings_sync: tauri::State<'_, AppSettingsWithDiskSync>,
    window: Window,
    id: ProjectId,
) -> Result<Option<ProjectInfo>, json::Error> {
    let project = match gitbutler_project::get_validated(id).ok() {
        Some(project) => project,
        None => {
            tracing::warn!("Project with ID {id} not found, cannot set it active");
            return Ok(None);
        }
    };
    let repo = git2::Repository::open(project.git_dir())
        // Only capture this information here to prevent spawning too many errors because of this
        // (the UI has many parallel calls in flight).
        .map_err(|err| {
            let code = err.code();
            let err = anyhow::Error::from(err);
            if code == git2::ErrorCode::Owner {
                err.context(but_error::Code::RepoOwnership)
            } else {
                err
            }
        })?;
    let ctx = &mut Context::new_from_legacy_project(project.clone())?.with_git2_repo(repo);
    // --> WARNING <-- Be sure this runs BEFORE the database on `ctx` is used.

    {
        let mut guard = ctx.exclusive_worktree_access();
        but_api::legacy::meta::reconcile_in_workspace_state_of_vb_toml(
            ctx,
            guard.write_permission(),
        )
        .ok();
    }

    let db_error = assure_database_valid(ctx.project_data_dir())?;
    let filter_error = warn_about_filters_and_git_lfs(&*ctx.repo.get()?)?;
    for err in [&db_error, &filter_error] {
        if let Some(err) = &err {
            tracing::error!("{err}");
        }
    }
    let mode =
        window_state.set_project_to_window(window.label(), &project, &app_settings_sync, ctx)?;
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
/// without having to lock explicitly.
#[tauri::command]
#[instrument(skip(handle), err(Debug))]
pub fn open_project_in_window(handle: tauri::AppHandle, id: ProjectId) -> Result<(), json::Error> {
    let label = std::time::UNIX_EPOCH
        .elapsed()
        .or_else(|_| std::time::UNIX_EPOCH.duration_since(std::time::SystemTime::now()))
        .map(|d| d.as_millis().to_string())
        .context("didn't manage to get any time-based unique ID")?;
    window::create(&handle, &label, id.to_string()).map_err(anyhow::Error::from)?;
    Ok(())
}

/// Fatal errors are returned as error, fixed errors for tracing will be `Some(err)`
#[instrument(level = tracing::Level::DEBUG)]
fn assure_database_valid(data_dir: PathBuf) -> anyhow::Result<Option<String>> {
    if let Err(err) = but_db::DbHandle::new_in_directory(&data_dir) {
        let db_path = but_db::DbHandle::db_file_path(&data_dir);
        let db_filename = db_path.file_name().unwrap();
        let max_attempts = 255;
        for round in 1..max_attempts {
            let backup_path = data_dir.join(format!(
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
        bail!(
            "Database file at '{db_path} has {max_attempts} corrupted copies - giving up, application probably won't work",
            db_path = db_path.display()
        );
    }
    Ok(None)
}

/// Return an error message that
fn warn_about_filters_and_git_lfs(repo: &gix::Repository) -> anyhow::Result<Option<String>> {
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

`git lfs pull --include="*"` can be used to restore git-lfs files after GitButler touched them."#,
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
