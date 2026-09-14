//! `but panel`: a live, read-only view of the workspace, served to the browser from localhost.
//!
//! The page and its script are compiled in. Its data comes from the same `but-api` functions the
//! GUI uses: the detailed workspace graph, the worktree changes, linked worktrees, commit details
//! and file diffs. Reviews and CI are the exception: they are only readable through legacy APIs
//! today, so they appear only in builds with the `legacy` feature.
//!
//! One server shows any project: the page names it with `?project=<path>`, and each project gets
//! its own `Context`, opened on first use. Running `but panel` again, from another project, reuses a
//! server that already holds the port and only points at that project's page. Requests are handled
//! one at a time on the calling thread, which keeps every `Context` free of concurrent access.

use std::{
    collections::HashMap,
    fmt::Write as _,
    io::{self, BufReader, Read as _, Write as _},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use but_api::{
    commit::json::ChangesSource,
    diff::{ComputeLineStats, json::CommitDetails},
    open::program::{OpenSpec, open_in_program_unchecked},
};
use but_core::sync::RepoShared;
use but_ctx::Context;
use but_workspace::ui::workspace::DetailedGraphRowData;
use serde_json::json;

use crate::{
    CliResult,
    args::panel::Platform,
    bad_input,
    theme::Theme,
    utils::{CliOutput, CliOutputHuman, WriteWithUtils},
};

const INDEX_HTML: &str = include_str!("index.html");
const APP_JS: &str = include_str!("app.js");

/// Requests are a request line and a few headers; anything larger is not from the page.
const MAX_REQUEST_BYTES: u64 = 16 * 1024;

/// Claim the panel's port for this project, or find a panel already serving it.
///
/// Returns what to print, and the server to [run](Server::run) once it's printed: printing first
/// puts the URL on stdout before this process starts serving and never returns.
pub fn start(ctx: &Context, args: Platform) -> CliResult<(PanelOutcome, Server)> {
    let Platform { port, no_open } = args;
    let root = repository_root(ctx)?;
    let url = project_url(port, &root);

    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(listener) => Some(listener),
        // A panel already runs here, and it can show this project too.
        Err(err) if err.kind() == io::ErrorKind::AddrInUse && is_panel(port) => None,
        Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
            return Err(bad_input(format!("Port {port} is already in use"))
                .hint("Pass `--port` to serve the panel on another port")
                .into());
        }
        Err(err) => {
            return Err(anyhow::Error::from(err)
                .context(format!("Could not listen on 127.0.0.1:{port}"))
                .into());
        }
    };

    let outcome = PanelOutcome {
        url: url.clone(),
        project: root.clone(),
        reused: listener.is_none(),
    };
    let server = Server {
        listener,
        port,
        url: if no_open { None } else { Some(url) },
        root,
    };
    Ok((outcome, server))
}

/// Where the panel shows this project, and whether an already running panel does.
#[must_use]
pub struct PanelOutcome {
    url: String,
    project: PathBuf,
    reused: bool,
}

impl CliOutputHuman for PanelOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self {
            url,
            project,
            reused,
        } = self;
        let name = repository_name(&project);
        if reused {
            writeln!(out, "Showing {name} in the running panel at {url}")?;
        } else {
            writeln!(
                out,
                "Serving the panel for {name} at {url} (Ctrl-C to stop)"
            )?;
        }
        Ok(())
    }
}

impl CliOutput for PanelOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(serde::Serialize)]
        struct Output {
            url: String,
            project: PathBuf,
            reused: bool,
        }

        let Self {
            url,
            project,
            reused,
        } = self;
        Output {
            url,
            project,
            reused,
        }
    }
}

/// The panel server this process runs, if it claimed the port.
pub struct Server {
    listener: Option<TcpListener>,
    port: u16,
    /// The page to open in a browser, unless `--no-open` was passed.
    url: Option<String>,
    root: PathBuf,
}

