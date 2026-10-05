//! A hosted, read-only but-server: machines publish branches over git, and the web UI reads them.
//!
//! Every request is made as a GitButler user, and sees only what that user published, in
//! `users/<id>/store/<project>.git` under the data directory: one bare repository per project
//! (its root commit), which is the project the API opens. Each machine publishes into its own
//! namespace, `refs/heads/<machine>/<branch>` with its snapshot at
//! `refs/gitbutler/snapshots/<machine>/<name>`.
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
use tokio::{io::AsyncWriteExt as _, sync::broadcast};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
};

use crate::cmd_result_to_json;

mod events;
use events::{EVENTS_BUFFER, Presence, ProjectEvent};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct UserId(u64);

impl UserId {
    /// Everything this user published.
    fn dir(self, config: &HostedConfig) -> PathBuf {
        config.data_dir.join("users").join(self.0.to_string())
    }
}

/// Signed-in browsers, by the session ID in their cookie, with when they signed in. In memory: a
/// restart signs them out.
type Sessions = Arc<Mutex<std::collections::HashMap<String, (UserId, std::time::Instant)>>>;

const SESSION_COOKIE: &str = "but_hosted_session";
/// How long a browser stays signed in.
const SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(14 * 24 * 60 * 60);

#[derive(Clone)]
struct HostedState {
    config: Arc<HostedConfig>,
    events: broadcast::Sender<ProjectEvent>,
    presence: Presence,
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

impl FromRef<HostedState> for Presence {
    fn from_ref(state: &HostedState) -> Self {
        state.presence.clone()
    }
}

impl FromRef<HostedState> for broadcast::Sender<ProjectEvent> {
    fn from_ref(state: &HostedState) -> Self {
        state.events.clone()
    }
}

/// Run the hosted server until interrupted.
pub async fn run(config: HostedConfig) -> anyhow::Result<()> {
    // The server's settings are its own, never a developer's.
    if std::env::var_os("E2E_TEST_APP_DATA_DIR").is_none() {
        bail!("hosted mode keeps its settings apart: set E2E_TEST_APP_DATA_DIR");
    }

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
        presence: Presence::default(),
        sessions: Sessions::default(),
        writing: Arc::default(),
        // A stalled GitButler API would otherwise hold every request waiting on it.
        http: reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .expect("a client with a timeout builds"),
    };
    let api = Router::new()
        .route(
            "/git/{project}/{*rest}",
            any(git_http).layer(DefaultBodyLimit::disable()),
        )
        .route("/sdk/{endpoint}", post(sdk_endpoint))
        .route("/get_app_settings", post(app_settings))
        .route("/events", get(events::events))
        .route("/session", get(|| async { StatusCode::NO_CONTENT }))
        .route("/machines", get(machines))
        .route("/send", post(send_branch))
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
    let mut sessions = state
        .sessions
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    sessions.retain(|_, (_, since)| since.elapsed() < SESSION_TTL);
    sessions.get(session_id(headers)?).map(|(user, _)| *user)
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
        .insert(session.clone(), (user, std::time::Instant::now()));
    // `Secure` behind the https proxy, which says so; plain HTTP on localhost goes without.
    let secure = headers
        .get("x-forwarded-proto")
        .is_some_and(|proto| proto == "https");
    let cookie = format!(
        "{SESSION_COOKIE}={session}; Path=/; HttpOnly; SameSite=Strict; Max-Age={}{}",
        SESSION_TTL.as_secs(),
        if secure { "; Secure" } else { "" }
    );
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

/// A branch another machine published, to send to one more of the account's machines.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SendRequest {
    /// The project's root commit.
    project: String,
    /// The machine that published it.
    from: String,
    /// Its short name, e.g. `agent/search`.
    branch: String,
    to: String,
}

