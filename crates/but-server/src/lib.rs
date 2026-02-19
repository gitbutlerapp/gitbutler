use std::{future::Future, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    body::Body,
    extract::{
        ConnectInfo, Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    middleware::Next,
    response::IntoResponse,
    routing::{any, post},
};
use but_api::{commit, diff, github, gitlab, json, legacy, platform};
use but_claude::{Broadcaster, Claude};
use but_ctx::Context;
use but_settings::AppSettingsWithDiskSync;
use futures_util::{SinkExt, StreamExt as _};
use gitbutler_project::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower_http::cors::{self, CorsLayer};

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

#[derive(Clone)]
struct AppState {
    app: Claude,
    extra: Extra,
    app_settings: AppSettingsWithDiskSync,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeGetSessionDetailsParams {
    project_id: ProjectId,
    session_id: uuid::Uuid,
}

/// Wraps a synchronous command handler that takes `serde_json::Value` params and returns
/// `anyhow::Result<serde_json::Value>` into an axum handler.
// TODO: implement these as actual `Handler`s so that boxing isn't required.
//       Maybe this could also be defined generically.
fn json_response<F>(
    handler: F,
) -> impl Fn(Json<serde_json::Value>) -> std::pin::Pin<Box<dyn Future<Output = Json<serde_json::Value>> + Send>> + Clone + Send
where
    F: Fn(serde_json::Value) -> anyhow::Result<serde_json::Value> + Clone + Send + 'static,
{
    move |Json(params)| {
        let res = handler(params);
        Box::pin(async move { cmd_result_to_json(res) })
    }
}

/// Wraps an async command handler that takes `serde_json::Value` params and returns
/// `anyhow::Result<serde_json::Value>` into an axum handler.
fn json_response_async<F, Fut>(
    handler: F,
) -> impl Fn(Json<serde_json::Value>) -> std::pin::Pin<Box<dyn Future<Output = Json<serde_json::Value>> + Send>> + Clone + Send
where
    F: Fn(serde_json::Value) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = anyhow::Result<serde_json::Value>> + Send + 'static,
{
    move |Json(params)| {
        let handler = handler.clone();
        Box::pin(async move { cmd_result_to_json(handler(params).await) })
    }
}

fn cmd_result_to_json(res: anyhow::Result<serde_json::Value>) -> Json<serde_json::Value> {
    match res {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => {
            let e = json::Error::from(e);
            Json(json!(Response::Error(json!(e))))
        }
    }
}

/// Middleware to ensure all connections are from localhost only
async fn localhost_only_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: axum::extract::Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    // Check if the connection is from localhost (127.0.0.1 or ::1)
    if addr.ip().is_loopback() {
        Ok(next.run(req).await)
    } else {
        tracing::warn!("Rejected non-localhost connection from: {}", addr);
        Err(StatusCode::FORBIDDEN)
    }
}

pub async fn run() {
    but_api::panic_capture::install_panic_hook();
    let cors = CorsLayer::new()
        .allow_methods(cors::Any)
        .allow_origin(cors::AllowOrigin::predicate(|origin, _parts| {
            origin
                .as_bytes()
                .strip_prefix(b"http://localhost")
                .is_some_and(|rest| rest.first().is_none_or(|b| *b == b':'))
        }))
        .allow_headers(cors::Any);

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
    let app_settings = AppSettingsWithDiskSync::new_with_customization(config_dir.clone(), None)
        .expect("failed to create app settings");

    let app = Claude {
        broadcaster: broadcaster.clone(),
        instance_by_stack: Default::default(),
    };

    let state = AppState {
        app,
        extra,
        app_settings,
    };

    let app = Router::new()
        .route(
            "/git_remote_branches",
            post(json_response(legacy::git::git_remote_branches_cmd)),
        )
        .route("/git_test_push", post(json_response(legacy::git::git_test_push_cmd)))
        .route("/git_test_fetch", post(json_response(legacy::git::git_test_fetch_cmd)))
        .route("/git_index_size", post(json_response(legacy::git::git_index_size_cmd)))
        .route(
            "/delete_all_data",
            post(json_response(legacy::git::delete_all_data_cmd)),
        )
        .route(
            "/git_set_global_config",
            post(json_response(legacy::git::git_set_global_config_cmd)),
        )
        .route(
            "/git_remove_global_config",
            post(json_response(legacy::git::git_remove_global_config_cmd)),
        )
        .route(
            "/git_get_global_config",
            post(json_response(legacy::git::git_get_global_config_cmd)),
        )
        .route(
            "/tree_change_diffs",
            post(json_response(legacy::diff::tree_change_diffs_cmd)),
        )
        .route(
            "/commit_details_with_line_stats",
            post(json_response(diff::commit_details_with_line_stats_cmd)),
        )
        .route("/branch_diff", post(json_response(but_api::branch::branch_diff_cmd)))
        .route(
            "/changes_in_worktree",
            post(json_response(legacy::diff::changes_in_worktree_cmd)),
        )
        .route("/assign_hunk", post(json_response(legacy::diff::assign_hunk_cmd)))
        .route(
            "/cherry_apply_status",
            post(json_response(legacy::cherry_apply::cherry_apply_status_cmd)),
        )
        .route(
            "/cherry_apply",
            post(json_response(legacy::cherry_apply::cherry_apply_cmd)),
        )
        .route("/stacks", post(json_response(legacy::workspace::stacks_cmd)))
        .route("/head_info", post(json_response(legacy::workspace::head_info_cmd)));

    #[cfg(unix)]
    let app = app.route(
        "/show_graph_svg",
        post(json_response(legacy::workspace::show_graph_svg_cmd)),
    );

    let app = app
        .route(
            "/stack_details",
            post(json_response(legacy::workspace::stack_details_cmd)),
        )
        .route(
            "/branch_details",
            post(json_response(legacy::workspace::branch_details_cmd)),
        )
        .route(
            "/create_commit_from_worktree_changes",
            post(json_response(
                legacy::workspace::create_commit_from_worktree_changes_cmd,
            )),
        )
        .route(
            "/amend_commit_from_worktree_changes",
            post(json_response(
                legacy::workspace::amend_commit_from_worktree_changes_cmd,
            )),
        )
        .route(
            "/discard_worktree_changes",
            post(json_response(
                legacy::workspace::discard_worktree_changes_cmd,
            )),
        )
        .route(
            "/move_changes_between_commits",
            post(json_response(
                legacy::workspace::move_changes_between_commits_cmd,
            )),
        )
        .route(
            "/split_branch",
            post(json_response(legacy::workspace::split_branch_cmd)),
        )
        .route(
            "/split_branch_into_dependent_branch",
            post(json_response(
                legacy::workspace::split_branch_into_dependent_branch_cmd,
            )),
        )
        .route(
            "/uncommit_changes",
            post(json_response(legacy::workspace::uncommit_changes_cmd)),
        )
        .route(
            "/stash_into_branch",
            post(json_response(legacy::workspace::stash_into_branch_cmd)),
        )
        .route(
            "/canned_branch_name",
            post(json_response(legacy::workspace::canned_branch_name_cmd)),
        )
        .route(
            "/target_commits",
            post(json_response(legacy::workspace::target_commits_cmd)),
        )
        .route(
            "/secret_get_global",
            post(json_response(legacy::secret::secret_get_global_cmd)),
        )
        .route(
            "/secret_set_global",
            post(json_response(legacy::secret::secret_set_global_cmd)),
        )
        .route(
            "/secret_delete_global",
            post(json_response(legacy::secret::secret_delete_global_cmd)),
        )
        // User management
        .route(
            "/get_user",
            post(json_response(legacy::users::get_user_cmd)),
        )
        .route(
            "/set_user",
            post(json_response(legacy::users::set_user_cmd)),
        )
        .route(
            "/delete_user",
            post(json_response(legacy::users::delete_user_cmd)),
        )
        .route(
            "/update_project",
            post(json_response(legacy::projects::update_project_cmd)),
        )
        .route(
            "/add_project",
            post(json_response(legacy::projects::add_project_cmd)),
        )
        .route(
            "/add_project_best_effort",
            post(json_response(legacy::projects::add_project_best_effort_cmd)),
        )
        .route(
            "/get_project",
            post(json_response(legacy::projects::get_project_cmd)),
        )
        .route(
            "/delete_project",
            post(json_response(legacy::projects::delete_project_cmd)),
        )
        .route(
            "/is_gerrit",
            post(json_response(legacy::projects::is_gerrit_cmd)),
        )
        // Virtual branches commands
        .route(
            "/normalize_branch_name",
            post(json_response(
                legacy::virtual_branches::normalize_branch_name_cmd,
            )),
        )
        .route(
            "/create_virtual_branch",
            post(json_response(
                legacy::virtual_branches::create_virtual_branch_cmd,
            )),
        )
        .route(
            "/delete_local_branch",
            post(json_response(
                legacy::virtual_branches::delete_local_branch_cmd,
            )),
        )
        .route(
            "/create_virtual_branch_from_branch",
            post(json_response(
                legacy::virtual_branches::create_virtual_branch_from_branch_cmd,
            )),
        )
        .route(
            "/integrate_upstream_commits",
            post(json_response(
                legacy::virtual_branches::integrate_upstream_commits_cmd,
            )),
        )
        .route(
            "/get_initial_integration_steps_for_branch",
            post(json_response(
                legacy::virtual_branches::get_initial_integration_steps_for_branch_cmd,
            )),
        )
        .route(
            "/integrate_branch_with_steps",
            post(json_response(
                legacy::virtual_branches::integrate_branch_with_steps_cmd,
            )),
        )
        .route(
            "/get_base_branch_data",
            post(json_response(
                legacy::virtual_branches::get_base_branch_data_cmd,
            )),
        )
        .route(
            "/set_base_branch",
            post(json_response(legacy::virtual_branches::set_base_branch_cmd)),
        )
        .route(
            "/switch_back_to_workspace",
            post(json_response(
                legacy::virtual_branches::switch_back_to_workspace_cmd,
            )),
        )
        .route(
            "/push_base_branch",
            post(json_response(
                legacy::virtual_branches::push_base_branch_cmd,
            )),
        )
        .route(
            "/update_stack_order",
            post(json_response(
                legacy::virtual_branches::update_stack_order_cmd,
            )),
        )
        .route(
            "/unapply_stack",
            post(json_response(legacy::virtual_branches::unapply_stack_cmd)),
        )
        .route(
            "/amend_virtual_branch",
            post(json_response(
                legacy::virtual_branches::amend_virtual_branch_cmd,
            )),
        )
        .route(
            "/undo_commit",
            post(json_response(legacy::virtual_branches::undo_commit_cmd)),
        )
        .route(
            "/reorder_stack",
            post(json_response(legacy::virtual_branches::reorder_stack_cmd)),
        )
        .route(
            "/commit_insert_blank",
            post(json_response(commit::commit_insert_blank_cmd)),
        )
        .route(
            "/list_branches",
            post(json_response(legacy::virtual_branches::list_branches_cmd)),
        )
        .route(
            "/get_branch_listing_details",
            post(json_response(
                legacy::virtual_branches::get_branch_listing_details_cmd,
            )),
        )
        .route(
            "/squash_commits",
            post(json_response(legacy::virtual_branches::squash_commits_cmd)),
        )
        .route(
            "/fetch_from_remotes",
            post(json_response(
                legacy::virtual_branches::fetch_from_remotes_cmd,
            )),
        )
        .route(
            "/move_commit",
            post(json_response(legacy::virtual_branches::move_commit_cmd)),
        )
        .route(
            "/move_branch",
            post(json_response(legacy::virtual_branches::move_branch_cmd)),
        )
        .route(
            "/tear_off_branch",
            post(json_response(legacy::virtual_branches::tear_off_branch_cmd)),
        )
        .route(
            "/update_commit_message",
            post(json_response(
                legacy::virtual_branches::update_commit_message_cmd,
            )),
        )
        .route(
            "/operating_mode",
            post(json_response(legacy::modes::operating_mode_cmd)),
        )
        .route(
            "/head_sha",
            post(json_response(legacy::modes::head_sha_cmd)),
        )
        .route(
            "/enter_edit_mode",
            post(json_response(legacy::modes::enter_edit_mode_cmd)),
        )
        .route(
            "/abort_edit_and_return_to_workspace",
            post(json_response(
                legacy::modes::abort_edit_and_return_to_workspace_cmd,
            )),
        )
        .route(
            "/save_edit_and_return_to_workspace",
            post(json_response(
                legacy::modes::save_edit_and_return_to_workspace_cmd,
            )),
        )
        .route(
            "/edit_initial_index_state",
            post(json_response(legacy::modes::edit_initial_index_state_cmd)),
        )
        .route(
            "/edit_changes_from_initial",
            post(json_response(legacy::modes::edit_changes_from_initial_cmd)),
        )
        .route(
            "/check_signing_settings",
            post(json_response(legacy::repo::check_signing_settings_cmd)),
        )
        .route(
            "/git_clone_repository",
            post(json_response_async(legacy::repo::git_clone_repository_cmd)),
        )
        .route(
            "/get_commit_file",
            post(json_response(legacy::repo::get_commit_file_cmd)),
        )
        .route(
            "/get_workspace_file",
            post(json_response(legacy::repo::get_workspace_file_cmd)),
        )
        .route(
            "/get_blob_file",
            post(json_response(legacy::repo::get_blob_file_cmd)),
        )
        .route(
            "/find_files",
            post(json_response(legacy::repo::find_files_cmd)),
        )
        .route(
            "/pre_commit_hook_diffspecs",
            post(json_response(legacy::repo::pre_commit_hook_diffspecs_cmd)),
        )
        .route(
            "/post_commit_hook",
            post(json_response(legacy::repo::post_commit_hook_cmd)),
        )
        .route(
            "/message_hook",
            post(json_response(legacy::repo::message_hook_cmd)),
        )
        .route(
            "/create_branch",
            post(json_response(legacy::stack::create_branch_cmd)),
        )
        .route(
            "/create_reference",
            post(json_response(legacy::stack::create_reference_cmd)),
        )
        .route(
            "/remove_branch",
            post(json_response(legacy::stack::remove_branch_cmd)),
        )
        .route(
            "/update_branch_name",
            post(json_response(legacy::stack::update_branch_name_cmd)),
        )
        .route(
            "/update_branch_pr_number",
            post(json_response(legacy::stack::update_branch_pr_number_cmd)),
        )
        .route(
            "/push_stack",
            post(json_response(legacy::stack::push_stack_cmd)),
        )
        // Undo/Snapshot commands
        .route(
            "/list_snapshots",
            post(json_response(legacy::oplog::list_snapshots_cmd)),
        )
        .route(
            "/restore_snapshot",
            post(json_response(legacy::oplog::restore_snapshot_cmd)),
        )
        .route(
            "/snapshot_diff",
            post(json_response(legacy::oplog::snapshot_diff_cmd)),
        )
        .route(
            "/get_gb_config",
            post(json_response(legacy::config::get_gb_config_cmd)),
        )
        .route(
            "/set_gb_config",
            post(json_response(legacy::config::set_gb_config_cmd)),
        )
        .route(
            "/store_author_globally_if_unset",
            post(json_response(
                legacy::config::store_author_globally_if_unset_cmd,
            )),
        )
        .route(
            "/get_author_info",
            post(json_response(legacy::config::get_author_info_cmd)),
        )
        .route(
            "/list_remotes",
            post(json_response(legacy::remotes::list_remotes_cmd)),
        )
        .route(
            "/add_remote",
            post(json_response(legacy::remotes::add_remote_cmd)),
        )
        .route(
            "/create_workspace_rule",
            post(json_response(legacy::rules::create_workspace_rule_cmd)),
        )
        .route(
            "/delete_workspace_rule",
            post(json_response(legacy::rules::delete_workspace_rule_cmd)),
        )
        .route(
            "/update_workspace_rule",
            post(json_response(legacy::rules::update_workspace_rule_cmd)),
        )
        .route(
            "/list_workspace_rules",
            post(json_response(legacy::rules::list_workspace_rules_cmd)),
        )
        .route(
            "/forget_github_account",
            post(json_response(github::forget_github_account_cmd)),
        )
        .route(
            "/clear_all_github_tokens",
            post(json_response(github::clear_all_github_tokens_cmd)),
        )
        .route(
            "/forget_gitlab_account",
            post(json_response(gitlab::forget_gitlab_account_cmd)),
        )
        .route(
            "/clear_all_gitlab_tokens",
            post(json_response(gitlab::clear_all_gitlab_tokens_cmd)),
        )
        // Forge commands
        .route(
            "/pr_templates",
            post(json_response(legacy::forge::pr_templates_cmd)),
        )
        .route(
            "/pr_template",
            post(json_response(legacy::forge::pr_template_cmd)),
        )
        .route(
            "/install_cli",
            post(json_response(legacy::cli::install_cli_cmd)),
        )
        .route("/cli_path", post(json_response(legacy::cli::cli_path_cmd)))
        .route("/open_url", post(json_response(legacy::open::open_url_cmd)))
        .route(
            "/open_in_terminal",
            post(json_response(legacy::open::open_in_terminal_cmd)),
        )
        .route(
            "/show_in_finder",
            post(json_response(legacy::open::show_in_finder_cmd)),
        )
        .route("/absorb", post(json_response(legacy::absorb::absorb_cmd)))
        .route(
            "/absorption_plan",
            post(json_response(legacy::absorb::absorption_plan_cmd)),
        )
        .route(
            "/claude_get_user_message",
            post(json_response(legacy::claude::claude_get_user_message_cmd)),
        )
        .route(
            "/claude_list_permission_requests",
            post(json_response(
                legacy::claude::claude_list_permission_requests_cmd,
            )),
        )
        .route(
            "/claude_update_permission_request",
            post(json_response(
                legacy::claude::claude_update_permission_request_cmd,
            )),
        )
        .route(
            "/claude_answer_ask_user_question",
            post(json_response(
                legacy::claude::claude_answer_ask_user_question_cmd,
            )),
        )
        .route(
            "/claude_list_prompt_templates",
            post(json_response(
                legacy::claude::claude_list_prompt_templates_cmd,
            )),
        )
        .route(
            "/claude_get_prompt_dirs",
            post(json_response(legacy::claude::claude_get_prompt_dirs_cmd)),
        )
        .route(
            "/claude_maybe_create_prompt_dir",
            post(json_response(
                legacy::claude::claude_maybe_create_prompt_dir_cmd,
            )),
        )
        .route(
            "/commit_reword",
            post(json_response(commit::commit_reword_cmd)),
        )
        .route(
            "/commit_move_changes_between",
            post(json_response(commit::commit_move_changes_between_cmd)),
        )
        .route(
            "/commit_uncommit_changes",
            post(json_response(commit::commit_uncommit_changes_cmd)),
        )
        .route("/build_type", post(json_response(platform::build_type_cmd)))
        // Catch-all for commands that need special handling (app, extra, app_settings_sync)
        .route("/{command}", post(post_handle_command_with_path))
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
        .layer(cors)
        // Middleware to ensure only localhost connections are accepted.
        // Note: In Axum, layers are applied in reverse order, so this middleware
        // runs BEFORE CORS processing, ensuring these security checks happen first.
        .layer(axum::middleware::from_fn(localhost_only_middleware))
        .with_state(state);

    let port = std::env::var("BUTLER_PORT").unwrap_or("6978".into());
    let host = std::env::var("BUTLER_HOST").unwrap_or("127.0.0.1".into());
    let url = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&url).await.unwrap();
    println!("Running at {url}");
    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}

