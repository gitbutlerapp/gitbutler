use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{
        WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
    routing::{any, get},
};
use but_api::{
    App, NoParams,
    commands::{
        askpass, claude, cli, config, diff, forge, git, github, modes, open, projects as iprojects,
        remotes, repo, rules, secret, settings, stack, undo, users, virtual_branches, workspace,
        zip,
    },
    error::ToError as _,
};
use but_broadcaster::Broadcaster;
use but_settings::AppSettingsWithDiskSync;
use futures_util::{SinkExt, StreamExt as _};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
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
}

pub async fn run() {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let config_dir = but_path::app_config_dir().unwrap();
    let app_data_dir = but_path::app_data_dir().unwrap();

    let broadcaster = Arc::new(Mutex::new(Broadcaster::new()));
    let extra = Extra {
        active_projects: Arc::new(Mutex::new(ActiveProjects::new())),
    };

    let app = App {
        app_settings: Arc::new(
            AppSettingsWithDiskSync::new(config_dir.clone())
                .expect("failed to create app settings"),
        ),
        broadcaster: broadcaster.clone(),
        archival: Arc::new(gitbutler_feedback::Archival {
            cache_dir: app_data_dir.join("cache").clone(),
            logs_dir: app_data_dir.join("logs").clone(),
        }),
        claudes: Default::default(),
    };

    // build our application with a single route
    let app = Router::new()
        .route(
            "/",
            get(|| async { "Hello, World!" }).post({
                let app = app.clone();
                let extra = extra.clone();
                move |req| handle_command(req, app, extra)
            }),
        )
        .route(
            "/ws",
            any({
                let broadcaster = broadcaster.clone();
                async move |req| handle_ws_request(req, broadcaster).await
            }),
        )
        .layer(ServiceBuilder::new().layer(cors));

    let port = std::env::var("BUTLER_PORT").unwrap_or("6978".into());
    let host = std::env::var("BUTLER_HOST").unwrap_or("172.0.0.1".into());
    let url = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&url).await.unwrap();
    println!("Running at {}", url);
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

fn run_cmd<
    D: DeserializeOwned,
    S: Serialize,
    Fun: Fn(&App, D) -> Result<S, but_api::error::Error>,
>(
    app: &App,
    params: serde_json::Value,
    fun: Fun,
) -> Result<serde_json::Value, but_api::error::Error> {
    let result = fun(app, serde_json::from_value(params).to_error()?)?;
    Ok(json!(result))
}

