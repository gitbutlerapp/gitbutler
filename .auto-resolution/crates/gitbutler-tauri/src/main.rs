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

use anyhow::bail;
use but_api::{commit, diff, github, legacy};
use but_claude::{Broadcaster, Claude};
use but_settings::AppSettingsWithDiskSync;
use gitbutler_tauri::{
    WindowState, action, askpass, bot, claude, csp::csp_with_extras, env, logs, menu, projects,
    settings, zip,
};
use tauri::{Emitter, Manager, generate_context};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_log::{Target, TargetKind};
use tokio::sync::Mutex;

fn main() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    #[cfg(feature = "builtin-but")]
    {
        let exe = std::env::current_exe()?;
        if exe.file_stem().is_some_and(|stem| stem == "but") || std::env::args_os().count() > 1 {
            return runtime.block_on(but::handle_args(std::env::args_os()));
        }
    }
    let performance_logging = std::env::var_os("GITBUTLER_PERFORMANCE_LOG").is_some();
    let tauri_debug_logging = std::env::var_os("GITBUTLER_TAURI_DEBUG_LOG").is_some();

    gitbutler_project::configure_git2();
    let mut tauri_context = generate_context!();
    but_secret::secret::set_application_namespace(&tauri_context.config().identifier);

    let config_dir = but_path::app_config_dir().expect("missing config dir");
    std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
    let mut app_settings =
        AppSettingsWithDiskSync::new(config_dir.clone()).expect("failed to create app settings");

    if let Ok(updated_csp) = csp_with_extras(
        tauri_context.config().app.security.csp.as_ref().cloned(),
        &app_settings,
    ) {
        tauri_context.config_mut().app.security.csp = updated_csp;
    };

    if let Some(project_to_open) =
        std::env::var_os("GITBUTLER_PROJECT_DIR").map(std::path::PathBuf::from)
    {
        bail!(
            "GUI says: how do we tell the frontend to open: {}? \
               We could figure out the project-ID while that's important, and pass it along somehow",
            project_to_open.display()
        );
    }
    let app_settings_for_menu = app_settings.clone();
    runtime.block_on(async {
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

                but_action::cli::auto_fix_broken_but_cli_symlink();
                inherit_interactive_login_shell_environment_if_not_launched_from_terminal();
                migrate_projects().ok();

                tracing::info!(
                    "system git executable for fetch/push: {git:?}",
                    git = gix::path::env::exe_invocation(),
                );
                if cfg!(windows) {
                    tracing::info!("system git bash: {bash:?}", bash = gix::path::env::shell());
                } else {
                    tracing::info!("SHELL env: {var:?}", var = std::env::var_os("SHELL"));
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

                let claude = Claude {
                    broadcaster: broadcaster.clone(),
                    instance_by_stack: Default::default(),
                };
                let archival = but_feedback::Archival {
                    cache_dir: app_cache_dir.clone(),
                    logs_dir: app_log_dir.clone(),
                };
                app_handle.manage(archival);
                app_handle.manage(app_settings);
                app_handle.manage(claude);

                tauri_app.on_menu_event(move |handle, event| {
                    menu::handle_event(handle, &window.clone(), &event)
                });

                let app_handle_for_deep_link = app_handle.clone();
                app_handle.deep_link().on_open_url(move |_| {
                    // Get main window
                    if let Some(window) = app_handle_for_deep_link.get_window("main") {
                        let _ = window.unminimize();
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                });
                Ok(())
            })
            .plugin(tauri_plugin_single_instance::init(|_, _, _| {}))
            .plugin(tauri_plugin_http::init())
            .plugin(tauri_plugin_shell::init())
            .plugin(tauri_plugin_os::init())
            .plugin(tauri_plugin_process::init())
            .plugin(tauri_plugin_deep_link::init())
            .plugin(tauri_plugin_updater::Builder::new().build())
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_fs::init())
            .plugin(tauri_plugin_clipboard_manager::init())
            .plugin(tauri_plugin_store::Builder::default().build())
            .plugin(log.build())
            .invoke_handler(tauri::generate_handler![
                github::tauri_init_device_oauth::init_device_oauth,
                github::tauri_check_auth_status::check_auth_status,
                github::tauri_store_github_pat::store_github_pat,
                github::tauri_store_github_enterprise_pat::store_github_enterprise_pat,
                github::tauri_get_gh_user::get_gh_user,
                github::tauri_forget_github_account::forget_github_account,
                github::tauri_list_known_github_accounts::list_known_github_accounts,
                github::tauri_clear_all_github_tokens::clear_all_github_tokens,
                diff::tauri_commit_details::commit_details,
                diff::tauri_commit_details_with_line_stats::commit_details_with_line_stats,
                legacy::git::tauri_git_remote_branches::git_remote_branches,
                legacy::git::tauri_delete_all_data::delete_all_data,
                legacy::git::tauri_git_set_global_config::git_set_global_config,
                legacy::git::tauri_git_remove_global_config::git_remove_global_config,
                legacy::git::tauri_git_get_global_config::git_get_global_config,
                legacy::git::tauri_git_test_push::git_test_push,
                legacy::git::tauri_git_test_fetch::git_test_fetch,
                legacy::git::tauri_git_index_size::git_index_size,
                legacy::users::tauri_set_user::set_user,
                legacy::users::tauri_delete_user::delete_user,
                legacy::users::tauri_get_user::get_user,
                legacy::projects::tauri_add_project::add_project,
                legacy::projects::tauri_add_project_best_effort::add_project_best_effort,
                legacy::projects::tauri_get_project::get_project,
                legacy::projects::tauri_update_project::update_project,
                legacy::projects::tauri_delete_project::delete_project,
                legacy::projects::tauri_is_gerrit::is_gerrit,
                legacy::repo::tauri_git_get_local_config::git_get_local_config,
                legacy::repo::tauri_git_set_local_config::git_set_local_config,
                legacy::repo::tauri_check_signing_settings::check_signing_settings,
                legacy::repo::tauri_git_clone_repository::git_clone_repository,
                legacy::repo::tauri_get_uncommitted_files::get_uncommitted_files,
                legacy::repo::tauri_get_commit_file::get_commit_file,
                legacy::repo::tauri_get_workspace_file::get_workspace_file,
                legacy::repo::tauri_get_blob_file::get_blob_file,
                legacy::repo::tauri_find_files::find_files,
                legacy::repo::tauri_pre_commit_hook::pre_commit_hook,
                legacy::repo::tauri_pre_commit_hook_diffspecs::pre_commit_hook_diffspecs,
                legacy::repo::tauri_post_commit_hook::post_commit_hook,
                legacy::repo::tauri_message_hook::message_hook,
                legacy::cherry_apply::tauri_cherry_apply_status::cherry_apply_status,
                legacy::cherry_apply::tauri_cherry_apply::cherry_apply,
                legacy::virtual_branches::tauri_create_virtual_branch::create_virtual_branch,
                legacy::virtual_branches::tauri_delete_local_branch::delete_local_branch,
                legacy::virtual_branches::tauri_get_base_branch_data::get_base_branch_data,
                legacy::virtual_branches::tauri_set_base_branch::set_base_branch,
                legacy::virtual_branches::tauri_push_base_branch::push_base_branch,
                legacy::virtual_branches::tauri_integrate_upstream_commits::integrate_upstream_commits,
                legacy::virtual_branches::tauri_get_initial_integration_steps_for_branch::get_initial_integration_steps_for_branch,
                legacy::virtual_branches::tauri_update_stack_order::update_stack_order,
                legacy::virtual_branches::tauri_unapply_stack::unapply_stack,
                legacy::virtual_branches::tauri_create_virtual_branch_from_branch::create_virtual_branch_from_branch,
                legacy::virtual_branches::tauri_can_apply_remote_branch::can_apply_remote_branch,
                legacy::virtual_branches::tauri_list_commit_files::list_commit_files,
                legacy::virtual_branches::tauri_amend_virtual_branch::amend_virtual_branch,
                legacy::virtual_branches::tauri_undo_commit::undo_commit,
                legacy::virtual_branches::tauri_insert_blank_commit::insert_blank_commit,
                legacy::virtual_branches::tauri_reorder_stack::reorder_stack,
                legacy::virtual_branches::tauri_update_commit_message::update_commit_message,
                legacy::virtual_branches::tauri_find_git_branches::find_git_branches,
                legacy::virtual_branches::tauri_list_branches::list_branches,
                legacy::virtual_branches::tauri_get_branch_listing_details::get_branch_listing_details,
                legacy::virtual_branches::tauri_integrate_branch_with_steps::integrate_branch_with_steps,
                legacy::virtual_branches::tauri_squash_commits::squash_commits,
                legacy::virtual_branches::tauri_fetch_from_remotes::fetch_from_remotes,
                legacy::virtual_branches::tauri_move_commit::move_commit,
                legacy::virtual_branches::tauri_move_branch::move_branch,
                legacy::virtual_branches::tauri_tear_off_branch::tear_off_branch,
                legacy::virtual_branches::tauri_normalize_branch_name::normalize_branch_name,
                legacy::virtual_branches::tauri_upstream_integration_statuses::upstream_integration_statuses,
                legacy::virtual_branches::tauri_integrate_upstream::integrate_upstream,
                legacy::virtual_branches::tauri_resolve_upstream_integration::resolve_upstream_integration,
                legacy::virtual_branches::tauri_find_commit::find_commit,
                legacy::stack::tauri_create_reference::create_reference,
                legacy::stack::tauri_create_branch::create_branch,
                legacy::stack::tauri_remove_branch::remove_branch,
                legacy::stack::tauri_update_branch_name::update_branch_name,
                legacy::stack::tauri_update_branch_description::update_branch_description,
                legacy::stack::tauri_update_branch_pr_number::update_branch_pr_number,
                legacy::stack::tauri_push_stack::push_stack,
                legacy::stack::tauri_push_stack_to_review::push_stack_to_review,
                legacy::secret::tauri_secret_get_global::secret_get_global,
                legacy::secret::tauri_secret_set_global::secret_set_global,
                legacy::secret::tauri_secret_delete_global::secret_delete_global,
                legacy::oplog::tauri_list_snapshots::list_snapshots,
                legacy::oplog::tauri_create_snapshot::create_snapshot,
                legacy::oplog::tauri_restore_snapshot::restore_snapshot,
                legacy::oplog::tauri_snapshot_diff::snapshot_diff,
                legacy::config::tauri_get_gb_config::get_gb_config,
                legacy::config::tauri_set_gb_config::set_gb_config,
                legacy::config::tauri_store_author_globally_if_unset::store_author_globally_if_unset,
                legacy::config::tauri_get_author_info::get_author_info,
                legacy::remotes::tauri_list_remotes::list_remotes,
                legacy::remotes::tauri_add_remote::add_remote,
                legacy::modes::tauri_operating_mode::operating_mode,
                legacy::modes::tauri_head_sha::head_sha,
                legacy::modes::tauri_enter_edit_mode::enter_edit_mode,
                legacy::modes::tauri_save_edit_and_return_to_workspace::save_edit_and_return_to_workspace,
                legacy::modes::tauri_abort_edit_and_return_to_workspace::abort_edit_and_return_to_workspace,
                legacy::modes::tauri_edit_initial_index_state::edit_initial_index_state,
                legacy::modes::tauri_edit_changes_from_initial::edit_changes_from_initial,
                legacy::open::tauri_open_url::open_url,
                legacy::open::tauri_show_in_finder::show_in_finder,
                legacy::forge::tauri_pr_templates::pr_templates,
                legacy::forge::tauri_pr_template::pr_template,
                legacy::forge::tauri_determine_forge_from_url::determine_forge_from_url,
                legacy::forge::tauri_list_reviews::list_reviews,
                legacy::forge::tauri_publish_review::publish_review,
                legacy::settings::tauri_get_app_settings::get_app_settings,
                legacy::cli::tauri_install_cli::install_cli,
                legacy::cli::tauri_cli_path::cli_path,
                legacy::rules::tauri_create_workspace_rule::create_workspace_rule,
                legacy::rules::tauri_delete_workspace_rule::delete_workspace_rule,
                legacy::rules::tauri_update_workspace_rule::update_workspace_rule,
                legacy::rules::tauri_list_workspace_rules::list_workspace_rules,
                legacy::workspace::tauri_head_info::head_info,
                legacy::workspace::tauri_stacks::stacks,
                legacy::workspace::tauri_stack_details::stack_details,
                legacy::workspace::tauri_branch_details::branch_details,
                legacy::workspace::tauri_create_commit_from_worktree_changes::create_commit_from_worktree_changes,
                legacy::workspace::tauri_amend_commit_from_worktree_changes::amend_commit_from_worktree_changes,
                legacy::workspace::tauri_discard_worktree_changes::discard_worktree_changes,
                legacy::workspace::tauri_stash_into_branch::stash_into_branch,
                legacy::workspace::tauri_canned_branch_name::canned_branch_name,
                legacy::workspace::tauri_target_commits::target_commits,
                legacy::workspace::tauri_move_changes_between_commits::move_changes_between_commits,
                legacy::workspace::tauri_uncommit_changes::uncommit_changes,
                legacy::workspace::tauri_split_branch::split_branch,
                legacy::workspace::tauri_split_branch_into_dependent_branch::split_branch_into_dependent_branch,
                legacy::diff::tauri_changes_in_worktree::changes_in_worktree,
                legacy::diff::tauri_changes_in_branch::changes_in_branch,
                legacy::diff::tauri_tree_change_diffs::tree_change_diffs,
                legacy::diff::tauri_assign_hunk::assign_hunk,
                #[cfg(unix)]
                legacy::workspace::tauri_show_graph_svg::show_graph_svg,
                legacy::claude::tauri_claude_get_session_details::claude_get_session_details,
                legacy::claude::tauri_claude_list_permission_requests::claude_list_permission_requests,
                legacy::claude::tauri_claude_update_permission_request::claude_update_permission_request,
                legacy::claude::tauri_claude_check_available::claude_check_available,
                legacy::claude::tauri_claude_list_prompt_templates::claude_list_prompt_templates,
                legacy::claude::tauri_claude_get_prompt_dirs::claude_get_prompt_dirs,
                legacy::claude::tauri_claude_maybe_create_prompt_dir::claude_maybe_create_prompt_dir,
                legacy::claude::tauri_claude_get_mcp_config::claude_get_mcp_config,
                legacy::claude::tauri_claude_get_sub_agents::claude_get_sub_agents,
                legacy::claude::tauri_claude_verify_path::claude_verify_path,
                legacy::claude::tauri_claude_get_user_message::claude_get_user_message,
                action::list_actions,
                action::handle_changes,
                action::list_workflows,
                action::auto_commit,
                action::auto_branch_changes,
                action::absorb,
                action::freestyle,
                askpass::submit_prompt_response,
                menu::menu_item_set_enabled,
                projects::list_projects,
                projects::set_project_active,
                projects::open_project_in_window,
                zip::get_logs_archive_path,
                zip::get_project_archive_path,
                zip::get_anonymous_graph_path,
                settings::update_onboarding_complete,
                settings::update_telemetry,
                settings::update_feature_flags,
                settings::update_telemetry_distinct_id,
                settings::update_claude,
                settings::update_fetch,
                settings::update_reviews,
                settings::update_ui,
                bot::bot,
                // Debug-only - not for production!
                #[cfg(debug_assertions)]
                env::env_vars,
                claude::claude_send_message,
                claude::claude_get_messages,
                claude::claude_cancel_session,
                claude::claude_is_stack_active,
                claude::claude_compact_history,
                commit::tauri_reword_commit::reword_commit
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
    Ok(())
}

/// read all objects, migrate them, and write them back if there was a migration.
fn migrate_projects() -> anyhow::Result<()> {
    for mut project in gitbutler_project::dangerously_list_projects_without_migration()? {
        if let Ok(true) = project.migrate() {
            let (title, worktree_dir) = (project.title.clone(), project.worktree_dir()?.to_owned());
            if let Err(err) = gitbutler_project::update(project.into()) {
                tracing::warn!(
                    "Failed to store migrated project {} at {}: {err}",
                    title,
                    worktree_dir.display()
                );
            } else {
                tracing::info!("Migrated project {} at {}", title, worktree_dir.display());
            }
        }
    }
    Ok(())
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
            tracing::info!(
                "Inheriting static interactive shell environment, valid for the entire runtime of the application"
            );
            for (key, value) in terminal_vars {
                unsafe {
                    std::env::set_var(key, value);
                }
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
