mod crdt;
mod delta_watchers;
mod fs;
mod projects;
mod storage;

use crdt::Delta;
use delta_watchers::watch;
use fs::list_files;
use log;
use projects::Project;
use std::collections::HashMap;
use std::thread;
use std::{fs::read_to_string, path::Path};
use storage::Storage;
use tauri::{InvokeError, Manager, State};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};

struct AppState {
    projects_storage: projects::Storage,
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
    let contents = read_to_string(file_path);
    if contents.is_ok() {
        return Ok(contents.unwrap());
    } else {
        return Err(contents.err().unwrap().to_string().into());
    }
}

#[tauri::command]
fn add_project(state: State<'_, AppState>, path: &str) -> Result<Project, InvokeError> {
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
        watch_project(&project);
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
    if let Some(project) = state.projects_storage.get_project(project_id)? {
        Ok(project.list_deltas())
    } else {
        Err("Project not found".into())
    }
}

fn watch_project(project: &Project) {
    log::info!("Watching project: {}", project.path);

    let project = project.clone();
    thread::spawn(move || {
        futures::executor::block_on(async {
            // TODO: figure out how to stop wathchers when project is deleted
            if let Err(e) = watch(&project).await {
                log::error!("Failed to watch project {}: {:?}", project.path, e)
            }
        });
    });
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
            for project in projects {
                watch_project(&project);
            }

            app.manage(AppState { projects_storage });

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
            delete_project,
            list_deltas
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
