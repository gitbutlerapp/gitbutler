mod deltas;
mod fs;
mod projects;
mod sessions;
mod storage;
mod watchers;

use deltas::Delta;
use fs::list_files;
use git2::Repository;
use log;
use projects::Project;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use storage::Storage;
use tauri::{Manager, Runtime, State, Window};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use watchers::{get_latest_file_contents, WatcherCollection};

struct AppState {
    watchers: WatcherCollection,
    projects_storage: projects::Storage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub message: String,
}

#[tauri::command]
fn list_project_files(state: State<'_, AppState>, project_id: &str) -> Result<Vec<String>, Error> {
    if let Some(project) = state
        .projects_storage
        .get_project(project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to get project".to_string(),
            }
        })?
    {
        let project_path = Path::new(&project.path);
        let repo = match Repository::open(project_path) {
            Ok(repo) => repo,
            Err(e) => panic!("failed to open: {}", e),
        };
        let files = list_files(project_path).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to list files".to_string(),
            }
        })?;
        let non_ignored_files: Vec<String> = files
            .into_iter()
            .filter_map(
                |file_path| match get_latest_file_contents(&repo, Path::new(&file_path)) {
                    Ok(Some(_)) => Some(file_path),
                    Ok(None) => None,
                    Err(e) => {
                        log::error!("{}", e);
                        None
                    }
                },
            )
            .collect();
        Ok(non_ignored_files)
    } else {
        Err(Error {
            message: "Project not found".to_string(),
        })
    }
}

#[tauri::command]
fn read_project_file(
    state: State<'_, AppState>,
    project_id: &str,
    file_path: &str,
) -> Result<Option<String>, Error> {
    if let Some(project) = state
        .projects_storage
        .get_project(project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to get project".to_string(),
            }
        })?
    {
        let project_path = Path::new(&project.path);
        let repo = Repository::open(project_path).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to open project".to_string(),
            }
        })?;

        let contents = get_latest_file_contents(&repo, Path::new(file_path)).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to read file".to_string(),
            }
        })?;

        Ok(contents)
    } else {
        Err(Error {
            message: "Project not found".to_string(),
        })
    }
}

#[tauri::command]
fn add_project<R: Runtime>(
    window: Window<R>,
    state: State<'_, AppState>,
    path: &str,
) -> Result<Project, Error> {
    for project in state.projects_storage.list_projects().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list projects".to_string(),
        }
    })? {
        if project.path == path {
            return Err(Error {
                message: "Project already exists".to_string(),
            });
        }
    }

    let project = projects::Project::from_path(path.to_string());
    if project.is_ok() {
        let project = project.unwrap();
        state.projects_storage.add_project(&project).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to add project".to_string(),
            }
        })?;
        watchers::watch(window, &state.watchers, &project).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to watch project".to_string(),
            }
        })?;
        return Ok(project);
    } else {
        return Err(Error {
            message: "Failed to add project".to_string(),
        });
    }
}

#[tauri::command]
fn list_projects(state: State<'_, AppState>) -> Result<Vec<Project>, Error> {
    state.projects_storage.list_projects().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list projects".to_string(),
        }
    })
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: &str) -> Result<(), Error> {
    if let Some(project) = state.projects_storage.get_project(id).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to get project".to_string(),
        }
    })? {
        watchers::unwatch(&state.watchers, project).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to unwatch project".to_string(),
            }
        })?;
    }
    state.projects_storage.delete_project(id).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to delete project".to_string(),
        }
    })?;

    Ok(())
}

#[tauri::command]
fn list_deltas(
    state: State<'_, AppState>,
    project_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    if let Some(project) = state
        .projects_storage
        .get_project(project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to get project".to_string(),
            }
        })?
    {
        let project_path = Path::new(&project.path);
        let deltas = deltas::list_current_deltas(project_path).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to list deltas".to_string(),
            }
        })?;
        Ok(deltas)
    } else {
        Err(Error {
            message: "Project not found".to_string(),
        })
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

    sentry_tauri::init(
        sentry::release_name!(),
        |_| {
            sentry::init((
                "https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032",
                sentry::ClientOptions {
                    release: sentry::release_name!(),
                    ..Default::default()
                },
            ))
        },
        |sentry_plugin| {
            tauri::Builder::default()
                .setup(move |app| {
                    let resolver = app.path_resolver();
                    let storage = Storage::new(&resolver);
                    let projects_storage = projects::Storage::new(storage);

                    let watchers = watchers::WatcherCollection::default();

                    if let Ok(projects) = projects_storage.list_projects() {
                        for project in projects {
                            watchers::watch(app.get_window("main").unwrap(), &watchers, &project)
                                .map_err(|e| e.to_string())?;
                        }
                    } else {
                        log::error!("Failed to list projects");
                    }

                    app.manage(AppState {
                        watchers,
                        projects_storage,
                    });

                    Ok(())
                })
                .plugin(sentry_plugin)
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
                .expect("error while running tauri application")
        },
    );
}
