use std::sync::Arc;

use axum::{
    Json, Router,
    body::Body,
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    middleware::Next,
    response::IntoResponse,
    routing::{any, get},
};
use but_api::{diff, github, json, legacy};
use but_claude::{Broadcaster, Claude};
use but_settings::AppSettingsWithDiskSync;
use futures_util::{SinkExt, StreamExt as _};
use gitbutler_project::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

mod projects;
use crate::projects::ActiveProjects;

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "subject", rename_all = "camelCase")]
enum Response {
    Success(serde_json::Value),
    Error(serde_json::Value),
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Request {
    command: String,
    params: serde_json::Value,
}

#[derive(Clone)]
pub(crate) struct Extra {
    active_projects: Arc<Mutex<ActiveProjects>>,
    archival: Arc<but_feedback::Archival>,
}

pub async fn run() {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let config_dir = but_path::app_config_dir().unwrap();
    let app_data_dir = but_path::app_data_dir().unwrap();

    let broadcaster = Arc::new(Mutex::new(Broadcaster::new()));
    let archival = Arc::new(but_feedback::Archival {
        cache_dir: app_data_dir.join("cache").clone(),
        logs_dir: app_data_dir.join("logs").clone(),
    });
    let extra = Extra {
        active_projects: Arc::new(Mutex::new(ActiveProjects::new())),
        archival,
    };
    let app_settings =
        AppSettingsWithDiskSync::new(config_dir.clone()).expect("failed to create app settings");

    let app = Claude {
        broadcaster: broadcaster.clone(),
        instance_by_stack: Default::default(),
    };

    // build our application with a single route
    let app = Router::new()
        .route(
            "/",
            get(|| async { "Hello, World!" }).post({
                let app = app.clone();
                let extra = extra.clone();
                move |req| handle_json_command(req, app, extra, app_settings)
            }),
        )
        .route(
            "/ws",
            any({
                let broadcaster = broadcaster.clone();
                async move |req| handle_ws_request(req, broadcaster).await
            }),
        )
        // Spawning in a separate thread to prevent abort if the client
        // disconnects. We need this to ensure locks are removed after
        // the claude processes finishes.
        .route_layer(axum::middleware::from_fn(
            |req: axum::extract::Request<Body>, next: Next| async move {
                tokio::task::spawn(next.run(req)).await.unwrap()
            },
        ))
        .layer(ServiceBuilder::new().layer(cors));

    let port = std::env::var("BUTLER_PORT").unwrap_or("6978".into());
    let host = std::env::var("BUTLER_HOST").unwrap_or("127.0.0.1".into());
    let url = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&url).await.unwrap();
    println!("Running at {url}");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_ws_request(
    ws: WebSocketUpgrade,
    broadcaster: Arc<Mutex<Broadcaster>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_websocket(socket, broadcaster))
}

async fn handle_websocket(socket: WebSocket, broadcaster: Arc<Mutex<Broadcaster>>) {
    let (send, mut recv) = tokio::sync::mpsc::unbounded_channel();
    let id = uuid::Uuid::new_v4();
    broadcaster.lock().await.register_sender(&id, send);

    let (mut socket_send, mut socket_recv) = socket.split();
    let thread = tokio::spawn(async move {
        while let Some(event) = recv.recv().await {
            socket_send
                .send(Message::Text(serde_json::to_string(&event).unwrap().into()))
                .await
                .unwrap();
        }
    });

    while let Some(Ok(msg)) = socket_recv.next().await {
        #[expect(clippy::single_match)]
        match msg {
            Message::Close(_) => {
                thread.abort();
                break;
            }
            _ => {}
        }
    }

    broadcaster.lock().await.deregister_sender(&id);
}

