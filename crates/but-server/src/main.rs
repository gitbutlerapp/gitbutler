use std::sync::Arc;

use axum::{Json, Router, routing::get};
use but_settings::AppSettingsWithDiskSync;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

use crate::projects::ActiveProjects;

mod commands;
mod diff;
mod modes;
mod projects;
mod secret;
mod settings;
mod stack;
mod users;
mod virtual_branches;
mod workspace;

pub(crate) struct RequestContext {
    app_settings: Arc<AppSettingsWithDiskSync>,
    user_controller: Arc<gitbutler_user::Controller>,
    project_controller: Arc<gitbutler_project::Controller>,
    active_projects: Arc<Mutex<ActiveProjects>>,
}

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

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    let config_dir = dirs::config_dir()
        .expect("missing config dir")
        .join("gitbutler");

    // TODO: This should probably be a real com.gitbutler.whatever directory
    let app_data_dir = dirs::config_dir()
        .expect("missing config dir")
        .join("gitbutler-server");

    let app_settings = Arc::new(
        AppSettingsWithDiskSync::new(config_dir.clone()).expect("failed to create app settings"),
    );
    let user_controller = Arc::new(gitbutler_user::Controller::from_path(&app_data_dir));
    let project_controller = Arc::new(gitbutler_project::Controller::from_path(&app_data_dir));
    let active_projects = Arc::new(Mutex::new(ActiveProjects::new()));

    // build our application with a single route
    let app = Router::new()
        .route(
            "/",
            get(|| async { "Hello, World!" }).post(move |req| {
                let ctx = RequestContext {
                    app_settings: Arc::clone(&app_settings),
                    user_controller: Arc::clone(&user_controller),
                    project_controller: Arc::clone(&project_controller),
                    active_projects: Arc::clone(&active_projects),
                };
                handle_command(req, ctx)
            }),
        )
        .layer(ServiceBuilder::new().layer(cors));

    // run our app with hyper, listening globally on port 6978
    let listener = tokio::net::TcpListener::bind("0.0.0.0:6978").await.unwrap();
    println!("Running at 0.0.0.0:6978");
    axum::serve(listener, app).await.unwrap();
}

