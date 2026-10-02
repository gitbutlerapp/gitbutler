//! A hosted, read-only but-server: machines publish checkouts over git, and the web UI reads them.
//!
//! Layout under the data directory:
//! * `store/<project>.git` - one bare repository per project (its root commit), shared by every
//!   machine. Each published checkout pushes into its own ref namespace, `<machine>/<checkout>`.
//! * `views/<project>/<machine>/<checkout>.git` - a bare repository per checkout holding only
//!   that namespace's refs, borrowing objects from the store. This is what reads open, so a
//!   published checkout is an ordinary project to `but-api`. A push updates it.

use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

use anyhow::{Context as _, bail};
use axum::{
    Json, Router,
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Path as UrlPath, Query, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, post},
};
use but_ctx::ProjectHandle;
use serde::Deserialize;
use serde_json::json;
use tokio::io::AsyncWriteExt as _;
use tower_http::services::{ServeDir, ServeFile};

use crate::cmd_result_to_json;

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
    /// The bearer token every API and git request must carry.
    pub token: String,
}

struct Hosted {
    data_dir: PathBuf,
    token: String,
}

impl Hosted {
    fn store(&self, project: &str) -> PathBuf {
        self.data_dir.join("store").join(format!("{project}.git"))
    }

    fn views(&self) -> PathBuf {
        self.data_dir.join("views")
    }

    fn view(&self, checkout: &Checkout) -> PathBuf {
        self.views()
            .join(&checkout.project)
            .join(&checkout.machine)
            .join(format!("{}.git", checkout.checkout))
    }
}

/// Run the hosted server until interrupted.
pub async fn run(config: HostedConfig) -> anyhow::Result<()> {
    let hosted = Arc::new(Hosted {
        data_dir: config.data_dir,
        token: config.token,
    });

    let api = Router::new()
        .route(
            "/git/{project}/{machine}/{checkout}/{*rest}",
            any(git_http).layer(DefaultBodyLimit::disable()),
        )
        .route("/sdk/{endpoint}", post(sdk_endpoint))
        .route_layer(middleware::from_fn_with_state(
            hosted.clone(),
            require_token,
        ))
        .with_state(hosted);

    let app = match config.web_dir {
        Some(web_dir) => api.fallback_service(
            ServeDir::new(&web_dir).fallback(ServeFile::new(web_dir.join("index.html"))),
        ),
        None => api,
    };

    let url = format!("{}:{}", config.bind_addr, config.port);
    let listener = tokio::net::TcpListener::bind(&url)
        .await
        .with_context(|| format!("Failed to bind to {url}"))?;
    println!("Hosted: http://{url}");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Accept the token as a bearer token, or as the password of git's basic auth.
async fn require_token(
    State(hosted): State<Arc<Hosted>>,
    request: Request,
    next: Next,
) -> Response {
    let authorized = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            if let Some(token) = value.strip_prefix("Bearer ") {
                return token == hosted.token;
            }
            value
                .strip_prefix("Basic ")
                .and_then(|encoded| {
                    use base64::Engine as _;
                    base64::engine::general_purpose::STANDARD
                        .decode(encoded)
                        .ok()
                })
                .and_then(|decoded| String::from_utf8(decoded).ok())
                .is_some_and(|credentials| {
                    credentials
                        .split_once(':')
                        .is_some_and(|(_user, password)| password == hosted.token)
                })
        });
    if authorized {
        next.run(request).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Basic realm=\"but-server\"")],
        )
            .into_response()
    }
}

