use std::{convert::Infallible, future::Future, net::SocketAddr, sync::Arc};

use colored::Colorize as _;

use axum::{
    Json, Router,
    body::Body,
    extract::{
        ConnectInfo, Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    middleware::{self, Next},
    response::IntoResponse,
    routing::{MethodRouter, any, get, post},
};
use but_api::{commit, diff, github, gitlab, json, legacy, platform};
use but_claude::{Broadcaster, Claude};
use but_ctx::{Context, ProjectHandleOrLegacyProjectId};
#[cfg(feature = "irc")]
use but_irc::IrcManager;
use but_settings::AppSettingsWithDiskSync;
use futures_util::{SinkExt, StreamExt as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{self, CorsLayer};

mod auth;
#[cfg(feature = "embedded-frontend")]
mod frontend;
#[cfg(feature = "irc")]
mod irc;
#[cfg(feature = "irc")]
mod irc_lifecycle;
mod projects;
mod tunnel;
use crate::projects::ActiveProjects;

/// Escapes a string for safe embedding in an HTML attribute value.
pub(crate) fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
#[cfg(feature = "irc")]
use but_irc::WorkingFilesBroadcast;

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
    /// When set, restricts project switching to only this project.
    pinned_project: Option<but_ctx::ProjectHandleOrLegacyProjectId>,
}

#[derive(Clone)]
struct AppState {
    app: Claude,
    extra: Extra,
    app_settings: AppSettingsWithDiskSync,
    #[cfg(feature = "irc")]
    irc_manager: IrcManager,
    #[cfg(feature = "irc")]
    working_files_broadcast: WorkingFilesBroadcast,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClaudeGetSessionDetailsParams {
    project_id: ProjectHandleOrLegacyProjectId,
    session_id: uuid::Uuid,
}

/// Converts a synchronous command handler into an axum `MethodRouter` that works with
/// `Router::route`.
fn but_post<F, S>(f: F) -> MethodRouter<S, Infallible>
where
    F: Fn(serde_json::Value) -> anyhow::Result<serde_json::Value> + Copy + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
{
    post(move |Json(params)| async move {
        let res = f(params);
        cmd_result_to_json(res)
    })
}

/// Converts an asynchronous command handler into an axum `MethodRouter` that works with
/// `Router::route`.
fn but_post_async<F, Fut, S>(f: F) -> MethodRouter<S, Infallible>
where
    F: Fn(serde_json::Value) -> Fut + Copy + Send + Sync + 'static,
    Fut: Future<Output = anyhow::Result<serde_json::Value>> + Send,
    S: Clone + Send + Sync + 'static,
{
    post(move |Json(params)| async move {
        let res = f(params).await;
        cmd_result_to_json(res)
    })
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

/// Check if an origin byte string is from localhost.
///
/// Matches `http://localhost`, `https://localhost`, `http://127.0.0.1`, and
/// `http://[::1]`, each optionally followed by `:<port>`.
fn is_localhost_origin(origin: &[u8]) -> bool {
    for prefix in [
        b"http://localhost".as_slice(),
        b"https://localhost".as_slice(),
        b"http://127.0.0.1".as_slice(),
        b"http://[::1]".as_slice(),
    ] {
        if let Some(rest) = origin.strip_prefix(prefix)
            && rest.first().is_none_or(|b| *b == b':')
        {
            return true;
        }
    }
    false
}

/// Holds the remote origin when it is established at runtime (e.g. from a
/// cloudflare tunnel) rather than from an env var.
/// Configuration for the but-server, populated from CLI args.
#[derive(Debug, Default)]
pub struct Config {
    /// Port to listen on. `but-server` defaults to 6978; `but local`/`but tunnel` default to 8080.
    pub port: Option<u16>,
    /// Host to bind to. Defaults to 127.0.0.1 in local mode, 0.0.0.0 when remote access is on.
    pub host: Option<String>,
    /// Spawn a Cloudflare quick tunnel and use its URL as the allowed remote origin.
    pub tunnel: bool,
    /// Named tunnel mode: cloudflared tunnel name (or UUID) to run.
    /// Must be paired with `tunnel_hostname`. Requires `cloudflared tunnel login`
    /// and `cloudflared tunnel route dns <name> <hostname>` to have been run already.
    pub tunnel_name: Option<String>,
    /// The public hostname routed to `tunnel_name` (e.g. `but.example.com`).
    /// Used as the CORS origin and display URL; not passed to cloudflared.
    pub tunnel_hostname: Option<String>,
    /// Allow remote access from this origin (e.g. `https://my-tunnel.trycloudflare.com`).
    pub remote_origin: Option<String>,
    /// Prefix all API routes with this path (e.g. `/api`).
    pub base_path: Option<String>,
    /// Disable authentication entirely. DANGEROUS — only use on trusted networks.
    pub allow_anyone: bool,
    /// Override the GitButler API base URL (default: <https://app.gitbutler.com>).
    pub api_url: Option<String>,
    /// If set, auto-activate this directory's project on startup and prevent switching to others.
    pub project_path: Option<std::path::PathBuf>,
}

static TUNNEL_ORIGIN: std::sync::OnceLock<String> = std::sync::OnceLock::new();
static ALLOW_ANYONE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();

/// Return the allowed remote origin (set from tunnel or --remote-origin arg).
fn allowed_remote_origin() -> Option<&'static str> {
    TUNNEL_ORIGIN.get().map(String::as_str)
}

/// Whether authentication is disabled via --dangerously-allow-anyone.
pub(crate) fn allow_anyone() -> bool {
    ALLOW_ANYONE.get().copied().unwrap_or(false)
}

/// Check if an origin matches the configured remote origin.
fn is_allowed_remote_origin(origin: &[u8]) -> bool {
    allowed_remote_origin().is_some_and(|allowed| origin == allowed.as_bytes())
}

/// Middleware to ensure all connections are from localhost only,
/// unless remote access is enabled via tunnel or `--remote-origin`.
///
/// Also rejects any browser request whose `Origin` header is neither
/// localhost nor the configured remote origin. Non-browser clients (curl,
/// API clients) don't send an `Origin` header and are unaffected. This
/// prevents a malicious website from using the visitor's browser to issue
/// cross-origin requests to a locally running but-server.
async fn localhost_only_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: axum::extract::Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    // Reject requests whose Origin header is not localhost or the allowed
    // remote origin, regardless of how the server is bound. This stops
    // cross-site requests from a malicious page the user happens to have open.
    if let Some(origin) = req.headers().get(axum::http::header::ORIGIN) {
        let origin_bytes = origin.as_bytes();
        if !is_localhost_origin(origin_bytes) && !is_allowed_remote_origin(origin_bytes) {
            tracing::warn!(
                "Rejected request with disallowed Origin: {}",
                String::from_utf8_lossy(origin_bytes)
            );
            return Err(StatusCode::FORBIDDEN);
        }
    }

    // When a remote origin is configured, cloudflared connects from localhost
    // too (it's a local process), so the IP check still passes. Skip it only
    // for explicit reverse-proxy setups where the proxy may be on another host.
    if allowed_remote_origin().is_some() {
        return Ok(next.run(req).await);
    }
    // Local-only mode: require a loopback address.
    if addr.ip().is_loopback() {
        Ok(next.run(req).await)
    } else {
        tracing::warn!("Rejected non-localhost connection from: {}", addr);
        Err(StatusCode::FORBIDDEN)
    }
}

#[cfg(feature = "irc")]
/// Return a copy of `irc` with `connection.enabled` forced to `false` when
/// the IRC feature flag is off. This lets the existing reconciliation logic
/// treat "flag turned off" the same as "user disabled the connection".
fn effective_irc(
    irc: &but_settings::app_settings::IrcSettings,
    feature_enabled: bool,
) -> but_settings::app_settings::IrcSettings {
    if feature_enabled {
        irc.clone()
    } else {
        let mut copy = irc.clone();
        copy.connection.enabled = false;
        copy
    }
}

pub async fn run(config: Config) -> anyhow::Result<()> {
    but_api::panic_capture::install_panic_hook();

    if config.allow_anyone {
        ALLOW_ANYONE.set(true).ok();
        eprintln!("WARNING: --dangerously-allow-anyone is set — authentication is disabled");
    }

    let api_url = config
        .api_url
        .unwrap_or_else(|| "https://app.gitbutler.com".into());

    let port: u16 = config
        .port
        .or_else(|| std::env::var("BUTLER_PORT").ok()?.parse().ok())
        .unwrap_or(6978);

    // Spawn cloudflared (quick or named tunnel) or set a static remote origin.
    // The child process must stay alive for the tunnel to remain open.
    let _tunnel_child =
        if let (Some(name), Some(hostname)) = (&config.tunnel_name, &config.tunnel_hostname) {
            println!(
                "{} {}",
                "Starting named cloudflare tunnel on port".dimmed(),
                port.to_string().dimmed()
            );
            let (url, child) = tunnel::start_named(name, hostname, port).await?;
            println!("{} {}", "Tunnel:".bold(), url.cyan().underline());
            TUNNEL_ORIGIN.set(url).ok();
            Some(child)
        } else if config.tunnel {
            println!(
                "{} {}",
                "Starting cloudflare tunnel on port".dimmed(),
                port.to_string().dimmed()
            );
            let (url, child) = tunnel::start(port).await?;
            println!("{} {}", "Tunnel:".bold(), url.cyan().underline());
            TUNNEL_ORIGIN.set(url).ok();
            Some(child)
        } else {
            if let Some(ref origin) = config.remote_origin {
                TUNNEL_ORIGIN
                    .set(origin.trim_end_matches('/').to_string())
                    .ok();
            }
            None
        };

    // CORS wildcards are forbidden when credentials are allowed, so always list explicitly.
    let allowed_headers: cors::AllowHeaders = vec![
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderName::from_static("x-auth-token"),
    ]
    .into();
    let allowed_methods: cors::AllowMethods = vec![
        axum::http::Method::GET,
        axum::http::Method::POST,
        axum::http::Method::PUT,
        axum::http::Method::DELETE,
        axum::http::Method::OPTIONS,
    ]
    .into();
    let cors = CorsLayer::new()
        .allow_methods(allowed_methods)
        .allow_origin(cors::AllowOrigin::predicate(|origin, _parts| {
            is_localhost_origin(origin.as_bytes()) || is_allowed_remote_origin(origin.as_bytes())
        }))
        .allow_headers(allowed_headers)
        .allow_credentials(true);

    let config_dir = but_path::app_config_dir().unwrap();
    let app_data_dir = but_path::app_data_dir().unwrap();

    let broadcaster = Arc::new(Mutex::new(Broadcaster::new()));
    let archival = Arc::new(but_feedback::Archival {
        cache_dir: app_data_dir.join("cache").clone(),
        logs_dir: app_data_dir.join("logs").clone(),
    });
    let mut extra = Extra {
        active_projects: Arc::new(Mutex::new(ActiveProjects::new())),
        archival,
        pinned_project: None,
    };
    #[cfg_attr(not(feature = "irc"), allow(unused_mut))]
    let mut app_settings =
        AppSettingsWithDiskSync::new_with_customization(config_dir.clone(), None)
            .expect("failed to create app settings");

    let app = Claude {
        broadcaster: broadcaster.clone(),
        instance_by_stack: Default::default(),
    };

    #[cfg(feature = "irc")]
    let irc_manager = IrcManager::new();

    // Auto-connect IRC connections based on settings (only when feature flag is on).
    #[cfg(feature = "irc")]
    if let Ok(settings) = app_settings.get() {
        let irc = effective_irc(&settings.irc, settings.feature_flags.irc);
        irc_lifecycle::auto_connect_on_startup(&irc_manager, &broadcaster, &irc);
    }

    // Watch for settings changes and reconcile IRC connections.
    // We track "effective" settings where connection.enabled is forced false
    // when the feature flag is off, so disabling the flag also disconnects.
    #[cfg(feature = "irc")]
    {
        let irc_manager = irc_manager.clone();
        let broadcaster = broadcaster.clone();
        let prev_irc_settings = std::sync::Mutex::new(
            app_settings
                .get()
                .ok()
                .map(|s| effective_irc(&s.irc, s.feature_flags.irc)),
        );

        app_settings
            .watch_in_background(move |app_settings| {
                let new_irc = effective_irc(&app_settings.irc, app_settings.feature_flags.irc);
                if let Ok(mut prev) = prev_irc_settings.lock() {
                    if let Some(old_irc) = prev.as_ref()
                        && old_irc != &new_irc
                    {
                        tracing::info!("IRC settings changed, reconciling connections");
                        irc_lifecycle::on_settings_changed(
                            &irc_manager,
                            &broadcaster,
                            old_irc,
                            &new_irc,
                        );
                    }
                    *prev = Some(new_irc);
                }
                Ok(())
            })
            .expect("failed to start settings watcher");
    }

    #[cfg(feature = "irc")]
    let irc_manager_for_shutdown = irc_manager.clone();
    #[cfg(feature = "irc")]
    let working_files_broadcast = WorkingFilesBroadcast::new(irc_manager.clone());

    // If a project path was provided, auto-activate that project and pin it.
    if let Some(ref project_path) = config.project_path {
        match but_ctx::Context::discover(project_path) {
            Ok(mut ctx) => {
                but_api::legacy::projects::prepare_project_for_activation(&mut ctx).ok();
                let project_id = ctx.legacy_project.id.clone();
                let mut active = extra.active_projects.lock().await;
                let activated = active
                    .set_active(
                        &mut ctx,
                        &app,
                        app_settings.clone(),
                        #[cfg(feature = "irc")]
                        working_files_broadcast.clone(),
                    )
                    .is_ok();
                drop(active);
                if activated {
                    extra.pinned_project = Some(project_id);
                } else {
                    tracing::warn!(
                        "Failed to activate project at {}; project switching will not be locked",
                        project_path.display()
                    );
                }
            }
            Err(err) => {
                tracing::warn!(
                    "Could not discover project at {}: {err}",
                    project_path.display()
                );
            }
        }
    }

    // Compute base path early — needed for both auth redirects and route nesting.
    // When --tunnel is active, default to /api so the embedded frontend (which
    // fetches from the same origin without a prefix) and API routes don't clash.
    let default_base = if config.tunnel { "/api" } else { "" };
    let mut api_base = config
        .base_path
        .as_deref()
        .unwrap_or(default_base)
        .trim_end_matches('/')
        .to_string();
    // Ensure the base path starts with '/' when non-empty so Router::nest doesn't panic.
    if !api_base.is_empty() && !api_base.starts_with('/') {
        api_base.insert(0, '/');
    }

    // Set up remote auth if a remote origin is configured (via --tunnel or --remote-origin).
    // Fail fast if no local user is found — remote access without auth would be a security hole.
    let auth_state: Option<Arc<auth::AuthState>> = if allowed_remote_origin().is_some() {
        if config.allow_anyone {
            // --dangerously-allow-anyone skips auth entirely; no user needed.
            None
        } else {
            match gitbutler_user::get_user() {
                Ok(Some(user)) => {
                    tracing::info!(
                        "Remote access enabled for user {} (id={}) via {}",
                        user.name.as_deref().unwrap_or("?"),
                        user.id,
                        api_url,
                    );
                    Some(Arc::new(auth::AuthState::new(user.id, &api_base, &api_url)))
                }
                Ok(None) => {
                    anyhow::bail!(
                        "Remote access is enabled but no local GitButler user is logged in.\n\
                         Open the GitButler desktop app and log in, then retry."
                    );
                }
                Err(e) => {
                    anyhow::bail!("Failed to read local user for remote auth: {e}");
                }
            }
        }
    } else {
        None
    };

    let state = AppState {
        app,
        extra,
        app_settings,
        #[cfg(feature = "irc")]
        irc_manager,
        #[cfg(feature = "irc")]
        working_files_broadcast,
    };

    let app = Router::new()
        .route(
            "/git_remote_branches",
            but_post(legacy::git::git_remote_branches_cmd),
        )
        .route("/git_test_push", but_post(legacy::git::git_test_push_cmd))
        .route("/git_test_fetch", but_post(legacy::git::git_test_fetch_cmd))
        .route("/git_index_size", but_post(legacy::git::git_index_size_cmd))
        .route(
            "/delete_all_data",
            but_post(legacy::git::delete_all_data_cmd),
        )
        .route(
            "/git_set_global_config",
            but_post(legacy::git::git_set_global_config_cmd),
        )
        .route(
            "/git_remove_global_config",
            but_post(legacy::git::git_remove_global_config_cmd),
        )
        .route(
            "/git_get_global_config",
            but_post(legacy::git::git_get_global_config_cmd),
        )
        .route("/tree_change_diffs", but_post(diff::tree_change_diffs_cmd))
        .route(
            "/commit_details_with_line_stats",
            but_post(diff::commit_details_with_line_stats_cmd),
        )
        .route("/branch_diff", but_post(but_api::branch::branch_diff_cmd))
        .route(
            "/changes_in_worktree",
            but_post(diff::changes_in_worktree_cmd),
        )
        .route("/assign_hunk", but_post(diff::assign_hunk_cmd))
        .route(
            "/cherry_apply_status",
            but_post(legacy::cherry_apply::cherry_apply_status_cmd),
        )
        .route(
            "/cherry_apply",
            but_post(legacy::cherry_apply::cherry_apply_cmd),
        )
        .route("/stacks", but_post(legacy::workspace::stacks_cmd))
        .route("/head_info", but_post(legacy::workspace::head_info_cmd));

    #[cfg(unix)]
    let app = app.route(
        "/show_graph_svg",
        but_post(legacy::workspace::show_graph_svg_cmd),
    );

    let app = app
        .route(
            "/stack_details",
            but_post(legacy::workspace::stack_details_cmd),
        )
        .route(
            "/branch_details",
            but_post(legacy::workspace::branch_details_cmd),
        )
        .route(
            "/create_commit_from_worktree_changes",
            but_post(legacy::workspace::create_commit_from_worktree_changes_cmd),
        )
        .route(
            "/amend_commit_from_worktree_changes",
            but_post(legacy::workspace::amend_commit_from_worktree_changes_cmd),
        )
        .route(
            "/discard_worktree_changes",
            but_post(legacy::workspace::discard_worktree_changes_cmd),
        )
        .route(
            "/move_changes_between_commits",
            but_post(legacy::workspace::move_changes_between_commits_cmd),
        )
        .route(
            "/split_branch",
            but_post(legacy::workspace::split_branch_cmd),
        )
        .route(
            "/split_branch_into_dependent_branch",
            but_post(legacy::workspace::split_branch_into_dependent_branch_cmd),
        )
        .route(
            "/uncommit_changes",
            but_post(legacy::workspace::uncommit_changes_cmd),
        )
        .route(
            "/stash_into_branch",
            but_post(legacy::workspace::stash_into_branch_cmd),
        )
        .route(
            "/canned_branch_name",
            but_post(legacy::workspace::canned_branch_name_cmd),
        )
        .route(
            "/target_commits",
            but_post(legacy::workspace::target_commits_cmd),
        )
        .route(
            "/secret_get_global",
            but_post(legacy::secret::secret_get_global_cmd),
        )
        .route(
            "/secret_set_global",
            but_post(legacy::secret::secret_set_global_cmd),
        )
        .route(
            "/secret_delete_global",
            but_post(legacy::secret::secret_delete_global_cmd),
        )
        // User management
        .route("/get_user", but_post(legacy::users::get_user_cmd))
        .route("/set_user", but_post(legacy::users::set_user_cmd))
        .route("/delete_user", but_post(legacy::users::delete_user_cmd))
        .route(
            "/update_project",
            but_post(legacy::projects::update_project_cmd),
        )
        .route("/add_project", but_post(legacy::projects::add_project_cmd))
        .route(
            "/add_project_best_effort",
            but_post(legacy::projects::add_project_best_effort_cmd),
        )
        .route("/get_project", but_post(legacy::projects::get_project_cmd))
        .route(
            "/delete_project",
            but_post(legacy::projects::delete_project_cmd),
        )
        .route("/is_gerrit", but_post(legacy::projects::is_gerrit_cmd))
        // Virtual branches commands
        .route(
            "/normalize_branch_name",
            but_post(legacy::virtual_branches::normalize_branch_name_cmd),
        )
        .route(
            "/create_virtual_branch",
            but_post(legacy::virtual_branches::create_virtual_branch_cmd),
        )
        .route(
            "/delete_local_branch",
            but_post(legacy::virtual_branches::delete_local_branch_cmd),
        )
        .route(
            "/create_virtual_branch_from_branch",
            but_post(legacy::virtual_branches::create_virtual_branch_from_branch_cmd),
        )
        .route(
            "/integrate_upstream_commits",
            but_post(legacy::virtual_branches::integrate_upstream_commits_cmd),
        )
        .route(
            "/get_initial_integration_steps_for_branch",
            but_post(legacy::virtual_branches::get_initial_integration_steps_for_branch_cmd),
        )
        .route(
            "/integrate_branch_with_steps",
            but_post(legacy::virtual_branches::integrate_branch_with_steps_cmd),
        )
        .route(
            "/get_base_branch_data",
            but_post(legacy::virtual_branches::get_base_branch_data_cmd),
        )
        .route(
            "/set_base_branch",
            but_post(legacy::virtual_branches::set_base_branch_cmd),
        )
        .route(
            "/switch_back_to_workspace",
            but_post(legacy::virtual_branches::switch_back_to_workspace_cmd),
        )
        .route(
            "/push_base_branch",
            but_post(legacy::virtual_branches::push_base_branch_cmd),
        )
        .route(
            "/update_stack_order",
            but_post(legacy::virtual_branches::update_stack_order_cmd),
        )
        .route(
            "/unapply_stack",
            but_post(legacy::virtual_branches::unapply_stack_cmd),
        )
        .route(
            "/amend_virtual_branch",
            but_post(legacy::virtual_branches::amend_virtual_branch_cmd),
        )
        .route(
            "/undo_commit",
            but_post(legacy::virtual_branches::undo_commit_cmd),
        )
        .route(
            "/reorder_stack",
            but_post(legacy::virtual_branches::reorder_stack_cmd),
        )
        .route(
            "/commit_insert_blank",
            but_post(commit::insert_blank::commit_insert_blank_cmd),
        )
        .route(
            "/list_branches",
            but_post(legacy::virtual_branches::list_branches_cmd),
        )
        .route(
            "/get_branch_listing_details",
            but_post(legacy::virtual_branches::get_branch_listing_details_cmd),
        )
        .route(
            "/squash_commits",
            but_post(legacy::virtual_branches::squash_commits_cmd),
        )
        .route(
            "/fetch_from_remotes",
            but_post(legacy::virtual_branches::fetch_from_remotes_cmd),
        )
        .route(
            "/move_commit",
            but_post(legacy::virtual_branches::move_commit_cmd),
        )
        .route(
            "/move_branch_legacy",
            but_post(legacy::virtual_branches::move_branch_legacy_cmd),
        )
        .route(
            "/tear_off_branch_legacy",
            but_post(legacy::virtual_branches::tear_off_branch_legacy_cmd),
        )
        .route(
            "/update_commit_message",
            but_post(legacy::virtual_branches::update_commit_message_cmd),
        )
        .route(
            "/operating_mode",
            but_post(legacy::modes::operating_mode_cmd),
        )
        .route("/head_sha", but_post(legacy::modes::head_sha_cmd))
        .route(
            "/enter_edit_mode",
            but_post(legacy::modes::enter_edit_mode_cmd),
        )
        .route(
            "/abort_edit_and_return_to_workspace",
            but_post(legacy::modes::abort_edit_and_return_to_workspace_cmd),
        )
        .route(
            "/save_edit_and_return_to_workspace",
            but_post(legacy::modes::save_edit_and_return_to_workspace_cmd),
        )
        .route(
            "/edit_initial_index_state",
            but_post(legacy::modes::edit_initial_index_state_cmd),
        )
        .route(
            "/edit_changes_from_initial",
            but_post(legacy::modes::edit_changes_from_initial_cmd),
        )
        .route(
            "/check_signing_settings",
            but_post(legacy::repo::check_signing_settings_cmd),
        )
        .route(
            "/git_clone_repository",
            but_post_async(legacy::repo::git_clone_repository_cmd),
        )
        .route(
            "/get_commit_file",
            but_post(legacy::repo::get_commit_file_cmd),
        )
        .route(
            "/get_workspace_file",
            but_post(legacy::repo::get_workspace_file_cmd),
        )
        .route("/get_blob_file", but_post(legacy::repo::get_blob_file_cmd))
        .route("/find_files", but_post(legacy::repo::find_files_cmd))
        .route(
            "/pre_commit_hook_diffspecs",
            but_post(legacy::repo::pre_commit_hook_diffspecs_cmd),
        )
        .route(
            "/post_commit_hook",
            but_post(legacy::repo::post_commit_hook_cmd),
        )
        .route("/message_hook", but_post(legacy::repo::message_hook_cmd))
        .route("/create_branch", but_post(legacy::stack::create_branch_cmd))
        .route(
            "/create_reference",
            but_post(legacy::stack::create_reference_cmd),
        )
        .route("/remove_branch", but_post(legacy::stack::remove_branch_cmd))
        .route(
            "/update_branch_name",
            but_post(legacy::stack::update_branch_name_cmd),
        )
        .route(
            "/update_branch_pr_number",
            but_post(legacy::stack::update_branch_pr_number_cmd),
        )
        .route("/push_stack", but_post(legacy::stack::push_stack_cmd))
        // Undo/Snapshot commands
        .route(
            "/list_snapshots",
            but_post(legacy::oplog::list_snapshots_cmd),
        )
        .route(
            "/restore_snapshot",
            but_post(legacy::oplog::restore_snapshot_cmd),
        )
        .route("/snapshot_diff", but_post(legacy::oplog::snapshot_diff_cmd))
        .route(
            "/get_gb_config",
            but_post(legacy::config::get_gb_config_cmd),
        )
        .route(
            "/set_gb_config",
            but_post(legacy::config::set_gb_config_cmd),
        )
        .route(
            "/store_author_globally_if_unset",
            but_post(legacy::config::store_author_globally_if_unset_cmd),
        )
        .route(
            "/get_author_info",
            but_post(legacy::config::get_author_info_cmd),
        )
        .route("/list_remotes", but_post(legacy::remotes::list_remotes_cmd))
        .route("/add_remote", but_post(legacy::remotes::add_remote_cmd))
        .route(
            "/create_workspace_rule",
            but_post(legacy::rules::create_workspace_rule_cmd),
        )
        .route(
            "/delete_workspace_rule",
            but_post(legacy::rules::delete_workspace_rule_cmd),
        )
        .route(
            "/update_workspace_rule",
            but_post(legacy::rules::update_workspace_rule_cmd),
        )
        .route(
            "/list_workspace_rules",
            but_post(legacy::rules::list_workspace_rules_cmd),
        )
        .route(
            "/forget_github_account",
            but_post(github::forget_github_account_cmd),
        )
        .route(
            "/list_known_github_accounts",
            but_post(github::list_known_github_accounts_cmd),
        )
        .route(
            "/clear_all_github_tokens",
            but_post(github::clear_all_github_tokens_cmd),
        )
        .route(
            "/forget_gitlab_account",
            but_post(gitlab::forget_gitlab_account_cmd),
        )
        .route(
            "/list_known_gitlab_accounts",
            but_post(gitlab::list_known_gitlab_accounts_cmd),
        )
        .route(
            "/clear_all_gitlab_tokens",
            but_post(gitlab::clear_all_gitlab_tokens_cmd),
        )
        // Forge commands
        .route("/pr_templates", but_post(legacy::forge::pr_templates_cmd))
        .route("/pr_template", but_post(legacy::forge::pr_template_cmd))
        .route(
            "/forge_provider",
            but_post(legacy::forge::forge_provider_cmd),
        )
        .route("/install_cli", but_post(legacy::cli::install_cli_cmd))
        .route("/cli_path", but_post(legacy::cli::cli_path_cmd))
        .route("/open_url", but_post(legacy::open::open_url_cmd))
        .route(
            "/open_in_terminal",
            but_post(legacy::open::open_in_terminal_cmd),
        )
        .route(
            "/show_in_finder",
            but_post(legacy::open::show_in_finder_cmd),
        )
        .route("/absorb", but_post(legacy::absorb::absorb_cmd))
        .route(
            "/absorption_plan",
            but_post(legacy::absorb::absorption_plan_cmd),
        )
        .route(
            "/claude_get_user_message",
            but_post(legacy::claude::claude_get_user_message_cmd),
        )
        .route(
            "/claude_list_permission_requests",
            but_post(legacy::claude::claude_list_permission_requests_cmd),
        )
        .route(
            "/claude_update_permission_request",
            but_post(legacy::claude::claude_update_permission_request_cmd),
        )
        .route(
            "/claude_answer_ask_user_question",
            but_post(legacy::claude::claude_answer_ask_user_question_cmd),
        )
        .route(
            "/claude_list_prompt_templates",
            but_post(legacy::claude::claude_list_prompt_templates_cmd),
        )
        .route(
            "/claude_get_prompt_dirs",
            but_post(legacy::claude::claude_get_prompt_dirs_cmd),
        )
        .route(
            "/claude_maybe_create_prompt_dir",
            but_post(legacy::claude::claude_maybe_create_prompt_dir_cmd),
        )
        .route(
            "/commit_reword",
            but_post(commit::reword::commit_reword_cmd),
        )
        .route(
            "/commit_create",
            but_post(commit::create::commit_create_cmd),
        )
        .route("/commit_amend", but_post(commit::amend::commit_amend_cmd))
        .route(
            "/commit_move_changes_between",
            but_post(commit::move_changes::commit_move_changes_between_cmd),
        )
        .route(
            "/commit_uncommit_changes",
            but_post(commit::uncommit::commit_uncommit_changes_cmd),
        )
        .route("/build_type", but_post(platform::build_type_cmd));

    // IRC commands — only registered when the `irc` feature is enabled.
    #[cfg(feature = "irc")]
    let app = app
        .route("/irc_connect", post(irc::irc_connect))
        .route("/irc_disconnect", post(irc::irc_disconnect))
        .route("/irc_state", post(irc::irc_state))
        .route("/irc_wait_ready", post(irc::irc_wait_ready))
        .route("/irc_join", post(irc::irc_join))
        .route("/irc_part", post(irc::irc_part))
        .route("/irc_auto_join", post(irc::irc_auto_join))
        .route("/irc_auto_leave", post(irc::irc_auto_leave))
        .route("/irc_send_message", post(irc::irc_send_message))
        .route(
            "/irc_send_message_with_data",
            post(irc::irc_send_message_with_data),
        )
        .route("/irc_request_history", post(irc::irc_request_history))
        .route(
            "/irc_request_history_before",
            post(irc::irc_request_history_before),
        )
        .route("/irc_send_raw", post(irc::irc_send_raw))
        .route("/irc_send_typing", post(irc::irc_send_typing))
        .route("/irc_send_reaction", post(irc::irc_send_reaction))
        .route("/irc_remove_reaction", post(irc::irc_remove_reaction))
        .route("/irc_redact_message", post(irc::irc_redact_message))
        .route("/irc_list_connections", post(irc::irc_list_connections))
        .route("/irc_exists", post(irc::irc_exists))
        .route("/irc_nick", post(irc::irc_nick))
        .route("/irc_messages", post(irc::irc_messages))
        .route("/irc_channels", post(irc::irc_channels))
        .route("/irc_users", post(irc::irc_users))
        .route("/irc_mark_read", post(irc::irc_mark_read))
        .route("/irc_clear_messages", post(irc::irc_clear_messages))
        .route(
            "/irc_get_all_commit_reactions",
            post(irc::irc_get_all_commit_reactions),
        )
        .route(
            "/irc_get_all_message_reactions",
            post(irc::irc_get_all_message_reactions),
        )
        .route(
            "/irc_get_file_message_reactions",
            post(irc::irc_get_file_message_reactions),
        )
        .route("/irc_get_working_files", post(irc::irc_get_working_files))
        .route(
            "/irc_start_working_files_broadcast",
            post(irc::irc_start_working_files_broadcast),
        )
        .route(
            "/irc_stop_working_files_broadcast",
            post(irc::irc_stop_working_files_broadcast),
        );

    // Auth routes (only functional when remote access is enabled).
    // When the frontend is embedded, GET / is handled by the frontend fallback
    // instead of the plain-HTML root handler.
    let auth_state_for_routes = auth_state.clone();
    let app = app
        .route(
            "/auth/login",
            get(auth::login).with_state(auth_state_for_routes.clone()),
        )
        .route(
            "/auth/callback",
            post(auth::callback).with_state(auth_state_for_routes.clone()),
        )
        .route("/auth/logout", get(auth::logout));

    // Catch-all for commands that need special handling (app, extra, app_settings_sync)
    let app = app
        .route("/{command}", post(post_handle_command_with_path))
        .route(
            "/ws",
            any(move |headers, ws| handle_ws_request(headers, ws, broadcaster)),
        )
        // Spawning in a separate thread to prevent abort if the client
        // disconnects. We need this to ensure locks are removed after
        // the claude processes finishes.
        .route_layer(middleware::from_fn(
            |req: axum::extract::Request<Body>, next: Next| async move {
                tokio::task::spawn(next.run(req)).await.unwrap()
            },
        ))
        .layer(
            ServiceBuilder::new()
                // Middleware to ensure only localhost connections are accepted.
                .layer(middleware::from_fn(localhost_only_middleware))
                // Auth middleware — validates remote tokens when a remote origin is configured.
                .layer(middleware::from_fn_with_state(
                    auth_state,
                    auth::auth_middleware,
                ))
                .layer(cors),
        )
        .with_state(state);

    // Optionally nest all API routes under a configurable base path.
    // e.g. --base-path=/api makes all endpoints available at /api/...
    // The embedded frontend fallback is attached to the outermost router so
    // that it handles / regardless of where the API lives.
    let app: Router = if api_base.is_empty() {
        app
    } else {
        Router::new().nest(&api_base, app)
    };

    // When the frontend is embedded, serve static files as a fallback for
    // all routes not handled by the API. This makes but-server self-contained
    // with no need for a separate frontend dev server or Caddy.
    // The api_url is injected into index.html via a <meta> tag so the frontend
    // can use the correct API URL at runtime.
    #[cfg(feature = "embedded-frontend")]
    let app = {
        let api_url_for_frontend = api_url.clone();
        app.fallback(move |uri| frontend::serve(uri, api_url_for_frontend.clone()))
    };

    // Collect SHA-256 hashes of every inline <script> in the embedded frontend
    // plus the auth login page script. These replace 'unsafe-inline' in the CSP.
    let script_hashes = {
        let login_hash = sha256_csp_hash(auth::login_page_script(&api_base).as_bytes());
        #[cfg(not(feature = "embedded-frontend"))]
        let hashes = vec![login_hash];
        #[cfg(feature = "embedded-frontend")]
        let hashes = std::iter::once(login_hash)
            .chain(frontend::inline_script_hashes())
            .collect::<Vec<_>>();
        hashes
    };

    // Add Content-Security-Policy header to all responses.
    // Adapted from crates/gitbutler-tauri/tauri.conf.json — Tauri-specific
    // schemes (tauri://, asset:, ipc:) are dropped; connect-src includes the
    // remote origin's WebSocket when remote access is configured.
    let csp = build_csp(allowed_remote_origin(), &script_hashes);
    let csp_value = axum::http::HeaderValue::from_str(&csp).expect("CSP is valid header value");
    let app = app
        .layer(axum::middleware::from_fn(
            move |req: axum::extract::Request<Body>, next: Next| {
                let csp_value = csp_value.clone();
                async move {
                    let mut response = next.run(req).await;
                    response
                        .headers_mut()
                        .insert(axum::http::header::CONTENT_SECURITY_POLICY, csp_value);
                    response
                }
            },
        ))
        .layer(CompressionLayer::new());

    // When remote access is configured, default to binding on all interfaces
    // so tunnel/reverse-proxy traffic can reach us.
    let default_host = if allowed_remote_origin().is_some() {
        "0.0.0.0"
    } else {
        "127.0.0.1"
    };
    let host_env = std::env::var("BUTLER_HOST").ok();
    let host = config
        .host
        .as_deref()
        .or(host_env.as_deref())
        .unwrap_or(default_host);
    let url = format!("{host}:{port}");
    let listener = match tokio::net::TcpListener::bind(&url).await {
        Ok(listener) => listener,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                tracing::error!(
                    "Failed to bind to {url}: {e}. Another instance of but-server may already be running on port {port}."
                );
            } else {
                tracing::error!("Failed to bind to {url}: {e}");
            }
            anyhow::bail!("Failed to bind to {url}: {e}");
        }
    };
    if !config.tunnel && config.tunnel_name.is_none() {
        println!(
            "{} {}",
            "Local:".bold(),
            format!("http://localhost:{port}").cyan().underline()
        );
    }
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );

    tokio::select! {
        result = server => { result.unwrap(); }
        _ = tokio::signal::ctrl_c() => {
            #[cfg(feature = "irc")]
            {
                tracing::info!("Shutdown signal received, closing IRC connections…");
                irc_manager_for_shutdown.shutdown().await;
            }
            // Kill the cloudflared tunnel process if one was started.
            if let Some(mut child) = _tunnel_child {
                let _ = child.kill().await;
            }
            // The settings file watcher (spawn_blocking with infinite loop) and
            // other background tasks prevent the tokio runtime from exiting
            // cleanly. It's safe to terminate immediately.
            std::process::exit(0);
        }
    }
    Ok(())
}

