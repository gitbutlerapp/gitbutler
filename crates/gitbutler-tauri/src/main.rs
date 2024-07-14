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

use gitbutler_repo::credentials::Helper;
use gitbutler_storage::storage;
use gitbutler_tauri::{
    app, askpass, commands, config, github, logs, menu, projects, remotes, repo, secret, undo,
    users, virtual_branches, zip, WindowState,
};
use tauri::{generate_context, Manager};
use tauri_plugin_log::LogTarget;

fn main() {
    let tauri_context = generate_context!();
    gitbutler_secret::secret::set_application_namespace(
        &tauri_context.config().tauri.bundle.identifier,
    );

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

            tauri::Builder::default()
                .setup(move |tauri_app| {
                    let window =
                        gitbutler_tauri::window::create(&tauri_app.handle(), "main", "index.html".into()).expect("Failed to create window");
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

                    // On MacOS, in dev mode with debug assertions, we encounter popups each time
                    // the binary is rebuilt. To counter that, use a git-credential based implementation.
                    // This isn't an issue for actual release build (i.e. nightly, production),
                    // hence the specific condition.
                    if cfg!(debug_assertions) && cfg!(target_os = "macos") {
                        gitbutler_secret::secret::git_credentials::setup().ok();
                    }

                    // SAFETY(qix-): This is safe because we're initializing the askpass broker here,
                    // SAFETY(qix-): before any other threads would ever access it.
                    unsafe {
                        gitbutler_repo::askpass::init({
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

                    app_handle.manage(WindowState::new(app_handle.clone()));

                    let projects_storage_controller = gitbutler_project::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(projects_storage_controller.clone());

                    let users_storage_controller = gitbutler_user::storage::Storage::new(storage_controller.clone());
                    app_handle.manage(users_storage_controller.clone());

                    let users_controller = gitbutler_user::Controller::new(users_storage_controller.clone());
                    app_handle.manage(users_controller.clone());

                    let projects_controller = gitbutler_project::Controller::new(
                        app_data_dir.clone(),
                        projects_storage_controller.clone()
                    );
                    app_handle.manage(projects_controller.clone());

                    let zipper = gitbutler_feedback::zipper::Zipper::new(&app_cache_dir);
                    app_handle.manage(zipper.clone());

                    app_handle.manage(gitbutler_feedback::controller::Controller::new(app_data_dir.clone(), app_log_dir.clone(), zipper.clone(), projects_controller.clone()));

                    let git_credentials_controller = Helper::default();
                    app_handle.manage(git_credentials_controller.clone());

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
                    commands::git_remove_global_config,
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
                    projects::commands::open_project_in_window,
                    repo::commands::git_get_local_config,
                    repo::commands::git_set_local_config,
                    repo::commands::check_signing_settings,
                    virtual_branches::commands::list_virtual_branches,
                    virtual_branches::commands::create_virtual_branch,
                    virtual_branches::commands::commit_virtual_branch,
                    virtual_branches::commands::get_base_branch_data,
                    virtual_branches::commands::set_base_branch,
                    virtual_branches::commands::update_base_branch,
                    virtual_branches::commands::integrate_upstream_commits,
                    virtual_branches::commands::update_virtual_branch,
                    virtual_branches::commands::delete_virtual_branch,
                    virtual_branches::commands::convert_to_real_branch,
                    virtual_branches::commands::unapply_ownership,
                    virtual_branches::commands::reset_files,
                    virtual_branches::commands::push_virtual_branch,
                    virtual_branches::commands::create_virtual_branch_from_branch,
                    virtual_branches::commands::can_apply_remote_branch,
                    virtual_branches::commands::list_remote_commit_files,
                    virtual_branches::commands::reset_virtual_branch,
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
                    secret::secret_get_global,
                    secret::secret_set_global,
                    undo::list_snapshots,
                    undo::restore_snapshot,
                    undo::snapshot_diff,
                    config::get_gb_config,
                    config::set_gb_config,
                    menu::menu_item_set_enabled,
                    menu::get_editor_link_scheme,
                    github::commands::init_device_oauth,
                    github::commands::check_auth_status,
                    askpass::commands::submit_prompt_response,
                    remotes::list_remotes,
                    remotes::add_remote
                ])
                .menu(menu::build(tauri_context.package_info()))
                .on_menu_event(|event|menu::handle_event(&event))
                .on_window_event(|event| {
                    let window = event.window();
                    match event.event() {
                        #[cfg(target_os = "macos")]
                        tauri::WindowEvent::CloseRequested { api, .. } => {
                            if window.app_handle().windows().len() == 1 {
                                tracing::debug!("Hiding all application windows and preventing exit");
                                window.app_handle().hide().ok();
                                api.prevent_close();
                            }
                        }
                        tauri::WindowEvent::Destroyed  => {
                            window.app_handle()
                                .state::<WindowState>()
                                .remove(window.label());
                        }
                        tauri::WindowEvent::Focused(focused) if *focused => {
                            window.app_handle()
                                .state::<WindowState>()
                                .flush(window.label()).ok();
                        },
                        _ => {}
                    }
                })
                .build(tauri_context)
                .expect("Failed to build tauri app")
                .run(|app_handle, event| {
                    #[cfg(target_os = "macos")]
                    if let tauri::RunEvent::ExitRequested { api, .. } = event {
                        tracing::debug!("Hiding all windows and preventing exit");
                        app_handle.hide().ok();
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
