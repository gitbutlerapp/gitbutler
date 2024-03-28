#![feature(error_generic_member_access)]
#![cfg_attr(windows, feature(windows_by_handle))]
#![cfg_attr(
    all(windows, not(test), not(debug_assertions)),
    windows_subsystem = "windows"
)]
// FIXME(qix-): Stuff we want to fix but don't have a lot of time for.
// FIXME(qix-): PRs welcome!
#![allow(
    clippy::used_underscore_binding,
    clippy::module_name_repetitions,
    clippy::struct_field_names,
    clippy::too_many_lines
)]

use gitbutler_app::analytics;
use gitbutler_app::app;
use gitbutler_app::askpass;
use gitbutler_app::assets;
use gitbutler_app::commands;
use gitbutler_app::database;
use gitbutler_app::deltas;
use gitbutler_app::git;
use gitbutler_app::github;
use gitbutler_app::keys;
use gitbutler_app::logs;
use gitbutler_app::menu;
use gitbutler_app::projects;
use gitbutler_app::sentry;
use gitbutler_app::sessions;
use gitbutler_app::storage;
use gitbutler_app::users;
use gitbutler_app::virtual_branches;
use gitbutler_app::watcher;
#[cfg(target_os = "windows")]
use gitbutler_app::windows;
use gitbutler_app::zip;

use std::path::PathBuf;

use anyhow::Context;
use tauri::{generate_context, Manager, Wry};

use tauri_plugin_log::LogTarget;
use tauri_plugin_store::{with_store, JsonValue, StoreCollection};

