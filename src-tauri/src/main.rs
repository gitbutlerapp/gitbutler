mod crdt;
mod delta_watchers;
mod fs;
mod projects;
mod storage;

use crdt::Delta;
use fs::list_files;
use git2::Repository;
use log;
use projects::Project;
use std::collections::HashMap;
use std::{fs::read_to_string, path::Path};
use storage::Storage;
use tauri::{InvokeError, Manager, Runtime, State, Window};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};

struct AppState {
    watchers: delta_watchers::WatcherCollection,
    projects_storage: projects::Storage,
}

#[tauri::command]
fn list_project_files(
    state: State<'_, AppState>,
    project_id: &str,
) -> Result<Vec<String>, InvokeError> {
    log::debug!("Listing project files for project: {}", project_id);
    if let Some(project) = state.projects_storage.get_project(project_id)? {
        let project_path = Path::new(&project.path);
        let repo = match Repository::open(project_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };
        let files = list_files(project_path);
        let meta_commit = delta_watchers::get_meta_commit(&repo);
        let tree = meta_commit.tree().unwrap();
        let non_ignored_files: Vec<String> = files
            .into_iter()
            .filter_map(|file| {
                let file_path = Path::new(&file);
                let relative_file_path = file_path.strip_prefix(project_path).unwrap();
                let relative_file_path = relative_file_path.to_str().unwrap();
                if let Ok(_object) = tree.get_path(Path::new(&relative_file_path)) {
                    Some(relative_file_path.to_string())
                } else {
                    None
                }
            })
            .collect();
        Ok(non_ignored_files)
    } else {
        Err("Project not found".into())
    }
}

#[tauri::command]
fn read_project_file(
    state: State<'_, AppState>,
    project_id: &str,
    file_path: &str,
) -> Result<Option<String>, InvokeError> {
    log::debug!(
        "Reading project file for project: {} and file: {}",
        project_id,
        file_path
    );
    if let Some(project) = state.projects_storage.get_project(project_id)? {
        let project_path = Path::new(&project.path);
        let repo = match Repository::open(project_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };
        let meta_commit = delta_watchers::get_meta_commit(&repo);
        let tree = meta_commit.tree().unwrap();
        if let Ok(object) = tree.get_path(Path::new(&file_path)) {
            let blob = object.to_object(&repo).unwrap().into_blob().unwrap();
            let contents = String::from_utf8(blob.content().to_vec()).unwrap();
            Ok(Some(contents))
        } else {
            Ok(None)
        }
    } else {
        Err("Project not found".into())
    }
}

#[tauri::command]
fn add_project<R: Runtime>(
    window: Window<R>,
    state: State<'_, AppState>,
    path: &str,
) -> Result<Project, InvokeError> {
    log::debug!("Adding project from path: {}", path);
    for project in state.projects_storage.list_projects()? {
        if project.path == path {
            return Err("Project already exists".into());
        }
    }

    let project = projects::Project::from_path(path.to_string());
    if project.is_ok() {
        let project = project.unwrap();
        state.projects_storage.add_project(&project)?;
        delta_watchers::watch(window, &state.watchers, project.clone())?;
        return Ok(project);
    } else {
        return Err(project.err().unwrap().into());
    }
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, InvokeError> {
    log::debug!("Listing projects");
    state.projects_storage.list_projects().map_err(|e| e.into())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: &str) -> Result<(), InvokeError> {
    log::debug!("Deleting project with id: {}", id);
    if let Some(project) = state.projects_storage.get_project(id)? {
        delta_watchers::unwatch(&state.watchers, project)
    }
    state
        .projects_storage
        .delete_project(id)
        .map_err(|e| e.into())
}

#[tauri::command]
fn list_deltas(
    state: State<'_, AppState>,
    project_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, InvokeError> {
    log::debug!("Listing deltas for project with id: {}", project_id);
    if let Some(project) = state.projects_storage.get_project(project_id)? {
        Ok(project.list_deltas())
    } else {
        Err("Project not found".into())
    }
}

fn main() {
    let colors = ColoredLevelConfig {
        error: Color::Red,
        warn: Color::Yellow,
        debug: Color::Blue,
        info: Color::BrightGreen,
        trace: Color::Cyan,
    };

    tauri::Builder::default()
        .setup(move |app| {
            let resolver = app.path_resolver();
            let storage = Storage::new(&resolver);
            let projects_storage = projects::Storage::new(storage);

            let projects = projects_storage.list_projects()?;
            let watchers = delta_watchers::WatcherCollection::default();

            for project in projects {
                delta_watchers::watch(app.get_window("main").unwrap(), &watchers, project)?;
            }

            app.manage(AppState {
                watchers,
                projects_storage,
            });

            Ok(())
        })
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Debug)
                .with_colors(colors)
                .targets([LogTarget::LogDir, LogTarget::Stdout, LogTarget::Webview])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            read_project_file,
            list_project_files,
            add_project,
            list_projects,
            delete_project,
            list_deltas
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
