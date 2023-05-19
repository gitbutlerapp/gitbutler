#[macro_use(defer)]
extern crate scopeguard;

mod app;
mod bookmarks;
mod database;
mod deltas;
mod sessions;
mod files;
mod gb_repository;
mod project_repository;
mod projects;
mod pty;
mod search;
mod users;
mod watcher;
mod reader;
mod writer;
mod zip;
mod events;
mod fs;
mod storage;

#[macro_use]
extern crate log;

use anyhow::{Context, Result};
use serde::{ser::SerializeMap, Serialize};
use std::{collections::HashMap, ops};
use tauri::{generate_context, Manager};
use tauri_plugin_log::{
    fern::colors::{Color, ColoredLevelConfig},
    LogTarget,
};
use thiserror::Error;
use timed::timed;

use crate::project_repository::activity;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Message(String),
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

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        sentry_anyhow::capture_anyhow(&e);
        log::error!("{:#}", e);
        Error::Unknown
    }
}

impl From<app::AddProjectError> for Error {
    fn from(e: app::AddProjectError) -> Self {
        match e {
            app::AddProjectError::ProjectAlreadyExists => Error::Message("Project already exists".to_string()),
            app::AddProjectError::OpenError(e) => Error::Message(e.to_string()),
            app::AddProjectError::Other(e) => e.into(),
        }
    }
}