/// Project names are root commits; machine and checkout names are single path components.
fn validate_names(project: &str, machine: &str, checkout: &str) -> anyhow::Result<()> {
    if !(project.len() == 40 || project.len() == 64)
        || !project.bytes().all(|b| b.is_ascii_hexdigit())
    {
        bail!("project must be a root commit id: {project}");
    }
    for name in [machine, checkout] {
        if name.is_empty()
            || name.starts_with('.')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            bail!("invalid machine or checkout name: {name}");
        }
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> anyhow::Result<String> {
    let out = std::process::Command::new("git")
        .arg("--git-dir")
        .arg(dir)
        .args(args)
        .output()
        .context("failed to run git")?;
    if !out.status.success() {
        bail!(
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

fn init_bare(dir: &Path) -> anyhow::Result<()> {
    if dir.join("HEAD").exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dir)?;
    git(dir, &["init", "--bare", "-q"])?;
    Ok(())
}

/// Git's smart HTTP protocol through `git http-backend`, inside the checkout's namespace.
async fn git_http(
    State(hosted): State<Arc<Hosted>>,
    UrlPath((project, machine, checkout, rest)): UrlPath<(String, String, String, String)>,
    method: Method,
    Query(query): Query<Vec<(String, String)>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    match git_http_inner(
        &hosted, &project, &machine, &checkout, &rest, method, &query, &headers, body,
    )
    .await
    {
        Ok(response) => response,
        Err(err) => (StatusCode::BAD_REQUEST, format!("{err:#}")).into_response(),
    }
}

#[expect(clippy::too_many_arguments)]
async fn git_http_inner(
    hosted: &Hosted,
    project: &str,
    machine: &str,
    checkout: &str,
    rest: &str,
    method: Method,
    query: &[(String, String)],
    headers: &HeaderMap,
    body: Bytes,
) -> anyhow::Result<Response> {
    validate_names(project, machine, checkout)?;
    let store = hosted.store(project);
    init_bare(&store)?;
    git(&store, &["config", "http.receivepack", "true"])?;

    let header = |name: header::HeaderName| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_owned()
    };
    let query_string = query
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&");
    let mut child = tokio::process::Command::new("git")
        .arg("http-backend")
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("GIT_PROJECT_ROOT", hosted.data_dir.join("store"))
        .env("GIT_HTTP_EXPORT_ALL", "1")
        .env("GIT_NAMESPACE", format!("{machine}/{checkout}"))
        .env("PATH_INFO", format!("/{project}.git/{rest}"))
        .env("REQUEST_METHOD", method.as_str())
        .env("QUERY_STRING", query_string)
        .env("CONTENT_TYPE", header(header::CONTENT_TYPE))
        .env("HTTP_CONTENT_ENCODING", header(header::CONTENT_ENCODING))
        .env(
            "GIT_PROTOCOL",
            header(header::HeaderName::from_static("git-protocol")),
        )
        .env("REMOTE_USER", machine)
        .env("REMOTE_ADDR", "127.0.0.1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to start git http-backend")?;
    let mut stdin = child.stdin.take().context("stdin")?;
    let writer = tokio::spawn(async move {
        stdin.write_all(&body).await.ok();
    });
    let out = child.wait_with_output().await?;
    writer.await.ok();
    if method == Method::POST && rest == "git-receive-pack" && out.status.success() {
        let checkout = Checkout {
            project: project.to_owned(),
            machine: machine.to_owned(),
            checkout: checkout.to_owned(),
        };
        if let Err(err) = update_view(hosted, &checkout) {
            tracing::warn!("failed to update the view of {checkout:?}: {err:#}");
        }
    }

    // CGI output: headers, a blank line, then the body.
    let split = out
        .stdout
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map(|at| (at, 4))
        .or_else(|| {
            out.stdout
                .windows(2)
                .position(|w| w == b"\n\n")
                .map(|at| (at, 2))
        })
        .context("git http-backend sent no headers")?;
    let (head, body) = (&out.stdout[..split.0], &out.stdout[split.0 + split.1..]);
    let mut response = Response::builder();
    for line in String::from_utf8_lossy(head).lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.eq_ignore_ascii_case("Status") {
            let code = value.split_whitespace().next().unwrap_or("200");
            response = response.status(code.parse::<u16>().unwrap_or(500));
        } else {
            response = response.header(name, value);
        }
    }
    Ok(response.body(Body::from(body.to_vec()))?)
}

/// One published checkout.
#[derive(Debug)]
struct Checkout {
    project: String,
    machine: String,
    checkout: String,
}

/// What a checkout announces in `published.json` on `refs/gitbutler/meta`, pushed with its refs.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Announcement {
    /// The format of this announcement; only 1 exists.
    version: u32,
    /// Shown in the project list, e.g. the repository's directory name.
    title: String,
    /// The ref `HEAD` points at in the checkout, e.g. `refs/heads/gitbutler/workspace`.
    head: String,
    target_ref: Option<String>,
    target_commit_id: Option<String>,
    push_remote: Option<String>,
    /// Remote names and their fetch URLs, so remote-tracking branches resolve.
    #[serde(default)]
    remotes: std::collections::BTreeMap<String, String>,
    /// Without uncommitted changes, an earlier publish's are stale.
    #[serde(default)]
    includes_uncommitted: bool,
}

