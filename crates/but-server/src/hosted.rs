//! A hosted, read-only but-server: machines publish worktrees over git, and the web UI reads them.
//!
//! Every request is made as a GitButler user, and sees only what that user published. Layout
//! under the data directory, per user:
//! * `users/<id>/store/<project>.git` - one bare repository per project (its root commit). It
//!   is the project the API opens, and every published worktree is one of its linked worktrees.
//! * `users/<id>/worktrees/<project>/<worktree>/` - the empty directory git requires each
//!   linked worktree to have. Its uncommitted changes come from the published snapshot instead.
//!
//! Browsers follow publishes over the `/events` WebSocket, see [`events`].

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex, PoisonError},
};

use anyhow::{Context as _, bail};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{
        DefaultBodyLimit, Extension, Form, FromRef, Path as UrlPath, RawQuery, Request, State,
    },
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{any, get, post},
};
use but_core::diff::PUBLISHED_WORKTREE_REF;
use tokio::{io::AsyncWriteExt as _, sync::broadcast};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use crate::cmd_result_to_json;

mod events;
use events::{EVENTS_BUFFER, ProjectEvent};

/// Configuration for the hosted server.
#[derive(Debug)]
pub struct HostedConfig {
    /// Port to listen on.
    pub port: u16,
    /// Address to bind to.
    pub bind_addr: String,
    /// Where published projects are stored.
    pub data_dir: PathBuf,
    /// The built Lite web bundle to serve, if any.
    pub web_dir: Option<PathBuf>,
    /// The GitButler API that tells who a token belongs to, as the apps pick it: production for
    /// release and nightly builds, staging for dev builds.
    pub gitbutler_api: String,
}

/// The GitButler account a request is made as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct UserId(u64);

impl UserId {
    /// Everything this user published.
    fn dir(self, config: &HostedConfig) -> PathBuf {
        config.data_dir.join("users").join(self.0.to_string())
    }
}

/// Signed-in browsers, by the session ID in their cookie. In memory: a restart signs them out.
type Sessions = Arc<Mutex<std::collections::HashMap<String, UserId>>>;

const SESSION_COOKIE: &str = "but_hosted_session";

#[derive(Clone)]
struct HostedState {
    config: Arc<HostedConfig>,
    events: broadcast::Sender<ProjectEvent>,
    sessions: Sessions,
    /// Held while a store is created, and for a push and its registration: pushes share their
    /// target ref and the store's config, so concurrent ones would reject each other.
    writing: Arc<tokio::sync::Mutex<()>>,
    http: reqwest::Client,
}

impl FromRef<HostedState> for Arc<HostedConfig> {
    fn from_ref(state: &HostedState) -> Self {
        state.config.clone()
    }
}

impl FromRef<HostedState> for broadcast::Sender<ProjectEvent> {
    fn from_ref(state: &HostedState) -> Self {
        state.events.clone()
    }
}