const IS_DEV: bool = cfg!(debug_assertions);

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
async fn get_project_archive_path(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let project = app
        .get_project(project_id)?.ok_or_else(|| Error::Message("Project not found".to_string()))?;

    let zipper = zip::Zipper::new(handle.path_resolver().app_cache_dir().unwrap().join("archives"));
    let zipped_logs = zipper.zip(project.path)?;
    Ok(zipped_logs.to_str().unwrap().to_string())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn get_project_data_archive_path(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<String, Error> {
    let zipper = zip::Zipper::new(handle.path_resolver().app_cache_dir().unwrap().join("archives"));
    let zipped_logs = zipper.zip(handle.path_resolver().app_local_data_dir().unwrap().join("projects").join(project_id))?;
    Ok(zipped_logs.to_str().unwrap().to_string())
}


#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn get_logs_archive_path(
    handle: tauri::AppHandle,
) -> Result<String, Error> {
    let zipper = zip::Zipper::new(handle.path_resolver().app_cache_dir().unwrap().join("archives"));
    let zipped_logs = zipper.zip(handle.path_resolver().app_log_dir().unwrap())?;
    Ok(zipped_logs.to_str().unwrap().to_string())
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
        range: ops::Range {
            start: timestamp_ms_gte.unwrap_or(0),
            end: timestamp_ms_lt.unwrap_or(u64::MAX),
        },
    };

    let results = app.search(&query).with_context(|| {
        format!(
            "failed to search for query {} in project {}",
            query.q, query.project_id
        )
    })?;

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
        .with_context(|| {
            format!(
                "failed to list sessions for project {}",
                project_id.to_string()
            )
        })?;
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
        .with_context(|| format!("failed to update project {}", project.id))?;

    Ok(project)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn add_project(handle: tauri::AppHandle, path: &str) -> Result<projects::Project, Error> {
    let app = handle.state::<app::App>();
    let project = app.add_project(path)?;
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

    app.delete_project(id).with_context(|| format!("failed to delete project {}", id))?;

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
        .with_context(|| {
            format!(
                "failed to list files for session {} in project {}",
                session_id, project_id
            )
        })?;
    Ok(files)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_deltas(
    handle: tauri::AppHandle,
    project_id: &str,
    session_id: &str,
    paths: Option<Vec<&str>>,
) -> Result<HashMap<String, Vec<deltas::Delta>>, Error> {
    let app = handle.state::<app::App>();
    let deltas = app
        .list_session_deltas(project_id, session_id, paths)
        .with_context(|| {
            format!(
                "failed to list deltas for session {} in project {}",
                session_id, project_id
            )
        })?;
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
        .with_context(|| format!("failed to get git activity for project {}", project_id))?;
    Ok(activity)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_status(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<HashMap<String, project_repository::FileStatus>, Error> {
    let app = handle.state::<app::App>();
    let status = app
        .git_status(project_id)
        .with_context(|| format!("failed to get git status for project {}", project_id))?;
    Ok(status)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_wd_diff(
    handle: tauri::AppHandle,
    project_id: &str,
    context_lines: usize,
) -> Result<HashMap<String, String>, Error> {
    let app = handle.state::<app::App>();
    let diff = app
        .git_wd_diff(project_id, context_lines)
        .with_context(|| format!("failed to get git wd diff for project {}", project_id))?;
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
        .with_context(|| {
            format!(
                "failed to get git match paths for project {} and pattern {}",
                project_id, match_pattern
            )
        })?;
    Ok(paths)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_branches(handle: tauri::AppHandle, project_id: &str) -> Result<Vec<String>, Error> {
    let app = handle.state::<app::App>();
    let branches = app
        .git_branches(project_id)
        .with_context(|| format!("failed to get git branches for project {}", project_id))?;
    Ok(branches)
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn git_head(handle: tauri::AppHandle, project_id: &str) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let head = app.git_head(project_id).with_context(|| {
        format!(
            "failed to get git head for project {}",
            project_id
        )
    })?;
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
        .with_context(|| format!("failed to switch git branch for project {}", project_id))?;
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
        .with_context(|| format!("failed to stage file for project {}", project_id))?;
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
        .with_context(|| format!("failed to unstage file for project {}", project_id))?;
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
        .with_context(|| format!("failed to commit for project {}", project_id))?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn delete_all_data(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.delete_all_data().context("failed to delete all data")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn upsert_bookmark(handle: tauri::AppHandle, bookmark: bookmarks::Bookmark) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.upsert_bookmark(&bookmark).context("failed to upsert bookmark")?;
    Ok(())
}

#[timed(duration(printer = "debug!"))]
#[tauri::command(async)]
async fn list_bookmarks(handle: tauri::AppHandle, project_id: &str, range: Option<ops::Range<u128>>) -> Result<Vec<bookmarks::Bookmark>,Error> {
    let app = handle.state::<app::App>();
    let bookmarks = app.list_bookmarks(project_id, range).context("failed to list bookmarks")?;
    Ok(bookmarks)
}

fn main() {
    let tauri_context = generate_context!();

    let _guard = sentry::init(("https://9d407634d26b4d30b6a42d57a136d255@o4504644069687296.ingest.sentry.io/4504649768108032", sentry::ClientOptions {
        release: Some(tauri_context.package_info().version.to_string().into()),
        attach_stacktrace: true,
        default_integrations: true,
        ..Default::default()
    }));

    let app_title = tauri_context.package_info().name.clone();

    let quit = tauri::CustomMenuItem::new("quit".to_string(), "Quit");
    let hide = tauri::CustomMenuItem::new("toggle".to_string(), format!("Hide {}", app_title));
    let tray_menu = tauri::SystemTrayMenu::new().add_item(hide).add_item(quit);
    let tray = tauri::SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(tray)
        .on_system_tray_event(|app_handle, event| match event {
            tauri::SystemTrayEvent::MenuItemClick { id, .. } => {
                let app_title = app_handle.package_info().name.clone();
                let item_handle = app_handle.tray_handle().get_item(&id);
                match id.as_str() {
                    "quit" => {
                        app_handle.exit(0);
                    }
                    "toggle" => match get_window(&app_handle) {
                        Some(window) => {
                            if window.is_visible().unwrap() {
                                hide_window(&app_handle).expect("Failed to hide window");
                            } else {
                                show_window(&app_handle).expect("Failed to show window");
                            }
                        }
                        None => {
                            create_window(&app_handle).expect("Failed to create window");
                            item_handle
                                .set_title(format!("Hide {}", app_title))
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
                hide_window(&event.window().app_handle()).expect("Failed to hide window");
                api.prevent_close();
            }
            _ => {}
        })
        .setup(move |tauri_app| {
            let window = create_window(&tauri_app.handle()).expect("Failed to create window");
            #[cfg(debug_assertions)]
            window.open_devtools();

            let app: app::App =
                app::App::new(
                    tauri_app.path_resolver().app_local_data_dir().unwrap(),
                    events::Sender::new(tauri_app.handle())
                )
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
            delete_all_data,
            get_logs_archive_path,
            get_project_archive_path,
            get_project_data_archive_path,
            upsert_bookmark,
            list_bookmarks,
        ])
        .build(tauri_context)
        .expect("Failed to build tauri app")
        .run(|app_handle, event| match event {
            tauri::RunEvent::WindowEvent { event, .. } => match event {
                tauri::WindowEvent::Focused(is_focused) => {
                    if is_focused {
                        set_toggle_menu_hide(&app_handle)
                            .expect("Failed to set toggle menu hide");
                    } else {
                        set_toggle_menu_show(&app_handle)
                            .expect("Failed to set toggle menu show");
                    }
                },
                _ => {}
            }
            tauri::RunEvent::ExitRequested { api, .. } => {
                hide_window(&app_handle).expect("Failed to hide window");
                api.prevent_exit();
            }
            _ => {}
        });
}

fn init(app_handle: tauri::AppHandle) -> Result<()> {
    let app = app_handle.state::<app::App>();
    if let Some(user) = app.get_user().context("failed to get user")? {
        sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())))
    }

    app.start_pty_server()
        .context("failed to start pty server")?;

    app.init().context("failed to init app")?;

    Ok(())
}

fn get_window(handle: &tauri::AppHandle) -> Option<tauri::Window> {
    handle.get_window("main")
}

#[cfg(not(target_os = "macos"))]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    log::info!("Creating window");
    let app_title = handle.package_info().name.clone();
    tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
        .resizable(true)
        .title(app_title)
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
        .title(handle.package_info().name.clone())
        .theme(Some(tauri::Theme::Dark))
        .min_inner_size(1024.0, 600.0)
        .inner_size(1024.0, 600.0)
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .build()
}

fn set_toggle_menu_hide(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle
        .tray_handle()
        .get_item("toggle")
        .set_title(format!("Hide {}", handle.package_info().name))
}

fn show_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    set_toggle_menu_hide(handle)?;


    #[cfg(target_os = "macos")]
    handle.show()?;

    if let Some(window) = get_window(handle) {
        window.set_focus()?;

        #[cfg(not(target_os = "macos"))]
        window.hide()?;
    }

    Ok(())
}

fn set_toggle_menu_show(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle
        .tray_handle()
        .get_item("toggle")
        .set_title(format!("Show {}", handle.package_info().name))
}

fn hide_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    set_toggle_menu_show(handle)?;

    #[cfg(target_os = "macos")]
    handle.hide()?;

    #[cfg(not(target_os = "macos"))]
    if let Some(window) = get_window(handle) {
        window.hide()?;
    }

    Ok(())
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