/// Make the checkout's view match what it last pushed.
fn update_view(hosted: &Hosted, published: &Checkout) -> anyhow::Result<()> {
    let store = hosted.store(&published.project);
    let view = hosted.view(published);
    init_bare(&view)?;
    std::fs::write(
        view.join("objects/info/alternates"),
        format!("{}\n", store.join("objects").display()),
    )?;
    let namespace = format!(
        "refs/namespaces/{}/refs/namespaces/{}/refs/*",
        published.machine, published.checkout
    );
    git(
        &view,
        &[
            "fetch",
            "--prune",
            "--no-tags",
            "--quiet",
            &store.display().to_string(),
            &format!("+{namespace}:refs/*"),
        ],
    )?;
    let announcement: Announcement = serde_json::from_str(
        &git(
            &view,
            &["cat-file", "blob", "refs/gitbutler/meta:published.json"],
        )
        .context("the push carried no announcement")?,
    )?;
    if announcement.version != 1 {
        bail!("unsupported announcement version {}", announcement.version);
    }
    if !announcement.includes_uncommitted {
        git(
            &view,
            &["update-ref", "-d", but_core::diff::PUBLISHED_WORKTREE_REF],
        )?;
        let store_ref = format!(
            "refs/namespaces/{}/refs/namespaces/{}/{}",
            published.machine,
            published.checkout,
            but_core::diff::PUBLISHED_WORKTREE_REF
        );
        git(&store, &["update-ref", "-d", &store_ref])?;
    }
    git(&view, &["symbolic-ref", "HEAD", &announcement.head])?;
    for (name, url) in &announcement.remotes {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            bail!("invalid remote name: {name}");
        }
        git(&view, &["config", &format!("remote.{name}.url"), url])?;
        git(
            &view,
            &[
                "config",
                &format!("remote.{name}.fetch"),
                &format!("+refs/heads/*:refs/remotes/{name}/*"),
            ],
        )?;
    }

    let repo = gix::open(&view)?;
    but_core::ref_metadata::ProjectMeta {
        target_ref: announcement
            .target_ref
            .as_deref()
            .map(TryInto::try_into)
            .transpose()?,
        target_commit_id: announcement
            .target_commit_id
            .as_deref()
            .map(|id| gix::ObjectId::from_hex(id.as_bytes()))
            .transpose()?,
        push_remote: announcement.push_remote.clone(),
    }
    .persist(&repo)?;

    let storage = view.join("gitbutler");
    std::fs::create_dir_all(&storage)?;
    std::fs::write(
        storage.join("published.json"),
        serde_json::to_vec(&json!({ "title": announcement.title }))?,
    )?;
    Ok(())
}

/// Every published checkout, as the project list Lite shows.
fn published_projects(hosted: &Hosted) -> anyhow::Result<serde_json::Value> {
    let mut projects = Vec::new();
    let read_dir = |dir: &Path| -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|entries| entries.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default()
    };
    for project in read_dir(&hosted.views()) {
        for machine in read_dir(&project) {
            for view in read_dir(&machine) {
                let title = std::fs::read(view.join("gitbutler/published.json"))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                    .and_then(|v| v["title"].as_str().map(ToOwned::to_owned))
                    .unwrap_or_default();
                let id = format!(
                    "{}/{}/{}",
                    project.file_name().unwrap_or_default().to_string_lossy(),
                    machine.file_name().unwrap_or_default().to_string_lossy(),
                    view.file_stem().unwrap_or_default().to_string_lossy(),
                );
                projects.push(json!({
                    "id": id,
                    "title": format!("{title} · {}", machine.file_name().unwrap_or_default().to_string_lossy()),
                    "path": id,
                    "isOpen": false,
                }));
            }
        }
    }
    Ok(serde_json::Value::Array(projects))
}

/// Read-only SDK access, confined to published checkouts.
async fn sdk_endpoint(
    State(hosted): State<Arc<Hosted>>,
    UrlPath(endpoint): UrlPath<String>,
    Json(params): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    if endpoint == "listProjectsStateless" {
        return cmd_result_to_json(published_projects(&hosted));
    }
    let is_read = inventory::iter::<but_schemars::ApiFnEntry>()
        .any(|entry| entry.js_name == endpoint && entry.provides.is_some());
    if !is_read {
        return cmd_result_to_json(Err(anyhow::anyhow!(
            "{endpoint} is not available on a hosted server"
        )));
    }
    let mut params = params;
    if let Err(err) = resolve_project(&hosted, &mut params) {
        return cmd_result_to_json(Err(err));
    }
    crate::call_sdk_endpoint(&endpoint, params).await
}

/// A hosted project ID names a published checkout as `<project>/<machine>/<checkout>`; the
/// endpoint gets the handle of its view instead.
fn resolve_project(hosted: &Hosted, params: &mut serde_json::Value) -> anyhow::Result<()> {
    let Some(project_id) = params.get_mut("projectId") else {
        return Ok(());
    };
    let id = project_id.as_str().context("projectId must be a string")?;
    let mut parts = id.splitn(3, '/');
    let (Some(project), Some(machine), Some(checkout)) = (parts.next(), parts.next(), parts.next())
    else {
        bail!("not a published checkout: {id}");
    };
    validate_names(project, machine, checkout)?;
    let view = hosted.view(&Checkout {
        project: project.to_owned(),
        machine: machine.to_owned(),
        checkout: checkout.to_owned(),
    });
    if !view.join("HEAD").exists() {
        bail!("not a published checkout: {id}");
    }
    *project_id = ProjectHandle::from_path(&view)?.to_string().into();
    Ok(())
}
