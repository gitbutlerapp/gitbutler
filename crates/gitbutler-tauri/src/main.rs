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

use std::sync::Arc;

use but_api::App;
use but_broadcaster::Broadcaster;
use but_settings::AppSettingsWithDiskSync;
use gitbutler_tauri::csp::csp_with_extras;
use gitbutler_tauri::{
    action, askpass, bot, claude, cli, commands, config, diff, env, forge, github, logs, menu,
    modes, open, projects, remotes, repo, rules, secret, settings, stack, undo, users,
    virtual_branches, workspace, zip, WindowState,
};
use tauri::Emitter;
use tauri::{generate_context, Manager};
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

fn main() {
    let performance_logging = std::env::var_os("GITBUTLER_PERFORMANCE_LOG").is_some();
    let tauri_debug_logging = std::env::var_os("GITBUTLER_TAURI_DEBUG_LOG").is_some();

    gitbutler_project::configure_git2();
    let mut tauri_context = generate_context!();
    gitbutler_secret::secret::set_application_namespace(&tauri_context.config().identifier);

    let config_dir = dirs::config_dir()
        .expect("missing config dir")
        .join("gitbutler");
    std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
    let mut app_settings =
        AppSettingsWithDiskSync::new(config_dir.clone()).expect("failed to create app settings");

    if let Ok(updated_csp) = csp_with_extras(
        tauri_context.config().app.security.csp.as_ref().cloned(),
        &app_settings,
    ) {
        tauri_context.config_mut().app.security.csp = updated_csp;
    };
    let app_settings_for_menu = app_settings.clone();

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            tauri::async_runtime::set(tokio::runtime::Handle::current());

            let log = tauri_plugin_log::Builder::default()
                .target(Target::new(TargetKind::LogDir {
                    file_name: Some("ui-logs".to_string()),
                }))
                .level(if tauri_debug_logging {
                    log::LevelFilter::Debug
                } else {
                    log::LevelFilter::Error
                });

            let builder = tauri::Builder::default()
                .setup(move |tauri_app| {
                    let window = gitbutler_tauri::window::create(
                        tauri_app.handle(),
                        "main",
                        "index.html".into(),
                    )
                    .expect("Failed to create window");

                    // TODO(mtsgrd): Is there a better way to disable devtools in E2E tests?
                    #[cfg(debug_assertions)]
                    if tauri_app.config().product_name != Some("GitButler Test".to_string()) {
                        window.open_devtools();
                    }

                    let app_handle = tauri_app.handle();

                    logs::init(app_handle, performance_logging);

                    inherit_interactive_login_shell_environment_if_not_launched_from_terminal();

                    tracing::info!(
                        "system git executable for fetch/push: {git:?}",
                        git = gix::path::env::exe_invocation(),
                    );
                    if cfg!(windows) {
                        tracing::info!("system git bash: {bash:?}", bash = gix::path::env::shell());
                    } else {
                        tracing::info!("SHELL env: {var:?}", var = std::env::var_os("SHELL"));
                    }

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
                        gitbutler_repo_actions::askpass::init({
                            let handle = app_handle.clone();
                            move |event| {
                                handle
                                    .emit("git_prompt", event)
                                    .expect("tauri event emission doesn't fail in practice")
                            }
                        });
                    }

                    let (app_data_dir, app_cache_dir, app_log_dir) = {
                        let paths = app_handle.path();
                        (
                            paths.app_data_dir().expect("missing app data dir"),
                            paths.app_cache_dir().expect("missing app cache dir"),
                            paths.app_log_dir().expect("missing app log dir"),
                        )
                    };
                    std::fs::create_dir_all(&app_data_dir).expect("failed to create app data dir");
                    std::fs::create_dir_all(&app_cache_dir).expect("failed to create cache dir");

                    tracing::info!(version = %app_handle.package_info().version,
                                   name = %app_handle.package_info().name, "starting app");

                    app_handle.manage(WindowState::new(app_handle.clone()));

                    app_settings.watch_in_background({
                        let app_handle = app_handle.clone();
                        move |app_settings| {
                            gitbutler_tauri::ChangeForFrontend::from(app_settings).send(&app_handle)
                        }
                    })?;

                    let broadcaster = Arc::new(Mutex::new(Broadcaster::new()));

                    let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
                    let broadcaster2 = broadcaster.clone();
                    tokio::spawn(async move {
                        broadcaster2
                            .lock()
                            .await
                            .register_sender(&uuid::Uuid::new_v4(), send)
                    });

                    let window2 = window.clone();
                    std::thread::spawn(move || {
                        while let Some(message) = recv.blocking_recv() {
                            window2.emit(&message.name, message.payload).unwrap();
                        }
                    });

                    let archival = Arc::new(but_feedback::Archival {
                        cache_dir: app_cache_dir.clone(),
                        logs_dir: app_log_dir.clone(),
                    });
                    let app = App {
                        broadcaster: broadcaster.clone(),
                        archival: archival.clone(),
                        claudes: Default::default(),
                    };

                    app_handle.manage(app_settings);
                    app_handle.manage(app);

                    tauri_app.on_menu_event(move |handle, event| {
                        menu::handle_event(handle, &window.clone(), &event)
                    });

                    #[cfg(target_os = "macos")]
                    use tauri::LogicalPosition;
                    #[cfg(target_os = "macos")]
                    use tauri_plugin_trafficlights_positioner::WindowExt;
                    #[cfg(target_os = "macos")]
                    // NOTE: Make sure you only call this ONCE per window.
                    {
                        if let Some(window) = tauri_app.get_window("main") {
                            #[cfg(target_os = "macos")]
                            // NOTE: Make sure you only call this ONCE per window.
                            window.setup_traffic_lights_inset(LogicalPosition::new(16.0, 25.0))?;
                        };
                    }

                    Ok(())
                })
                .plugin(tauri_plugin_http::init())
                .plugin(tauri_plugin_shell::init())
                .plugin(tauri_plugin_os::init())
                .plugin(tauri_plugin_process::init())
                .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
                .plugin(tauri_plugin_updater::Builder::new().build())
                .plugin(tauri_plugin_dialog::init())
                .plugin(tauri_plugin_fs::init())
                .plugin(tauri_plugin_clipboard_manager::init())
                // .plugin(tauri_plugin_context_menu::init())
                .plugin(tauri_plugin_store::Builder::default().build())
                .plugin(log.build())
                .invoke_handler(tauri::generate_handler![
                    commands::git_remote_branches,
                    commands::git_head,
                    commands::delete_all_data,
                    commands::git_set_global_config,
                    commands::git_remove_global_config,
                    commands::git_get_global_config,
                    commands::git_test_push,
                    commands::git_test_fetch,
                    commands::git_index_size,
                    zip::commands::get_logs_archive_path,
                    zip::commands::get_project_archive_path,
                    zip::commands::get_anonymous_graph_path,
                    users::set_user,
                    users::delete_user,
                    users::get_user,
                    projects::add_project,
                    projects::get_project,
                    projects::update_project,
                    projects::delete_project,
                    projects::list_projects,
                    projects::set_project_active,
                    projects::open_project_in_window,
                    repo::git_get_local_config,
                    repo::git_set_local_config,
                    repo::check_signing_settings,
                    repo::git_clone_repository,
                    repo::get_uncommited_files,
                    repo::get_commit_file,
                    repo::get_workspace_file,
                    repo::pre_commit_hook,
                    repo::pre_commit_hook_diffspecs,
                    repo::post_commit_hook,
                    repo::message_hook,
                    virtual_branches::create_virtual_branch,
                    virtual_branches::delete_local_branch,
                    virtual_branches::get_base_branch_data,
                    virtual_branches::set_base_branch,
                    virtual_branches::push_base_branch,
                    virtual_branches::integrate_upstream_commits,
                    virtual_branches::update_stack_order,
                    virtual_branches::unapply_stack,
                    virtual_branches::create_virtual_branch_from_branch,
                    virtual_branches::can_apply_remote_branch,
                    virtual_branches::list_commit_files,
                    virtual_branches::amend_virtual_branch,
                    virtual_branches::undo_commit,
                    virtual_branches::insert_blank_commit,
                    virtual_branches::reorder_stack,
                    virtual_branches::update_commit_message,
                    virtual_branches::find_git_branches,
                    virtual_branches::list_branches,
                    virtual_branches::get_branch_listing_details,
                    virtual_branches::squash_commits,
                    virtual_branches::fetch_from_remotes,
                    virtual_branches::move_commit,
                    virtual_branches::normalize_branch_name,
                    virtual_branches::upstream_integration_statuses,
                    virtual_branches::integrate_upstream,
                    virtual_branches::resolve_upstream_integration,
                    virtual_branches::find_commit,
                    stack::create_reference,
                    stack::create_branch,
                    stack::remove_branch,
                    stack::update_branch_name,
                    stack::update_branch_description,
                    stack::update_branch_pr_number,
                    stack::push_stack,
                    stack::push_stack_to_review,
                    secret::secret_get_global,
                    secret::secret_set_global,
                    undo::list_snapshots,
                    undo::restore_snapshot,
                    undo::snapshot_diff,
                    config::get_gb_config,
                    config::set_gb_config,
                    config::store_author_globally_if_unset,
                    config::get_author_info,
                    menu::menu_item_set_enabled,
                    github::init_device_oauth,
                    github::check_auth_status,
                    askpass::submit_prompt_response,
                    remotes::list_remotes,
                    remotes::add_remote,
                    modes::operating_mode,
                    modes::enter_edit_mode,
                    modes::save_edit_and_return_to_workspace,
                    modes::abort_edit_and_return_to_workspace,
                    modes::edit_initial_index_state,
                    modes::edit_changes_from_initial,
                    open::open_url,
                    open::show_in_finder,
                    forge::pr_templates,
                    forge::pr_template,
                    settings::get_app_settings,
                    settings::update_onboarding_complete,
                    settings::update_telemetry,
                    settings::update_feature_flags,
                    settings::update_telemetry_distinct_id,
                    settings::update_claude,
                    action::list_actions,
                    action::handle_changes,
                    action::list_workflows,
                    action::auto_commit,
                    action::auto_branch_changes,
                    action::absorb,
                    action::freestyle,
                    bot::bot,
                    cli::install_cli,
                    cli::cli_path,
                    rules::create_workspace_rule,
                    rules::delete_workspace_rule,
                    rules::update_workspace_rule,
                    rules::list_workspace_rules,
                    workspace::stacks,
                    workspace::stack_details,
                    workspace::branch_details,
                    workspace::create_commit_from_worktree_changes,
                    workspace::amend_commit_from_worktree_changes,
                    workspace::discard_worktree_changes,
                    workspace::stash_into_branch,
                    workspace::canned_branch_name,
                    workspace::target_commits,
                    workspace::move_changes_between_commits,
                    workspace::uncommit_changes,
                    workspace::split_branch,
                    workspace::split_branch_into_dependent_branch,
                    diff::changes_in_worktree,
                    diff::commit_details,
                    diff::changes_in_branch,
                    diff::tree_change_diffs,
                    diff::assign_hunk,
                    claude::claude_get_session_details,
                    // Debug-only - not for production!
                    #[cfg(debug_assertions)]
                    env::env_vars,
                    #[cfg(unix)]
                    workspace::show_graph_svg,
                    claude::claude_send_message,
                    claude::claude_get_messages,
                    claude::claude_list_permission_requests,
                    claude::claude_update_permission_request,
                    claude::claude_cancel_session,
                    claude::claude_check_available
                ])
                .menu(move |handle| menu::build(handle, &app_settings_for_menu))
                .on_window_event(|window, event| match event {
                    #[cfg(target_os = "macos")]
                    tauri::WindowEvent::CloseRequested { .. } => {
                        let app_handle = window.app_handle();
                        if app_handle.windows().len() == 1 {
                            app_handle.cleanup_before_exit();
                            app_handle.exit(0);
                        }
                    }
                    tauri::WindowEvent::Destroyed => {
                        window
                            .app_handle()
                            .state::<WindowState>()
                            .remove(window.label());
                    }
                    tauri::WindowEvent::Focused(focused) if *focused => {
                        window
                            .app_handle()
                            .state::<WindowState>()
                            .flush(window.label())
                            .ok();
                    }
                    _ => {}
                });

            #[cfg(not(target_os = "linux"))]
            let builder = builder.plugin(tauri_plugin_window_state::Builder::default().build());

            builder
                .build(tauri_context)
                .expect("Failed to build tauri app")
                .run(|app_handle, event| {
                    let _ = (app_handle, event);
                });
        });
}

/// Launch a shell as interactive login shell, similar to what a login terminal would do if we are not already in a terminal.
///
/// That way, each process launched by the backend will act similar to what users would get in their terminal,
/// something vital to act more similar to Git, which is also launched from an interactive shell most of the time.
fn inherit_interactive_login_shell_environment_if_not_launched_from_terminal() {
    if std::env::var_os("TERM").is_some() {
        tracing::info!(
            "TERM is set - assuming the app is run from a terminal with suitable environment variables"
        );
        return;
    }

    fn doit() {
        if let Some(terminal_vars) = but_core::cmd::extract_interactive_login_shell_environment() {
            tracing::info!("Inheriting static interactive shell environment, valid for the entire runtime of the application");
            for (key, value) in terminal_vars {
                std::env::set_var(key, value);
            }
        } else {
            tracing::info!(
                "SHELL variable isn't set - launching with default GUI application environment "
            );
        }
    }
    if cfg!(windows) {
        // This can be slow on Windows IF it runs, so background it.
        // This could also trigger a race, so let's only do it when we must, and hope that this works
        // in the few occasions where it may run.
        std::thread::spawn(doit);
    } else {
        doit();
    }
}