impl Server {
    /// Open the browser, then serve until interrupted if this process holds the port.
    pub fn run(self, ctx: &mut Context) -> anyhow::Result<()> {
        let Self {
            listener,
            port,
            url,
            root,
        } = self;
        if let Some(url) = url
            && let Err(err) = but_api::open::open_url(url)
        {
            tracing::warn!(?err, "could not open a browser for the panel");
        }
        let Some(listener) = listener else {
            return Ok(());
        };

        let mut projects = Projects {
            default: ctx,
            default_root: root,
            opened: HashMap::new(),
            roots: HashMap::new(),
        };
        // A commit's files never change, so each is listed once, whichever project it's in.
        let mut commit_files = HashMap::new();
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            if let Err(err) = handle_connection(&mut projects, &mut commit_files, port, stream) {
                // A dropped or malformed connection only affects that one request.
                tracing::debug!(?err, "panel request failed");
            }
        }
        Ok(())
    }
}

/// The page for the project at `root`.
fn project_url(port: u16, root: &Path) -> String {
    format!(
        "http://localhost:{port}/?project={}",
        percent_encode(&root.to_string_lossy())
    )
}

/// Whether the server holding `port` is a panel, which answers `/api/ping`.
fn is_panel(port: u16) -> bool {
    let ask = || -> io::Result<String> {
        let mut stream = TcpStream::connect(("127.0.0.1", port))?;
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        write!(
            stream,
            "GET /api/ping HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n"
        )?;
        let mut response = String::new();
        stream
            .take(MAX_REQUEST_BYTES)
            .read_to_string(&mut response)?;
        Ok(response)
    };
    ask().is_ok_and(|response| response.contains(r#""panel":true"#))
}

/// The canonical directory a project is known by: its main worktree, so a linked worktree's path
/// and the main checkout's path name the same project.
fn repository_root(ctx: &Context) -> anyhow::Result<PathBuf> {
    ctx.workdir_or_gitdir()?
        .canonicalize()
        .context("Could not resolve the repository root")
}

fn repository_name(root: &Path) -> String {
    root.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| root.display().to_string())
}

/// Every project GitButler knows, as name and canonical path. The project list is only readable
/// through legacy APIs today.
#[cfg(feature = "legacy")]
fn known_projects() -> Option<Vec<(String, PathBuf)>> {
    let projects = but_api::legacy::projects::list_projects_stateless().ok()?;
    let projects = serde_json::to_value(projects).ok()?;
    Some(
        projects
            .as_array()?
            .iter()
            .filter_map(|project| {
                let path = PathBuf::from(project.get("path")?.as_str()?);
                // A project whose directory is gone can't be shown.
                let path = path.canonicalize().ok()?;
                let name = project
                    .get("title")
                    .and_then(|title| title.as_str())
                    .map_or_else(|| repository_name(&path), str::to_owned);
                Some((name, path))
            })
            .collect(),
    )
}

#[cfg(not(feature = "legacy"))]
fn known_projects() -> Option<Vec<(String, PathBuf)>> {
    None
}

/// The projects this server has shown, each with the context it reads through.
struct Projects<'ctx> {
    /// The project `but panel` was started in.
    default: &'ctx mut Context,
    default_root: PathBuf,
    /// Other projects, by repository root.
    opened: HashMap<PathBuf, Context>,
    /// Which root each requested path resolved to, so a path is only discovered once.
    roots: HashMap<PathBuf, PathBuf>,
}