/// Send what `from` last published of a branch to `to`, as if `from` had sent it: the same inbox
/// ref a push would write, checked and announced the same way. For the hosted page, which has no
/// machine of its own to send from.
async fn send_branch(
    State(state): State<HostedState>,
    Extension(user): Extension<UserId>,
    headers: HeaderMap,
    Json(request): Json<SendRequest>,
) -> Response {
    // Another site can't make a signed-in browser send.
    if !events::same_origin(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let result = async {
        let SendRequest {
            project,
            from,
            branch,
            to,
        } = request;
        let store = store(&user.dir(&state.config), &project)?;
        if !store.join("HEAD").exists() {
            bail!("nothing was published to {project}");
        }
        // As the client names it in refs: anything but a few characters becomes a dash.
        let name = branch.replace(
            |c: char| !c.is_ascii_alphanumeric() && !"._-".contains(c),
            "-",
        );
        if !is_name(&from) || !is_name(&to) || !is_name(&name) {
            bail!("not a machine or branch name");
        }
        if from == to {
            bail!("{branch} is already on {to}");
        }
        let _writing = state.writing.lock().await;
        let snapshot = git(
            &store,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{SNAPSHOTS}{from}/{name}"),
            ],
        )
        .with_context(|| format!("{from} hasn't published {branch}"))?;
        let inbox = format!("{INBOX}{to}/{from}/{name}");
        git(&store, &["update-ref", &inbox, &snapshot])?;
        let sent = record_send(&store, &inbox)?;
        events::announce_send(&state.events, user, &project, sent);
        anyhow::Ok(())
    }
    .await;
    match result {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => (StatusCode::BAD_REQUEST, format!("{err:#}")).into_response(),
    }
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

/// Git's smart HTTP protocol through `git http-backend`; a push that moves a snapshot publishes its branch.
async fn git_http(
    State(HostedState {
        config,
        events,
        writing,
        ..
    }): State<HostedState>,
    Extension(user): Extension<UserId>,
    UrlPath((project, rest)): UrlPath<(String, String)>,
    method: Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = async {
        let dir = user.dir(&config);
        let store = store(&dir, &project)?;
        let pushing = rest == "git-receive-pack";
        let writing = writing.lock().await;
        // Only a push makes a project. Anything else of one nobody published, a push's ref
        // advertisement included, reads an empty repository, which advertises the same nothing.
        let (root, path) = if pushing || store.join("HEAD").exists() {
            if !store.join("HEAD").exists() {
                std::fs::create_dir_all(&store)?;
                git(&store, &["init", "--bare", "-q"])?;
                // Bare repositories keep no reflogs by default; these record every machine's pushes.
                git(&store, &["config", "core.logAllRefUpdates", "always"])?;
            }
            (dir.join("store"), format!("/{project}.git/{rest}"))
        } else {
            let empty = config.data_dir.join("empty.git");
            if !empty.join("HEAD").exists() {
                std::fs::create_dir_all(&empty)?;
                git(&empty, &["init", "--bare", "-q"])?;
            }
            (config.data_dir.clone(), format!("/empty.git/{rest}"))
        };
        let _writing = pushing.then_some(writing);
        // A push git rejects, e.g. one planned against refs another push has since moved,
        // still ends well for `http-backend`; only a moved snapshot means a publish.
        let published_before = if pushing {
            published_refs(&store)?
        } else {
            Default::default()
        };

        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        let mut child = tokio::process::Command::new("git")
            .arg("http-backend")
            .env("GIT_PROJECT_ROOT", root)
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("PATH_INFO", path)
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
        if pushing && out.status.success() {
            let after = published_refs(&store)?;
            let moved = after
                .iter()
                .filter(|(name, id)| published_before.get(*name) != Some(*id));
            for (name, _) in moved {
                // The refs landed but can't be shown, so the push mustn't look like it worked.
                if name.starts_with(SNAPSHOTS) {
                    let tip = record_publish(&store, name)
                        .with_context(|| format!("the push landed, but {name} isn't valid"))?;
                    events::announce_publish(&events, user, &project, tip);
                } else {
                    let sent = record_send(&store, name)
                        .with_context(|| format!("the push landed, but {name} isn't valid"))?;
                    events::announce_send(&events, user, &project, sent);
                }
            }
            // Dismissed or pulled, so other windows of the receiving machine catch up.
            if published_before
                .keys()
                .any(|name| !after.contains_key(name))
            {
                events::announce_project_changed(&events, user, &project);
            }
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

const SNAPSHOTS: &str = "refs/gitbutler/snapshots/";
const INBOX: &str = "refs/gitbutler/inbox/";

/// Every machine's snapshots in `store`, `refs/gitbutler/snapshots/<machine>/<name>`, and what
/// machines sent each other, `refs/gitbutler/inbox/<to>/<from>/<name>`, with the commit each
/// points at.
fn published_refs(store: &Path) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
    let refs = git(
        store,
        &[
            "for-each-ref",
            "--format=%(refname) %(objectname)",
            SNAPSHOTS,
            INBOX,
        ],
    )?;
    Ok(refs
        .lines()
        .filter_map(|line| line.split_once(' '))
        .map(|(name, id)| (name.to_owned(), id.to_owned()))
        .collect())
}

/// Check a pushed inbox ref: it must be the sender's latest snapshot of that branch, pushed with
/// it. Returns what's announced to the receiving machine.
fn record_send(store: &Path, inbox_ref: &str) -> anyhow::Result<events::Sent> {
    let (to, rest) = inbox_ref
        .strip_prefix(INBOX)
        .and_then(|rest| rest.split_once('/'))
        .context("an inbox ref names its receiver")?;
    let (from, name) = rest.split_once('/').context("and its sender")?;
    let snapshot = format!("{SNAPSHOTS}{from}/{name}");
    if git(store, &["rev-parse", inbox_ref])? != git(store, &["rev-parse", &snapshot])? {
        bail!("it isn't {from}'s latest {name}");
    }
    let message: Snapshot =
        serde_json::from_str(&git(store, &["log", "-1", "--format=%B", &snapshot])?)?;
    Ok(events::Sent {
        from: from.to_owned(),
        to: to.to_owned(),
        branch: message
            .head
            .strip_prefix("refs/heads/")
            .unwrap_or(&message.head)
            .to_owned(),
        title: message.title,
    })
}

/// What a snapshot commit says about its branch, as its message.
#[derive(serde::Deserialize)]
struct Snapshot {
    /// The format of this message; only 1 exists.
    version: u32,
    /// The project's name, e.g. its directory.
    title: String,
    /// The branch it was taken of, e.g. `refs/heads/feature`.
    head: String,
    /// The remote-tracking branch its work is based on, e.g. `refs/remotes/origin/main`.
    target: Option<String>,
}

/// Check a pushed snapshot, and on a project's first publish take its name and target from it.
/// Returns the branch's tip, the snapshot's parent.
fn record_publish(store: &Path, snapshot_ref: &str) -> anyhow::Result<String> {
    let snapshot: Snapshot =
        serde_json::from_str(&git(store, &["log", "-1", "--format=%B", snapshot_ref])?)?;
    if snapshot.version != 1 {
        bail!("unsupported snapshot version {}", snapshot.version);
    }
    if !snapshot.head.starts_with("refs/heads/")
        || snapshot.head == "refs/heads/gitbutler/workspace"
    {
        bail!("only branches can be published: {}", snapshot.head);
    }

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
        // The project's own HEAD sits on the target, which every machine's branches build on.
        git(store, &["update-ref", "--no-deref", "HEAD", &target])?;
    }
    git(store, &["rev-parse", &format!("{snapshot_ref}^")])
}

/// The server's own settings.
async fn app_settings() -> Json<serde_json::Value> {
    cmd_result_to_json((|| {
        let settings = but_settings::AppSettingsWithDiskSync::new_with_customization(
            but_path::app_config_dir()?,
            None,
        )?;
        Ok(serde_json::to_value(&*settings.get()?)?)
    })())
}

/// What a user's machines published, across all of their projects: what a client needs to list
/// machines and repos without fetching each project.
async fn machines(
    State(config): State<Arc<HostedConfig>>,
    Extension(user): Extension<UserId>,
    headers: HeaderMap,
) -> Response {
    // The asking machine, as it names itself on `/events`, to count what was sent to it.
    let asking = headers
        .get("x-but-machine")
        .and_then(|value| value.to_str().ok())
        .filter(|name| is_name(name));
    match published_machines(&user.dir(&config), asking) {
        Ok(projects) => Json(projects).into_response(),
        Err(err) => {
            tracing::warn!("listing machines failed: {err:#}");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

/// Per project with publishes in `dir`: its root commit, name, and each machine's latest publish
/// and branch count, most recent first, with how many of its branches wait in `asking`'s inbox.
fn published_machines(dir: &Path, asking: Option<&str>) -> anyhow::Result<serde_json::Value> {
    let mut projects = Vec::new();
    for entry in std::fs::read_dir(dir.join("store"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let store = entry.path();
        let Ok(title) = git(&store, &["config", "gitbutler.title"]) else {
            continue;
        };
        let root = entry
            .file_name()
            .to_string_lossy()
            .trim_end_matches(".git")
            .to_owned();
        let refs = git(
            &store,
            &[
                "for-each-ref",
                "--format=%(refname) %(committerdate:unix)",
                SNAPSHOTS,
            ],
        )?;
        // Machine name to its latest publish, in seconds, and how many branches it published.
        let mut machines = std::collections::BTreeMap::<String, (i64, usize)>::new();
        for line in refs.lines() {
            let Some((name, time)) = line.split_once(' ') else {
                continue;
            };
            let Some((machine, _)) = name.trim_start_matches(SNAPSHOTS).split_once('/') else {
                continue;
            };
            let entry = machines.entry(machine.to_owned()).or_default();
            entry.0 = entry.0.max(time.parse().unwrap_or_default());
            entry.1 += 1;
        }
        // Sender to how many of its branches wait for the asking machine.
        let mut sent = std::collections::BTreeMap::<String, usize>::new();
        if let Some(asking) = asking {
            let inbox = format!("{INBOX}{asking}/");
            for name in git(&store, &["for-each-ref", "--format=%(refname)", &inbox])?.lines() {
                if let Some((from, _)) = name.trim_start_matches(&inbox).split_once('/') {
                    *sent.entry(from.to_owned()).or_default() += 1;
                }
            }
        }
        let mut machines: Vec<_> = machines.into_iter().collect();
        machines.sort_by_key(|(_, (at, _))| std::cmp::Reverse(*at));
        projects.push(serde_json::json!({
            "root": root,
            "title": title,
            "machines": machines.into_iter().map(|(name, (at, branches))| serde_json::json!({
                "sent": sent.get(&name).copied().unwrap_or_default(),
                "name": name,
                "publishedAt": at * 1000,
                "branches": branches,
            })).collect::<Vec<_>>(),
        }));
    }
    Ok(projects.into())
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
        // `hostedAccount` asks a hub; this is one.
        "workspaceFetchFromRemotes" | "getAiConfiguration" | "hostedAccount"
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
