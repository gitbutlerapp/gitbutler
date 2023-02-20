mod deltas;
mod events;
mod fs;
mod projects;
mod repositories;
mod sessions;
mod storage;
mod users;
mod watchers;

use deltas::Delta;
use log;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use storage::Storage;
use tauri::{Manager, Window};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};

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
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<sessions::Session>, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage.clone());
    let users_storage = users::Storage::new(storage);

    let repo = repositories::Repository::open(&projects_storage, &users_storage, project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to open project".to_string(),
            }
        })?;

    let sessions = repo.sessions().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list sessions".to_string(),
        }
    })?;

    Ok(sessions)
}

#[tauri::command]
fn get_user(handle: tauri::AppHandle) -> Result<Option<users::User>, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let users_storage = users::Storage::new(storage);

    users_storage.get().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to get user".to_string(),
        }
    })
}

#[tauri::command]
fn set_user(handle: tauri::AppHandle, user: users::User) -> Result<(), Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let users_storage = users::Storage::new(storage);

    users_storage.set(&user).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to save user".to_string(),
        }
    })?;
    Ok(())
}

#[tauri::command]
fn delete_user(handle: tauri::AppHandle) -> Result<(), Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let users_storage = users::Storage::new(storage);

    users_storage.delete().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to delete user".to_string(),
        }
    })?;
    Ok(())
}

#[tauri::command]
fn update_project(
    handle: tauri::AppHandle,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage);

    projects_storage.update_project(&project).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to update project".to_string(),
        }
    })
}

#[tauri::command]
fn add_project(
    handle: tauri::AppHandle,
    window: Window,
    path: &str,
) -> Result<projects::Project, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage.clone());
    let users_storage = users::Storage::new(storage);
    let watchers_collection = handle.state::<watchers::WatcherCollection>();
    let watchers = watchers::Watcher::new(
        &watchers_collection,
        projects_storage.clone(),
        users_storage.clone(),
    );

    for project in projects_storage.list_projects().map_err(|e| {
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
        projects_storage.add_project(&project).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to add project".to_string(),
            }
        })?;
        watchers.watch(window, &project).map_err(|e| {
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
fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage);

    projects_storage.list_projects().map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list projects".to_string(),
        }
    })
}

#[tauri::command]
fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage.clone());
    let watchers_collection = handle.state::<watchers::WatcherCollection>();
    let users_storage = users::Storage::new(storage);
    let watchers = watchers::Watcher::new(
        &watchers_collection,
        projects_storage.clone(),
        users_storage.clone(),
    );

    match projects_storage.get_project(id) {
        Ok(Some(project)) => {
            watchers.unwatch(project).map_err(|e| {
                log::error!("{}", e);
                Error {
                    message: "Failed to unwatch project".to_string(),
                }
            })?;

            projects_storage.delete_project(id).map_err(|e| {
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
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
) -> Result<HashMap<String, String>, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage.clone());
    let users_storage = users::Storage::new(storage);

    let repo = repositories::Repository::open(&projects_storage, &users_storage, project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to open project".to_string(),
            }
        })?;

    let files = repo.files(session_id).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list files".to_string(),
        }
    })?;

    Ok(files)
}

#[tauri::command]
fn list_deltas(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let path_resolver = handle.path_resolver();
    let storage = storage::Storage::from_path_resolver(&path_resolver);
    let projects_storage = projects::Storage::new(storage.clone());
    let users_storage = users::Storage::new(storage);

    let repo = repositories::Repository::open(&projects_storage, &users_storage, project_id)
        .map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to open project".to_string(),
            }
        })?;

    let deltas = repo.deltas(session_id).map_err(|e| {
        log::error!("{}", e);
        Error {
            message: "Failed to list deltas".to_string(),
        }
    })?;

    Ok(deltas)
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
                    log::info!(
                        "Local data dir: {:?}",
                        resolver.app_local_data_dir().unwrap()
                    );

                    let storage = Storage::from_path_resolver(&resolver);
                    let projects_storage = projects::Storage::new(storage.clone());
                    let users_storage = users::Storage::new(storage);
                    let watcher_collection = watchers::WatcherCollection::default();
                    let watchers = watchers::Watcher::new(
                        &watcher_collection,
                        projects_storage.clone(),
                        users_storage.clone(),
                    );

                    if let Ok(projects) = projects_storage.list_projects() {
                        for project in projects {
                            watchers
                                .watch(app.get_window("main").unwrap(), &project)
                                .map_err(|e| e.to_string())?;
                        }
                    } else {
                        log::error!("Failed to list projects");
                    }

                    #[cfg(debug_assertions)]
                    app.get_window("main").unwrap().open_devtools();

                    app.manage(watcher_collection);

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