impl Projects<'_> {
    /// The projects to offer in the page's switcher, by name: every GitButler project where the
    /// project list is readable, and otherwise the ones this server has shown.
    fn list(&self) -> serde_json::Value {
        let mut listed: Vec<(String, PathBuf)> = known_projects().unwrap_or_else(|| {
            std::iter::once(&self.default_root)
                .chain(self.opened.keys())
                .map(|root| (repository_name(root), root.clone()))
                .collect()
        });
        listed.sort_by_key(|(name, _)| name.to_lowercase());
        listed.dedup_by(|a, b| a.1 == b.1);
        serde_json::Value::Array(
            listed
                .into_iter()
                .map(|(name, path)| json!({ "name": name, "path": path }))
                .collect(),
        )
    }

    /// The context and root for the project at `requested`, or the default project without one.
    fn get(&mut self, requested: Option<&str>) -> anyhow::Result<(&mut Context, PathBuf)> {
        let Some(requested) = requested else {
            return Ok((&mut *self.default, self.default_root.clone()));
        };
        let requested = Path::new(requested)
            .canonicalize()
            .with_context(|| format!("No project at {requested}"))?;
        let root = match self.roots.get(&requested) {
            Some(root) => root.clone(),
            None => {
                let ctx = Context::discover(&requested).with_context(|| {
                    format!("Could not open a repository at {}", requested.display())
                })?;
                let root = repository_root(&ctx)?;
                if root != self.default_root {
                    self.opened.entry(root.clone()).or_insert(ctx);
                }
                self.roots.insert(requested, root.clone());
                root
            }
        };
        let ctx = if root == self.default_root {
            &mut *self.default
        } else {
            self.opened
                .get_mut(&root)
                .expect("every resolved root other than the default was opened")
        };
        Ok((ctx, root))
    }
}

fn handle_connection(
    projects: &mut Projects<'_>,
    commit_files: &mut HashMap<gix::ObjectId, serde_json::Value>,
    port: u16,
    stream: TcpStream,
) -> anyhow::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let request = read_request(BufReader::new((&stream).take(MAX_REQUEST_BYTES)))?;
    let route = route(&request, port);
    let project = requested_project(&request);
    let response =
        match route {
            Route::Index => Response::Page {
                content_type: "text/html; charset=utf-8",
                body: INDEX_HTML,
            },
            Route::Script => Response::Page {
                content_type: "text/javascript; charset=utf-8",
                body: APP_JS,
            },
            Route::Ping => Response::Json(json!({ "panel": true })),
            Route::Projects => data_response(Ok(projects.list())),
            Route::Workspace => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, root)| workspace_json(ctx, &root)),
            ),
            Route::Commit { id } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| commit_json(ctx, &id)),
            ),
            Route::Diff {
                path,
                commit,
                worktree,
            } => data_response(projects.get(project.as_deref()).and_then(|(ctx, _)| {
                diff_json(ctx, &path, commit.as_deref(), worktree.as_deref())
            })),
            Route::CommitFiles => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| commit_files_json(ctx, commit_files)),
            ),
            Route::Programs { path } => data_response(Ok(programs_json(&path))),
            Route::Open {
                path,
                program,
                worktree,
            } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| open_file(ctx, &program, &path, worktree.as_deref())),
            ),
            Route::Error { status } => Response::Error { status },
        };
    write_response(&stream, response)
}

/// Everything the page shows at once: the workspace graph, the uncommitted changes, the linked
/// worktrees, and each branch's review.
fn workspace_json(ctx: &mut Context, root: &Path) -> anyhow::Result<serde_json::Value> {
    let mut guard = ctx.exclusive_worktree_access();
    // This command keeps one context for its whole run; drop the cached workspace so each request
    // sees changes made by other processes since the last one.
    ctx.invalidate_workspace(guard.write_permission());
    let perm = guard.read_permission();

    let workspace = but_api::workspace::get_workspace(ctx, perm)?;
    let changes =
        but_api::diff::changes_in_worktree_with_perm(ctx, ChangesSource::Head, false, perm)?;
    let branch_names: Vec<String> = workspace
        .stacks
        .iter()
        .flat_map(|stack| &stack.rows)
        .filter_map(|row| match &row.data {
            DetailedGraphRowData::Reference(reference) => {
                Some(reference.ref_name.display_name.clone())
            }
            DetailedGraphRowData::Commit(_) => None,
        })
        .collect();

    Ok(json!({
        "repo": repository_name(root),
        "project": root,
        "workspace": serde_json::to_value(workspace)?,
        "changes": serde_json::to_value(changes.worktree_changes.changes)?,
        "worktrees": worktrees_json(ctx, perm),
        "reviews": reviews_json(ctx, &branch_names),
    }))
}

