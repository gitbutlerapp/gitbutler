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
    extract::{DefaultBodyLimit, Path as UrlPath, RawQuery, Request, State},
    http::{HeaderMap, Method, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, post},
};
use but_core::diff::PUBLISHED_WORKTREE_REF;
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

/// Run the hosted server until interrupted.
pub async fn run(config: HostedConfig) -> anyhow::Result<()> {
    let url = format!("{}:{}", config.bind_addr, config.port);
    let web_dir = config.web_dir.clone();
    let config = Arc::new(config);
    let api = Router::new()
        .route(
            "/git/{project}/{machine}/{checkout}/{*rest}",
            any(git_http).layer(DefaultBodyLimit::disable()),
        )
        .route("/sdk/{endpoint}", post(sdk_endpoint))
        .route_layer(middleware::from_fn_with_state(
            config.clone(),
            require_token,
        ))
        .with_state(config);
    let app = match web_dir {
        Some(dir) => api
            .fallback_service(ServeDir::new(&dir).fallback(ServeFile::new(dir.join("index.html")))),
        None => api,
    };

    let listener = tokio::net::TcpListener::bind(&url)
        .await
        .with_context(|| format!("Failed to bind to {url}"))?;
    println!("Hosted: http://{url}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn require_token(
    State(config): State<Arc<HostedConfig>>,
    request: Request,
    next: Next,
) -> Response {
    let authorization = request.headers().get(header::AUTHORIZATION);
    if authorization.and_then(|value| value.to_str().ok())
        == Some(&format!("Bearer {}", config.token))
    {
        next.run(request).await
    } else {
        StatusCode::UNAUTHORIZED.into_response()
    }
}

/// One published checkout, named `<project>/<machine>/<checkout>` where the project is its
/// root commit. The same names make up its push URL and its project ID.
struct Checkout {
    project: String,
    machine: String,
    checkout: String,
}

fn is_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with('.')
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
}

impl Checkout {
    fn new(project: &str, machine: &str, checkout: &str) -> anyhow::Result<Self> {
        let is_root_commit =
            matches!(project.len(), 40 | 64) && project.bytes().all(|b| b.is_ascii_hexdigit());
        if !is_root_commit || !is_name(machine) || !is_name(checkout) {
            bail!("not a published checkout: {project}/{machine}/{checkout}");
        }
        Ok(Checkout {
            project: project.into(),
            machine: machine.into(),
            checkout: checkout.into(),
        })
    }

    fn store(&self, config: &HostedConfig) -> PathBuf {
        config
            .data_dir
            .join("store")
            .join(format!("{}.git", self.project))
    }

    fn view(&self, config: &HostedConfig) -> PathBuf {
        config
            .data_dir
            .join("views")
            .join(&self.project)
            .join(&self.machine)
            .join(format!("{}.git", self.checkout))
    }

