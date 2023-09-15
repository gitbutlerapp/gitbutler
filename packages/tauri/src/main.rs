mod assets;
mod zip;

use std::{collections::HashMap, ops, path, time};

use anyhow::{Context, Result};
use futures::future::join_all;
use tauri::{generate_context, Manager};
use tracing::instrument;

use gitbutler::{virtual_branches::VirtualBranchCommit, *};

use crate::{error::Error, git, project_repository::activity};

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn get_project_archive_path(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let project = app
        .get_project(project_id)?
        .context("failed to get project")?;

    let zipper = handle.state::<zip::Zipper>();
    let zipped_logs = zipper.zip(project.path)?;
    Ok(zipped_logs.to_str().unwrap().to_string())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn get_project_data_archive_path(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<String, Error> {
    let zipper = handle.state::<zip::Zipper>();
    let zipped_logs = zipper.zip(
        handle
            .path_resolver()
            .app_local_data_dir()
            .unwrap()
            .join("projects")
            .join(project_id),
    )?;
    Ok(zipped_logs.to_str().unwrap().to_string())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn get_logs_archive_path(handle: tauri::AppHandle) -> Result<String, Error> {
    let zipper = handle.state::<zip::Zipper>();
    let zipped_logs = zipper.zip(handle.path_resolver().app_log_dir().unwrap())?;
    Ok(zipped_logs.to_str().unwrap().to_string())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn search(
    handle: tauri::AppHandle,
    project_id: &str,
    query: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<search::Results, Error> {
    let app = handle.state::<app::App>();

    let query = search::Query {
        project_id: project_id.to_string(),
        q: query.to_string(),
        limit: limit.unwrap_or(100),
        offset,
    };

    let results = app.search(&query).with_context(|| {
        format!(
            "failed to search for query {} in project {}",
            query.q, query.project_id
        )
    })?;

    Ok(results)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn list_sessions(
    handle: tauri::AppHandle,
    project_id: &str,
    earliest_timestamp_ms: Option<u128>,
) -> Result<Vec<sessions::Session>, Error> {
    let app = handle.state::<app::App>();
    let sessions = app
        .list_sessions(project_id, earliest_timestamp_ms)
        .with_context(|| format!("failed to list sessions for project {}", project_id))?;
    Ok(sessions)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn get_user(handle: tauri::AppHandle) -> Result<Option<users::User>, Error> {
    let app = handle.state::<app::App>();
    let proxy = handle.state::<assets::Proxy>();

    match app.get_user().context("failed to get user")? {
        Some(user) => {
            let remote_picture = url::Url::parse(&user.picture).context("invalid picture url")?;
            let local_picture = match proxy.proxy(&remote_picture).await {
                Ok(picture) => picture,
                Err(e) => {
                    tracing::error!("{:#}", e);
                    remote_picture
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

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn set_user(handle: tauri::AppHandle, user: users::User) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.set_user(&user).context("failed to set user")?;

    sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn delete_user(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.delete_user().context("failed to delete user")?;

    sentry::configure_scope(|scope| scope.set_user(None));

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn update_project(
    handle: tauri::AppHandle,
    project: projects::UpdateRequest,
) -> Result<projects::Project, Error> {
    let app = handle.state::<app::App>();

    let project = app
        .update_project(&project)
        .with_context(|| format!("failed to update project {}", project.id))?;
    if project.api.is_some() {
        app.git_gb_push(&project.id)
            .with_context(|| format!("failed to push git branch for project {}", &project.id))?;
    }
    Ok(project)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn add_project(handle: tauri::AppHandle, path: &str) -> Result<projects::Project, Error> {
    let app = handle.state::<app::App>();
    let project = app.add_project(path)?;
    Ok(project)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn get_project(
    handle: tauri::AppHandle,
    id: &str,
) -> Result<Option<projects::Project>, Error> {
    let app = handle.state::<app::App>();
    let project = app.get_project(id)?;
    Ok(project)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn list_projects(handle: tauri::AppHandle) -> Result<Vec<projects::Project>, Error> {
    let app = handle.state::<app::App>();

    let projects = app.list_projects().context("failed to list projects")?;

    Ok(projects)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn delete_project(handle: tauri::AppHandle, id: &str) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.delete_project(id)
        .with_context(|| format!("failed to delete project {}", id))?;

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_wd_diff(
    handle: tauri::AppHandle,
    project_id: &str,
    context_lines: u32,
) -> Result<HashMap<path::PathBuf, String>, Error> {
    let app = handle.state::<app::App>();
    let diff = app
        .git_wd_diff(project_id, context_lines)
        .with_context(|| format!("failed to get git wd diff for project {}", project_id))?;
    Ok(diff)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_commit_diff(
    handle: tauri::AppHandle,
    project_id: &str,
    commit_id: &str,
) -> Result<HashMap<path::PathBuf, String>, Error> {
    let app = handle.state::<app::App>();
    let diff = app
        .git_commit_diff(project_id, commit_id)
        .with_context(|| {
            format!(
                "failed to get git diff for project {} and commit {}",
                project_id, commit_id
            )
        })?;
    Ok(diff)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_remote_branches(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<git::RemoteBranchName>, Error> {
    let app = handle.state::<app::App>();
    let branches = app.git_remote_branches(project_id).with_context(|| {
        format!(
            "failed to get remote git branches for project {}",
            project_id
        )
    })?;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_remote_branches_data(
    handle: tauri::AppHandle,
    project_id: &str,
) -> Result<Vec<virtual_branches::RemoteBranch>, Error> {
    let app = handle.state::<app::App>();
    let branches = app
        .git_remote_branches_data(project_id)
        .with_context(|| format!("failed to get git branches for project {}", project_id))?;

    let branches = join_all(
        branches
            .into_iter()
            .map(|branch| {
                let proxy = handle.state::<assets::Proxy>();
                async move {
                    virtual_branches::RemoteBranch {
                        commits: join_all(
                            branch
                                .commits
                                .into_iter()
                                .map(|commit| {
                                    let proxy = proxy.clone();
                                    async move {
                                        VirtualBranchCommit {
                                            author: virtual_branches::Author {
                                                gravatar_url: proxy
                                                    .proxy(&commit.author.gravatar_url)
                                                    .await
                                                    .unwrap_or_else(|e| {
                                                        tracing::error!(
                                                            "failed to proxy gravatar url {}: {:#}",
                                                            commit.author.gravatar_url,
                                                            e
                                                        );
                                                        commit.author.gravatar_url.clone()
                                                    }),
                                                ..commit.author.clone()
                                            },
                                            ..commit.clone()
                                        }
                                    }
                                })
                                .collect::<Vec<_>>(),
                        )
                        .await,
                        ..branch
                    }
                }
            })
            .collect::<Vec<_>>(),
    )
    .await;
    Ok(branches)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_head(handle: tauri::AppHandle, project_id: &str) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let head = app
        .git_head(project_id)
        .with_context(|| format!("failed to get git head for project {}", project_id))?;
    Ok(head)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
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

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_commit(
    handle: tauri::AppHandle,
    project_id: &str,
    message: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.git_commit(project_id, message)
        .with_context(|| format!("failed to commit for project {}", project_id))?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn delete_all_data(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.delete_all_data().context("failed to delete all data")?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn upsert_bookmark(
    handle: tauri::AppHandle,
    project_id: String,
    timestamp_ms: u64,
    note: String,
    deleted: bool,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    let now = time::UNIX_EPOCH
        .elapsed()
        .context("failed to get time")?
        .as_millis();
    let bookmark = bookmarks::Bookmark {
        project_id,
        timestamp_ms: timestamp_ms
            .try_into()
            .context("failed to convert timestamp")?,
        created_timestamp_ms: now,
        updated_timestamp_ms: now,
        note,
        deleted,
    };
    app.upsert_bookmark(&bookmark)
        .context("failed to upsert bookmark")?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn list_bookmarks(
    handle: tauri::AppHandle,
    project_id: &str,
    range: Option<ops::Range<u128>>,
) -> Result<Vec<bookmarks::Bookmark>, Error> {
    let app = handle.state::<app::App>();
    let bookmarks = app
        .list_bookmarks(project_id, range)
        .context("failed to list bookmarks")?;
    Ok(bookmarks)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn fetch_from_target(handle: tauri::AppHandle, project_id: &str) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.fetch_from_target(project_id)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn mark_resolved(
    handle: tauri::AppHandle,
    project_id: &str,
    path: &str,
) -> Result<(), Error> {
    let app = handle.state::<app::App>();
    app.mark_resolved(project_id, path)?;
    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_set_global_config(
    handle: tauri::AppHandle,
    key: &str,
    value: &str,
) -> Result<String, Error> {
    let app = handle.state::<app::App>();
    let result = app.git_set_global_config(key, value)?;
    Ok(result)
}

#[tauri::command(async)]
#[instrument(skip(handle))]
async fn git_get_global_config(
    handle: tauri::AppHandle,
    key: &str,
) -> Result<Option<String>, Error> {
    let app = handle.state::<app::App>();
    let result = app.git_get_global_config(key)?;
    Ok(result)
}

#[tokio::main]
async fn main() {
    tauri::async_runtime::set(tokio::runtime::Handle::current());

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
        .on_system_tray_event(|app_handle, event| {
            if let tauri::SystemTrayEvent::MenuItemClick { id, .. } = event {
                let app_title = app_handle.package_info().name.clone();
                let item_handle = app_handle.tray_handle().get_item(&id);
                match id.as_str() {
                    "quit" => {
                        tracing::info!("Quitting app");
                        app_handle.exit(0);
                    }
                    "toggle" => match get_window(app_handle) {
                        Some(window) => {
                            if window.is_visible().unwrap() {
                                hide_window(app_handle).expect("Failed to hide window");
                            } else {
                                show_window(app_handle).expect("Failed to show window");
                            }
                        }
                        None => {
                            create_window(app_handle).expect("Failed to create window");
                            item_handle
                                .set_title(format!("Hide {}", app_title))
                                .unwrap();
                        }
                    },
                    _ => {}
                }
            }
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                hide_window(&event.window().app_handle()).expect("Failed to hide window");
                api.prevent_close();
            }
        })
        .setup(move |tauri_app| {
            let window = create_window(&tauri_app.handle()).expect("Failed to create window");
            #[cfg(debug_assertions)]
            window.open_devtools();

            let app_handle = tauri_app.handle();

            logs::init(&app_handle);

            tracing::info!("Starting app");

            let analytics_cfg = if cfg!(debug_assertions) {
                analytics::Config {
                    posthog_token: Some("phc_t7VDC9pQELnYep9IiDTxrq2HLseY5wyT7pn0EpHM7rr"),
                }
            } else {
                analytics::Config {
                    posthog_token: Some("phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG"),
                }
            };
            let analytics_client = analytics::Client::new(&app_handle, analytics_cfg);
            tauri_app.manage(analytics_client);

            let watchers =
                watcher::Watchers::try_from(&app_handle).expect("failed to initialize watchers");
            tauri_app.manage(watchers);

            let zipper = zip::Zipper::try_from(&app_handle).expect("failed to initialize zipper");
            tauri_app.manage(zipper);

            let proxy = assets::Proxy::try_from(&app_handle).expect("failed to initialize proxy");
            tauri_app.manage(proxy);

            let database =
                database::Database::try_from(&app_handle).expect("failed to initialize database");
            app_handle.manage(database);

            let storage =
                storage::Storage::try_from(&app_handle).expect("failed to initialize storage");
            app_handle.manage(storage);

            let search =
                search::Searcher::try_from(&app_handle).expect("failed to initialize search");
            app_handle.manage(search);

            let vbranch_contoller = virtual_branches::controller::Controller::try_from(&app_handle)
                .expect("failed to initialize virtual branches controller");
            app_handle.manage(vbranch_contoller);

            let app: app::App =
                app::App::try_from(&tauri_app.app_handle()).expect("failed to initialize app");

            if let Some(user) = app.get_user().context("failed to get user")? {
                sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())))
            }

            app.init().context("failed to init app")?;

            app_handle.manage(app);

            Ok(())
        })
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            add_project,
            get_project,
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
            git_remote_branches,
            git_remote_branches_data,
            git_head,
            git_commit,
            git_stage,
            git_unstage,
            git_wd_diff,
            git_commit_diff,
            delete_all_data,
            get_logs_archive_path,
            get_project_archive_path,
            get_project_data_archive_path,
            upsert_bookmark,
            list_bookmarks,
            virtual_branches::commands::list_virtual_branches,
            virtual_branches::commands::create_virtual_branch,
            virtual_branches::commands::commit_virtual_branch,
            virtual_branches::commands::get_base_branch_data,
            virtual_branches::commands::set_base_branch,
            virtual_branches::commands::update_base_branch,
            virtual_branches::commands::update_virtual_branch,
            virtual_branches::commands::delete_virtual_branch,
            virtual_branches::commands::apply_branch,
            virtual_branches::commands::unapply_branch,
            virtual_branches::commands::push_virtual_branch,
            virtual_branches::commands::create_virtual_branch_from_branch,
            fetch_from_target,
            mark_resolved,
            git_set_global_config,
            git_get_global_config,
            keys::commands::get_public_key,
        ])
        .build(tauri_context)
        .expect("Failed to build tauri app")
        .run(|app_handle, event| match event {
            tauri::RunEvent::WindowEvent {
                event: tauri::WindowEvent::Focused(is_focused),
                ..
            } => {
                if is_focused {
                    set_toggle_menu_hide(app_handle).expect("Failed to set toggle menu hide");
                } else {
                    set_toggle_menu_show(app_handle).expect("Failed to set toggle menu show");
                }
            }
            tauri::RunEvent::ExitRequested { api, .. } => {
                hide_window(app_handle).expect("Failed to hide window");
                api.prevent_exit();
            }
            _ => {}
        });
}

fn get_window(handle: &tauri::AppHandle) -> Option<tauri::Window> {
    handle.get_window("main")
}

#[cfg(not(target_os = "macos"))]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    let app_title = handle.package_info().name.clone();
    let window =
        tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
            .resizable(true)
            .title(app_title)
            .disable_file_drop_handler()
            .min_inner_size(600.0, 300.0)
            .inner_size(800.0, 600.0)
            .build()?;
    tracing::info!("app window created");
    Ok(window)
}

#[cfg(target_os = "macos")]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    let window =
        tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
            .resizable(true)
            .title(handle.package_info().name.clone())
            .min_inner_size(1024.0, 600.0)
            .inner_size(1024.0, 600.0)
            .hidden_title(true)
            .disable_file_drop_handler()
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .build()?;
    tracing::info!("window created");
    Ok(window)
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
