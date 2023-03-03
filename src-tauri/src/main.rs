mod deltas;
mod events;
mod fs;
mod projects;
mod repositories;
mod search;
mod sessions;
mod storage;
mod users;
mod watchers;

use anyhow::{Context, Result};
use deltas::Delta;
use log;
use serde::{ser::SerializeMap, Serialize};
use std::{
    collections::HashMap,
    sync::{mpsc, Mutex},
};
use storage::Storage;
use tauri::{generate_context, Manager};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    ProjectError(projects::CreateError),
    #[error("Project already exists")]
    ProjectAlreadyExists,
    #[error("Something went wrong")]
    Unknown,
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(1))?;
        map.serialize_entry("message", &self.to_string())?;
        map.end()
    }
}

impl From<projects::CreateError> for Error {
    fn from(e: projects::CreateError) -> Self {
        Error::ProjectError(e)
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        log::error!("{:#}", e);
        Error::Unknown
    }
}

struct App {
    pub projects_storage: projects::Storage,
    pub users_storage: users::Storage,
    pub deltas_searcher: Mutex<search::Deltas>,
    pub watchers: Mutex<watchers::Watcher>,
}

impl App {
    pub fn new(resolver: tauri::PathResolver) -> Self {
        let local_data_dir = resolver.app_local_data_dir().unwrap();
        log::info!("Local data dir: {:?}", local_data_dir,);
        let storage = Storage::from_path_resolver(&resolver);
        let projects_storage = projects::Storage::new(storage.clone());
        let users_storage = users::Storage::new(storage.clone());
        let deltas_searcher = search::Deltas::at(local_data_dir);
        let watchers = watchers::Watcher::new(
            projects_storage.clone(),
            users_storage.clone(),
            deltas_searcher.clone(),
        );
        Self {
            projects_storage,
            users_storage,
            deltas_searcher: deltas_searcher.into(),
            watchers: watchers.into(),
        }
    }
}

const IS_DEV: bool = cfg!(debug_assertions);

fn app_title() -> String {
    if IS_DEV {
        "GitButler (dev)".to_string()
    } else {
        "GitButler".to_string()
    }
}

fn build_asset_url(path: &str) -> String {
    format!("asset://localhost/{}", urlencoding::encode(path))
}

fn proxy_image(handle: tauri::AppHandle, src: &str) -> Result<String> {
    if src.starts_with("asset://") {
        return Ok(src.to_string());
    }

    let images_dir = handle
        .path_resolver()
        .app_cache_dir()
        .unwrap()
        .join("images");

    let hash = md5::compute(src);
    let ext = src.split('.').last().unwrap_or("jpg");
    let save_to = images_dir.join(format!("{:X}.{}", hash, ext));

    if save_to.exists() {
        return Ok(build_asset_url(&save_to.display().to_string()));
    }

    let resp = reqwest::blocking::get(src)?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image {}: {}",
            src,
            resp.status()
        ));
    }

    let bytes = resp.bytes()?;
    std::fs::create_dir_all(&images_dir)?;
    std::fs::write(&save_to, bytes)?;

    Ok(build_asset_url(&save_to.display().to_string()))
}

#[tauri::command]
fn list_sessions(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<sessions::Session>, Error> {
    let app_state = handle.state::<App>();

    let repo = repositories::Repository::open(
        &app_state.projects_storage,
        &app_state.users_storage,
        project_id,
    )
    .with_context(|| format!("Failed to open repository for project {}", project_id))?;

    let sessions = repo
        .sessions()
        .with_context(|| format!("Failed to list sessions for project {}", project_id))?;

    Ok(sessions)
}

#[tauri::command]
fn get_user(handle: tauri::AppHandle) -> Result<Option<users::User>, Error> {
    let app_state = handle.state::<App>();

    match app_state
        .users_storage
        .get()
        .with_context(|| "Failed to get user".to_string())?
    {
        Some(user) => {
            let local_picture = match proxy_image(handle, &user.picture) {
                Ok(picture) => picture,
                Err(e) => {
                    log::error!("{:#}", e);
                    user.picture
                }
            };

            let user = users::User {
                picture: local_picture.to_string(),
                ..user
            };

            Ok(Some(user))
        }
        None => Ok(None),
    }
}

#[tauri::command]
fn set_user(handle: tauri::AppHandle, user: users::User) -> Result<(), Error> {
    let app_state = handle.state::<App>();

    app_state
        .users_storage
        .set(&user)
        .with_context(|| "Failed to set user".to_string())?;

    sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));

    Ok(())
}