    /// The store's prefix for this checkout's refs.
    fn namespace(&self) -> String {
        format!(
            "refs/namespaces/{}/refs/namespaces/{}/",
            self.machine, self.checkout
        )
    }
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

fn init_bare(dir: &Path) -> anyhow::Result<()> {
    if !dir.join("HEAD").exists() {
        std::fs::create_dir_all(dir)?;
        git(dir, &["init", "--bare", "-q"])?;
    }
    Ok(())
}

/// Git's smart HTTP protocol through `git http-backend`, inside the checkout's namespace.
async fn git_http(
    State(config): State<Arc<HostedConfig>>,
    UrlPath((project, machine, checkout, rest)): UrlPath<(String, String, String, String)>,
    method: Method,
    RawQuery(query): RawQuery,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let result = async {
        let checkout = Checkout::new(&project, &machine, &checkout)?;
        let store = checkout.store(&config);
        init_bare(&store)?;
        git(&store, &["config", "http.receivepack", "true"])?;

        let header = |name: &str| {
            headers
                .get(name)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        let mut child = tokio::process::Command::new("git")
            .arg("http-backend")
            .env("GIT_PROJECT_ROOT", config.data_dir.join("store"))
            .env("GIT_HTTP_EXPORT_ALL", "1")
            .env("GIT_NAMESPACE", format!("{machine}/{}", checkout.checkout))
            .env("PATH_INFO", format!("/{project}.git/{rest}"))
            .env("REQUEST_METHOD", method.as_str())
            .env("QUERY_STRING", query.unwrap_or_default())
            .env("CONTENT_TYPE", header("content-type"))
            .env("HTTP_CONTENT_ENCODING", header("content-encoding"))
            .env("GIT_PROTOCOL", header("git-protocol"))
            .env("REMOTE_USER", &machine)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut stdin = child.stdin.take().context("stdin")?;
        let writer = tokio::spawn(async move { stdin.write_all(&body).await });
        let out = child.wait_with_output().await?;
        writer.await??;
        if rest == "git-receive-pack" && out.status.success() {
            update_view(&config, &checkout)
                .inspect_err(|err| tracing::warn!("failed to update a view: {err:#}"))
                .ok();
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

/// What a checkout announces in `published.json` on `refs/gitbutler/meta`, pushed with its refs.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct Announcement {
    /// The format of this announcement; only 1 exists.
    version: u32,
    /// The ref `HEAD` points at in the checkout, e.g. `refs/heads/gitbutler/workspace`.
    head: String,
    target_ref: Option<String>,
    target_commit_id: Option<String>,
    push_remote: Option<String>,
    /// Remote names and their fetch URLs, so remote-tracking branches resolve.
    remotes: std::collections::BTreeMap<String, String>,
    /// Without uncommitted changes, an earlier publish's are stale.
    includes_uncommitted: bool,
}

/// Make the checkout's view match what it last pushed.
fn update_view(config: &HostedConfig, checkout: &Checkout) -> anyhow::Result<()> {
    let (store, view, namespace) = (
        checkout.store(config),
        checkout.view(config),
        checkout.namespace(),
    );
    let announcement: Announcement = serde_json::from_str(&git(
        &store,
        &[
            "cat-file",
            "blob",
            &format!("{namespace}refs/gitbutler/meta:published.json"),
        ],
    )?)?;
    if announcement.version != 1 {
        bail!("unsupported announcement version {}", announcement.version);
    }
    if !announcement.includes_uncommitted {
        git(
            &store,
            &[
                "update-ref",
                "-d",
                &format!("{namespace}{PUBLISHED_WORKTREE_REF}"),
            ],
        )?;
    }

    init_bare(&view)?;
    std::fs::write(
        view.join("objects/info/alternates"),
        format!("{}\n", store.join("objects").display()),
    )?;
    let store_path = store.display().to_string();
    let refspec = format!("+{namespace}refs/*:refs/*");
    git(
        &view,
        &[
            "fetch",
            "--prune",
            "--no-tags",
            "--quiet",
            &store_path,
            &refspec,
        ],
    )?;
    git(&view, &["symbolic-ref", "HEAD", &announcement.head])?;
    for (name, url) in &announcement.remotes {
        if !is_name(name) {
            bail!("invalid remote name: {name}");
        }
        git(&view, &["config", &format!("remote.{name}.url"), url])?;
        let fetch = format!("+refs/heads/*:refs/remotes/{name}/*");
        git(&view, &["config", &format!("remote.{name}.fetch"), &fetch])?;
    }
    but_core::ref_metadata::ProjectMeta {
        target_ref: announcement.target_ref.map(TryInto::try_into).transpose()?,
        target_commit_id: announcement
            .target_commit_id
            .map(|id| gix::ObjectId::from_hex(id.as_bytes()))
            .transpose()?,
        push_remote: announcement.push_remote,
    }
    .persist(&gix::open(&view)?)?;
    Ok(())
}

/// Every published checkout, as the project list Lite shows.
fn published_projects(config: &HostedConfig) -> anyhow::Result<serde_json::Value> {
    let entries = |dir: &Path| -> Vec<String> {
        std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| {
                e.file_name()
                    .to_string_lossy()
                    .trim_end_matches(".git")
                    .to_owned()
            })
            .collect()
    };
    let views = config.data_dir.join("views");
    let mut projects = Vec::new();
    for project in entries(&views) {
        for machine in entries(&views.join(&project)) {
            for checkout in entries(&views.join(&project).join(&machine)) {
                let id = format!("{project}/{machine}/{checkout}");
                projects.push(serde_json::json!({
                    "id": id,
                    "title": format!("{checkout} · {machine}"),
                    "path": id,
                    "isOpen": false,
                }));
            }
        }
    }
    Ok(projects.into())
}

/// Read-only SDK access, confined to published checkouts.
async fn sdk_endpoint(
    State(config): State<Arc<HostedConfig>>,
    UrlPath(endpoint): UrlPath<String>,
    Json(mut params): Json<serde_json::Value>,
) -> Json<serde_json::Value> {
    if endpoint == "listProjectsStateless" {
        return cmd_result_to_json(published_projects(&config));
    }
    let is_read = inventory::iter::<but_schemars::ApiFnEntry>()
        .any(|entry| entry.js_name == endpoint && entry.provides.is_some());
    if !is_read {
        return cmd_result_to_json(Err(anyhow::anyhow!(
            "{endpoint} is not available on a hosted server"
        )));
    }
    if let Err(err) = resolve_project(&config, &mut params) {
        return cmd_result_to_json(Err(err));
    }
    crate::call_sdk_endpoint(&endpoint, params).await
}

/// Endpoints get the handle of a checkout's view in place of its hosted project ID.
fn resolve_project(config: &HostedConfig, params: &mut serde_json::Value) -> anyhow::Result<()> {
    let Some(project_id) = params.get_mut("projectId") else {
        return Ok(());
    };
    let id = project_id.as_str().context("projectId must be a string")?;
    let parts: Vec<&str> = id.splitn(3, '/').collect();
    let [project, machine, checkout] = parts[..] else {
        bail!("not a published checkout: {id}");
    };
    let view = Checkout::new(project, machine, checkout)?.view(config);
    if !view.join("HEAD").exists() {
        bail!("not a published checkout: {id}");
    }
    *project_id = but_ctx::ProjectHandle::from_path(&view)?.to_string().into();
    Ok(())
}
