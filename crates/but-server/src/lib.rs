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
    routing::{MethodRouter, any, post},
};
use but_api::{commit, diff, github, gitlab, json, legacy, open, platform, workspace};
use but_ctx::ProjectHandleOrLegacyProjectId;

mod broadcaster;
use broadcaster::Broadcaster;
#[cfg(feature = "irc")]
use but_irc::IrcManager;
use but_settings::AppSettingsWithDiskSync;
use futures_util::{SinkExt, StreamExt as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;
use tower::ServiceBuilder;
use tower_http::cors::{self, CorsLayer};

#[cfg(feature = "irc")]
mod irc;
#[cfg(feature = "irc")]
mod irc_lifecycle;
mod projects;
use crate::projects::ActiveProjects;

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
}

#[derive(Clone)]
struct AppState {
    broadcaster: Arc<Mutex<Broadcaster>>,
    extra: Extra,
    app_settings: AppSettingsWithDiskSync,
    #[cfg(feature = "irc")]
    irc_manager: IrcManager,
    #[cfg(feature = "irc")]
    working_files_broadcast: WorkingFilesBroadcast,
}

/// Converts a synchronous command handler into an axum `MethodRouter` that works with
/// `Router::route`.
fn but_post<F, S>(f: F) -> MethodRouter<S, Infallible>
where
    F: Fn(serde_json::Value) -> anyhow::Result<serde_json::Value> + Copy + Send + Sync + 'static,
    S: Clone + Send + Sync + 'static,
{
    post(move |Json(params)| async move {
        let res = tokio::task::spawn_blocking(move || f(params))
            .await
            .unwrap_or_else(|e| Err(anyhow::anyhow!("handler task panicked: {e}")));
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

fn server_capabilities(_params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    Ok(serde_json::to_value(
        but_api::platform::ServerCapabilities {
            is_remote: false,
            can_add_projects: true,
        },
    )?)
}

/// Opens a native directory picker on the machine running but-server.
///
/// `rfd` cannot be used here because but-server is a headless process without
/// an NSApplication run loop, so on macOS it would panic trying to show a
/// dialog off the main thread. Instead we shell out to `osascript` (macOS) or
/// `zenity`/`kdialog` (Linux) which work from any thread and any process.
async fn pick_directory(_params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    let path = tokio::task::spawn_blocking(native_pick_directory).await??;
    match path {
        Some(p) => Ok(json!({ "path": p })),
        None => Ok(json!({ "path": null })),
    }
}

/// Shell out to a platform-native directory picker.
fn native_pick_directory() -> anyhow::Result<Option<String>> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("osascript")
            .arg("-e")
            .arg(
                r#"set theFolder to choose folder with prompt "Select a Git repository"
return POSIX path of theFolder"#,
            )
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            // osascript exits with code 1 and "User canceled" on cancel
            if stderr.contains("User canceled") || stderr.contains("(-128)") {
                return Ok(None);
            }
            anyhow::bail!(
                "osascript directory picker failed (exit {:?}): {}",
                output.status.code(),
                if stderr.is_empty() {
                    "unknown error"
                } else {
                    &stderr
                }
            );
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            return Ok(None);
        }
        // osascript returns paths with a trailing slash — strip it
        Ok(Some(path.trim_end_matches('/').to_string()))
    }

    #[cfg(target_os = "linux")]
    {
        // Try zenity first, fall back to kdialog
        let output = std::process::Command::new("zenity")
            .args([
                "--file-selection",
                "--directory",
                "--title=Select a Git repository",
            ])
            .output()
            .or_else(|_| {
                std::process::Command::new("kdialog")
                    .args([
                        "--getexistingdirectory",
                        ".",
                        "--title",
                        "Select a Git repository",
                    ])
                    .output()
            })?;
        if !output.status.success() {
            // zenity exits 1 on cancel, kdialog exits 1 on cancel
            let code = output.status.code().unwrap_or(-1);
            if code == 1 {
                return Ok(None);
            }
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "directory picker failed (exit {code}): {}",
                if stderr.is_empty() {
                    "unknown error"
                } else {
                    &stderr
                }
            );
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            return Ok(None);
        }
        Ok(Some(path))
    }

    #[cfg(target_os = "windows")]
    {
        // Use the modern IFileOpenDialog via PowerShell (STA is required for
        // any Windows Forms/COM dialog). FolderBrowserDialog is directory-only.
        // The script outputs the selected path on OK, or empty string on cancel.
        // A non-zero exit means PowerShell itself failed (e.g. Add-Type error).
        let output = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-STA",
                "-Command",
                r#"Add-Type -AssemblyName System.Windows.Forms; $f = New-Object System.Windows.Forms.FolderBrowserDialog; $f.Description = 'Select a Git repository'; $f.UseDescriptionForTitle = $true; if ($f.ShowDialog() -eq 'OK') { $f.SelectedPath } else { '' }"#,
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            anyhow::bail!(
                "PowerShell directory picker failed (exit {:?}): {}",
                output.status.code(),
                if stderr.is_empty() {
                    "unknown error"
                } else {
                    &stderr
                }
            );
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if path.is_empty() {
            return Ok(None);
        }
        Ok(Some(path))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        anyhow::bail!("Native file picker is not supported on this platform")
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

/// Check if an origin byte string is from localhost.
///
/// Matches `http(s)://localhost`, `http(s)://127.0.0.1`, and
/// `http(s)://[::1]`, each optionally followed by `:<port>`.
pub(crate) fn is_localhost_origin(origin: &[u8]) -> bool {
    for prefix in [
        b"http://localhost".as_slice(),
        b"https://localhost".as_slice(),
        b"http://127.0.0.1".as_slice(),
        b"https://127.0.0.1".as_slice(),
        b"http://[::1]".as_slice(),
        b"https://[::1]".as_slice(),
    ] {
        if let Some(rest) = origin.strip_prefix(prefix)
            && rest.first().is_none_or(|b| *b == b':')
        {
            return true;
        }
    }
    false
}

/// Check if a `Host` header value is a localhost address.
///
/// Matches `localhost`, `127.0.0.1`, and `[::1]`, each optionally followed
/// by `:<port>`. Used to defend against DNS rebinding: browsers always set
/// `Host` to the *domain name* being requested (not the resolved IP), so a
/// rebinding attack using `evil.com` will have `Host: evil.com:PORT` and be
/// rejected here.
fn is_localhost_host(host: &[u8]) -> bool {
    for prefix in [
        b"localhost".as_slice(),
        b"127.0.0.1".as_slice(),
        b"[::1]".as_slice(),
    ] {
        if let Some(rest) = host.strip_prefix(prefix)
            && rest.first().is_none_or(|b| *b == b':')
        {
            return true;
        }
    }
    false
}

/// Configuration for the but-server, populated from CLI args.
#[derive(Debug, Default)]
pub struct Config {
    /// Port to listen on. `but-server` defaults to 6978.
    pub port: Option<u16>,
    /// Address to bind to. Defaults to 127.0.0.1. Override with --bind-addr if needed
    /// (e.g. 0.0.0.0 in a container environment).
    pub bind_addr: Option<String>,
    /// Prefix all API routes with this path (e.g. `/api`).
    pub base_path: Option<String>,
    /// If set, auto-activate this directory's project on startup.
    pub project_path: Option<std::path::PathBuf>,
}

/// Middleware to ensure all connections are from localhost only.
///
/// Rejects non-loopback peer addresses and `Host` headers that don't match
/// `localhost`/`127.0.0.1`/`[::1]`. The Host check defends against DNS
/// rebinding: an attacker's domain pointed at 127.0.0.1 connects from
/// loopback (so the IP check passes) but the browser sets Host to the
/// attacker's domain, which fails here.
async fn localhost_only_middleware(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    req: axum::extract::Request<Body>,
    next: Next,
) -> Result<impl IntoResponse, StatusCode> {
    if let Some(origin) = req.headers().get(axum::http::header::ORIGIN)
        && !is_localhost_origin(origin.as_bytes())
    {
        tracing::warn!(
            "Rejected request with disallowed Origin: {}",
            String::from_utf8_lossy(origin.as_bytes())
        );
        return Err(StatusCode::FORBIDDEN);
    }
    if !addr.ip().is_loopback() {
        tracing::warn!("Rejected non-localhost connection from: {}", addr);
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some(host) = req.headers().get(axum::http::header::HOST)
        && !is_localhost_host(host.as_bytes())
    {
        tracing::warn!(
            "Rejected DNS-rebinding attempt: Host header was {}",
            String::from_utf8_lossy(host.as_bytes())
        );
        return Err(StatusCode::FORBIDDEN);
    }
    Ok(next.run(req).await)
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

    let port: u16 = config
        .port
        .or_else(|| std::env::var("BUTLER_PORT").ok()?.parse().ok())
        .unwrap_or(6978);

    // CORS wildcards are forbidden when credentials are allowed, so always list explicitly.
    // `baggage` and `sentry-trace` are injected by Sentry's performance monitoring into
    // outgoing fetch requests; without them the browser blocks the preflight.
    let allowed_headers: cors::AllowHeaders = vec![
        axum::http::header::CONTENT_TYPE,
        axum::http::header::AUTHORIZATION,
        axum::http::HeaderName::from_static("baggage"),
        axum::http::HeaderName::from_static("sentry-trace"),
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
            is_localhost_origin(origin.as_bytes())
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
    let extra = Extra {
        active_projects: Arc::new(Mutex::new(ActiveProjects::new())),
        archival,
    };
    #[cfg_attr(not(feature = "irc"), allow(unused_mut))]
    let mut app_settings =
        AppSettingsWithDiskSync::new_with_customization(config_dir.clone(), None)
            .expect("failed to create app settings");

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

    // If a project path was provided, auto-activate that project.
    if let Some(ref project_path) = config.project_path {
        match but_ctx::Context::discover(project_path) {
            Ok(mut ctx) => {
                but_api::legacy::projects::prepare_project_for_activation(&mut ctx).ok();
                let mut active = extra.active_projects.lock().await;
                if active
                    .set_active(
                        &ctx,
                        &broadcaster,
                        app_settings.clone(),
                        #[cfg(feature = "irc")]
                        working_files_broadcast.clone(),
                    )
                    .is_err()
                {
                    tracing::warn!("Failed to activate project at {}", project_path.display());
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

    let mut api_base = config
        .base_path
        .as_deref()
        .unwrap_or("")
        .trim_end_matches('/')
        .to_string();
    // Ensure the base path starts with '/' when non-empty so Router::nest doesn't panic.
    if !api_base.is_empty() && !api_base.starts_with('/') {
        api_base.insert(0, '/');
    }

    let state = AppState {
        broadcaster: broadcaster.clone(),
        extra,
        app_settings,
        #[cfg(feature = "irc")]
        irc_manager,
        #[cfg(feature = "irc")]
        working_files_broadcast,
    };

    let app = Router::new()
        .route("/server_capabilities", but_post(server_capabilities))
        .route("/pick_directory", but_post_async(pick_directory))
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
        .route("/get_workspace", but_post(workspace::get_workspace_cmd))
        .route(
            "/commit_details_with_line_stats",
            but_post(diff::commit_details_with_line_stats_cmd),
        )
        .route("/branch_diff", but_post(but_api::branch::branch_diff_cmd))
        .route("/move_branch", but_post(but_api::branch::move_branch_cmd))
        .route("/apply", but_post(but_api::branch::apply_cmd))
        .route("/review_apply", but_post(legacy::forge::review_apply_cmd))
        .route(
            "/get_initial_branch_integration",
            but_post(but_api::branch::get_initial_branch_integration_cmd),
        )
        .route(
            "/apply_branch_integration",
            but_post(but_api::branch::apply_branch_integration_cmd),
        )
        .route(
            "/tear_off_branch",
            but_post(but_api::branch::tear_off_branch_cmd),
        )
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
            "/discard_worktree_changes",
            but_post(legacy::workspace::discard_worktree_changes_cmd),
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
            "/workspace_branch_and_ancestors_push",
            but_post(legacy::workspace::workspace_branch_and_ancestors_push_cmd),
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
            "/get_login_token",
            but_post(legacy::users::get_login_token_cmd),
        )
        .route(
            "/login_with_token",
            but_post(legacy::users::login_with_token_cmd),
        )
        .route(
            "/get_user_profile",
            but_post(legacy::users::get_user_profile_cmd),
        )
        .route(
            "/update_user_profile",
            but_post(legacy::users::update_user_profile_cmd),
        )
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
            "/fetch_from_remotes",
            but_post(legacy::virtual_branches::fetch_from_remotes_cmd),
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
        .route("/open_url", but_post(open::open_url_cmd))
        .route("/open_in_terminal", but_post(open::open_in_terminal_cmd))
        .route("/show_in_finder", but_post(open::show_in_finder_cmd))
        .route(
            "/get_terminal_options_for_platform",
            but_post(open::terminal::get_terminal_options_for_platform_cmd),
        )
        .route(
            "/get_recommended_terminal_for_platform",
            but_post(open::terminal::get_recommended_terminal_for_platform_cmd),
        )
        .route("/absorb", but_post(legacy::absorb::absorb_cmd))
        .route(
            "/absorption_plan",
            but_post(legacy::absorb::absorption_plan_cmd),
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
            "/commit_move",
            but_post(commit::move_commit::commit_move_cmd),
        )
        .route(
            "/commit_move_changes_between",
            but_post(commit::move_changes::commit_move_changes_between_cmd),
        )
        .route(
            "/commit_squash",
            but_post(commit::squash::commit_squash_cmd),
        )
        .route(
            "/commit_uncommit_changes",
            but_post(commit::uncommit::commit_uncommit_changes_cmd),
        )
        .route(
            "/commit_uncommit",
            but_post(commit::uncommit::commit_uncommit_cmd),
        )
        .route(
            "/workspace_integrate_upstream",
            but_post(workspace::workspace_integrate_upstream_cmd),
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

    // Catch-all for commands that need special handling (app, extra, app_settings_sync)
    let app = app
        .route("/{command}", post(post_handle_command_with_path))
        .route(
            "/ws",
            any(move |headers, ws| handle_ws_request(headers, ws, broadcaster)),
        )
        // Spawning in a separate thread to prevent abort if the client
        // disconnects.
        .route_layer(middleware::from_fn(
            |req: axum::extract::Request<Body>, next: Next| async move {
                tokio::task::spawn(next.run(req)).await.unwrap()
            },
        ))
        .with_state(state);

    // Optionally nest all API routes under a configurable base path.
    // e.g. --base-path=/api makes all endpoints available at /api/...
    let app: Router = if api_base.is_empty() {
        app
    } else {
        Router::new().nest(&api_base, app)
    };

    let app = app
        .layer(ServiceBuilder::new().layer(middleware::from_fn(localhost_only_middleware)))
        .layer(cors);

    let default_host = "127.0.0.1";
    let host_env = std::env::var("BUTLER_HOST").ok();
    let host = config
        .bind_addr
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
    println!(
        "{} {}",
        "Local:".bold(),
        format!("http://localhost:{port}").cyan().underline()
    );
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
            // The settings file watcher (spawn_blocking with infinite loop) and
            // other background tasks prevent the tokio runtime from exiting
            // cleanly. It's safe to terminate immediately.
            std::process::exit(0);
        }
    }
    Ok(())
}

/// Handler that extracts the command from the URL path.
/// This allows calling `POST /command_name` with params as the JSON body.
async fn post_handle_command_with_path(
    State(state): State<AppState>,
    Path(command): Path<String>,
    Json(params): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let broadcaster = state.broadcaster;
    let extra = state.extra;
    let app_settings_sync = state.app_settings;
    #[cfg(feature = "irc")]
    let working_files_broadcast = state.working_files_broadcast;
    let req = Request { command, params };
    #[cfg(feature = "irc")]
    let res = handle_command(
        req,
        broadcaster,
        extra,
        app_settings_sync,
        working_files_broadcast,
    )
    .await;
    #[cfg(not(feature = "irc"))]
    let res = handle_command(req, broadcaster, extra, app_settings_sync).await;
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
    if !is_localhost_origin(origin.as_bytes()) {
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
    broadcaster: Arc<Mutex<Broadcaster>>,
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
        "list_projects" => projects::list_projects(&extra).await,
        "set_project_active" => {
            #[cfg(feature = "irc")]
            {
                return projects::set_project_active(
                    &broadcaster,
                    &extra,
                    app_settings_sync,
                    working_files_broadcast,
                    request.params,
                )
                .await;
            }
            #[cfg(not(feature = "irc"))]
            projects::set_project_active(&broadcaster, &extra, app_settings_sync, request.params)
                .await
        }
        // Async virtual branches commands (not yet migrated due to different pattern)
        "upstream_integration_statuses" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result =
                        legacy::virtual_branches::upstream_integration_statuses_cmd(params);
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
        "get_review" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::get_review_cmd(params);
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "get_review_merge_status" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::get_review_merge_status_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "get_review_base_repo_url" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::get_review_base_repo_url_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "update_review" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::update_review_cmd(params).await;
                    result.map(|_| json!({"result": "success"}))
                }
                Err(e) => Err(e),
            }
        }
        "get_repo_info" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::get_repo_info_cmd(params).await;
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "forge_info" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::forge_info_cmd(params);
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "forge_compare_branch_url" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::forge_compare_branch_url_cmd(params);
                    result.map(|r| json!(r))
                }
                Err(e) => Err(e),
            }
        }
        "list_ci_checks" => {
            let params = deserialize_json(request.params);
            match params {
                Ok(params) => {
                    let result = legacy::forge::list_ci_checks_cmd(params);
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
    fn localhost_origin_accepts_valid() {
        // Basic schemes
        assert!(is_localhost_origin(b"http://localhost"));
        assert!(is_localhost_origin(b"https://localhost"));
        assert!(is_localhost_origin(b"http://127.0.0.1"));
        assert!(is_localhost_origin(b"https://127.0.0.1"));
        assert!(is_localhost_origin(b"http://[::1]"));
        assert!(is_localhost_origin(b"https://[::1]"));

        // With port
        assert!(is_localhost_origin(b"http://localhost:3000"));
        assert!(is_localhost_origin(b"https://127.0.0.1:8080"));
        assert!(is_localhost_origin(b"http://[::1]:6978"));
    }

    #[test]
    fn localhost_origin_rejects_invalid() {
        // Missing scheme
        assert!(!is_localhost_origin(b"localhost"));
        assert!(!is_localhost_origin(b"localhost:3000"));

        // DNS rebinding — hostname that starts with "localhost" but isn't
        assert!(!is_localhost_origin(b"http://localhost.evil.com"));
        assert!(!is_localhost_origin(b"http://localhost.evil.com:8080"));

        // IP prefix attacks
        assert!(!is_localhost_origin(b"http://127.0.0.10"));
        assert!(!is_localhost_origin(b"http://127.0.0.1.evil.com"));

        // Other hosts
        assert!(!is_localhost_origin(b"http://evil.com"));
        assert!(!is_localhost_origin(b"https://192.168.1.1"));

        // Empty
        assert!(!is_localhost_origin(b""));
    }

    #[test]
    fn localhost_host_accepts_valid() {
        // Bare hostnames
        assert!(is_localhost_host(b"localhost"));
        assert!(is_localhost_host(b"127.0.0.1"));
        assert!(is_localhost_host(b"[::1]"));

        // With port
        assert!(is_localhost_host(b"localhost:8080"));
        assert!(is_localhost_host(b"127.0.0.1:3000"));
        assert!(is_localhost_host(b"[::1]:6978"));
    }

    #[test]
    fn localhost_host_rejects_invalid() {
        // DNS rebinding — must not match hostnames that merely start with "localhost"
        assert!(!is_localhost_host(b"localhost.evil.com"));
        assert!(!is_localhost_host(b"localhost.evil.com:8080"));

        // IP prefix attacks
        assert!(!is_localhost_host(b"127.0.0.10"));
        assert!(!is_localhost_host(b"127.0.0.1.evil.com"));

        // Other hosts
        assert!(!is_localhost_host(b"evil.com"));
        assert!(!is_localhost_host(b"192.168.1.1"));

        // Empty
        assert!(!is_localhost_host(b""));
    }
}
