mod crdt;
mod projects;
mod storage;
mod watch;

use crdt::TextDocument;
use log;
use projects::Project;
use std::{fs, path::Path};
use storage::Storage;
use tauri::{InvokeError, Manager, State};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use watch::Watchers;

struct AppState {
    watchers: Watchers,
    projects_storage: projects::Storage,
}

// return a list of files in directory recursively
fn list_files(path: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if path.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                files.append(&mut list_files(&path));
            } else {
                files.push(path.to_str().unwrap().to_string());
            }
        }
    }
    files.sort();
    files
}

// returns a list of files in directory recursively
#[tauri::command]
fn read_dir(path: &str) -> Result<Vec<String>, InvokeError> {
    let path = Path::new(path);
    if path.is_dir() {
        let files = list_files(path);
        return Ok(files);
    } else {
        return Err("Path is not a directory".into());
    }
}

// reads file contents and returns it
#[tauri::command]
fn read_file(file_path: &str) -> Result<String, InvokeError> {
    let contents = fs::read_to_string(file_path);
    if contents.is_ok() {
        return Ok(contents.unwrap());
    } else {
        return Err(contents.err().unwrap().to_string().into());
    }
}

#[tauri::command]
fn add_project(state: State<'_, AppState>, path: &str) -> Result<Project, InvokeError> {
    for project in state.projects_storage.list_projects()? {
        if project.path == path {
            return Err("Project already exists".into());
        }
    }

    let project = projects::Project::from_path(path.to_string());
    if project.is_ok() {
        let project = project.unwrap();
        state.projects_storage.add_project(&project)?;
        return Ok(project);
    } else {
        return Err(project.err().unwrap().into());
    }
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, InvokeError> {
    state.projects_storage.list_projects().map_err(|e| e.into())
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: &str) -> Result<(), InvokeError> {
    let project = state.projects_storage.get_project(id)?;
    if project.is_some() {
        let project = project.unwrap();
        if state.watchers.unwatch(&project).is_err() {
            log::error!("Failed to unwatch project: {}", project.path)
        }
    }
    state
        .projects_storage
        .delete_project(id)
        .map_err(|e| e.into())
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
            let watchers = Watchers::new();

            let projects = projects_storage.list_projects()?;
            for project in projects {
                log::info!("Watching project: {}", project.path);
                if watchers
                    .watch(&project, |_project_id: &str, _event: notify::Event| {
                        log::info!("Project changed: {}", _project_id);
                    })
                    .is_err()
                {
                    log::error!("Failed to watch project: {}", project.path)
                }
            }

            app.manage(AppState {
                watchers,
                projects_storage,
            });
            Ok(())
        })
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin(tauri_plugin_fs_watch::init())
        .plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Debug)
                .with_colors(colors)
                .targets([LogTarget::LogDir, LogTarget::Stdout, LogTarget::Webview])
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            read_file,
            read_dir,
            add_project,
            list_projects,
            delete_project
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