/// The linked worktrees, active ones with their uncommitted changes, or none when the
/// `worktreeManipulation` feature flag is off. Archived worktrees are listed without changes, which
/// can only be read from an active worktree.
fn worktrees_json(ctx: &Context, perm: &RepoShared) -> Vec<serde_json::Value> {
    let Ok(listing) = but_api::worktrees::worktrees_list_with_perm(ctx, perm) else {
        return Vec::new();
    };
    let archived = listing
        .archived
        .into_iter()
        .map(|worktree| json!({ "worktree": worktree, "archived": true }));
    listing
        .active
        .into_iter()
        .map(|worktree| {
            let source = ChangesSource::Worktree(worktree.name.to_string());
            let changes = but_api::diff::changes_in_worktree_with_perm(ctx, source, false, perm);
            let mut value = json!({ "worktree": worktree });
            match changes {
                Ok(changes) => value["changes"] = json!(changes.worktree_changes.changes),
                // One unreadable worktree shouldn't hide the others.
                Err(err) => value["error"] = json!(format!("{err:#}")),
            }
            value
        })
        .chain(archived)
        .collect()
}

/// Each workspace branch's review with a summary of its CI, keyed by branch name. Both are read
/// from the forge cache only, so polling never reaches the network.
#[cfg(feature = "legacy")]
fn reviews_json(ctx: &Context, branch_names: &[String]) -> serde_json::Value {
    let cache = Some(but_forge::CacheConfig::CacheOnly);
    // No forge or no account just means no reviews to show.
    let reviews = but_api::legacy::forge::list_reviews(ctx, cache.clone()).unwrap_or_default();
    let mut by_branch = serde_json::Map::new();
    for branch in branch_names {
        // GitHub reports a forked pull request's head as `owner:branch`.
        let Some(review) = reviews
            .iter()
            .find(|review| review.source_branch.rsplit(':').next() == Some(branch.as_str()))
        else {
            continue;
        };
        let checks = but_api::legacy::forge::list_ci_checks_for_ref(ctx, branch, cache.clone())
            .unwrap_or_default();
        by_branch.insert(
            branch.clone(),
            json!({
                "number": review.number,
                "url": review.html_url,
                "draft": review.draft,
                "ci": ci_summary(&checks),
            }),
        );
    }
    serde_json::Value::Object(by_branch)
}

#[cfg(not(feature = "legacy"))]
fn reviews_json(_ctx: &Context, _branch_names: &[String]) -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Summarise checks the way `but status` does: any failure wins, then anything still running,
/// then any success. Other conclusions count as neither.
#[cfg(feature = "legacy")]
fn ci_summary(checks: &[but_forge::CiCheck]) -> Option<&'static str> {
    use but_forge::{CiConclusion, CiStatus};

    let (mut failing, mut pending, mut passing) = (false, false, false);
    for check in checks {
        match &check.status {
            CiStatus::InProgress | CiStatus::Queued => pending = true,
            CiStatus::Complete { conclusion, .. } => match conclusion {
                CiConclusion::Success => passing = true,
                CiConclusion::Failure => failing = true,
                CiConclusion::ActionRequired
                | CiConclusion::Cancelled
                | CiConclusion::Neutral
                | CiConclusion::Skipped
                | CiConclusion::TimedOut
                | CiConclusion::Unknown => {}
            },
            CiStatus::Unknown => {}
        }
    }
    if failing {
        Some("failing")
    } else if pending {
        Some("pending")
    } else if passing {
        Some("passing")
    } else {
        None
    }
}

/// A commit's metadata, changed files and line statistics.
fn commit_json(ctx: &Context, id: &str) -> anyhow::Result<serde_json::Value> {
    let commit_id = parse_commit_id(id)?;
    let details: CommitDetails =
        but_api::diff::commit_details_with_line_stats(ctx, commit_id)?.into();
    Ok(serde_json::to_value(details)?)
}