#[tauri::command]
fn delete_user(handle: tauri::AppHandle) -> Result<(), Error> {
    let app_state = handle.state::<App>();

    app_state
        .users_storage
        .delete()
        .with_context(|| "Failed to delete user".to_string())?;

    sentry::configure_scope(|scope| scope.set_user(None));

    Ok(())
}

#[tauri::command]
fn update_project(
    handle: tauri::AppHandle,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    let app_state = handle.state::<App>();

    let project = app_state
        .projects_storage
        .update_project(&project)
        .with_context(|| format!("Failed to update project {}", project.id))?;

    Ok(project)
}

#[tauri::command]
fn add_project(handle: tauri::AppHandle, path: &str) -> Result<projects::Project, Error> {
    let app_state = handle.state::<App>();

    for project in app_state
        .projects_storage
        .list_projects()
        .with_context(|| "Failed to list projects".to_string())?
    {
        if project.path == path {
            if !project.deleted {
                return Err(Error::ProjectAlreadyExists);
            } else {
                app_state
                    .projects_storage
                    .update_project(&projects::UpdateRequest {
                        id: project.id.clone(),
                        deleted: Some(false),
                        ..Default::default()
                    })?;
                return Ok(project);
            }
        }
    }

    let project = projects::Project::from_path(path.to_string())?;
    app_state.projects_storage.add_project(&project)?;

    let (tx, rx): (mpsc::Sender<events::Event>, mpsc::Receiver<events::Event>) = mpsc::channel();
    app_state.watchers.lock().unwrap().watch(tx, &project)?;
    watch_events(handle, rx);

    Ok(project)
}

#[tauri::command]
fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
    let app_state = handle.state::<App>();

    let projects = app_state.projects_storage.list_projects()?;

    Ok(projects)
}

#[tauri::command]
fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let app_state = handle.state::<App>();

    match app_state.projects_storage.get_project(id)? {
        Some(project) => {
            app_state.watchers.lock().unwrap().unwatch(project)?;

            app_state
                .projects_storage
                .update_project(&projects::UpdateRequest {
                    id: id.to_string(),
                    deleted: Some(true),
                    ..Default::default()
                })?;

            Ok(())
        }
        None => Ok(()),
    }
}

#[tauri::command]
fn list_session_files(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<String, String>, Error> {
    let app_state = handle.state::<App>();

    let repo = repositories::Repository::open(
        &app_state.projects_storage,
        &app_state.users_storage,
        project_id,
    )?;

    let files = repo.files(session_id, paths)?;

    Ok(files)
}

#[tauri::command]
fn list_deltas(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let app_state = handle.state::<App>();

    let repo = repositories::Repository::open(
        &app_state.projects_storage,
        &app_state.users_storage,
        project_id,
    )?;

    let deltas = repo.deltas(session_id)?;

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

            let app_state: App = App::new(app.path_resolver());

            // setup senty
            if let Some(user) = app_state.users_storage.get().expect("Failed to get user") {
                sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())))
            }

            // start watching projects
            let (tx, rx): (mpsc::Sender<events::Event>, mpsc::Receiver<events::Event>) =
                mpsc::channel();
            let projects = app_state
                .projects_storage
                .list_projects()
                .with_context(|| "Failed to list projects")?;

            for project in projects {
                app_state
                    .watchers
                    .lock()
                    .unwrap()
                    .watch(tx.clone(), &project)
                    .with_context(|| format!("Failed to watch project: {}", project.id))?;
                let repo = git2::Repository::open(&project.path)
                    .with_context(|| format!("Failed to open git repository: {}", project.path))?;

                app_state
                    .deltas_searcher
                    .lock()
                    .unwrap()
                    .reindex_project(&repo, &project)
                    .with_context(|| format!("Failed to reindex project: {}", project.id))?;
            }
            watch_events(app.handle(), rx);

            app.manage(app_state);
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
    tauri::async_runtime::spawn(async move {
        while let Ok(event) = rx.recv() {
            if let Some(window) = handle.get_window("main") {
                log::info!("Emitting event: {}", event.name);
                match window.emit(&event.name, event.payload) {
                    Err(e) => log::error!("Failed to emit event: {:#}", e),
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
        .min_inner_size(1024.0, 600.0)
        .inner_size(1024.0, 600.0)
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