/// Run the hosted server until interrupted.
pub async fn run(config: HostedConfig) -> anyhow::Result<()> {
    // Published worktrees are linked worktrees, which the API only shows with this flag on.
    // It is turned on in the server's own settings, never a developer's.
    if std::env::var_os("E2E_TEST_APP_DATA_DIR").is_none() {
        bail!("hosted mode keeps its settings apart: set E2E_TEST_APP_DATA_DIR");
    }
    but_settings::AppSettingsWithDiskSync::new_with_customization(
        but_path::app_config_dir()?,
        None,
    )?
    .update_feature_flags(but_settings::api::FeatureFlagsUpdate {
        worktree_manipulation: Some(true),
        ..Default::default()
    })?;

    let url = format!("{}:{}", config.bind_addr, config.port);
    let app = router(config);
    let listener = tokio::net::TcpListener::bind(&url)
        .await
        .with_context(|| format!("Failed to bind to {url}"))?;
    println!("Hosted: http://{url}");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Every hosted route, ready to serve; `config.port` and `config.bind_addr` are left to the caller.
pub fn router(config: HostedConfig) -> Router {
    let web_dir = config.web_dir.clone();
    let config = Arc::new(config);
    let state = HostedState {
        config: config.clone(),
        events: broadcast::channel(EVENTS_BUFFER).0,
        sessions: Sessions::default(),
        writing: Arc::default(),
        http: reqwest::Client::new(),
    };
    let api = Router::new()
        .route(
            "/git/{project}/{worktree}/{*rest}",
            any(git_http).layer(DefaultBodyLimit::disable()),
        )
        .route("/sdk/{endpoint}", post(sdk_endpoint))
        .route("/get_app_settings", post(app_settings))
        .route("/events", get(events::events))
        .route("/session", get(|| async { StatusCode::NO_CONTENT }))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_user))
        .route("/sign-in", get(sign_in_page).post(sign_in))
        .route("/sign-in/gitbutler", get(gitbutler_sign_in))
        .route("/sign-out", post(sign_out))
        .with_state(state);
    let cache = |value| {
        SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static(value),
        )
    };
    match web_dir {
        // The page is never stored, so it can't go stale; the assets it names are
        // content-hashed, so they never change.
        Some(dir) => api
            .nest_service(
                "/assets",
                tower::ServiceBuilder::new()
                    .layer(cache("public, max-age=31536000, immutable"))
                    .service(ServeDir::new(dir.join("assets"))),
            )
            .fallback_service(
                tower::ServiceBuilder::new()
                    .layer(cache("no-store"))
                    .service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html")))),
            ),
        None => api,
    }
}

/// Only a signed-in browser or a client with a GitButler access token gets through, as that user.
async fn require_user(
    State(state): State<HostedState>,
    mut request: Request,
    next: Next,
) -> Response {
    let user = match session_user(&state, request.headers()) {
        Some(user) => Some(user),
        None => match request
            .headers()
            .get("x-auth-token")
            .and_then(|v| v.to_str().ok())
        {
            Some(token) => whoami(&state, token)
                .await
                .inspect_err(|err| tracing::debug!("refused a token: {err:#}"))
                .ok(),
            None => None,
        },
    };
    let Some(user) = user else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    request.extensions_mut().insert(user);
    next.run(request).await
}

/// Who `token` belongs to, as GitButler says. Asked on every request: nothing about a token is
/// kept, so a revoked one stops working at once.
async fn whoami(state: &HostedState, token: &str) -> anyhow::Result<UserId> {
    #[derive(serde::Deserialize)]
    struct Whoami {
        id: u64,
    }
    let whoami: Whoami = state
        .http
        .get(format!("{}/api/login/whoami", state.config.gitbutler_api))
        .header("X-Auth-Token", token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(UserId(whoami.id))
}

fn session_id(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .find_map(|cookie| {
            cookie
                .trim()
                .strip_prefix(SESSION_COOKIE)?
                .strip_prefix('=')
        })
}

fn session_user(state: &HostedState, headers: &HeaderMap) -> Option<UserId> {
    let sessions = state
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    sessions.get(session_id(headers)?).copied()
}

/// Where a browser signs in, by pasting the access token GitButler shows after signing in there.
async fn sign_in_page() -> Html<&'static str> {
    Html(include_str!("hosted/sign-in.html"))
}

/// GitButler's own sign-in, started as the desktop app starts it. Without a client named, it ends
/// by showing the access token to copy.
async fn gitbutler_sign_in(State(state): State<HostedState>) -> Response {
    #[derive(serde::Deserialize)]
    struct LoginToken {
        url: String,
    }
    let login = async {
        state
            .http
            .post(format!(
                "{}/api/login/token.json",
                state.config.gitbutler_api
            ))
            .send()
            .await?
            .error_for_status()?
            .json::<LoginToken>()
            .await
    };
    match login.await {
        Ok(login) => Redirect::to(&login.url).into_response(),
        Err(err) => (
            StatusCode::BAD_GATEWAY,
            format!("GitButler didn't answer: {err}"),
        )
            .into_response(),
    }
}

#[derive(serde::Deserialize)]
struct SignIn {
    token: String,
}