/// Compute a `'sha256-<base64>'` CSP hash for a script body.
pub(crate) fn sha256_csp_hash(data: &[u8]) -> String {
    use base64::Engine as _;
    use sha2::Digest as _;
    let hash = sha2::Sha256::digest(data);
    format!(
        "'sha256-{}'",
        base64::engine::general_purpose::STANDARD.encode(hash)
    )
}

/// Build a Content-Security-Policy header value.
///
/// Mirrors `crates/gitbutler-tauri/tauri.conf.json` with Tauri-specific
/// schemes removed. When a remote origin is configured its WebSocket form is
/// added to `connect-src` so the embedded frontend can open the `/ws` stream.
fn build_csp(remote_origin: Option<&str>, script_hashes: &[String]) -> String {
    // Derive the WebSocket form of the remote origin if present
    // (https://foo.com → wss://foo.com).
    let extra_connect: String = match remote_origin {
        Some(origin) => {
            let ws = origin
                .replacen("https://", "wss://", 1)
                .replacen("http://", "ws://", 1);
            format!(" {ws}")
        }
        None => String::new(),
    };

    [
        "default-src 'self'",
        "img-src 'self' data: blob: \
             https://avatars.githubusercontent.com \
             https://*.gitbutler.com \
             https://gitbutler-public.s3.amazonaws.com \
             https://*.gravatar.com \
             https://io.wp.com https://i0.wp.com https://i1.wp.com \
             https://i2.wp.com https://i3.wp.com \
             https://github.com \
             https://*.googleusercontent.com \
             https://*.giphy.com/",
        &format!(
            "connect-src 'self'{extra_connect} \
             https://eu.posthog.com https://eu.i.posthog.com \
             https://eu-assets.i.posthog.com \
             https://app.gitbutler.com \
             https://app.staging.gitbutler.com \
             https://o4504644069687296.ingest.sentry.io \
             https://github.com https://api.github.com \
             https://api.openai.com https://api.anthropic.com \
             https://*.gitlab.com https://gitlab.com \
             wss://irc.gitbutler.com:8097 data:"
        ),
        &format!(
            "script-src 'self' 'wasm-unsafe-eval' {} \
             https://eu.posthog.com https://eu.i.posthog.com \
             https://eu-assets.i.posthog.com",
            script_hashes.join(" ")
        ),
        "style-src 'self' 'unsafe-inline'",
    ]
    .join("; ")
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
    #[cfg(feature = "irc")]
    let working_files_broadcast = state.working_files_broadcast;
    let req = Request { command, params };
    #[cfg(feature = "irc")]
    let res = handle_command(req, app, extra, app_settings_sync, working_files_broadcast).await;
    #[cfg(not(feature = "irc"))]
    let res = handle_command(req, app, extra, app_settings_sync).await;
    match res {
        Ok(value) => Json(json!(Response::Success(value))),
        Err(e) => {
            let e = json::Error::from(e);
            Json(json!(Response::Error(json!(e))))
        }
    }
}