/// The patch of one file changed in `commit`, or without one, in the uncommitted changes of
/// `worktree` or of the main worktree.
fn diff_json(
    ctx: &Context,
    path: &str,
    commit: Option<&str>,
    worktree: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let patch = match commit {
        Some(id) => {
            let change =
                but_api::diff::commit_details(ctx, parse_commit_id(id)?, ComputeLineStats::No)?
                    .diff_with_first_parent
                    .into_iter()
                    .find(|change| change.path == path.as_bytes())
                    .with_context(|| format!("'{path}' has no change to show"))?;
            but_api::diff::tree_change_diffs(ctx, change.into())?
        }
        None => {
            let source = worktree.map_or(ChangesSource::Head, |name| {
                ChangesSource::Worktree(name.to_owned())
            });
            let change = but_api::diff::changes_in_worktree(ctx, source.clone(), false)?
                .worktree_changes
                .changes
                .into_iter()
                .find(|change| change.path_bytes == path.as_bytes())
                .with_context(|| format!("'{path}' has no change to show"))?;
            but_api::diff::tree_change_diffs_from_source(ctx, source, change)?
        }
    };
    Ok(serde_json::to_value(patch)?)
}

/// The changed files of every commit in the workspace, by commit ID, for the page's file search.
fn commit_files_json(
    ctx: &mut Context,
    cache: &mut HashMap<gix::ObjectId, serde_json::Value>,
) -> anyhow::Result<serde_json::Value> {
    let commit_ids: Vec<gix::ObjectId> = {
        let mut guard = ctx.exclusive_worktree_access();
        ctx.invalidate_workspace(guard.write_permission());
        but_api::workspace::get_workspace(ctx, guard.read_permission())?
            .stacks
            .iter()
            .flat_map(|stack| &stack.rows)
            .filter_map(|row| match &row.data {
                DetailedGraphRowData::Commit(commit) => Some(commit.id),
                DetailedGraphRowData::Reference(_) => None,
            })
            .collect()
    };

    let mut files = serde_json::Map::new();
    for commit_id in commit_ids {
        let changes = match cache.get(&commit_id) {
            Some(changes) => changes.clone(),
            None => {
                let details: CommitDetails =
                    but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?.into();
                let changes = serde_json::to_value(details.changes)?;
                cache.insert(commit_id, changes.clone());
                changes
            }
        };
        files.insert(commit_id.to_string(), changes);
    }
    Ok(serde_json::Value::Object(files))
}

/// The GUI programs that suit `path`, by its extension, as `but open` offers them.
fn programs_json(path: &str) -> serde_json::Value {
    but_api::open::list_program_specs_for_file(Path::new(path))
        .into_iter()
        .filter(|program| !program.requires_terminal())
        .map(|program| json!({ "id": program.id, "name": program.name }))
        .collect()
}

/// Open the working copy of `path` with `program`: in the linked `worktree` when given, and
/// otherwise in the main worktree.
fn open_file(
    ctx: &mut Context,
    program: &str,
    path: &str,
    worktree: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    match worktree {
        None => but_api::open::open_in_program(ctx, program.to_owned(), path.to_owned(), None)?,
        Some(name) => {
            let guard = ctx.shared_worktree_access();
            let worktree =
                but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?
                    .active
                    .into_iter()
                    .find(|worktree| worktree.name.as_slice() == name.as_bytes())
                    .with_context(|| format!("No active worktree named '{name}'"))?;
            // The same check `open_in_program` makes, which only resolves paths in the main
            // worktree.
            let program = but_api::open::list_program_specs()
                .into_iter()
                .find(|spec| spec.id == program && !spec.requires_terminal())
                .with_context(|| format!("'{program}' is not a program the panel can open"))?;
            open_in_program_unchecked(&program, OpenSpec::File(worktree.path.join(path)))?;
        }
    }
    Ok(json!({ "opened": true }))
}

fn parse_commit_id(id: &str) -> anyhow::Result<gix::ObjectId> {
    id.parse()
        .with_context(|| format!("Invalid commit ID: {id}"))
}

fn data_response(result: anyhow::Result<serde_json::Value>) -> Response {
    Response::Json(match result {
        Ok(data) => json!({ "ok": true, "data": data }),
        Err(err) => json!({ "ok": false, "error": format!("{err:#}") }),
    })
}