async fn sign_in(
    State(state): State<HostedState>,
    headers: HeaderMap,
    Form(SignIn { token }): Form<SignIn>,
) -> Response {
    // Another site can't sign a browser in here as someone else.
    if !events::same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let Ok(user) = whoami(&state, token.trim()).await else {
        return Redirect::to("/sign-in?failed").into_response();
    };
    let session = uuid::Uuid::new_v4().to_string();
    state
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(session.clone(), user);
    // Not `Secure`, so it also works over plain HTTP on localhost; one of the design doc's shortcuts.
    let cookie = format!("{SESSION_COOKIE}={session}; Path=/; HttpOnly; SameSite=Strict");
    ([(header::SET_COOKIE, cookie)], Redirect::to("/")).into_response()
}

async fn sign_out(State(state): State<HostedState>, headers: HeaderMap) -> Response {
    if let Some(session) = session_id(&headers) {
        state
            .sessions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(session);
        // Sockets opened while signed in would otherwise go on hearing this user's events.
        state
            .events
            .send(ProjectEvent::SignedOut {
                session: session.to_owned(),
            })
            .ok();
    }
    let cookie = format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    ([(header::SET_COOKIE, cookie)], Redirect::to("/sign-in")).into_response()
}

/// The store of a project something was pushed to or fetched from, in a user's `dir`.
fn existing_store(dir: &Path, project: &str) -> anyhow::Result<PathBuf> {
    let store = store(dir, project)?;
    if !store.join("HEAD").exists() {
        bail!("not a published project: {project}");
    }
    Ok(store)
}

