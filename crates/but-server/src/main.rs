use std::sync::Arc;

use axum::{Json, Router, routing::get};
use but_settings::AppSettingsWithDiskSync;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

mod projects;
mod settings;
mod users;

pub(crate) struct RequestContext {
    app_settings: Arc<AppSettingsWithDiskSync>,
    user_controller: Arc<gitbutler_user::Controller>,
    project_controller: Arc<gitbutler_project::Controller>,
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

    // build our application with a single route
    let app = Router::new()
        .route(
            "/",
            get(|| async { "Hello, World!" }).post(move |req| {
                let ctx = RequestContext {
                    app_settings: Arc::clone(&app_settings),
                    user_controller: Arc::clone(&user_controller),
                    project_controller: Arc::clone(&project_controller),
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
        // User management
        "get_user" => users::get_user(&ctx),
        "set_user" => users::set_user(&ctx, request.params),
        "delete_user" => users::delete_user(&ctx, request.params),
        // Project management
        "update_project" => projects::update_project(&ctx, request.params),
        "add_project" => projects::add_project(&ctx, request.params),
        "get_project" => projects::get_project(&ctx, request.params),
        "list_projects" => projects::list_projects(&ctx, request.params),
        "delete_project" => projects::delete_project(&ctx, request.params),
        _ => Err(anyhow::anyhow!("Command {} not found!", command)),
    };

    match result {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => Json(json!(Response::Error(json!(e.to_string())))),
    }
}
