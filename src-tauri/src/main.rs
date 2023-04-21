#[macro_use(defer)]
extern crate scopeguard;

mod app;
mod deltas;
mod events;
mod fs;
mod git;
mod projects;
mod pty;
mod search;
mod sessions;
mod storage;
mod users;

#[macro_use]
extern crate log;

use anyhow::{Context, Result};
use deltas::Delta;
use git::activity;
use serde::{ser::SerializeMap, Serialize};
use std::{collections::HashMap, ops::Range};
use tauri::{generate_context, Manager};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use thiserror::Error;
use timed::timed;

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

#[timed(duration(printer = "debug!"))]
async fn proxy_image(handle: tauri::AppHandle, src: &str) -> Result<String> {
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

    let resp = reqwest::get(src).await?;
    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Failed to download image {}: {}",
            src,
            resp.status()
        ));
    }

    let bytes = resp.bytes().await?;
    std::fs::create_dir_all(&images_dir)?;
    std::fs::write(&save_to, bytes)?;

    Ok(build_asset_url(&save_to.display().to_string()))
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn search(
    handle: tauri::AppHandle,
    project_id: &str,
    query: &str,
    limit: Option<usize>,
    offset: Option<usize>,
    timestamp_ms_gte: Option<u64>,
    timestamp_ms_lt: Option<u64>,
) -> Result<search::SearchResults, Error> {
    let app = handle.state::<app::App>();

    let query = search::SearchQuery {
        project_id: project_id.to_string(),
        q: query.to_string(),
        limit: limit.unwrap_or(100),
        offset,
        range: Range {
            start: timestamp_ms_gte.unwrap_or(0),
            end: timestamp_ms_lt.unwrap_or(u64::MAX),
        },
    };

    let results = app.search(&query).context("failed to search")?;

    Ok(results)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_sessions(
    handle: tauri::AppHandle,
    project_id: &str,
    earliest_timestamp_ms: Option<u128>,
) -> Result<Vec<sessions::Session>, Error> {
    let app = handle.state::<app::App>();
    let sessions = app
        .list_sessions(project_id, earliest_timestamp_ms)
        .context("failed to list sessions")?;
    Ok(sessions)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn get_user(handle: tauri::AppHandle) -> Result<Option<users::User>, Error> {
    let app = handle.state::<app::App>();

    match app.get_user().context("failed to get user")? {
        Some(user) => {
            let local_picture = match proxy_image(handle, &user.picture).await {
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

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn set_user(handle: tauri::AppHandle, user: users::User) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.set_user(&user).context("failed to set user")?;

    sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));

    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn delete_user(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.delete_user().context("failed to delete user")?;

    sentry::configure_scope(|scope| scope.set_user(None));

    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn update_project(
    handle: tauri::AppHandle,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    let app = handle.state::<app::App>();

    let project = app
        .update_project(&project)
        .context("failed to update project")?;

    Ok(project)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn add_project(handle: tauri::AppHandle, path: &str) -> Result<projects::Project, Error> {
    let app = handle.state::<app::App>();

    let (tx, rx) = std::sync::mpsc::channel::<events::Event>();
    let project = app.add_project(path, tx).context("failed to add project")?;

    watch_events(handle, rx);

    Ok(project)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
    let app = handle.state::<app::App>();

    let projects = app.list_projects().context("failed to list projects")?;

    Ok(projects)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.delete_project(id).context("failed to delete project")?;

    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_session_files(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<String, String>, Error> {
    let app = handle.state::<app::App>();
    let files = app
        .list_session_files(project_id, session_id, paths)
        .context("failed to list session files")?;
    Ok(files)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_deltas(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<String, Vec<Delta>>, Error> {
    let app = handle.state::<app::App>();
    let deltas = app
        .list_session_deltas(project_id, session_id, paths)
        .context("failed to list deltas")?;
    Ok(deltas)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_activity(
    handle: tauri::AppHandle,
    project_id: &str,
    start_time_ms: Option<u128>,
) -> Result<Vec<activity::Activity>, Error> {
    let app = handle.state::<app::App>();
    let activity = app
        .git_activity(project_id, start_time_ms)
        .context("failed to get git activity")?;
    Ok(activity)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_status(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<HashMap<String, app::FileStatus>, Error> {
    let app = handle.state::<app::App>();
    let status = app
        .git_status(project_id)
        .context("failed to get git status")?;
    Ok(status)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_wd_diff(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<HashMap<String, String>, Error> {
    let app = handle.state::<app::App>();
    let diff = app
        .git_wd_diff(project_id, 100)
        .context("failed to get git wd diff")?;
    Ok(diff)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_match_paths(
    handle: tauri::AppHandle,
    project_id: &str,
    match_pattern: &str,
) -> Result<Vec<String>, Error> {
    let app = handle.state::<app::App>();
    let paths = app
        .git_match_paths(project_id, match_pattern)
        .context("failed to get git match paths")?;
    Ok(paths)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_branches(handle: tauri::AppHandle, project_id: &str) -> Result<Vec<String>, Error> {
    let app = handle.state::<app::App>();
    let branches = app
        .git_branches(project_id)
        .context("failed to get git branches")?;
    Ok(branches)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_head(handle: tauri::AppHandle, project_id: &str) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let head = app.git_head(project_id).context("failed to get git head")?;
    Ok(head)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_switch_branch(
    handle: tauri::AppHandle,
    project_id: &str,
    branch: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.git_switch_branch(project_id, branch)
        .context("failed to switch git branch")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_stage(
    handle: tauri::AppHandle,
    project_id: &str,
    paths: Vec<&str>,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.git_stage_files(project_id, paths)
        .context("failed to stage file")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_unstage(
    handle: tauri::AppHandle,
    project_id: &str,
    paths: Vec<&str>,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.git_unstage_files(project_id, paths)
        .context("failed to unstage file")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_commit(
    handle: tauri::AppHandle,
    project_id: &str,
    message: &str,
    push: bool,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.git_commit(project_id, message, push)
        .context("failed to commit")?;
    Ok(())
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
        .setup(move |tauri_app| {
            let window = create_window(&tauri_app.handle()).expect("Failed to create window");
            #[cfg(debug_assertions)]
            window.open_devtools();

            let app: app::App =
                app::App::new(tauri_app.path_resolver().app_local_data_dir().unwrap())
                    .expect("failed to initialize app");

            // TODO: REMOVE THIS
            // debug_test_consistency(&app_state, "fec3d50c-503f-4021-89fb-e7ec2433ceae")
            //     .expect("FAIL");

            tauri_app.manage(app);

            let app_handle = tauri_app.handle();
            tauri::async_runtime::spawn_blocking(move || {
                if let Err(e) = init(app_handle) {
                    log::error!("failed to app: {:#}", e);
                }
            });

            Ok(())
        })
        .plugin(tauri_plugin_websocket::init())
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
                .filter(|metadata| {
                    // only show logs from git_butler
                    metadata.target().starts_with("git_butler")
                        // or if the log level is info or higher
                        || metadata.level() < log::LevelFilter::Info
                })
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
            get_user,
            search,
            git_status,
            git_activity,
            git_match_paths,
            git_branches,
            git_head,
            git_switch_branch,
            git_commit,
            git_stage,
            git_unstage,
            git_wd_diff,
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

fn init(app_handle: tauri::AppHandle) -> Result<()> {
    let app = app_handle.state::<app::App>();
    if let Some(user) = app.get_user().context("failed to get user")? {
        sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())))
    }

    let (events_tx, events_rx) = std::sync::mpsc::channel::<events::Event>();

    app.start_pty_server()
        .context("failed to start pty server")?;

    app.init(events_tx).context("failed to init app")?;

    watch_events(app_handle, events_rx);

    Ok(())
}

fn watch_events(handle: tauri::AppHandle, rx: std::sync::mpsc::Receiver<events::Event>) {
    tauri::async_runtime::spawn_blocking(move || {
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

// fn debug_test_consistency(app_state: &App, project_id: &str) -> Result<()> {
//     let repo = app_state
//         .repositories_storage
//         .lock()
//         .unwrap()
//         .get(&project_id)?;

//     let sessions = repo.sessions(None)?;
//     let session_deltas: Vec<HashMap<String, Vec<Delta>>> = sessions
//         .iter()
//         .map(|session| {
//             let deltas = repo
//                 .deltas(&session.id, None)
//                 .expect("Failed to list deltas");
//             deltas
//         })
//         .collect();

//     let deltas: HashMap<String, Vec<Delta>> =
//         session_deltas
//             .iter()
//             .fold(HashMap::new(), |mut acc, deltas| {
//                 for (path, deltas) in deltas {
//                     acc.entry(path.to_string())
//                         .or_insert_with(Vec::new)
//                         .extend(deltas.clone());
//                 }
//                 acc
//             });

//     if sessions.is_empty() {
//         return Ok(());
//     }

//     let first_session = &sessions[sessions.len() - 1];
//     let files = repo.files(&first_session.id, None)?;

//     files.iter().for_each(|(path, content)| {
//         println!("Testing consistency for {}", path);
//         let mut file_deltas = deltas.get(path).unwrap_or(&Vec::new()).clone();
//         file_deltas.sort_by(|a, b| a.timestamp_ms.cmp(&b.timestamp_ms));
//         let mut text: Vec<char> = content.chars().collect();
//         for delta in file_deltas {
//             println!("Applying delta: {:?}", delta.timestamp_ms);
//             for operation in delta.operations {
//                 println!("Applying operation: {:?}", operation);
//                 operation
//                     .apply(&mut text)
//                     .expect("Failed to apply operation");
//             }
//         }
//     });

//     Ok(())
// }