/// The project's store in a user's `dir`, named by its root commit.
fn store(dir: &Path, project: &str) -> anyhow::Result<PathBuf> {
    if !matches!(project.len(), 40 | 64) || !project.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("not a published project: {project}");
    }
    Ok(dir.join("store").join(format!("{project}.git")))
}

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(dir)
        .args(args)
        .output()?;
    if !out.status.success() {
        bail!("git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Git's smart HTTP protocol through `git http-backend`; a successful push publishes a worktree.
async fn git_http(
    State(HostedState {
        config,
        events,
        writing,
        ..
    }): State<HostedState>,
    Extension(user): Extension<UserId>,
    UrlPath((project, worktree, rest)): UrlPath<(String, String, String)>,
    method: Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = async {
        let dir = user.dir(&config);
        let store = store(&dir, &project)?;
        if !is_name(&worktree) {
            bail!("invalid worktree name: {worktree}");
        }
        let writing = writing.lock().await;
        if !store.join("HEAD").exists() {
            std::fs::create_dir_all(&store)?;
            git(&store, &["init", "--bare", "-q"])?;
            // Published worktrees have their branches checked out but no files to keep in step.
            git(&store, &["config", "receive.denyCurrentBranch", "ignore"])?;
        }
        let pushing = rest == "git-receive-pack";
        let _writing = pushing.then_some(writing);
        // A push git rejects, e.g. one planned against refs another push has since moved,
        // still ends well for `http-backend`; only a moved snapshot means a publish.
        let snapshot_ref = format!("refs/gitbutler/snapshots/{worktree}");
        let snapshot_before = git(&store, &["rev-parse", "-q", "--verify", &snapshot_ref]).ok();

        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        let mut child = tokio::process::Command::new("git")
            .arg("http-backend")
            .env("GIT_PROJECT_ROOT", dir.join("store"))
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("PATH_INFO", format!("/{project}.git/{rest}"))
            .env("REQUEST_METHOD", method.as_str())
            .env("QUERY_STRING", query.unwrap_or_default())
            .env("CONTENT_TYPE", header("content-type"))
            .env("HTTP_CONTENT_ENCODING", header("content-encoding"))
            .env("GIT_PROTOCOL", header("git-protocol"))
            .env("REMOTE_USER", "but")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().context("stdin")?;
        let writer = tokio::spawn(async move { stdin.write_all(&body).await });
        let out = child.wait_with_output().await?;
        writer.await??;
        if pushing
            && out.status.success()
            && git(&store, &["rev-parse", "-q", "--verify", &snapshot_ref]).ok() != snapshot_before
        {
            // The refs landed but can't be shown, so the push mustn't look like it worked.
            let tip = register_worktree(&dir, &project, &store, &worktree).with_context(|| {
                format!("the push landed, but {worktree} couldn't be published")
            })?;
            events::announce_publish(&events, user, &project, tip);
        }

        // CGI output: headers, a blank line, then the body.
        let split = out
            .stdout
            .windows(4)
            .position(|w| w == b"\r\n\r\n")
            .context("git http-backend sent no headers")?;
        let mut response = Response::builder();
        for line in String::from_utf8_lossy(&out.stdout[..split]).lines() {
            match line.split_once(": ") {
                Some(("Status", status)) => {
                    response = response.status(status[..3].parse::<u16>()?);
                }
                Some((name, value)) => response = response.header(name, value),
                None => {}
            }
        }
        Ok::<_, anyhow::Error>(response.body(Body::from(out.stdout[split + 4..].to_vec()))?)
    };
    result
        .await
        .unwrap_or_else(|err| (StatusCode::BAD_REQUEST, format!("{err:#}")).into_response())
}

/// What a worktree's snapshot commit says about it, as its message.
#[derive(serde::Deserialize)]
struct Snapshot {
    /// The format of this message; only 1 exists.
    version: u32,
    /// The project's name, e.g. its directory.
    title: String,
    /// The branch the worktree has checked out, e.g. `refs/heads/feature`.
    head: String,
    /// The remote-tracking branch its work is based on, e.g. `refs/remotes/origin/main`.
    target: Option<String>,
}

/// Make the pushed snapshot `worktree`, a linked worktree of the project, checked out on its
/// branch with the snapshot as its uncommitted changes. Returns the branch's tip.
fn register_worktree(
    dir: &Path,
    project: &str,
    store: &Path,
    worktree: &str,
) -> anyhow::Result<String> {
    // Kept as pushed too, so others can fetch the worktree.
    let pushed = format!("refs/gitbutler/snapshots/{worktree}");
    let snapshot: Snapshot =
        serde_json::from_str(&git(store, &["log", "-1", "--format=%B", &pushed])?)?;
    if snapshot.version != 1 {
        bail!("unsupported snapshot version {}", snapshot.version);
    }
    if !snapshot.head.starts_with("refs/heads/")
        || snapshot.head == "refs/heads/gitbutler/workspace"
    {
        bail!("only branches can be published: {}", snapshot.head);
    }

    let admin = store.join("worktrees").join(worktree);
    let checkout = dir.join("worktrees").join(project).join(worktree);
    std::fs::create_dir_all(&admin)?;
    std::fs::create_dir_all(&checkout)?;
    std::fs::write(admin.join("commondir"), "../..\n")?;
    // Locked: git would otherwise prune a worktree whose files it can't find.
    std::fs::write(admin.join("locked"), "published\n")?;
    // The snapshot first: a worktree without one would show its empty directory's files as deleted.
    let snapshot_ref = format!("worktrees/{worktree}/{PUBLISHED_WORKTREE_REF}");
    git(store, &["update-ref", &snapshot_ref, &pushed])?;
    std::fs::write(
        checkout.join(".git"),
        format!("gitdir: {}\n", admin.display()),
    )?;
    std::fs::write(
        admin.join("gitdir"),
        format!("{}\n", checkout.join(".git").display()),
    )?;
    std::fs::write(admin.join("HEAD"), format!("ref: {}\n", snapshot.head))?;

    // The project's name and target come from its first publish; later ones from branches
    // based elsewhere would otherwise move every branch's base.
    let first_publish = git(store, &["config", "gitbutler.title"]).is_err();
    if first_publish {
        git(store, &["config", "gitbutler.title", &snapshot.title])?;
    }
    if let Some(target) = snapshot.target.filter(|_| first_publish) {
        let remote = target
            .strip_prefix("refs/remotes/")
            .and_then(|rest| rest.split_once('/'))
            .map(|(remote, _)| remote)
            .filter(|remote| is_name(remote))
            .context("the target must be a remote-tracking branch")?;
        let fetch = format!("+refs/heads/*:refs/remotes/{remote}/*");
        git(
            store,
            &["config", &format!("remote.{remote}.fetch"), &fetch],
        )?;
        but_core::ref_metadata::ProjectMeta {
            target_ref: Some(target.as_str().try_into()?),
            target_commit_id: None,
            push_remote: Some(remote.to_owned()),
        }
        .persist(&gix::open(store)?)?;
        // The project's own HEAD sits on the target, so the published worktrees are its work.
        git(store, &["update-ref", "--no-deref", "HEAD", &target])?;
    }

    // Worktrees that exist when GitButler first looks are archived; a published one is shown.
    let mut ctx =
        but_ctx::Context::new_from_project_handle(but_ctx::ProjectHandle::from_path(store)?)?;
    but_api::worktrees::worktree_set_archived(&mut ctx, worktree.to_owned(), false)?;
    git(store, &["rev-parse", &snapshot.head])
}

/// The server's own settings, with worktrees turned on.
async fn app_settings() -> Json<serde_json::Value> {
    cmd_result_to_json((|| {
        let settings = but_settings::AppSettingsWithDiskSync::new_with_customization(
            but_path::app_config_dir()?,
            None,
        )?;
        Ok(serde_json::to_value(&*settings.get()?)?)
    })())
}

/// Every project published to a user's `dir`, as the project list Lite shows.
fn published_projects(dir: &Path) -> anyhow::Result<serde_json::Value> {
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(dir.join("store"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let store = entry.path();
        let id = entry
            .file_name()
            .to_string_lossy()
            .trim_end_matches(".git")
            .to_owned();
        // A store nothing was published to yet, e.g. created by a fetch, isn't a project.
        let Ok(title) = git(&store, &["config", "gitbutler.title"]) else {
            continue;
        };
        projects.push(serde_json::json!({ "id": id, "title": title, "path": id, "isOpen": false }));
    }
    Ok(projects.into())
}

/// Read-only SDK access, confined to published projects.
async fn sdk_endpoint(
    State(config): State<Arc<HostedConfig>>,
    Extension(user): Extension<UserId>,
    UrlPath(endpoint): UrlPath<String>,
    Json(mut params): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    let dir = user.dir(&config);
    match endpoint.as_str() {
        "listProjectsStateless" => return cmd_result_to_json(published_projects(&dir)),
        // About the machine the app runs on; a server has none of these set up.
        "listEditors"
        | "listPrograms"
        | "listKnownGithubAccounts"
        | "listKnownGitlabAccounts"
        | "listKnownBitbucketAccounts" => return cmd_result_to_json(Ok(serde_json::json!([]))),
        "getUserProfileLocal" => return cmd_result_to_json(Ok(serde_json::Value::Null)),
        _ => {}
    }
    // Reads that don't declare what they provide yet.
    let undeclared_read = matches!(
        endpoint.as_str(),
        "getTerminalOptionsForPlatform" | "getInitialBranchIntegration"
    );
    // Declared as reads, but they fetch from remotes, or read the server's own settings
    // rather than a user's.
    let not_for_users = matches!(
        endpoint.as_str(),
        "workspaceFetchFromRemotes" | "getAiConfiguration"
    );
    let is_read = undeclared_read
        || !not_for_users
            && inventory::iter::<but_schemars::ApiFnEntry>()
                .any(|entry| entry.js_name == endpoint && entry.provides.is_some());
    if !is_read {
        return cmd_result_to_json(Err(anyhow::anyhow!(
            "{endpoint} is not available on a hosted server"
        )));
    }
    if let Err(err) = resolve_project(&dir, &mut params) {
        return cmd_result_to_json(Err(err));
    }
    crate::call_sdk_endpoint(&endpoint, params).await
}

/// Endpoints get the handle of the project's store in place of its hosted project ID. Every
/// read names its project, so `dir`, the user's own, confines them all.
fn resolve_project(dir: &Path, params: &mut serde_json::Value) -> anyhow::Result<()> {
    let Some(project_id) = params.get_mut("projectId") else {
        return Ok(());
    };
    let store = existing_store(
        dir,
        project_id.as_str().context("projectId must be a string")?,
    )?;
    *project_id = but_ctx::ProjectHandle::from_path(&store)?
        .to_string()
        .into();
    Ok(())
}