/// Handler that extracts the command from the URL path.
/// This allows calling `POST /command_name` with params as the JSON body.
async fn post_handle_command_with_path(
    State(state): State<AppState>,
    Path(command): Path<String>,
    Json(params): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let app = state.app;
    let extra = state.extra;
    let app_settings_sync = state.app_settings;
    let req = Request { command, params };
    let res = handle_command(req, app, extra, app_settings_sync).await;
    match res {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => {
            let e = json::Error::from(e);
            Json(json!(Response::Error(json!(e))))
        }
    }
}

async fn handle_ws_request(ws: WebSocketUpgrade, broadcaster: Arc<Mutex<Broadcaster>>) -> impl IntoResponse {
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
    request: Request,
    // TODO: this is due to mixing UI broadcasting into Claude related state (which also broadcasts)
    app: Claude,
    extra: Extra,
    app_settings_sync: AppSettingsWithDiskSync,
    // TODO: make this anyhow::Result<serde_json::Value>
) -> anyhow::Result<serde_json::Value> {
    let command: &str = &request.command;
    match command {
        // App settings (need app_settings_sync)
        "get_app_settings" => Ok(to_json_or_panic(app_settings_sync.get()?.clone())),
        "update_onboarding_complete" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_onboarding_complete(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_telemetry" => deserialize_json(request.params)
            .and_then(|params| legacy::settings::update_telemetry(&app_settings_sync, params).map(|r| json!(r))),
        "update_telemetry_distinct_id" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_telemetry_distinct_id(&app_settings_sync, params).map(|r| json!(r))
        }),
        "update_feature_flags" => deserialize_json(request.params)
            .and_then(|params| legacy::settings::update_feature_flags(&app_settings_sync, params).map(|r| json!(r))),
        "update_claude" => deserialize_json(request.params)
            .and_then(|params| legacy::settings::update_claude(&app_settings_sync, params).map(|r| json!(r))),
        "update_fetch" => deserialize_json(request.params)
            .and_then(|params| legacy::settings::update_fetch(&app_settings_sync, params).map(|r| json!(r))),
        "update_reviews" => deserialize_json(request.params)
            .and_then(|params| legacy::settings::update_reviews(&app_settings_sync, params).map(|r| json!(r))),
        // Project management (need extra or app)
        "list_projects" => projects::list_projects(&extra).await,
        "set_project_active" => projects::set_project_active(&app, &extra, app_settings_sync, request.params).await,
        // Async virtual branches commands (not yet migrated due to different pattern)
        "upstream_integration_statuses" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::virtual_branches::upstream_integration_statuses_cmd(params).await;
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
                    let result = legacy::virtual_branches::resolve_upstream_integration_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // GitHub commands (async, not yet migrated)
        "init_github_device_oauth" => {
            let result = github::init_github_device_oauth().await;
            result.map(|r| json!(r))
        }
        "check_github_auth_status" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = github::check_github_auth_status_cmd(params).await;
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
        "list_known_github_accounts" => github::list_known_github_accounts().await.map(|r| json!(r)),
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
        // GitLab commands (async, not yet migrated)
        "store_gitlab_pat" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = gitlab::store_gitlab_pat_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "store_gitlab_selfhosted_pat" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = gitlab::store_gitlab_selfhosted_pat_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "list_known_gitlab_accounts" => gitlab::list_known_gitlab_accounts().await.map(|r| json!(r)),
        "get_gl_user" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = but_api::gitlab::get_gl_user_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        // Forge commands (some async, not yet migrated)
        "list_reviews" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::list_reviews_cmd(params);
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
        "merge_review" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::merge_review_cmd(params).await;
                    result.map(|_| json!({"result": "success"}))
                }
                Err(e) => Err(e),
            }
        }
        "update_review_footers" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::update_review_footers_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
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

        // Zip/Archive commands (need extra)
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
        // Claude commands (need app)
        "claude_send_message" => {
            let params = deserialize_json(request.params)?;
            let result = legacy::claude::claude_send_message(&app, params).await;
            result.map(|r| json!(r))
        }
        "claude_get_config" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Params {
                project_id: ProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let ctx = Context::new_from_legacy_project_id(params.project_id)?;
            let result = legacy::claude::claude_get_config(ctx.into_sync()).await;
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
            let params = deserialize_json(request.params);
            match params {
                Ok(ClaudeGetSessionDetailsParams { project_id, session_id }) => {
                    let ctx = Context::new_from_legacy_project_id(project_id)?;
                    let result = legacy::claude::claude_get_session_details(ctx.into_sync(), session_id).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
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
        "claude_get_sub_agents" => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Params {
                project_id: ProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let ctx = Context::new_from_legacy_project_id(params.project_id)?;
            let result = legacy::claude::claude_get_sub_agents(ctx.into_sync()).await;
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
            let ctx = Context::new_from_legacy_project_id(params.project_id)?;
            let result = legacy::claude::claude_verify_path(ctx.into_sync(), params.path).await;
            result.map(|r| json!(r))
        }

        _ => Err(anyhow::anyhow!("Command {command} not found!")),
    }
}

fn to_json_or_panic(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

fn deserialize_json<T: serde::de::DeserializeOwned>(value: serde_json::Value) -> anyhow::Result<T> {
    Ok(serde_json::from_value(value)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn claude_get_session_details_params_deserialize_uuid_string() {
        let project_id = uuid::Uuid::new_v4();
        let session_id = uuid::Uuid::new_v4();
        let params: ClaudeGetSessionDetailsParams = deserialize_json(json!({
            "projectId": project_id,
            "sessionId": session_id,
        }))
        .expect("params should deserialize");

        assert_eq!(params.project_id.to_string(), project_id.to_string());
        assert_eq!(params.session_id, session_id);
    }
}