#[derive(Debug, PartialEq)]
struct Request {
    method: String,
    target: String,
    host: Option<String>,
    origin: Option<String>,
}

fn read_request(mut reader: impl io::BufRead) -> anyhow::Result<Request> {
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    let mut parts = request_line.split_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        anyhow::bail!("malformed request line: {request_line:?}");
    };

    let (mut host, mut origin) = (None, None);
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        let header = line.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            if name.eq_ignore_ascii_case("host") {
                host = Some(value.trim().to_owned());
            } else if name.eq_ignore_ascii_case("origin") {
                origin = Some(value.trim().to_owned());
            }
        }
    }

    Ok(Request {
        method: method.to_owned(),
        target: target.to_owned(),
        host,
        origin,
    })
}

enum Response {
    Page {
        content_type: &'static str,
        body: &'static str,
    },
    Json(serde_json::Value),
    Error {
        status: &'static str,
    },
}

#[derive(Debug, PartialEq)]
enum Route {
    Index,
    Script,
    /// Lets a second `but panel` recognise a running panel.
    Ping,
    /// The projects the page can switch between.
    Projects,
    Workspace,
    Commit {
        id: String,
    },
    /// One file's patch, in `commit` or in the uncommitted changes of `worktree` or the main one.
    Diff {
        path: String,
        commit: Option<String>,
        worktree: Option<String>,
    },
    /// The changed files of every workspace commit, for searching.
    CommitFiles,
    /// The programs that can open `path`.
    Programs {
        path: String,
    },
    /// Open `path`, relative to `worktree` or the main worktree, with `program`. The panel's only
    /// action, and the only `POST`.
    Open {
        path: String,
        program: String,
        worktree: Option<String>,
    },
    Error {
        status: &'static str,
    },
}

fn route(request: &Request, port: u16) -> Route {
    // Only the page itself may call in. Checking `Host` stops another site from reaching this
    // server by pointing its own domain at 127.0.0.1 (DNS rebinding).
    let host_is_local = request.host.as_deref().is_some_and(|host| {
        host == format!("localhost:{port}") || host == format!("127.0.0.1:{port}")
    });
    if !host_is_local {
        return Route::Error {
            status: "403 Forbidden",
        };
    }
    let (path, query) = request
        .target
        .split_once('?')
        .unwrap_or((request.target.as_str(), ""));
    let Some(params) = query_params(query) else {
        return Route::Error {
            status: "400 Bad Request",
        };
    };
    let param = |name: &str| {
        params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .filter(|value| !value.is_empty())
    };

    // Everything but opening a file only reads, so it's a `GET`. Opening launches a program, so it
    // is a `POST` from the page itself: another site open in the browser can send a `POST` here,
    // but not with this server's own origin.
    let is_open = path == "/api/open";
    let origin_is_local = request.origin.as_deref().is_some_and(|origin| {
        origin == format!("http://localhost:{port}") || origin == format!("http://127.0.0.1:{port}")
    });
    match (request.method.as_str(), is_open) {
        ("GET", false) => {}
        ("POST", true) if origin_is_local => {}
        ("POST", true) => {
            return Route::Error {
                status: "403 Forbidden",
            };
        }
        _ => {
            return Route::Error {
                status: "405 Method Not Allowed",
            };
        }
    }

    match path {
        "/" => Route::Index,
        "/app.js" => Route::Script,
        "/api/ping" => Route::Ping,
        "/api/projects" => Route::Projects,
        "/api/workspace" => Route::Workspace,
        "/api/commit" => match param("id") {
            Some(id) => Route::Commit { id },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/diff" => match param("path") {
            Some(path) => Route::Diff {
                path,
                commit: param("commit"),
                worktree: param("worktree"),
            },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/commit-files" => Route::CommitFiles,
        "/api/programs" => match param("path") {
            Some(path) => Route::Programs { path },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/open" => match (param("path"), param("program")) {
            (Some(path), Some(program)) if is_inside_checkout(&path) => Route::Open {
                path,
                program,
                worktree: param("worktree"),
            },
            _ => Route::Error {
                status: "400 Bad Request",
            },
        },
        _ => Route::Error {
            status: "404 Not Found",
        },
    }
}

/// Whether `path` stays inside the checkout it's joined to: relative, and never climbing out.
fn is_inside_checkout(path: &str) -> bool {
    let path = Path::new(path);
    path.is_relative()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// The project a request names with `?project=<path>`, if any.
fn requested_project(request: &Request) -> Option<String> {
    let (_, query) = request.target.split_once('?')?;
    query_params(query)?
        .into_iter()
        .find(|(key, value)| key == "project" && !value.is_empty())
        .map(|(_, value)| value)
}

/// Escape everything but unreserved characters and `/`, which keeps paths readable in a URL.
fn percent_encode(input: &str) -> String {
    let mut encoded = String::with_capacity(input.len());
    for byte in input.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~' | b'/') {
            encoded.push(char::from(byte));
        } else {
            write!(encoded, "%{byte:02X}").expect("writing to a String cannot fail");
        }
    }
    encoded
}