async fn handle_command(
    Json(request): Json<Request>,
    ctx: RequestContext,
) -> Json<serde_json::Value> {
    let command: &str = &request.command;
    let result = match command {
        // App settings
        "get_app_settings" => settings::get_app_settings(&ctx),
        "update_onboarding_complete" => settings::update_onboarding_complete(&ctx, request.params),
        "update_telemetry" => settings::update_telemetry(&ctx, request.params),
        "update_telemetry_distinct_id" => {
            settings::update_telemetry_distinct_id(&ctx, request.params)
        }
        "update_feature_flags" => settings::update_feature_flags(&ctx, request.params),
        // Secret management
        "secret_get_global" => secret::secret_get_global(&ctx, request.params),
        "secret_set_global" => secret::secret_set_global(&ctx, request.params),
        // User management
        "get_user" => users::get_user(&ctx),
        "set_user" => users::set_user(&ctx, request.params),
        "delete_user" => users::delete_user(&ctx, request.params),
        // Project management
        "update_project" => projects::update_project(&ctx, request.params),
        "add_project" => projects::add_project(&ctx, request.params),
        "get_project" => projects::get_project(&ctx, request.params),
        "list_projects" => projects::list_projects(&ctx).await,
        "delete_project" => projects::delete_project(&ctx, request.params),
        "set_project_active" => projects::set_project_active(&ctx, request.params).await,
        // Virtual branches commands
        "normalize_branch_name" => virtual_branches::normalize_branch_name(request.params),
        "create_virtual_branch" => virtual_branches::create_virtual_branch(&ctx, request.params),
        "delete_local_branch" => virtual_branches::delete_local_branch(&ctx, request.params),
        "create_virtual_branch_from_branch" => virtual_branches::create_virtual_branch_from_branch(&ctx, request.params),
        "integrate_upstream_commits" => virtual_branches::integrate_upstream_commits(&ctx, request.params),
        "get_base_branch_data" => virtual_branches::get_base_branch_data(&ctx, request.params),
        "set_base_branch" => virtual_branches::set_base_branch(&ctx, request.params),
        "push_base_branch" => virtual_branches::push_base_branch(&ctx, request.params),
        "update_stack_order" => virtual_branches::update_stack_order(&ctx, request.params),
        "unapply_stack" => virtual_branches::unapply_stack(&ctx, request.params).await,
        "can_apply_remote_branch" => virtual_branches::can_apply_remote_branch(&ctx, request.params),
        "list_commit_files" => virtual_branches::list_commit_files(&ctx, request.params),
        "amend_virtual_branch" => virtual_branches::amend_virtual_branch(&ctx, request.params),
        "move_commit_file" => virtual_branches::move_commit_file(&ctx, request.params),
        "undo_commit" => virtual_branches::undo_commit(&ctx, request.params),
        "insert_blank_commit" => virtual_branches::insert_blank_commit(&ctx, request.params),
        "reorder_stack" => virtual_branches::reorder_stack(&ctx, request.params),
        "find_git_branches" => virtual_branches::find_git_branches(&ctx, request.params),
        "list_branches" => virtual_branches::list_branches(&ctx, request.params),
        "get_branch_listing_details" => virtual_branches::get_branch_listing_details(&ctx, request.params),
        "squash_commits" => virtual_branches::squash_commits(&ctx, request.params),
        "fetch_from_remotes" => virtual_branches::fetch_from_remotes(&ctx, request.params),
        "move_commit" => virtual_branches::move_commit(&ctx, request.params),
        "update_commit_message" => virtual_branches::update_commit_message(&ctx, request.params),
        "find_commit" => virtual_branches::find_commit(&ctx, request.params),
        "upstream_integration_statuses" => virtual_branches::upstream_integration_statuses(&ctx, request.params),
        "integrate_upstream" => virtual_branches::integrate_upstream(&ctx, request.params),
        "resolve_upstream_integration" => virtual_branches::resolve_upstream_integration(&ctx, request.params),
        // General commands
        "git_remote_branches" => commands::git_remote_branches(&ctx, request.params),
        "git_test_push" => commands::git_test_push(&ctx, request.params),
        "git_test_fetch" => commands::git_test_fetch(&ctx, request.params),
        "git_index_size" => commands::git_index_size(&ctx, request.params),
        "git_head" => commands::git_head(&ctx, request.params),
        "delete_all_data" => commands::delete_all_data(&ctx, request.params),
        "git_set_global_config" => commands::git_set_global_config(&ctx, request.params),
        "git_remove_global_config" => commands::git_remove_global_config(&ctx, request.params),
        "git_get_global_config" => commands::git_get_global_config(&ctx, request.params),
        // Operating modes commands
        "operating_mode" => modes::operating_mode(&ctx, request.params),
        "enter_edit_mode" => modes::enter_edit_mode(&ctx, request.params),
        "abort_edit_and_return_to_workspace" => modes::abort_edit_and_return_to_workspace(&ctx, request.params),
        "save_edit_and_return_to_workspace" => modes::save_edit_and_return_to_workspace(&ctx, request.params),
        "edit_initial_index_state" => modes::edit_initial_index_state(&ctx, request.params),
        // Stack commands
        "create_branch" => stack::create_branch(&ctx, request.params),
        "remove_branch" => stack::remove_branch(&ctx, request.params),
        "update_branch_name" => stack::update_branch_name(&ctx, request.params),
        "update_branch_description" => stack::update_branch_description(&ctx, request.params),
        "update_branch_pr_number" => stack::update_branch_pr_number(&ctx, request.params),
        "push_stack" => stack::push_stack(&ctx, request.params),
        "push_stack_to_review" => stack::push_stack_to_review(&ctx, request.params),
        // Workspace commands
        "stacks" => workspace::stacks(&ctx, request.params),
        #[cfg(unix)]
        "show_graph_svg" => workspace::show_graph_svg(&ctx, request.params),
        "stack_details" => workspace::stack_details(&ctx, request.params),
        "branch_details" => workspace::branch_details(&ctx, request.params),
        "create_commit_from_worktree_changes" => workspace::create_commit_from_worktree_changes(&ctx, request.params),
        "amend_commit_from_worktree_changes" => workspace::amend_commit_from_worktree_changes(&ctx, request.params),
        "discard_worktree_changes" => workspace::discard_worktree_changes(&ctx, request.params),
        "move_changes_between_commits" => workspace::move_changes_between_commits(&ctx, request.params),
        "split_branch" => workspace::split_branch(&ctx, request.params),
        "split_branch_into_dependent_branch" => workspace::split_branch_into_dependent_branch(&ctx, request.params),
        "uncommit_changes" => workspace::uncommit_changes(&ctx, request.params),
        "stash_into_branch" => workspace::stash_into_branch(&ctx, request.params),
        "canned_branch_name" => workspace::canned_branch_name(&ctx, request.params),
        "target_commits" => workspace::target_commits(&ctx, request.params),
        // Diff commands
        "tree_change_diffs" => diff::tree_change_diffs(&ctx, request.params),
        "commit_details" => diff::commit_details(&ctx, request.params),
        "changes_in_branch" => diff::changes_in_branch(&ctx, request.params),
        "changes_in_worktree" => diff::changes_in_worktree(&ctx, request.params),
        "assign_hunk" => diff::assign_hunk(&ctx, request.params),
        _ => Err(anyhow::anyhow!("Command {} not found!", command)),
    };

    match result {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => Json(json!(Response::Error(json!(e.to_string())))),
    }
}