#[allow(clippy::too_many_lines)]
fn main() {
    let tauri_context = generate_context!();

    let app_name = tauri_context.package_info().name.clone();
    let app_version = tauri_context.package_info().version.clone().to_string();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tauri::async_runtime::set(tokio::runtime::Handle::current());

            let log = tauri_plugin_log::Builder::default()
                .log_name("ui-logs")
                .target(LogTarget::LogDir)
                .level(log::LevelFilter::Error);

            let builder = tauri::Builder::default();

            #[cfg(target_os = "macos")]
            let builder = builder
                .on_window_event(|event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                        hide_window(&event.window().app_handle()).expect("Failed to hide window");
                        api.prevent_close();
                    }
                });

            builder
                .setup(move |tauri_app| {
                    let window =
                        create_window(&tauri_app.handle()).expect("Failed to create window");
                    #[cfg(debug_assertions)]
                    window.open_devtools();

                    tokio::task::spawn(async move {
                        let mut six_hours = tokio::time::interval(tokio::time::Duration::new(6 * 60 * 60, 0));
                        loop {
                            six_hours.tick().await;
                            _ = window.emit_and_trigger("tauri://update", ());
                        }
                    });

                    let app_handle = tauri_app.handle();

                    logs::init(&app_handle);

                    let app_data_dir = app_handle.path_resolver().app_data_dir().expect("missing app data dir");
                    let app_cache_dir = app_handle.path_resolver().app_cache_dir().expect("missing app cache dir");
                    let app_log_dir = app_handle.path_resolver().app_log_dir().expect("missing app log dir");

                    std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
                    std::fs::create_dir_all(&app_cache_dir).expect("failed to create cache dir");

                    tracing::info!(version = %app_handle.package_info().version, name = %app_handle.package_info().name, "starting app");

                    let askpass_broker = askpass::AskpassBroker::init(app_handle.clone());
                    app_handle.manage(askpass_broker);

                    let storage_controller = storage::Storage::new(&app_data_dir);
                    app_handle.manage(storage_controller.clone());

                    let watcher_controller = watcher::Watchers::new(app_handle.clone());
                    app_handle.manage(watcher_controller.clone());

                    let projects_storage_controller = projects::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(projects_storage_controller.clone());

                    let users_storage_controller = users::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(users_storage_controller.clone());

                    let users_controller = users::Controller::new(users_storage_controller.clone());
                    app_handle.manage(users_controller.clone());

                    let projects_controller = projects::Controller::new(
                        app_data_dir.clone(),
                        projects_storage_controller.clone(),
                        users_controller.clone(),
                        Some(watcher_controller.clone())
                    );
                    app_handle.manage(projects_controller.clone());

                    app_handle.manage(assets::Proxy::new(app_cache_dir.join("images")));

                    let database_controller = database::Database::open_in_directory(&app_data_dir).expect("failed to open database");
                    app_handle.manage(database_controller.clone());

                    let zipper = zip::Zipper::new(&app_cache_dir);
                    app_handle.manage(zipper.clone());

                    app_handle.manage(zip::Controller::new(app_data_dir.clone(), app_log_dir.clone(), zipper.clone(), projects_controller.clone()));

                    let deltas_database_controller = deltas::database::Database::new(database_controller.clone());
                    app_handle.manage(deltas_database_controller.clone());

                    let deltas_controller = deltas::Controller::new(deltas_database_controller.clone());
                    app_handle.manage(deltas_controller);

                    let keys_storage_controller = keys::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(keys_storage_controller.clone());

                    let keys_controller = keys::Controller::new(keys_storage_controller.clone());
                    app_handle.manage(keys_controller.clone());

                    let git_credentials_controller = git::credentials::Helper::new(
                        keys_controller.clone(),
                        users_controller.clone(),
                        std::env::var_os("HOME").map(PathBuf::from)
                    );
                    app_handle.manage(git_credentials_controller.clone());

                    app_handle.manage(virtual_branches::controller::Controller::new(
                        app_data_dir.clone(),
                        projects_controller.clone(),
                        users_controller.clone(),
                        keys_controller.clone(),
                        git_credentials_controller.clone(),
                    ));

                    let stores = tauri_app.state::<StoreCollection<Wry>>();
                    if let Some(path) = app_handle.path_resolver().app_config_dir().map(|path| path.join(PathBuf::from("settings.json"))) {
                        if let Ok((metrics_enabled, error_reporting_enabled)) = with_store(app_handle.clone(), stores, path, |store| {
                            let metrics_enabled = store.get("appMetricsEnabled")
                                .and_then(JsonValue::as_bool)
                                .unwrap_or(true);
                            let error_reporting_enabled = store.get("appErrorReportingEnabled")
                                .and_then(JsonValue::as_bool)
                                .unwrap_or(true);
                            Ok((metrics_enabled, error_reporting_enabled))
                        }) {
                            if metrics_enabled {
                                let analytics_cfg = if cfg!(debug_assertions) {
                                    analytics::Config {
                                        posthog_token: Some("phc_t7VDC9pQELnYep9IiDTxrq2HLseY5wyT7pn0EpHM7rr"),
                                    }
                                } else {
                                    analytics::Config {
                                        posthog_token: Some("phc_yJx46mXv6kA5KTuM2eEQ6IwNTgl5YW3feKV5gi7mfGG"),
                                    }
                                };
                                let analytics_client = analytics::Client::new(&app_handle, &analytics_cfg);
                                tauri_app.manage(analytics_client);
                            }

                            if error_reporting_enabled {
                                let _guard = sentry::init(app_name.as_str(), app_version);
                                sentry::configure_scope(users_controller.get_user().context("failed to get user")?.as_ref());
                            }
                        };
                    }

                    let sessions_database_controller = sessions::database::Database::new(database_controller.clone());
                    app_handle.manage(sessions_database_controller.clone());

                    app_handle.manage(sessions::Controller::new(
                        app_data_dir.clone(),
                        sessions_database_controller.clone(),
                        projects_controller.clone(),
                        users_controller.clone(),
                    ));

                    let app = app::App::new(
                        app_data_dir,
                        projects_controller,
                        users_controller,
                        watcher_controller,
                        sessions_database_controller,
                    );

                    app.init().context("failed to init app")?;

                    app_handle.manage(app);

                    Ok(())
                })
                .plugin(tauri_plugin_window_state::Builder::default().build())
                .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
                .plugin(tauri_plugin_context_menu::init())
                .plugin(tauri_plugin_store::Builder::default().build())
                .plugin(log.build())
                .invoke_handler(tauri::generate_handler![
                    commands::list_session_files,
                    commands::git_remote_branches,
                    commands::git_head,
                    commands::delete_all_data,
                    commands::mark_resolved,
                    commands::git_set_global_config,
                    commands::git_get_global_config,
                    commands::project_flush_and_push,
                    commands::git_test_push,
                    commands::git_test_fetch,
                    commands::git_index_size,
                    zip::commands::get_logs_archive_path,
                    zip::commands::get_project_archive_path,
                    zip::commands::get_project_data_archive_path,
                    users::commands::set_user,
                    users::commands::delete_user,
                    users::commands::get_user,
                    projects::commands::add_project,
                    projects::commands::get_project,
                    projects::commands::update_project,
                    projects::commands::delete_project,
                    projects::commands::list_projects,
                    projects::commands::git_get_local_config,
                    projects::commands::git_set_local_config,
                    sessions::commands::list_sessions,
                    deltas::commands::list_deltas,
                    virtual_branches::commands::list_virtual_branches,
                    virtual_branches::commands::create_virtual_branch,
                    virtual_branches::commands::commit_virtual_branch,
                    virtual_branches::commands::get_base_branch_data,
                    virtual_branches::commands::set_base_branch,
                    virtual_branches::commands::update_base_branch,
                    virtual_branches::commands::merge_virtual_branch_upstream,
                    virtual_branches::commands::update_virtual_branch,
                    virtual_branches::commands::delete_virtual_branch,
                    virtual_branches::commands::apply_branch,
                    virtual_branches::commands::unapply_branch,
                    virtual_branches::commands::unapply_ownership,
                    virtual_branches::commands::reset_files,
                    virtual_branches::commands::push_virtual_branch,
                    virtual_branches::commands::create_virtual_branch_from_branch,
                    virtual_branches::commands::can_apply_virtual_branch,
                    virtual_branches::commands::can_apply_remote_branch,
                    virtual_branches::commands::list_remote_commit_files,
                    virtual_branches::commands::reset_virtual_branch,
                    virtual_branches::commands::cherry_pick_onto_virtual_branch,
                    virtual_branches::commands::amend_virtual_branch,
                    virtual_branches::commands::list_remote_branches,
                    virtual_branches::commands::get_remote_branch_data,
                    virtual_branches::commands::squash_branch_commit,
                    virtual_branches::commands::fetch_from_target,
                    virtual_branches::commands::move_commit,
                    virtual_branches::commands::save_vbranches_state,
                    menu::menu_item_set_enabled,
                    keys::commands::get_public_key,
                    github::commands::init_device_oauth,
                    github::commands::check_auth_status,
                    askpass::commands::submit_prompt_response,
                ])
                .menu(menu::build(tauri_context.package_info()))
                .on_menu_event(|event|menu::handle_event(&event))
                .build(tauri_context)
                .expect("Failed to build tauri app")
                .run(|app_handle, event| {
                    #[cfg(target_os = "macos")]
                    if let tauri::RunEvent::ExitRequested { api, .. } = event {
                        hide_window(app_handle).expect("Failed to hide window");
                        api.prevent_exit();
                    }

                    // To make the compiler happy.
                    #[cfg(not(target_os = "macos"))]
                    {
                        let _ = (app_handle, event);
                    }
                });
        });
}

#[cfg(not(target_os = "macos"))]
fn create_window(handle: &tauri::AppHandle) -> tauri::Result<tauri::Window> {
    let app_title = handle.package_info().name.clone();
    let window =
        tauri::WindowBuilder::new(handle, "main", tauri::WindowUrl::App("index.html".into()))
            .resizable(true)
            .title(app_title)
            .disable_file_drop_handler()
            .min_inner_size(800.0, 600.0)
            .inner_size(1160.0, 720.0)
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
            .min_inner_size(800.0, 600.0)
            .inner_size(1160.0, 720.0)
            .hidden_title(true)
            .disable_file_drop_handler()
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .build()?;
    tracing::info!("window created");
    Ok(window)
}

#[cfg(target_os = "macos")]
fn hide_window(handle: &tauri::AppHandle) -> tauri::Result<()> {
    handle.hide()?;
    Ok(())
}
