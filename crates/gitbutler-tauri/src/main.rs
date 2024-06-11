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

use gitbutler_core::{assets, git, storage};
use gitbutler_tauri::{
    app, askpass, commands, config, github, keys, logs, menu, projects, remotes, undo, users,
    virtual_branches, watcher, zip,
};
use tauri::{generate_context, Manager};
use tauri_plugin_log::LogTarget;

fn main() {
    let tauri_context = generate_context!();

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

                    // SAFETY(qix-): This is safe because we're initializing the askpass broker here,
                    // SAFETY(qix-): before any other threads would ever access it.
                    unsafe {
                        gitbutler_core::askpass::init({
                            let handle = app_handle.clone();
                            move |event| {
                                handle.emit_all("git_prompt", event).expect("tauri event emission doesn't fail in practice")
                            }
                        });
                    }

                    let app_data_dir = app_handle.path_resolver().app_data_dir().expect("missing app data dir");
                    let app_cache_dir = app_handle.path_resolver().app_cache_dir().expect("missing app cache dir");
                    let app_log_dir = app_handle.path_resolver().app_log_dir().expect("missing app log dir");

                    std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
                    std::fs::create_dir_all(&app_cache_dir).expect("failed to create cache dir");

                    tracing::info!(version = %app_handle.package_info().version, name = %app_handle.package_info().name, "starting app");

                    let storage_controller = storage::Storage::new(&app_data_dir);
                    app_handle.manage(storage_controller.clone());

                    let watcher_controller = watcher::Watchers::new(app_handle.clone());
                    app_handle.manage(watcher_controller.clone());

                    let projects_storage_controller = gitbutler_core::projects::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(projects_storage_controller.clone());

                    let users_storage_controller = gitbutler_core::users::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(users_storage_controller.clone());

                    let users_controller = gitbutler_core::users::Controller::new(users_storage_controller.clone());
                    app_handle.manage(users_controller.clone());

                    let projects_controller = gitbutler_core::projects::Controller::new(
                        app_data_dir.clone(),
                        projects_storage_controller.clone(),
                        Some(watcher_controller.clone())
                    );
                    app_handle.manage(projects_controller.clone());

                    app_handle.manage(assets::Proxy::new(app_cache_dir.join("images")));

                    let zipper = gitbutler_core::zip::Zipper::new(&app_cache_dir);
                    app_handle.manage(zipper.clone());

                    app_handle.manage(gitbutler_core::zip::Controller::new(app_data_dir.clone(), app_log_dir.clone(), zipper.clone(), projects_controller.clone()));

                    let keys_storage_controller = gitbutler_core::keys::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(keys_storage_controller.clone());

                    let keys_controller = gitbutler_core::keys::Controller::new(keys_storage_controller.clone());
                    app_handle.manage(keys_controller.clone());

                    let git_credentials_controller = git::credentials::Helper::new(
                        keys_controller.clone(),
                    );
                    app_handle.manage(git_credentials_controller.clone());

                    app_handle.manage(gitbutler_core::virtual_branches::controller::Controller::new(
                        projects_controller.clone(),
                        users_controller.clone(),
                        git_credentials_controller.clone(),
                    ));

                    let remotes_controller = gitbutler_core::remotes::controller::Controller::new(
                        projects_controller.clone(),
                    );

                    app_handle.manage(remotes_controller.clone());

                    let app = app::App::new(
                        projects_controller,
                    );

                    app_handle.manage(app);

                    Ok(())
                })
                .plugin(tauri_plugin_window_state::Builder::default().build())
                .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
                .plugin(tauri_plugin_context_menu::init())
                .plugin(tauri_plugin_store::Builder::default().build())
                .plugin(log.build())
                .invoke_handler(tauri::generate_handler![
                    commands::git_remote_branches,
                    commands::git_head,
                    commands::delete_all_data,
                    commands::mark_resolved,
                    commands::git_set_global_config,
                    commands::git_get_global_config,
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
                    projects::commands::set_project_active,
                    projects::commands::git_get_local_config,
                    projects::commands::git_set_local_config,
                    projects::commands::check_signing_settings,
                    virtual_branches::commands::list_virtual_branches,
                    virtual_branches::commands::create_virtual_branch,
                    virtual_branches::commands::commit_virtual_branch,
                    virtual_branches::commands::get_base_branch_data,
                    virtual_branches::commands::set_base_branch,
                    virtual_branches::commands::update_base_branch,
                    virtual_branches::commands::integrate_upstream_commits,
                    virtual_branches::commands::update_virtual_branch,
                    virtual_branches::commands::delete_virtual_branch,
                    virtual_branches::commands::apply_branch,
                    virtual_branches::commands::convert_to_real_branch,
                    virtual_branches::commands::unapply_ownership,
                    virtual_branches::commands::reset_files,
                    virtual_branches::commands::push_virtual_branch,
                    virtual_branches::commands::create_virtual_branch_from_branch,
                    virtual_branches::commands::can_apply_remote_branch,
                    virtual_branches::commands::list_remote_commit_files,
                    virtual_branches::commands::reset_virtual_branch,
                    virtual_branches::commands::cherry_pick_onto_virtual_branch,
                    virtual_branches::commands::amend_virtual_branch,
                    virtual_branches::commands::move_commit_file,
                    virtual_branches::commands::undo_commit,
                    virtual_branches::commands::insert_blank_commit,
                    virtual_branches::commands::reorder_commit,
                    virtual_branches::commands::update_commit_message,
                    virtual_branches::commands::list_remote_branches,
                    virtual_branches::commands::get_remote_branch_data,
                    virtual_branches::commands::squash_branch_commit,
                    virtual_branches::commands::fetch_from_remotes,
                    virtual_branches::commands::move_commit,
                    undo::list_snapshots,
                    undo::restore_snapshot,
                    undo::snapshot_diff,
                    config::get_gb_config,
                    config::set_gb_config,
                    menu::menu_item_set_enabled,
                    menu::resolve_vscode_variant,
                    keys::commands::get_public_key,
                    github::commands::init_device_oauth,
                    github::commands::check_auth_status,
                    askpass::commands::submit_prompt_response,
                    remotes::list_remotes,
                    remotes::add_remote
                ])
                .menu(menu::build(tauri_context.package_info()))
                .on_menu_event(|event|menu::handle_event(&event))
                .on_window_event(|event| {
                    if let tauri::WindowEvent::Focused(focused) = event.event() {
                        if *focused {
                            tokio::task::spawn(async move {
                                let _ = event.window().app_handle()
                                    .state::<watcher::Watchers>()
                                    .flush().await;
                            });
                        }
                    }
                })
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