async fn handle_ws_request(
    headers: axum::http::HeaderMap,
    ws: WebSocketUpgrade,
    broadcaster: Arc<Mutex<Broadcaster>>,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate the Origin header to prevent cross-site WebSocket hijacking.
    // CORS headers don't protect WebSocket upgrades, so we must check manually.
    let origin = headers
        .get(axum::http::header::ORIGIN)
        .ok_or(StatusCode::FORBIDDEN)?;
    if !is_localhost_origin(origin.as_bytes()) && !is_allowed_remote_origin(origin.as_bytes()) {
        tracing::warn!("Rejected WebSocket connection from origin: {origin:?}");
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(ws.on_upgrade(move |socket| handle_websocket(socket, broadcaster)))
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
    #[cfg(feature = "irc")] working_files_broadcast: WorkingFilesBroadcast,
    // TODO: make this anyhow::Result<serde_json::Value>
) -> anyhow::Result<serde_json::Value> {
    let command: &str = &request.command;
    match command {
        // App settings (need app_settings_sync)
        "get_app_settings" => Ok(to_json_or_panic(app_settings_sync.get()?.clone())),
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
        "update_irc" => deserialize_json(request.params).and_then(|params| {
            legacy::settings::update_irc(&app_settings_sync, params).map(|r| json!(r))
        }),
        // Project management (need extra or app)
        "server_info" => Ok(json!({
            "projectPinned": extra.pinned_project.is_some(),
            "pinnedProjectId": extra.pinned_project.as_ref().map(|p| p.to_string()),
        })),
        "list_projects" => projects::list_projects(&extra).await,
        "set_project_active" => {
            #[cfg(feature = "irc")]
            {
                return projects::set_project_active(
                    &app,
                    &extra,
                    app_settings_sync,
                    working_files_broadcast,
                    request.params,
                )
                .await;
            }
            #[cfg(not(feature = "irc"))]
            projects::set_project_active(&app, &extra, app_settings_sync, request.params).await
        }
        // Async virtual branches commands (not yet migrated due to different pattern)
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
        "set_review_auto_merge" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::set_review_auto_merge_cmd(params).await;
                    result.map(|_| json!({"result": "success"}))
                }
                Err(e) => Err(e),
            }
        }
        "set_review_draftiness" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::set_review_draftiness_cmd(params).await;
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
                pub project_id: ProjectHandleOrLegacyProjectId,
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
                project_id: ProjectHandleOrLegacyProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let ctx: Context = params.project_id.try_into()?;
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
                Ok(ClaudeGetSessionDetailsParams {
                    project_id,
                    session_id,
                }) => {
                    let ctx: Context = project_id.try_into()?;
                    let result =
                        legacy::claude::claude_get_session_details(ctx.into_sync(), session_id)
                            .await;
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
                project_id: ProjectHandleOrLegacyProjectId,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let ctx: Context = params.project_id.try_into()?;
            let result = legacy::claude::claude_get_sub_agents(ctx.into_sync()).await;
            result.map(|r| json!(r))
        }
        "claude_verify_path" => {
            #[derive(Debug, Deserialize)]
            #[serde(rename_all = "camelCase")]
            pub struct Params {
                pub project_id: ProjectHandleOrLegacyProjectId,
                pub path: String,
            }
            let params = serde_json::from_value::<Params>(request.params)?;
            let ctx: Context = params.project_id.try_into()?;
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