/// Split and decode `a=1&b=2`, or `None` if an escape is malformed.
fn query_params(query: &str) -> Option<Vec<(String, String)>> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

/// Decode the `%XX` escapes `encodeURIComponent` produces, or `None` if they are malformed.
fn percent_decode(input: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = input.get(i + 1..i + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            decoded.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn write_response(mut stream: &TcpStream, response: Response) -> anyhow::Result<()> {
    let (status, content_type, body) = match response {
        Response::Page { content_type, body } => ("200 OK", content_type, body.to_owned()),
        Response::Json(value) => (
            "200 OK",
            "application/json",
            serde_json::to_string(&value).context("serializing the panel response")?,
        ),
        Response::Error { status } => (status, "text/plain; charset=utf-8", status.to_owned()),
    };
    write!(
        stream,
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {len}\r\n\
         Cache-Control: no-store\r\n\
         X-Content-Type-Options: nosniff\r\n\
         Connection: close\r\n\r\n\
         {body}",
        len = body.len(),
    )?;
    stream.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get(target: &str, host: Option<&str>) -> Request {
        Request {
            method: "GET".to_owned(),
            target: target.to_owned(),
            host: host.map(str::to_owned),
            origin: None,
        }
    }

    fn post(target: &str, origin: Option<&str>) -> Request {
        Request {
            method: "POST".to_owned(),
            origin: origin.map(str::to_owned),
            ..get(target, LOCAL)
        }
    }

    const LOCAL: Option<&str> = Some("localhost:7789");

    #[test]
    fn reads_the_request_line_and_host_header() {
        let raw = "GET /api/workspace HTTP/1.1\r\nhost: localhost:7789\r\nOrigin: http://localhost:7789\r\n\r\n";
        let request = read_request(raw.as_bytes()).expect("a well-formed request parses");
        assert_eq!(
            request,
            Request {
                origin: Some("http://localhost:7789".into()),
                ..get("/api/workspace", LOCAL)
            },
            "header names match case-insensitively"
        );
    }

    #[test]
    fn routes_the_page_and_its_data() {
        assert_eq!(route(&get("/", LOCAL), 7789), Route::Index);
        assert_eq!(route(&get("/app.js", LOCAL), 7789), Route::Script);
        assert_eq!(route(&get("/api/workspace", LOCAL), 7789), Route::Workspace);
        assert_eq!(route(&get("/api/ping", LOCAL), 7789), Route::Ping);
        assert_eq!(route(&get("/api/projects", LOCAL), 7789), Route::Projects);
        assert_eq!(
            route(&get("/api/commit?id=abc123", LOCAL), 7789),
            Route::Commit {
                id: "abc123".into()
            }
        );
        assert_eq!(
            route(&get("/api/diff?path=src%2Fa%20b.rs", LOCAL), 7789),
            Route::Diff {
                path: "src/a b.rs".into(),
                commit: None,
                worktree: None,
            },
            "a path without a commit asks for the uncommitted change, decoded"
        );
        assert_eq!(
            route(&get("/api/diff?path=a.rs&commit=abc123", LOCAL), 7789),
            Route::Diff {
                path: "a.rs".into(),
                commit: Some("abc123".into()),
                worktree: None,
            },
        );
        assert_eq!(
            route(&get("/api/diff?path=a.rs&worktree=feature-wt", LOCAL), 7789),
            Route::Diff {
                path: "a.rs".into(),
                commit: None,
                worktree: Some("feature-wt".into()),
            },
            "a worktree's uncommitted change names the worktree"
        );
    }

    #[test]
    fn names_the_project_in_the_url() {
        let path = "/Users/me/My Repo/fliege";
        let url = project_url(7789, Path::new(path));
        assert_eq!(
            url, "http://localhost:7789/?project=/Users/me/My%20Repo/fliege",
            "slashes stay readable, the space is escaped"
        );
        let target = url.trim_start_matches("http://localhost:7789");
        assert_eq!(
            requested_project(&get(&format!("/api/workspace{}", &target[1..]), LOCAL)),
            Some(path.to_owned()),
            "the page's request carries the same path back"
        );
        assert_eq!(
            requested_project(&get("/api/workspace", LOCAL)),
            None,
            "without a project the server shows the one it started in"
        );
    }

    #[test]
    fn opens_files_only_when_the_page_asks() {
        const PAGE: Option<&str> = Some("http://localhost:7789");
        assert_eq!(
            route(
                &post("/api/open?path=src%2Fa.rs&program=vscode", PAGE),
                7789
            ),
            Route::Open {
                path: "src/a.rs".into(),
                program: "vscode".into(),
                worktree: None,
            },
        );
        assert_eq!(
            route(
                &post(
                    "/api/open?path=a.rs&program=vscode",
                    Some("https://evil.example")
                ),
                7789
            ),
            Route::Error {
                status: "403 Forbidden"
            },
            "another site in the browser can't open files"
        );
        assert_eq!(
            route(&post("/api/open?path=a.rs&program=vscode", None), 7789),
            Route::Error {
                status: "403 Forbidden"
            },
            "a POST without an origin isn't from the page"
        );
        assert_eq!(
            route(&get("/api/open?path=a.rs&program=vscode", LOCAL), 7789),
            Route::Error {
                status: "405 Method Not Allowed"
            },
            "a GET, which any image tag can send, never opens anything"
        );
        for escape in ["..%2Fsecret", "%2Fetc%2Fpasswd", "src%2F..%2F..%2Fsecret"] {
            assert_eq!(
                route(
                    &post(&format!("/api/open?path={escape}&program=vscode"), PAGE),
                    7789
                ),
                Route::Error {
                    status: "400 Bad Request"
                },
                "{escape} would leave the checkout"
            );
        }
    }

    #[test]
    fn rejects_requests_the_page_would_not_make() {
        let forbidden = Route::Error {
            status: "403 Forbidden",
        };
        let bad = Route::Error {
            status: "400 Bad Request",
        };
        assert_eq!(
            route(&get("/", Some("evil.example:7789")), 7789),
            forbidden,
            "a rebound foreign host is refused"
        );
        assert_eq!(
            route(&get("/", Some("localhost:1234")), 7789),
            forbidden,
            "the host must name this server's port"
        );
        assert_eq!(
            route(&get("/", None), 7789),
            forbidden,
            "a host is required"
        );
        assert_eq!(
            route(
                &Request {
                    method: "POST".into(),
                    ..get("/api/workspace", LOCAL)
                },
                7789
            ),
            Route::Error {
                status: "405 Method Not Allowed"
            },
            "the panel is read-only"
        );
        assert_eq!(
            route(&get("/api/diff?path=%zz", LOCAL), 7789),
            bad,
            "malformed escapes are not guessed at"
        );
        assert_eq!(
            route(&get("/api/commit", LOCAL), 7789),
            bad,
            "a commit needs an id"
        );
        assert_eq!(
            route(&get("/nope", LOCAL), 7789),
            Route::Error {
                status: "404 Not Found"
            },
            "only the page, its script and the data routes exist"
        );
    }
}
