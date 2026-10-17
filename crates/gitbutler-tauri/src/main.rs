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

use anyhow::{Context, bail};
use but_api::{
    bitbucket, branch, commit, diff, github, gitlab, land, legacy, open, platform, resolve,
    workspace, worktrees,
};
use but_settings::AppSettingsWithDiskSync;
use gitbutler_tauri::{
    Frontend, WindowState, askpass, csp::csp_with_extras, env, logs, menu, projects, settings, zip,
};
use tauri::{Emitter, Manager, generate_context};
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_log::{Target, TargetKind};

fn main() -> anyhow::Result<()> {
    but_api::panic_capture::install_panic_hook();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    #[cfg(feature = "builtin-but")]
    {
        if but::is_executed_as_but()? {
            but_askpass::disable();
            return runtime.block_on(but::handle_args(std::env::args_os()));
        }
    }
    let performance_logging = std::env::var_os("GITBUTLER_PERFORMANCE_LOG").is_some();
    let tauri_debug_logging = std::env::var_os("GITBUTLER_TAURI_DEBUG_LOG").is_some();

    let mut tauri_context = generate_context!();
    but_secret::secret::set_application_namespace(&tauri_context.config().identifier);

    // Set the macOS notification bundle ID so notifications appear as GitButler.
    #[cfg(target_os = "macos")]
    {
        if let Err(e) = notify_rust::set_application(&tauri_context.config().identifier) {
            tracing::warn!(error = %e, "Failed to set notification application");
        }
    }

    let config_dir = but_path::app_config_dir().expect("missing config dir");
    std::fs::create_dir_all(&config_dir).expect("failed to create config dir");
    let custom_settings = cfg!(feature = "packaged-but-distribution")
        .then(but_settings::customization::packaged_but_binary);
    // While it serves a function, this behavior is sub-optimal. The proper solution is to decouple:
    // - Checking for updates from
    // - Performing an update
    // This way people can be informed that there is an update even if self-updating is not possible (i.e. installed via package manager).
    let custom_settings = if cfg!(feature = "disable-auto-updates") {
        but_settings::customization::merge_two(
            but_settings::customization::disable_auto_update_checks(),
            custom_settings,
        )
        .into()
    } else {
        custom_settings
    };
    let mut app_settings =
        AppSettingsWithDiskSync::new_with_customization(config_dir.clone(), custom_settings)
            .expect("failed to create app settings");

    if let Ok(updated_csp) = csp_with_extras(
        tauri_context.config().app.security.csp.as_ref().cloned(),
        &app_settings,
    ) {
        tauri_context.config_mut().app.security.csp = updated_csp;
    };
    let frontend = if app_settings.get()?.feature_flags.lite_frontend
        && gitbutler_tauri::lite_frontend::serve(&mut tauri_context)
    {
        Frontend::Lite
    } else {
        Frontend::Svelte
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
    let (app_data_dir, app_cache_dir, app_log_dir) = (
        but_path::app_data_dir()?,
        but_path::app_cache_dir()?,
        but_path::app_log_dir()?,
    );
    std::fs::create_dir_all(&app_data_dir).context("failed to create app data dir")?;
    std::fs::create_dir_all(&app_cache_dir).context("failed to create app cache dir")?;
    std::fs::create_dir_all(&app_log_dir).context("failed to create app log dir")?;

    let tokio_debug = matches!(std::env::var("GITBUTLER_TOKIO_DEBUG").as_deref(), Ok("1"));
    runtime.block_on(async {
        tauri::async_runtime::set(tokio::runtime::Handle::current());

        let log = tauri_plugin_log::Builder::default()
            .target(Target::new(TargetKind::LogDir {
                file_name: Some("ui-logs".to_string()),
            }))
            .level(if tauri_debug_logging {
                tauri_plugin_log::log::LevelFilter::Debug
            } else {
                tauri_plugin_log::log::LevelFilter::Error
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
                if tauri_app.config().product_name.as_deref() != Some("GitButler Test") {
                    window.open_devtools();
                }

                let app_handle = tauri_app.handle();

                logs::init(app_handle, &app_log_dir, performance_logging, tokio_debug);

                #[cfg(target_os = "macos")]
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

                but_askpass::init({
                    let handle = app_handle.clone();
                    move |event| {
                        handle
                            .emit("git_prompt", &event)
                            .expect("tauri event emission doesn't fail in practice");
                        handle
                            .emit("askpass_prompt", askpass::LitePromptEvent::from(event))
                            .expect("tauri event emission doesn't fail in practice");
                    }
                });


                tracing::info!(version = %app_handle.package_info().version,
                                   name = %app_handle.package_info().name, "starting app");

                app_handle.manage(WindowState::new(app_handle.clone(), frontend));

                app_settings.watch_in_background({
                    let app_handle = app_handle.clone();
                    move |app_settings| {
                        gitbutler_tauri::ChangeForFrontend::from(app_settings).send(&app_handle)
                    }
                })?;

                let archival = but_feedback::Archival {
                    cache_dir: app_cache_dir.clone(),
                    logs_dir: app_log_dir.clone(),
                };
                app_handle.manage(archival);
                app_handle.manage(app_settings);

                tauri_app.on_menu_event(move |handle, event| {
                    let target_window = handle
                        .webview_windows()
                        .into_values()
                        .find(|webview| webview.is_focused().unwrap_or(false))
                        .or_else(|| handle.get_webview_window("main"));

                    if let Some(webview) = target_window {
                        menu::handle_event(&webview, &event);
                    } else {
                        tracing::warn!(
                            menu_event = %event.id().as_ref(),
                            "no webview window available to handle menu event"
                        );
                    }
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
                github::tauri_init_github_device_oauth::init_github_device_oauth,
                github::tauri_check_github_auth_status::check_github_auth_status,
                github::tauri_store_github_pat::store_github_pat,
                github::tauri_store_github_enterprise_pat::store_github_enterprise_pat,
                github::tauri_get_gh_user::get_gh_user,
                github::tauri_forget_github_account::forget_github_account,
                github::tauri_list_known_github_accounts::list_known_github_accounts,
                github::tauri_clear_all_github_tokens::clear_all_github_tokens,
                gitlab::tauri_store_gitlab_pat::store_gitlab_pat,
                gitlab::tauri_store_gitlab_selfhosted_pat::store_gitlab_selfhosted_pat,
                gitlab::tauri_get_gl_user::get_gl_user,
                gitlab::tauri_forget_gitlab_account::forget_gitlab_account,
                gitlab::tauri_list_known_gitlab_accounts::list_known_gitlab_accounts,
                gitlab::tauri_clear_all_gitlab_tokens::clear_all_gitlab_tokens,
                bitbucket::tauri_store_bitbucket_api_token::store_bitbucket_api_token,
                bitbucket::tauri_get_bb_user::get_bb_user,
                bitbucket::tauri_forget_bitbucket_account::forget_bitbucket_account,
                bitbucket::tauri_list_known_bitbucket_accounts::list_known_bitbucket_accounts,
                bitbucket::tauri_clear_all_bitbucket_tokens::clear_all_bitbucket_tokens,
                bitbucket::tauri_check_bitbucket_credentials::check_bitbucket_credentials,
                diff::tauri_commit_details::commit_details,
                diff::tauri_commit_details_with_line_stats::commit_details_with_line_stats,
                workspace::tauri_get_workspace::get_workspace,
                workspace::tauri_set_target_ref_and_init_project::set_target_ref_and_init_project,
                workspace::tauri_resolve_worktree_conflicts::resolve_worktree_conflicts,
                but_api::branch::tauri_branch_diff::branch_diff,
                but_api::branch::tauri_move_branch::move_branch,
                but_api::branch::tauri_tear_off_branch::tear_off_branch,
                but_api::branch::tauri_branch_remove::branch_remove,
                but_api::branch::tauri_branch_create::branch_create,
                but_api::branch::tauri_branch_rename::branch_rename,
                but_api::branch::tauri_branch_name_segment::branch_name_segment,
                but_api::branch::tauri_branch_canned_name::branch_canned_name,
                but_api::target_commits::tauri_workspace_target_commits::workspace_target_commits,
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
                legacy::users::tauri_get_login_token::get_login_token,
                legacy::users::tauri_login_with_token::login_with_token,
                legacy::users::tauri_login_and_persist::login_and_persist,
                legacy::users::tauri_get_user_profile::get_user_profile,
                legacy::users::tauri_update_user_profile::update_user_profile,
                legacy::projects::tauri_add_project::add_project,
                legacy::projects::tauri_add_project_best_effort::add_project_best_effort,
                legacy::projects::tauri_get_project::get_project,
                legacy::projects::tauri_update_project::update_project,
                legacy::projects::tauri_delete_project::delete_project,
                legacy::projects::tauri_is_gerrit::is_gerrit,
                legacy::repo::tauri_check_signing_settings::check_signing_settings,
                legacy::repo::tauri_git_clone_repository::git_clone_repository,
                legacy::repo::tauri_get_commit_file::get_commit_file,
                legacy::repo::tauri_get_workspace_file::get_workspace_file,
                legacy::repo::tauri_get_blob_file::get_blob_file,
                legacy::repo::tauri_find_files::find_files,
                legacy::repo::tauri_pre_commit_hook_diffspecs::pre_commit_hook_diffspecs,
                legacy::repo::tauri_post_commit_hook::post_commit_hook,
                legacy::repo::tauri_message_hook::message_hook,
                legacy::virtual_branches::tauri_create_virtual_branch::create_virtual_branch,
                legacy::virtual_branches::tauri_delete_local_branch::delete_local_branch,
                legacy::virtual_branches::tauri_get_base_branch_data::get_base_branch_data,
                legacy::virtual_branches::tauri_set_base_branch::set_base_branch,
                legacy::virtual_branches::tauri_switch_back_to_workspace::switch_back_to_workspace,
                legacy::virtual_branches::tauri_update_stack_order::update_stack_order,
                legacy::virtual_branches::tauri_unapply_stack::unapply_stack,
                legacy::virtual_branches::tauri_list_branches::list_branches,
                legacy::virtual_branches::tauri_get_branch_listing_details::get_branch_listing_details,
                legacy::virtual_branches::tauri_fetch_from_remotes::fetch_from_remotes,
                legacy::virtual_branches::tauri_normalize_branch_name::normalize_branch_name,
                branch::tauri_apply::apply,
                branch::tauri_get_initial_branch_integration::get_initial_branch_integration,
                branch::tauri_apply_branch_integration::apply_branch_integration,
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
                open::tauri_open_url::open_url,
                open::tauri_open_in_terminal::open_in_terminal,
                open::tauri_show_in_finder::show_in_finder,
                open::tauri_workdir_path::workdir_path,
                open::terminal::tauri_get_terminal_options_for_platform::get_terminal_options_for_platform,
                open::terminal::tauri_get_recommended_terminal_for_platform::get_recommended_terminal_for_platform,
                legacy::forge::tauri_pr_templates::pr_templates,
                legacy::forge::tauri_pr_template::pr_template,
                legacy::forge::tauri_forge_provider::forge_provider,
                legacy::forge::tauri_forge_info::forge_info,
                legacy::forge::tauri_forge_compare_branch_url::forge_compare_branch_url,
                legacy::forge::tauri_list_reviews::list_reviews,
                legacy::forge::tauri_review_apply::review_apply,
                legacy::forge::tauri_get_review::get_review,
                legacy::forge::tauri_get_review_merge_status::get_review_merge_status,
                legacy::forge::tauri_get_review_base_repo_url::get_review_base_repo_url,
                legacy::forge::tauri_get_repo_info::get_repo_info,
                legacy::forge::tauri_update_review::update_review,
                legacy::forge::tauri_list_ci_checks::list_ci_checks,
                legacy::forge::tauri_publish_review::publish_review,
                legacy::forge::tauri_merge_review::merge_review,
                legacy::forge::tauri_set_review_auto_merge::set_review_auto_merge,
                legacy::forge::tauri_set_review_draftiness::set_review_draftiness,
                legacy::forge::tauri_update_review_footers::update_review_footers,
                legacy::cli::tauri_install_cli::install_cli,
                legacy::cli::tauri_cli_path::cli_path,
                legacy::workspace::tauri_head_info::head_info,
                legacy::workspace::tauri_branch_details::branch_details,
                legacy::workspace::tauri_discard_worktree_changes::discard_worktree_changes,
                legacy::workspace::tauri_stash_into_branch::stash_into_branch,
                legacy::workspace::tauri_workspace_branch_and_ancestors_push::workspace_branch_and_ancestors_push,
                legacy::absorb::tauri_absorb::absorb,
                legacy::absorb::tauri_absorption_plan::absorption_plan,
                diff::tauri_changes_in_worktree::changes_in_worktree,
                diff::tauri_tree_change_diffs::tree_change_diffs,
                diff::tauri_tree_change_diffs_from_source::tree_change_diffs_from_source,
                diff::tauri_assign_hunk::assign_hunk,
                worktrees::tauri_worktree_set_archived::worktree_set_archived,
                worktrees::tauri_worktree_remove::worktree_remove,
                worktrees::tauri_worktree_new::worktree_new,
                #[cfg(unix)]
                legacy::workspace::tauri_show_graph_svg::show_graph_svg,
                askpass::submit_prompt_response,
                projects::list_projects,
                projects::server_capabilities,
                projects::set_project_active,
                projects::open_project_in_window,
                zip::get_logs_archive_path,
                zip::get_project_archive_path,
                zip::get_anonymous_graph_path,
                settings::get_app_settings,
                settings::update_onboarding_complete,
                settings::update_telemetry,
                settings::update_feature_flags,
                settings::update_telemetry_distinct_id,
                settings::update_fetch,
                settings::update_reviews,
                settings::update_ui,
                // Debug-only - not for production!
                #[cfg(debug_assertions)]
                env::env_vars,
                commit::reword::tauri_commit_reword::commit_reword,
                commit::insert_blank::tauri_commit_insert_blank::commit_insert_blank,
                commit::create::tauri_commit_create::commit_create,
                commit::amend::tauri_commit_amend::commit_amend,
                commit::cherry_pick::tauri_commit_cherry_pick::commit_cherry_pick,
                commit::move_commit::tauri_commit_move::commit_move,
                commit::move_changes::tauri_commit_move_changes_between::commit_move_changes_between,
                commit::squash::tauri_commit_squash::commit_squash,
                commit::uncommit::tauri_commit_uncommit_changes::commit_uncommit_changes,
                commit::uncommit::tauri_commit_uncommit_changes_from_commits::commit_uncommit_changes_from_commits,
                commit::uncommit::tauri_commit_uncommit::commit_uncommit,
                workspace::tauri_workspace_integrate_upstream::workspace_integrate_upstream,
                workspace::tauri_workspace_fetch_from_remotes::workspace_fetch_from_remotes,
                workspace::tauri_workspace_fetch_status::workspace_fetch_status,
                land::tauri_branch_land::branch_land,
                resolve::tauri_resolve_commit_conflicts_ai::resolve_commit_conflicts_ai,
                resolve::tauri_commit_conflicts::commit_conflicts,
                resolve::tauri_resolve_commit_conflict_hunks::resolve_commit_conflict_hunks,
                platform::tauri_build_type::build_type,
                but_api::legacy::forge::tauri_add_comment_reaction::add_comment_reaction,
                but_api::legacy::forge::tauri_add_review_labels::add_review_labels,
                but_api::legacy::forge::tauri_add_review_reaction::add_review_reaction,
                but_api::legacy::forge::tauri_add_submission_reaction::add_submission_reaction,
                but_api::branch::tauri_branch_checkout::branch_checkout,
                but_api::branch::tauri_branch_checkout_new::branch_checkout_new,
                but_api::branch::tauri_branch_list::branch_list,
                but_api::comments::tauri_comment_archive::comment_archive,
                but_api::comments::tauri_comment_create::comment_create,
                but_api::comments::tauri_comment_update::comment_update,
                but_api::comments::tauri_comments_list::comments_list,
                but_api::commit::discard_commit::tauri_commit_discard::commit_discard,
                but_api::commit::discard_commit::tauri_commit_discard_changes::commit_discard_changes,
                but_api::legacy::forge::tauri_create_review_comment::create_review_comment,
                but_api::legacy::forge::tauri_create_review_thread_reply::create_review_thread_reply,
                but_api::legacy::forge::tauri_current_forge_login::current_forge_login,
                but_api::legacy::forge::tauri_delete_review_comment::delete_review_comment,
                but_api::ai::tauri_get_ai_configuration::get_ai_configuration,
                but_api::legacy::oplog::tauri_get_redo_target_snapshot::get_redo_target_snapshot,
                but_api::legacy::oplog::tauri_get_undo_target_snapshot::get_undo_target_snapshot,
                but_api::legacy::users::tauri_get_user_profile_local::get_user_profile_local,
                but_api::legacy::repo::tauri_get_workspace_file_from_source::get_workspace_file_from_source,
                but_api::legacy::forge::tauri_list_comment_reactions::list_comment_reactions,
                but_api::open::tauri_list_editors::list_editors,
                but_api::legacy::projects::tauri_list_projects_stateless::list_projects_stateless,
                but_api::legacy::forge::tauri_list_repo_labels::list_repo_labels,
                but_api::legacy::forge::tauri_list_review_comments::list_review_comments,
                but_api::legacy::forge::tauri_list_review_reactions::list_review_reactions,
                but_api::legacy::forge::tauri_list_review_submissions::list_review_submissions,
                but_api::legacy::forge::tauri_list_review_threads::list_review_threads,
                but_api::legacy::forge::tauri_list_review_timeline_events::list_review_timeline_events,
                but_api::legacy::forge::tauri_list_reviewer_candidates::list_reviewer_candidates,
                but_api::legacy::forge::tauri_new_review_target::new_review_target,
                but_api::open::tauri_open_in_program::open_in_program,
                but_api::legacy::oplog::tauri_peel_restore_snapshot::peel_restore_snapshot,
                but_api::legacy::forge::tauri_remove_comment_reaction::remove_comment_reaction,
                but_api::legacy::forge::tauri_remove_review_label::remove_review_label,
                but_api::legacy::forge::tauri_remove_review_reaction::remove_review_reaction,
                but_api::legacy::forge::tauri_remove_submission_reaction::remove_submission_reaction,
                but_api::legacy::forge::tauri_request_review::request_review,
                but_api::ai::tauri_reset_ai_configuration::reset_ai_configuration,
                but_api::legacy::oplog::tauri_restore_snapshot_with_kind::restore_snapshot_with_kind,
                but_api::legacy::forge::tauri_set_review_thread_resolved::set_review_thread_resolved,
                gitbutler_tauri::ai::stream_ai_response,
                but_api::ai::tauri_update_ai_configuration::update_ai_configuration,
                but_api::legacy::users::tauri_update_profile_and_persist::update_profile_and_persist,
                but_api::legacy::projects::tauri_update_project_settings::update_project_settings,
                but_api::legacy::forge::tauri_update_review_comment::update_review_comment,
                but_api::legacy::users::tauri_upload_file::upload_file,
                but_api::legacy::forge::tauri_withdraw_review_request::withdraw_review_request,
                but_api::worktrees::tauri_worktrees_list::worktrees_list,
            ])
            .menu(menu::build)
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
            .run(|_app_handle, _event| {});
    });
    Ok(())
}

/// read all objects, migrate them, and write them back if there was a migration.
fn migrate_projects() -> anyhow::Result<()> {
    for mut project in gitbutler_project::dangerously_list_projects_without_migration()? {
        if let Ok(true) = project.migrate() {
            let (title, git_dir) = (project.title.clone(), project.git_dir().to_owned());
            if let Err(err) = gitbutler_project::update(project.into()) {
                tracing::warn!(
                    "Failed to store migrated project {} at {}: {err}",
                    title,
                    git_dir.display()
                );
            } else {
                tracing::info!("Migrated project {} at {}", title, git_dir.display());
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
