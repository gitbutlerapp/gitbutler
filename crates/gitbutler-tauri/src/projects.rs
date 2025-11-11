use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{WindowState, window, window::state::ProjectAccessMode};
use anyhow::{Context, bail};
use but_api::error::Error;
use but_settings::{AppSettings, AppSettingsWithDiskSync};
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gix::bstr::ByteSlice;
use tauri::{State, Window};
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(window_state), err(Debug))]
pub fn list_projects(
    window_state: State<'_, WindowState>,
) -> Result<Vec<but_api::projects::ProjectForFrontend>, Error> {
    let open_projects = window_state.open_projects();
    but_api::projects::list_projects(open_projects)
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
) -> Result<Option<ProjectInfo>, Error> {
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
    let ctx = &mut CommandContext::open_from(
        &project,
        AppSettings::load_from_default_path_creating()?,
        repo,
    )?;
    // --> WARNING <-- Be sure this runs BEFORE the database on `ctx` is used.

    reconcile_in_workspace_state_of_vb_toml(ctx);

    let db_error = assure_database_valid(project.gb_dir())?;
    let filter_error = warn_about_filters_and_git_lfs(ctx.gix_repo_local_only()?)?;
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

/// Validate and fix workspace stack `in_workspace` status of `virtual_branches.toml`
/// so they match what's actually in the workspace.
/// If there is a change, the data is written back.
///
/// Errors are silently ignored to allow the application to continue loading even if
/// the migration fails - the workspace will still be functional, just potentially
/// with stale metadata that can confuse 'old' code.
///
/// NOTE: This isn't needed for new code - it won't base any decisions on the metadata.
fn reconcile_in_workspace_state_of_vb_toml(ctx: &mut CommandContext) -> Option<()> {
    let mut guard = ctx.project().exclusive_worktree_access();
    let perm = guard.write_permission();
    let (_repo, mut meta, graph) = ctx
        .graph_and_meta_mut_and_repo_from_reference(
            "refs/heads/gitbutler/workspace"
                .try_into()
                .expect("statically known to be valid"),
            perm,
        )
        .ok()?;
    let ws = graph.to_workspace().ok()?;

    let mut seen = BTreeSet::new();
    for in_workspace_stack_id in ws.stacks.iter().filter_map(|s| s.id) {
        seen.insert(in_workspace_stack_id);
        let Some(vb_stack) = meta.data_mut().branches.get_mut(&in_workspace_stack_id) else {
            continue;
        };

        if !vb_stack.in_workspace {
            tracing::warn!(
                "Fixing stale metadata of stack {in_workspace_stack_id} to be considered inside the workspace",
            );
            vb_stack.in_workspace = true;
            meta.set_changed_to_necessitate_write();
        }
    }

    let stack_ids_to_put_in_workspace: Vec<_> = meta
        .data()
        .branches
        .keys()
        .filter(|stack_id| !seen.contains(stack_id))
        .copied()
        .collect();
    for stack_id_not_in_workspace in stack_ids_to_put_in_workspace {
        let vb_stack = meta
            .data_mut()
            .branches
            .get_mut(&stack_id_not_in_workspace)
            .expect("BUG: we just traversed this stack-id");
        if vb_stack.in_workspace {
            tracing::warn!(
                "Fixing stale metadata of stack {stack_id_not_in_workspace} to be considered outside the workspace",
            );
            vb_stack.in_workspace = false;
            meta.set_changed_to_necessitate_write();
        }
    }
    None
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
        bail!(
            "Database file at '{db_path} has {max_attempts} corrupted copies - giving up, application probably won't work",
            db_path = db_path.display()
        );
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