async fn handle_command(
    Json(request): Json<Request>,
    // TODO: this is due to mixing UI broadcasting into Claude related state (which also broadcasts)
    app: Claude,
    extra: Extra,
    app_settings_sync: AppSettingsWithDiskSync,
    // TODO: make this anyhow::Result<serde_json::Value>
) -> anyhow::Result<serde_json::Value> {
    let command: &str = &request.command;
    match command {
        // General commands
        "git_remote_branches" => legacy::git::git_remote_branches_cmd(request.params),
        "git_test_push" => legacy::git::git_test_push_cmd(request.params),
        "git_test_fetch" => legacy::git::git_test_fetch_cmd(request.params),
        "git_index_size" => legacy::git::git_index_size_cmd(request.params),
        "delete_all_data" => legacy::git::delete_all_data_cmd(request.params),
        "git_set_global_config" => legacy::git::git_set_global_config_cmd(request.params),
        "git_remove_global_config" => legacy::git::git_remove_global_config_cmd(request.params),
        "git_get_global_config" => legacy::git::git_get_global_config_cmd(request.params),
        // Diff commands
        "tree_change_diffs" => legacy::diff::tree_change_diffs_cmd(request.params),
        "commit_details_with_line_stats" => {
            diff::commit_details_with_line_stats_cmd(request.params)
        }
        "changes_in_branch" => legacy::diff::changes_in_branch_cmd(request.params),
        "changes_in_worktree" => legacy::diff::changes_in_worktree_cmd(request.params),
        "assign_hunk" => legacy::diff::assign_hunk_cmd(request.params),
        // Cherry apply commands
        "cherry_apply_status" => legacy::cherry_apply::cherry_apply_status_cmd(request.params),
        "cherry_apply" => legacy::cherry_apply::cherry_apply_cmd(request.params),
        // Workspace commands
        "stacks" => legacy::workspace::stacks_cmd(request.params),
        "head_info" => legacy::workspace::head_info_cmd(request.params),
        #[cfg(unix)]
        "show_graph_svg" => legacy::workspace::show_graph_svg_cmd(request.params),
        "stack_details" => legacy::workspace::stack_details_cmd(request.params),
        "branch_details" => legacy::workspace::branch_details_cmd(request.params),
        "create_commit_from_worktree_changes" => {
            legacy::workspace::create_commit_from_worktree_changes_cmd(request.params)
        }
        "amend_commit_from_worktree_changes" => {
            legacy::workspace::amend_commit_from_worktree_changes_cmd(request.params)
        }
        "discard_worktree_changes" => {
            legacy::workspace::discard_worktree_changes_cmd(request.params)
        }
        "move_changes_between_commits" => {
            legacy::workspace::move_changes_between_commits_cmd(request.params)
        }
        "split_branch" => legacy::workspace::split_branch_cmd(request.params),
        "split_branch_into_dependent_branch" => {
            legacy::workspace::split_branch_into_dependent_branch_cmd(request.params)
        }
        "uncommit_changes" => legacy::workspace::uncommit_changes_cmd(request.params),
        "stash_into_branch" => legacy::workspace::stash_into_branch_cmd(request.params),
        "canned_branch_name" => legacy::workspace::canned_branch_name_cmd(request.params),
        "target_commits" => legacy::workspace::target_commits_cmd(request.params),
        // App settings
        "get_app_settings" => legacy::settings::get_app_settings_cmd(request.params),
        "update_onboarding_complete" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_onboarding_complete(&app_settings_sync, params)
                .map(|r| json!(r))
        }),
        "update_telemetry" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_telemetry(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_telemetry_distinct_id" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_telemetry_distinct_id(&app_settings_sync, params)
                .map(|r| json!(r))
        }),
        "update_feature_flags" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_feature_flags(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_claude" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_claude(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_fetch" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_fetch(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_reviews" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_reviews(&app_settings_sync, params).map(|r| json!(r))
        }),
        // Secret management
        "secret_get_global" => legacy::secret::secret_get_global_cmd(request.params),
        "secret_set_global" => legacy::secret::secret_set_global_cmd(request.params),
        "secret_delete_global" => legacy::secret::secret_delete_global_cmd(request.params),
        // User management
        "get_user" => legacy::users::get_user_cmd(request.params),
        "set_user" => legacy::users::set_user_cmd(request.params),
        "delete_user" => legacy::users::delete_user_cmd(request.params),
        // Project management
        "update_project" => legacy::projects::update_project_cmd(request.params),
        "add_project" => legacy::projects::add_project_cmd(request.params),
        "add_project_best_effort" => legacy::projects::add_project_best_effort_cmd(request.params),
        "get_project" => legacy::projects::get_project_cmd(request.params),
        "delete_project" => legacy::projects::delete_project_cmd(request.params),
        "is_gerrit" => legacy::projects::is_gerrit_cmd(request.params),
        "list_projects" => projects::list_projects(&extra).await,
        "set_project_active" => {
            projects::set_project_active(&app, &extra, app_settings_sync, request.params).await
        }
        // Virtual branches commands
        "normalize_branch_name" => {
            legacy::virtual_branches::normalize_branch_name_cmd(request.params)
        }
        "create_virtual_branch" => {
            legacy::virtual_branches::create_virtual_branch_cmd(request.params)
        }
        "delete_local_branch" => legacy::virtual_branches::delete_local_branch_cmd(request.params),
        "create_virtual_branch_from_branch" => {
            legacy::virtual_branches::create_virtual_branch_from_branch_cmd(request.params)
        }
        "integrate_upstream_commits" => {
            legacy::virtual_branches::integrate_upstream_commits_cmd(request.params)
        }
        "get_initial_integration_steps_for_branch" => {
            legacy::virtual_branches::get_initial_integration_steps_for_branch_cmd(request.params)
        }
        "integrate_branch_with_steps" => {
            legacy::virtual_branches::integrate_branch_with_steps_cmd(request.params)
        }
        "get_base_branch_data" => {
            legacy::virtual_branches::get_base_branch_data_cmd(request.params)
        }
        "set_base_branch" => legacy::virtual_branches::set_base_branch_cmd(request.params),
        "push_base_branch" => legacy::virtual_branches::push_base_branch_cmd(request.params),
        "update_stack_order" => legacy::virtual_branches::update_stack_order_cmd(request.params),
        "unapply_stack" => legacy::virtual_branches::unapply_stack_cmd(request.params),
        "can_apply_remote_branch" => {
            legacy::virtual_branches::can_apply_remote_branch_cmd(request.params)
        }
        "list_commit_files" => legacy::virtual_branches::list_commit_files_cmd(request.params),
        "amend_virtual_branch" => {
            legacy::virtual_branches::amend_virtual_branch_cmd(request.params)
        }
        "undo_commit" => legacy::virtual_branches::undo_commit_cmd(request.params),
        "insert_blank_commit" => legacy::virtual_branches::insert_blank_commit_cmd(request.params),
        "reorder_stack" => legacy::virtual_branches::reorder_stack_cmd(request.params),
        "find_git_branches" => legacy::virtual_branches::find_git_branches_cmd(request.params),
        "list_branches" => legacy::virtual_branches::list_branches_cmd(request.params),
        "get_branch_listing_details" => {
            legacy::virtual_branches::get_branch_listing_details_cmd(request.params)
        }
        "squash_commits" => legacy::virtual_branches::squash_commits_cmd(request.params),
        "fetch_from_remotes" => legacy::virtual_branches::fetch_from_remotes_cmd(request.params),
        "move_commit" => legacy::virtual_branches::move_commit_cmd(request.params),
        "move_branch" => legacy::virtual_branches::move_branch_cmd(request.params),
        "tear_off_branch" => legacy::virtual_branches::tear_off_branch_cmd(request.params),
        "update_commit_message" => {
            legacy::virtual_branches::update_commit_message_cmd(request.params)
        }
        "find_commit" => legacy::virtual_branches::find_commit_cmd(request.params),
        "upstream_integration_statuses" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result =
                        legacy::virtual_branches::upstream_integration_statuses_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "integrate_upstream" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::virtual_branches::integrate_upstream_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "resolve_upstream_integration" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result =
                        legacy::virtual_branches::resolve_upstream_integration_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Operating modes commands
        "operating_mode" => legacy::modes::operating_mode_cmd(request.params),
        "head_sha" => legacy::modes::head_sha_cmd(request.params),
        "enter_edit_mode" => legacy::modes::enter_edit_mode_cmd(request.params),
        "abort_edit_and_return_to_workspace" => {
            legacy::modes::abort_edit_and_return_to_workspace_cmd(request.params)
        }
        "save_edit_and_return_to_workspace" => {
            legacy::modes::save_edit_and_return_to_workspace_cmd(request.params)
        }
        "edit_initial_index_state" => legacy::modes::edit_initial_index_state_cmd(request.params),
        "edit_changes_from_initial" => legacy::modes::edit_changes_from_initial_cmd(request.params),
        // Repository commands
        "git_get_local_config" => legacy::repo::git_get_local_config_cmd(request.params),
        "git_set_local_config" => legacy::repo::git_set_local_config_cmd(request.params),
        "check_signing_settings" => legacy::repo::check_signing_settings_cmd(request.params),
        "git_clone_repository" => legacy::repo::git_clone_repository_cmd(request.params).await,
        "get_uncommitted_files" => legacy::repo::get_uncommitted_files_cmd(request.params),
        "get_commit_file" => legacy::repo::get_commit_file_cmd(request.params),
        "get_workspace_file" => legacy::repo::get_workspace_file_cmd(request.params),
        "find_files" => legacy::repo::find_files_cmd(request.params),
        "pre_commit_hook" => legacy::repo::pre_commit_hook_cmd(request.params),
        "pre_commit_hook_diffspecs" => legacy::repo::pre_commit_hook_diffspecs_cmd(request.params),
        "post_commit_hook" => legacy::repo::post_commit_hook_cmd(request.params),
        "message_hook" => legacy::repo::message_hook_cmd(request.params),
        // Stack management commands
        "create_branch" => legacy::stack::create_branch_cmd(request.params),
        "create_reference" => legacy::stack::create_reference_cmd(request.params),
        "remove_branch" => legacy::stack::remove_branch_cmd(request.params),
        "update_branch_name" => legacy::stack::update_branch_name_cmd(request.params),
        "update_branch_description" => legacy::stack::update_branch_description_cmd(request.params),
        "update_branch_pr_number" => legacy::stack::update_branch_pr_number_cmd(request.params),
        "push_stack" => legacy::stack::push_stack_cmd(request.params),
        "push_stack_to_review" => legacy::stack::push_stack_to_review_cmd(request.params),
        // Undo/Snapshot commands
        "list_snapshots" => legacy::oplog::list_snapshots_cmd(request.params),
        "restore_snapshot" => legacy::oplog::restore_snapshot_cmd(request.params),
        "snapshot_diff" => legacy::oplog::snapshot_diff_cmd(request.params),
        // "oplog_diff_worktrees" => undo::oplog_diff_worktrees(&ctx, request.params),
        // Config management commands
        "get_gb_config" => legacy::config::get_gb_config_cmd(request.params),
        "set_gb_config" => legacy::config::set_gb_config_cmd(request.params),
        "store_author_globally_if_unset" => {
            legacy::config::store_author_globally_if_unset_cmd(request.params)
        }
        "get_author_info" => legacy::config::get_author_info_cmd(request.params),
        // Remotes management commands
        "list_remotes" => legacy::remotes::list_remotes_cmd(request.params),
        "add_remote" => legacy::remotes::add_remote_cmd(request.params),
        // Rules/Workspace rules commands
        "create_workspace_rule" => legacy::rules::create_workspace_rule_cmd(request.params),
        "delete_workspace_rule" => legacy::rules::delete_workspace_rule_cmd(request.params),
        "update_workspace_rule" => legacy::rules::update_workspace_rule_cmd(request.params),
        "list_workspace_rules" => legacy::rules::list_workspace_rules_cmd(request.params),
        "init_device_oauth" => {
            let result = github::init_device_oauth().await;
            result.map(|r| json!(r))
        }
        "check_auth_status" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = github::check_auth_status_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "store_github_pat" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = github::store_github_pat_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "store_github_enterprise_pat" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = github::store_github_enterprise_pat_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "forget_github_account" => github::forget_github_account_cmd(request.params),
        "list_known_github_accounts" => {
            github::list_known_github_accounts().await.map(|r| json!(r))
        }
        "clear_all_github_tokens" => github::clear_all_github_tokens_cmd(request.params),
        "get_gh_user" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = github::get_gh_user_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Forge commands
        "pr_templates" => legacy::forge::pr_templates_cmd(request.params),
        "pr_template" => legacy::forge::pr_template_cmd(request.params),
        "determine_forge_from_url" => legacy::forge::determine_forge_from_url_cmd(request.params),
        "list_reviews" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::list_reviews_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "publish_review" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::publish_review_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // // Menu commands (limited - no menu_item_set_enabled as it's Tauri-specific)
        // "get_editor_link_scheme" => menu::get_editor_link_scheme(&ctx, request.params),
        // CLI commands
        "install_cli" => legacy::cli::install_cli_cmd(request.params),
        "cli_path" => legacy::cli::cli_path_cmd(request.params),
        // Askpass commands (async)
        "submit_prompt_response" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::askpass::submit_prompt_response(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Open/System commands (limited - no open_project_in_window as it's Tauri-specific)
        "open_url" => legacy::open::open_url_cmd(request.params),
        "show_in_finder" => legacy::open::show_in_finder_cmd(request.params),

        // TODO: Tauri-specific commands that cannot be ported to HTTP API:
        //
        // AI-Integrated Action Commands (require Tauri AppHandle for real-time UI updates):
        // - "auto_commit" => action::auto_commit() // Needs AppHandle for real-time AI progress updates
        // - "auto_branch_changes" => action::auto_branch_changes() // Needs AppHandle for real-time AI progress updates
        // - "absorb" => action::absorb() // Needs AppHandle for real-time AI progress updates
        // - "freestyle" => action::freestyle() // Needs AppHandle for real-time AI progress updates
        //
        // UI Management Commands (require Tauri window/menu system):
        // - "menu_item_set_enabled" => menu::menu_item_set_enabled() // Requires Tauri menu management
        // - "open_project_in_window" => projects::open_project_in_window() // Requires Tauri window creation
        //
        // Zip/Archive commands
        "get_project_archive_path" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct GetProjectArchivePathParams {
                pub project_id: ProjectId,
            }
            let params = serde_json::from_value::<GetProjectArchivePathParams>(request.params)?;
            extra
                .archival
                .zip_entire_repository(params.project_id)
                .map(to_json_or_panic)
        }
        "get_logs_archive_path" => {
            let result = extra.archival.zip_logs();
            result.map(|r| json!(r))
        }
        "claude_send_message" => {
            let params = deserialize_json(request.params)?;
            let result = legacy::claude::claude_send_message(&app, params).await;
            result.map(|r| json!(r))
        }
        "claude_get_mcp_config" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Params {
                project_id: ProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let result = legacy::claude::claude_get_mcp_config(params.project_id).await;
            result.map(|r| json!(r))
        }
        "claude_get_messages" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::claude::claude_get_messages(&app, params);
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_get_session_details" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Params {
                project_id: ProjectId,
                session_id: String,
            }
            let params = deserialize_json(request.params);
            match params {
                Ok(Params {
                    project_id,
                    session_id,
                }) => {
                    let result =
                        legacy::claude::claude_get_session_details(project_id, session_id).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_get_user_message" => legacy::claude::claude_get_user_message_cmd(request.params),
        "claude_list_permission_requests" => {
            legacy::claude::claude_list_permission_requests_cmd(request.params)
        }
        "claude_update_permission_request" => {
            legacy::claude::claude_update_permission_request_cmd(request.params)
        }
        "claude_cancel_session" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::claude::claude_cancel_session(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_check_available" => {
            let result = legacy::claude::claude_check_available().await;
            result.map(|r| json!(r))
        }
        "claude_is_stack_active" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::claude::claude_is_stack_active(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_compact_history" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::claude::claude_compact_history(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_list_prompt_templates" => {
            legacy::claude::claude_list_prompt_templates_cmd(request.params)
        }
        "claude_get_prompt_dirs" => legacy::claude::claude_get_prompt_dirs_cmd(request.params),
        "claude_maybe_create_prompt_dir" => {
            legacy::claude::claude_maybe_create_prompt_dir_cmd(request.params)
        }
        "claude_get_sub_agents" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Params {
                project_id: ProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let result = legacy::claude::claude_get_sub_agents(params.project_id).await;
            result.map(|r| json!(r))
        }
        "claude_verify_path" => {
            #[derive(Debug, Deserialize)]
            #[serde(rename_all = "camelCase")]
            pub struct Params {
                pub project_id: ProjectId,
                pub path: String,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let result = legacy::claude::claude_verify_path(params.project_id, params.path).await;
            result.map(|r| json!(r))
        }

        _ => Err(anyhow::anyhow!("Command {} not found!", command)),
    }
}

async fn handle_json_command(
    req: Json<Request>,
    // TODO: this is due to mixing UI broadcasting into Claude related state (which also broadcasts)
    app: Claude,
    extra: Extra,
    app_settings_sync: AppSettingsWithDiskSync,
    // TODO: make this anyhow::Result<serde_json::Value>
) -> Json<serde_json::Value> {
    let res = handle_command(req, app, extra, app_settings_sync).await;
    match res {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => {
            let e = json::Error::from(e);
            Json(json!(Response::Error(json!(e))))
        }
    }
}

fn to_json_or_panic(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> anyhow::Result<T> {
    Ok(serde_json::from_value(value)?)
}
