mod butler;
mod deltas;
mod events;
mod fs;
mod projects;
mod sessions;
mod storage;
mod users;
mod watchers;

use deltas::Delta;
use git2::Repository;
use log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use storage::Storage;
use tauri::{Manager, Runtime, State, Window};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use watchers::WatcherCollection;

struct AppState {
    watchers: WatcherCollection,
    projects_storage: projects::Storage,
    users_storage: users::Storage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Error {
    pub message: String,
}

const IS_DEV: bool = cfg!(debug_assertions);

fn app_title() -> String {
    if IS_DEV {
        "GitButler (dev)".to_string()
    } else {
        "GitButler".to_string()
    }
}

#[tauri::command]
fn list_sessions(
    state: State<'_, AppState>,
    project_id: &str,
) -> Result<Vec<sessions::Session>, Error> {
    match state.projects_storage.get_project(project_id) {
        Ok(Some(project)) => {
            let repo = Repository::open(project.path).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to open project".to_string(),
                }
            })?;
            let sessions = sessions::list(&repo).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to list sessions".to_string(),
                }
            })?;
            Ok(sessions)
        }
        Ok(None) => Err(Error {
            message: "Project not found".to_string(),
        }),
        Err(e) => {
            log::error!("{}", e);
            Err(Error {
                message: "Failed to get project".to_string(),
            })
        }
    }
}

#[tauri::command]
fn get_user(state: State<'_, AppState>) -> Result<Option<users::User>, Error> {
    state.users_storage.get().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to get user".to_string(),
        }
    })
}

#[tauri::command]
fn set_user(state: State<'_, AppState>, user: users::User) -> Result<(), Error> {
    state.users_storage.set(&user).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to save user".to_string(),
        }
    })?;
    Ok(())
}

#[tauri::command]
fn delete_user(state: State<'_, AppState>) -> Result<(), Error> {
    state.users_storage.delete().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to delete user".to_string(),
        }
    })?;
    Ok(())
}

#[tauri::command]
fn update_project(
    state: State<'_, AppState>,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    state
        .projects_storage
        .update_project(&project)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to update project".to_string(),
            }
        })
}

#[tauri::command]
fn add_project<R: Runtime>(
    window: Window<R>,
    state: State<'_, AppState>,
    path: &str,
) -> Result<projects::Project, Error> {
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
fn list_projects(state: State<'_, AppState>) -> Result<Vec<projects::Project>, Error> {
    state.projects_storage.list_projects().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list projects".to_string(),
        }
    })
}

#[tauri::command]
fn delete_project(state: State<'_, AppState>, id: &str) -> Result<(), Error> {
    match state.projects_storage.get_project(id) {
        Ok(Some(project)) => {
            watchers::unwatch(&state.watchers, project).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to unwatch project".to_string(),
                }
            })?;

            state.projects_storage.delete_project(id).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to delete project".to_string(),
                }
            })?;

            Ok(())
        }
        Ok(None) => Ok(()),
        Err(e) => {
            log::error!("{}", e);
            Err(Error {
                message: "Failed to get project".to_string(),
            })
        }
    }
}

#[tauri::command]
fn list_session_files(
    state: State<'_, AppState>,
    project_id: &str,
    session_id: &str,
) -> Result<HashMap<String, String>, Error> {
    match state.projects_storage.get_project(project_id) {
        Ok(Some(project)) => {
            let repo = Repository::open(&project.path).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to open project".to_string(),
                }
            })?;

            let files = sessions::list_files(&repo, session_id).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to list files".to_string(),
                }
            })?;

            Ok(files)
        }
        Ok(None) => Err(Error {
            message: "Project not found".to_string(),
        }),
        Err(e) => {
            log::error!("{}", e);
            Err(Error {
                message: "Failed to get project".to_string(),
            })
        }
    }
}

#[tauri::command]
fn list_deltas(
    state: State<'_, AppState>,
    project_id: &str,
    session_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    match state.projects_storage.get_project(project_id) {
        Ok(Some(project)) => {
            let repo = Repository::open(&project.path).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to open project".to_string(),
                }
            })?;

            let deltas = deltas::list(&repo, session_id).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to list deltas".to_string(),
                }
            })?;

            Ok(deltas)
        }
        Ok(None) => Err(Error {
            message: "Project not found".to_string(),
        }),
        Err(e) => {
            log::error!("{}", e);
            Err(Error {
                message: "Failed to get project".to_string(),
            })
        }
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
            let quit = tauri::CustomMenuItem::new("quit".to_string(), "Quit");
            let hide =
                tauri::CustomMenuItem::new("toggle".to_string(), format!("Hide {}", app_title()));
            let tray_menu = tauri::SystemTrayMenu::new().add_item(hide).add_item(quit);
            let tray = tauri::SystemTray::new().with_menu(tray_menu);

            tauri::Builder::default()
                .system_tray(tray)
                .on_window_event(|event| match event.event() {
                    // Hide window instead of closing.
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        event.window().hide().unwrap();
                        event
                            .window()
                            .app_handle()
                            .tray_handle()
                            .get_item("toggle")
                            .set_title(format!("Show {}", app_title()))
                            .unwrap();
                    }
                    _ => {}
                })
                .on_system_tray_event(|app, event| match event {
                    tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
                        let item_handle = app.tray_handle().get_item(&id);
                        match id.as_str() {
                            "quit" => {
                                app.exit(0);
                            }
                            "toggle" => {
                                let main_window = app.get_window("main").unwrap();
                                if main_window.is_visible().unwrap() {
                                    main_window.hide().unwrap();
                                    item_handle
                                        .set_title(format!("Show {}", app_title()))
                                        .unwrap();
                                } else {
                                    main_window.show().unwrap();
                                    item_handle
                                        .set_title(format!("Hide {}", app_title()))
                                        .unwrap();
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                })
                .setup(move |app| {
                    let resolver = app.path_resolver();
                    let storage = Storage::new(&resolver);
                    let projects_storage = projects::Storage::new(storage.clone());
                    let users_storage = users::Storage::new(storage.clone());

                    let watchers = watchers::WatcherCollection::default();

                    if let Ok(projects) = projects_storage.list_projects() {
                        for project in projects {
                            watchers::watch(app.get_window("main").unwrap(), &watchers, &project)
                                .map_err(|e| e.to_string())?;
                        }
                    } else {
                        log::error!("Failed to list projects");
                    }

                    #[cfg(debug_assertions)]
                    app.get_window("main").unwrap().open_devtools();

                    app.manage(AppState {
                        watchers,
                        projects_storage,
                        users_storage,
                    });

                    Ok(())
                })
                .plugin(sentry_plugin)
                .plugin(tauri_plugin_window_state::Builder::default().build())
                .plugin(
                    tauri_plugin_log::Builder::default()
                        .level(match IS_DEV {
                            true => log::LevelFilter::Debug,
                            false => log::LevelFilter::Info,
                        })
                        .with_colors(colors)
                        .targets([LogTarget::LogDir, LogTarget::Stdout, LogTarget::Webview])
                        .build(),
                )
                .invoke_handler(tauri::generate_handler![
                    add_project,
                    list_projects,
                    delete_project,
                    update_project,
                    list_deltas,
                    list_sessions,
                    list_session_files,
                    set_user,
                    delete_user,
                    get_user
                ])
                .run(tauri::generate_context!())
                .expect("error while running tauri application")
        },
    );
}
