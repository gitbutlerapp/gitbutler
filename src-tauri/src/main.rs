mod deltas;
mod events;
mod fs;
mod projects;
mod repositories;
mod sessions;
mod storage;
mod users;
mod watchers;

use anyhow::Result;
use deltas::Delta;
use log;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::mpsc, thread};
use storage::Storage;
use tauri::{generate_context, Manager};
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

    sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));

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

    sentry::configure_scope(|scope| scope.set_user(None));

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
fn add_project(handle: tauri::AppHandle, path: &str) -> Result<projects::Project, Error> {
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
            if !project.deleted {
                return Err(Error {
                    message: "Project already exists".to_string(),
                });
            } else {
                projects_storage
                    .update_project(&projects::UpdateRequest {
                        id: project.id.clone(),
                        deleted: Some(false),
                        ..Default::default()
                    })
                    .map_err(|e| {
                        log::error!("{}", e);
                        Error {
                            message: "Failed to undelete project".to_string(),
                        }
                    })?;
                return Ok(project);
            }
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

        let (tx, rx): (mpsc::Sender<events::Event>, mpsc::Receiver<events::Event>) =
            mpsc::channel();

        watchers.watch(tx, &project).map_err(|e| {
            log::error!("{}", e);
            Error {
                message: "Failed to watch project".to_string(),
            }
        })?;

        watch_events(handle, rx);

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

            projects_storage
                .update_project(&projects::UpdateRequest {
                    id: id.to_string(),
                    deleted: Some(true),
                    ..Default::default()
                })
                .map_err(|e| {
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
    paths: Option<Vec<&str>>,
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

    let files = repo.files(session_id, paths).map_err(|e| {
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
    let quit = tauri::CustomMenuItem::new("quit".to_string(), "Quit");
    let hide = tauri::CustomMenuItem::new("toggle".to_string(), format!("Hide {}", app_title()));
    let tray_menu = tauri::SystemTrayMenu::new().add_item(hide).add_item(quit);
    let tray = tauri::SystemTray::new().with_menu(tray_menu);

    let tauri_app_builder = tauri::Builder::default()
        .system_tray(tray)
        .on_system_tray_event(|app_handle, event| match event {
            tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
                let item_handle = app_handle.tray_handle().get_item(&id);
                match id.as_str() {
                    "quit" => {
                        app_handle.exit(0);
                    }
                    "toggle" => match get_window(&app_handle) {
                        Some(window) => {
                            if window.is_visible().unwrap() {
                                window.hide().unwrap();
                                item_handle
                                    .set_title(format!("Show {}", app_title()))
                                    .unwrap();
                            } else {
                                window.show().unwrap();
                                window.set_focus().unwrap();
                                item_handle
                                    .set_title(format!("Hide {}", app_title()))
                                    .unwrap();
                            }
                        }
                        None => {
                            create_window(&app_handle).expect("Failed to create window");
                            item_handle
                                .set_title(format!("Hide {}", app_title()))
                                .unwrap();
                        }
                    },
                    _ => {}
                }
            }
            _ => {}
        })
        .on_window_event(|event| match event.event() {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                let window = event.window();

                window
                    .app_handle()
                    .tray_handle()
                    .get_item("toggle")
                    .set_title(format!("Show {}", app_title()))
                    .expect("Failed to set tray item title");

                window.hide().expect("Failed to hide window");
            }
            _ => {}
        })
        .setup(move |app| {
            let window = create_window(&app.handle()).expect("Failed to create window");
            #[cfg(debug_assertions)]
            window.open_devtools();

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

            users_storage
                .get()
                .and_then(|user| match user {
                    Some(user) => {
                        sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));
                        Ok(())
                    }
                    None => Ok(()),
                })
                .expect("Failed to set user");

            let (tx, rx): (mpsc::Sender<events::Event>, mpsc::Receiver<events::Event>) =
                mpsc::channel();

            if let Ok(projects) = projects_storage.list_projects() {
                for project in projects {
                    watchers
                        .watch(tx.clone(), &project)
                        .map_err(|e| e.to_string())?;
                }
            } else {
                log::error!("Failed to list projects");
            }

            watch_events(app.handle(), rx);

            app.manage(watcher_collection);

            Ok(())
        })
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .plugin({
            let targets = [
                LogTarget::LogDir,
                #[cfg(debug_assertions)]
                LogTarget::Stdout,
                #[cfg(debug_assertions)]
                LogTarget::Webview,
            ];
            tauri_plugin_log::Builder::default()
                .level(match IS_DEV {
                    true => log::LevelFilter::Debug,
                    false => log::LevelFilter::Info,
                })
                .with_colors(ColoredLevelConfig {
                    error: Color::Red,
                    warn: Color::Yellow,
                    debug: Color::Blue,
                    info: Color::BrightGreen,
                    trace: Color::Cyan,
                })
                .targets(targets)
                .build()
        })
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
        ]);

    let tauri_context = generate_context!();
    let app_version = tauri_context.package_info().version.to_string();

    sentry_tauri::init(
        app_version.clone(),
        |_| {
            sentry::init((
                "https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032",
                sentry::ClientOptions {
                    release: Some(std::borrow::Cow::from(app_version)),
                    ..Default::default()
                },
            ))
        },
        |sentry_plugin| {
            let tauri_app = tauri_app_builder
                .plugin(sentry_plugin)
                .build(tauri_context)
                .expect("Failed to build tauri app");

            tauri_app.run(|app_handle, event| match event {
                tauri::RunEvent::ExitRequested { api, .. } => {
                    hide_window(&app_handle).expect("Failed to hide window");
                    api.prevent_exit();
                }
                _ => {}
            });
        },
    );
}

fn watch_events(handle: tauri::AppHandle, rx: mpsc::Receiver<events::Event>) {
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            if let Some(window) = handle.get_window("main") {
                match window.emit(&event.name, event.payload) {
                    Err(e) => log::error!("Failed to emit event: {}", e),
                    _ => {}
                }
            }
        }
    });
}

fn get_window(handle: &tauri::AppHandle) -> Option<tauri::Window> {
    handle.get_window("main")
}

#[cfg(not(target_os = "macos"))]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    log::info!("Creating window");
    tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
        .resizable(true)
        .title(app_title())
        .theme(Some(tauri::Theme::Dark))
        .min_inner_size(600.0, 300.0)
        .inner_size(800.0, 600.0)
        .build()
}

#[cfg(target_os = "macos")]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    log::info!("Creating window");
    tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
        .resizable(true)
        .title(app_title())
        .theme(Some(tauri::Theme::Dark))
        .min_inner_size(600.0, 300.0)
        .inner_size(800.0, 600.0)
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .build()
}

fn hide_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle
        .tray_handle()
        .get_item("toggle")
        .set_title(format!("Show {}", app_title()))?;

    match get_window(handle) {
        Some(window) => {
            if window.is_visible()? {
                window.hide()
            } else {
                Ok(())
            }
        }
        None => Ok(()),
    }
}