async fn handle_command(
    Json(request): Json<Request>,
    app: App,
    extra: Extra,
) -> Json<serde_json::Value> {
    let command: &str = &request.command;
    let result = match command {
        // General commands
        "git_remote_branches" => run_cmd(&app, request.params, git::git_remote_branches),
        "git_test_push" => run_cmd(&app, request.params, git::git_test_push),
        "git_test_fetch" => run_cmd(&app, request.params, git::git_test_fetch),
        "git_index_size" => run_cmd(&app, request.params, git::git_index_size),
        "git_head" => run_cmd(&app, request.params, git::git_head),
        "delete_all_data" => run_cmd(&app, request.params, git::delete_all_data),
        "git_set_global_config" => run_cmd(&app, request.params, git::git_set_global_config),
        "git_remove_global_config" => run_cmd(&app, request.params, git::git_remove_global_config),
        "git_get_global_config" => run_cmd(&app, request.params, git::git_get_global_config),
        // Diff commands
        "tree_change_diffs" => run_cmd(&app, request.params, diff::tree_change_diffs),
        "commit_details" => run_cmd(&app, request.params, diff::commit_details),
        "changes_in_branch" => run_cmd(&app, request.params, diff::changes_in_branch),
        "changes_in_worktree" => run_cmd(&app, request.params, diff::changes_in_worktree),
        "assign_hunk" => run_cmd(&app, request.params, diff::assign_hunk),
        // Workspace commands
        "stacks" => run_cmd(&app, request.params, workspace::stacks),
        #[cfg(unix)]
        "show_graph_svg" => run_cmd(&app, request.params, workspace::show_graph_svg),
        "stack_details" => run_cmd(&app, request.params, workspace::stack_details),
        "branch_details" => run_cmd(&app, request.params, workspace::branch_details),
        "create_commit_from_worktree_changes" => run_cmd(
            &app,
            request.params,
            workspace::create_commit_from_worktree_changes,
        ),
        "amend_commit_from_worktree_changes" => run_cmd(
            &app,
            request.params,
            workspace::amend_commit_from_worktree_changes,
        ),
        "discard_worktree_changes" => {
            run_cmd(&app, request.params, workspace::discard_worktree_changes)
        }
        "move_changes_between_commits" => run_cmd(
            &app,
            request.params,
            workspace::move_changes_between_commits,
        ),
        "split_branch" => run_cmd(&app, request.params, workspace::split_branch),
        "split_branch_into_dependent_branch" => run_cmd(
            &app,
            request.params,
            workspace::split_branch_into_dependent_branch,
        ),
        "uncommit_changes" => run_cmd(&app, request.params, workspace::uncommit_changes),
        "stash_into_branch" => run_cmd(&app, request.params, workspace::stash_into_branch),
        "canned_branch_name" => run_cmd(&app, request.params, workspace::canned_branch_name),
        "target_commits" => run_cmd(&app, request.params, workspace::target_commits),
        // App settings
        "get_app_settings" => run_cmd(&app, request.params, settings::get_app_settings),
        "update_onboarding_complete" => {
            run_cmd(&app, request.params, settings::update_onboarding_complete)
        }
        "update_telemetry" => run_cmd(&app, request.params, settings::update_telemetry),
        "update_telemetry_distinct_id" => {
            run_cmd(&app, request.params, settings::update_telemetry_distinct_id)
        }
        "update_feature_flags" => run_cmd(&app, request.params, settings::update_feature_flags),
        // Secret management
        "secret_get_global" => run_cmd(&app, request.params, secret::secret_get_global),
        "secret_set_global" => run_cmd(&app, request.params, secret::secret_set_global),
        // User management
        "get_user" => run_cmd(&app, request.params, users::get_user),
        "set_user" => run_cmd(&app, request.params, users::set_user),
        "delete_user" => run_cmd(&app, request.params, users::delete_user),
        // Project management
        "update_project" => run_cmd(&app, request.params, iprojects::update_project),
        "add_project" => run_cmd(&app, request.params, iprojects::add_project),
        "get_project" => run_cmd(&app, request.params, iprojects::get_project),
        "delete_project" => run_cmd(&app, request.params, iprojects::delete_project),
        "list_projects" => projects::list_projects(&extra).await,
        "set_project_active" => projects::set_project_active(&app, &extra, request.params).await,
        // Virtual branches commands
        "normalize_branch_name" => run_cmd(
            &app,
            request.params,
            virtual_branches::normalize_branch_name,
        ),
        "create_virtual_branch" => run_cmd(
            &app,
            request.params,
            virtual_branches::create_virtual_branch,
        ),
        "delete_local_branch" => {
            run_cmd(&app, request.params, virtual_branches::delete_local_branch)
        }
        "create_virtual_branch_from_branch" => run_cmd(
            &app,
            request.params,
            virtual_branches::create_virtual_branch_from_branch,
        ),
        "integrate_upstream_commits" => run_cmd(
            &app,
            request.params,
            virtual_branches::integrate_upstream_commits,
        ),
        "get_base_branch_data" => {
            run_cmd(&app, request.params, virtual_branches::get_base_branch_data)
        }
        "set_base_branch" => run_cmd(&app, request.params, virtual_branches::set_base_branch),
        "push_base_branch" => run_cmd(&app, request.params, virtual_branches::push_base_branch),
        "update_stack_order" => run_cmd(&app, request.params, virtual_branches::update_stack_order),
        "unapply_stack" => run_cmd(&app, request.params, virtual_branches::unapply_stack),
        "can_apply_remote_branch" => run_cmd(
            &app,
            request.params,
            virtual_branches::can_apply_remote_branch,
        ),
        "list_commit_files" => run_cmd(&app, request.params, virtual_branches::list_commit_files),
        "amend_virtual_branch" => {
            run_cmd(&app, request.params, virtual_branches::amend_virtual_branch)
        }
        "undo_commit" => run_cmd(&app, request.params, virtual_branches::undo_commit),
        "insert_blank_commit" => {
            run_cmd(&app, request.params, virtual_branches::insert_blank_commit)
        }
        "reorder_stack" => run_cmd(&app, request.params, virtual_branches::reorder_stack),
        "find_git_branches" => run_cmd(&app, request.params, virtual_branches::find_git_branches),
        "list_branches" => run_cmd(&app, request.params, virtual_branches::list_branches),
        "get_branch_listing_details" => run_cmd(
            &app,
            request.params,
            virtual_branches::get_branch_listing_details,
        ),
        "squash_commits" => run_cmd(&app, request.params, virtual_branches::squash_commits),
        "fetch_from_remotes" => run_cmd(&app, request.params, virtual_branches::fetch_from_remotes),
        "move_commit" => run_cmd(&app, request.params, virtual_branches::move_commit),
        "update_commit_message" => run_cmd(
            &app,
            request.params,
            virtual_branches::update_commit_message,
        ),
        "find_commit" => run_cmd(&app, request.params, virtual_branches::find_commit),
        "upstream_integration_statuses" => run_cmd(
            &app,
            request.params,
            virtual_branches::upstream_integration_statuses,
        ),
        "integrate_upstream" => run_cmd(&app, request.params, virtual_branches::integrate_upstream),
        "resolve_upstream_integration" => run_cmd(
            &app,
            request.params,
            virtual_branches::resolve_upstream_integration,
        ),
        // Operating modes commands
        "operating_mode" => run_cmd(&app, request.params, modes::operating_mode),
        "enter_edit_mode" => run_cmd(&app, request.params, modes::enter_edit_mode),
        "abort_edit_and_return_to_workspace" => run_cmd(
            &app,
            request.params,
            modes::abort_edit_and_return_to_workspace,
        ),
        "save_edit_and_return_to_workspace" => run_cmd(
            &app,
            request.params,
            modes::save_edit_and_return_to_workspace,
        ),
        "edit_initial_index_state" => {
            run_cmd(&app, request.params, modes::edit_initial_index_state)
        }
        "edit_changes_from_initial" => {
            run_cmd(&app, request.params, modes::edit_changes_from_initial)
        }
        // Repository commands
        "git_get_local_config" => run_cmd(&app, request.params, repo::git_get_local_config),
        "git_set_local_config" => run_cmd(&app, request.params, repo::git_set_local_config),
        "check_signing_settings" => run_cmd(&app, request.params, repo::check_signing_settings),
        "git_clone_repository" => run_cmd(&app, request.params, repo::git_clone_repository),
        "get_uncommited_files" => run_cmd(&app, request.params, repo::get_uncommitted_files),
        "get_commit_file" => run_cmd(&app, request.params, repo::get_commit_file),
        "get_workspace_file" => run_cmd(&app, request.params, repo::get_workspace_file),
        "pre_commit_hook" => run_cmd(&app, request.params, repo::pre_commit_hook),
        "pre_commit_hook_diffspecs" => {
            run_cmd(&app, request.params, repo::pre_commit_hook_diffspecs)
        }
        "post_commit_hook" => run_cmd(&app, request.params, repo::post_commit_hook),
        "message_hook" => run_cmd(&app, request.params, repo::message_hook),
        // Stack management commands
        "create_branch" => run_cmd(&app, request.params, stack::create_branch),
        "remove_branch" => run_cmd(&app, request.params, stack::remove_branch),
        "update_branch_name" => run_cmd(&app, request.params, stack::update_branch_name),
        "update_branch_description" => {
            run_cmd(&app, request.params, stack::update_branch_description)
        }
        "update_branch_pr_number" => run_cmd(&app, request.params, stack::update_branch_pr_number),
        "push_stack" => run_cmd(&app, request.params, stack::push_stack),
        "push_stack_to_review" => run_cmd(&app, request.params, stack::push_stack_to_review),
        // Undo/Snapshot commands
        "list_snapshots" => run_cmd(&app, request.params, undo::list_snapshots),
        "restore_snapshot" => run_cmd(&app, request.params, undo::restore_snapshot),
        "snapshot_diff" => run_cmd(&app, request.params, undo::snapshot_diff),
        // "oplog_diff_worktrees" => undo::oplog_diff_worktrees(&ctx, request.params),
        // Config management commands
        "get_gb_config" => run_cmd(&app, request.params, config::get_gb_config),
        "set_gb_config" => run_cmd(&app, request.params, config::set_gb_config),
        // Remotes management commands
        "list_remotes" => run_cmd(&app, request.params, remotes::list_remotes),
        "add_remote" => run_cmd(&app, request.params, remotes::add_remote),
        // Rules/Workspace rules commands
        "create_workspace_rule" => run_cmd(&app, request.params, rules::create_workspace_rule),
        "delete_workspace_rule" => run_cmd(&app, request.params, rules::delete_workspace_rule),
        "update_workspace_rule" => run_cmd(&app, request.params, rules::update_workspace_rule),
        "list_workspace_rules" => run_cmd(&app, request.params, rules::list_workspace_rules),
        "init_device_oauth" => {
            let result = github::init_device_oauth(&app, NoParams {}).await;
            result.map(|r| json!(r))
        }
        "check_auth_status" => {
            let params = serde_json::from_value(request.params).to_error();
            match params {
                Ok(params) => {
                    let result = github::check_auth_status(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Forge commands
        "pr_templates" => run_cmd(&app, request.params, forge::pr_templates),
        "pr_template" => run_cmd(&app, request.params, forge::pr_template),
        // // Menu commands (limited - no menu_item_set_enabled as it's Tauri-specific)
        // "get_editor_link_scheme" => menu::get_editor_link_scheme(&ctx, request.params),
        // CLI commands
        "install_cli" => run_cmd(&app, request.params, cli::install_cli),
        "cli_path" => run_cmd(&app, request.params, cli::cli_path),
        // Askpass commands (async)
        "submit_prompt_response" => {
            let params = serde_json::from_value(request.params).to_error();
            match params {
                Ok(params) => {
                    let result = askpass::submit_prompt_response(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Open/System commands (limited - no open_project_in_window as it's Tauri-specific)
        "open_url" => run_cmd(&app, request.params, open::open_url),
        "show_in_finder" => run_cmd(&app, request.params, open::show_in_finder),

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
        "get_project_archive_path" => run_cmd(&app, request.params, zip::get_project_archive_path),
        "get_logs_archive_path" => run_cmd(&app, request.params, zip::get_logs_archive_path),
        "claude_send_message" => {
            let params = serde_json::from_value(request.params).to_error();
            match params {
                Ok(params) => {
                    let result = claude::claude_send_message(&app, params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_get_messages" => {
            let params = serde_json::from_value(request.params).to_error();
            match params {
                Ok(params) => {
                    let result = claude::claude_get_messages(&app, params);
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "claude_get_session_details" => {
            run_cmd(&app, request.params, claude::claude_get_session_details)
        }
        "claude_list_permission_requests" => run_cmd(
            &app,
            request.params,
            claude::claude_list_permission_requests,
        ),
        "claude_update_permission_request" => run_cmd(
            &app,
            request.params,
            claude::claude_update_permission_request,
        ),

        _ => Err(anyhow::anyhow!("Command {} not found!", command).into()),
    };

    match result {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => Json(json!(Response::Error(json!(e)))),
    }
}
