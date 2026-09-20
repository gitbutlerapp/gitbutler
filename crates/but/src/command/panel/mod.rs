//! `but panel`: a live view of the workspace, served to the browser from localhost. On request it
//! opens files, folders and forge pages, fetches, pushes, and pulls the target's new commits into
//! the workspace; it never edits commits or the worktree.
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
//!
//! No invocation owns that server: without `--foreground`, `but panel` starts it as a background
//! process of its own, so it outlives the terminal or chat that first asked for it, and
//! `but panel --stop` ends it.
//!
//! By default only this machine reaches it. With `--host` it listens on the network too, and as
//! the panel can push and open files, another device must then present the server's access token
//! with everything but the page itself. The token is part of the network URL `but panel` prints.

use std::{
    collections::HashMap,
    fmt::Write as _,
    io::{self, BufReader, Read as _, Write as _},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use but_api::{
    commit::json::ChangesSource,
    diff::{ComputeLineStats, json::CommitDetails},
    open::program::{OpenSpec, ProgramSpec, open_in_program_unchecked},
};
use but_core::sync::RepoShared;
use but_ctx::Context;
use but_workspace::ui::workspace::DetailedGraphRowData;
use nonempty::NonEmpty;
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
const ICON_SVG: &str = include_str!("icon.svg");
const SERVICE_WORKER_JS: &str = include_str!("sw.js");

/// Requests are a request line and a few headers; anything larger is not from the page.
const MAX_REQUEST_BYTES: u64 = 16 * 1024;

/// How long a background server gets to answer its first ping.
const BACKGROUND_START_TIMEOUT: Duration = Duration::from_secs(10);

/// Find the panel serving `port`, which can show this project too, or start one: in the background, or from this process with
/// `--foreground`. With `--stop`, end the one that runs instead.
///
/// Returns what to print, and the server to [run](Server::run) once it's printed: printing first
/// puts the URL on stdout before a foreground server starts serving and never returns.
pub fn start(ctx: &Context, args: Platform) -> CliResult<(PanelOutcome, Server)> {
    let Platform {
        host,
        port,
        no_open,
        app,
        foreground,
        stop,
    } = args;
    let root = repository_root(ctx)?;
    let url = project_url(port, &root);
    // Where this machine reaches the server: over loopback, unless it listens on one address only.
    let at = SocketAddr::new(
        if host.is_unspecified() {
            IpAddr::V4(Ipv4Addr::LOCALHOST)
        } else {
            host
        },
        port,
    );

    if stop {
        let stopped = is_panel(at) && stop_panel(at);
        let outcome = PanelOutcome {
            url,
            network_url: None,
            project: root.clone(),
            state: if stopped {
                PanelState::Stopped
            } else {
                PanelState::NotRunning
            },
        };
        let server = Server {
            listener: None,
            port,
            url: None,
            app,
            root,
            network: None,
        };
        return Ok((outcome, server));
    }

    // Ask before binding: next to a server that listens on every address, binding one of them can
    // succeed, and would start a second panel in front of it.
    let listener = if is_panel(at) {
        None
    } else {
        match TcpListener::bind((host, port)) {
            Ok(listener) if foreground => Some(listener),
            // The port is free: hand it to a server of its own, which outlives this invocation.
            Ok(listener) => {
                drop(listener);
                start_in_background(host, at, &root)?;
                None
            }
            Err(err) if err.kind() == io::ErrorKind::AddrInUse => {
                return Err(bad_input(format!("Port {port} is already in use"))
                    .hint("Pass `--port` to serve the panel on another port")
                    .into());
            }
            Err(err) => {
                return Err(anyhow::Error::from(err)
                    .context(format!("Could not listen on {host}:{port}"))
                    .into());
            }
        }
    };

    // A server this process runs makes up its network access; one that runs already is asked.
    let network = if listener.is_some() {
        (!host.is_loopback()).then(|| Network::new(host))
    } else {
        network_of(at)
    };
    if !host.is_loopback() && network.is_none() {
        return Err(bad_input(format!(
            "The panel on port {port} only listens on this machine"
        ))
        .hint("Stop it with `but panel --stop`, then start it with `--host` again")
        .into());
    }
    let outcome = PanelOutcome {
        url: url.clone(),
        network_url: network
            .as_ref()
            .map(|network| network.project_url(port, &root)),
        project: root.clone(),
        state: if listener.is_some() {
            PanelState::Foreground
        } else {
            PanelState::Background
        },
    };
    let server = Server {
        listener,
        port,
        url: if no_open { None } else { Some(url) },
        app,
        root,
        network,
    };
    Ok((outcome, server))
}

/// How other devices reach a server that listens on the network.
#[derive(Debug, Clone, PartialEq)]
struct Network {
    /// This machine's address on the network.
    address: IpAddr,
    /// What another device must present: the panel can push and open files, and has no login.
    token: String,
}

impl Network {
    fn new(host: IpAddr) -> Self {
        use rand::Rng as _;

        let token = rand::rng()
            .sample_iter(rand::distr::Alphanumeric)
            .take(32)
            .map(char::from)
            .collect();
        Network {
            address: if host.is_unspecified() {
                network_address().unwrap_or(host)
            } else {
                host
            },
            token,
        }
    }

    /// The page for the project at `root`, for another device.
    fn project_url(&self, port: u16, root: &Path) -> String {
        format!(
            "http://{}/?project={}&token={}",
            SocketAddr::new(self.address, port),
            percent_encode(&root.to_string_lossy()),
            self.token
        )
    }
}

/// The address this machine has on the network it reaches the internet through. Connecting a UDP
/// socket only picks the route; nothing is sent.
fn network_address() -> Option<IpAddr> {
    let socket = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    socket.connect(("8.8.8.8", 80)).ok()?;
    Some(socket.local_addr().ok()?.ip())
}

/// Where the panel shows this project, and which server does.
#[must_use]
pub struct PanelOutcome {
    url: String,
    /// Where another device finds it, if the server listens on the network.
    network_url: Option<String>,
    project: PathBuf,
    state: PanelState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
enum PanelState {
    /// A background server shows the project, started now or earlier.
    Background,
    /// This process serves it until interrupted.
    Foreground,
    /// `--stop` ended the server.
    Stopped,
    /// `--stop` found no server to end.
    NotRunning,
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
            network_url,
            project,
            state,
        } = self;
        let name = repository_name(&project);
        let (showing, stop) = match state {
            PanelState::Background => ("Showing", "`but panel --stop` to stop it"),
            PanelState::Foreground => ("Serving", "Ctrl-C to stop"),
            PanelState::Stopped | PanelState::NotRunning => ("", ""),
        };
        match (state, network_url) {
            (PanelState::Background | PanelState::Foreground, None) => {
                writeln!(out, "{showing} {name} in the panel at {url} ({stop})")?
            }
            (PanelState::Background | PanelState::Foreground, Some(network_url)) => {
                writeln!(out, "{showing} {name} in the panel ({stop})")?;
                writeln!(out, "  On this machine: {url}")?;
                writeln!(out, "  On your network: {network_url}")?;
            }
            (PanelState::Stopped, _) => writeln!(out, "Stopped the panel")?,
            (PanelState::NotRunning, _) => writeln!(out, "No panel is running")?,
        }
        Ok(())
    }
}

impl CliOutput for PanelOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(serde::Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            url: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            network_url: Option<String>,
            project: PathBuf,
            state: PanelState,
        }

        let Self {
            url,
            network_url,
            project,
            state,
        } = self;
        Output {
            url,
            network_url,
            project,
            state,
        }
    }
}

/// The panel server this process runs, if it claimed the port.
pub struct Server {
    listener: Option<TcpListener>,
    port: u16,
    /// The page to open in a browser, unless `--no-open` was passed.
    url: Option<String>,
    /// Open it in a window of its own rather than a tab.
    app: bool,
    root: PathBuf,
    /// How other devices reach it, if it listens on the network.
    network: Option<Network>,
}

impl Server {
    /// Open the browser, then serve until interrupted if this process holds the port.
    pub fn run(self, ctx: &mut Context) -> anyhow::Result<()> {
        let Self {
            listener,
            port,
            url,
            app,
            root,
            network,
        } = self;
        if let Some(url) = url {
            let opened = if app {
                open_app_window(&url)
            } else {
                but_api::open::open_url(url)
            };
            if let Err(err) = opened {
                tracing::warn!(?err, "could not open a browser for the panel");
            }
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
        // Each project's last graph, by repository root, reused while nothing it depends on changed.
        let mut graphs = HashMap::new();
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            match handle_connection(
                &mut projects,
                &mut commit_files,
                &mut graphs,
                port,
                network.as_ref(),
                stream,
            ) {
                Ok(Served::Stop) => break,
                Ok(Served::Continue) => {}
                // A dropped or malformed connection only affects that one request.
                Err(err) => tracing::debug!(?err, "panel request failed"),
            }
        }
        Ok(())
    }
}

/// Open `url` in a window of its own, the way Chromium browsers do with `--app`: the first of the
/// known ones that is installed takes it. With none, the default browser opens it as a tab.
fn open_app_window(url: &str) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};

    let app_flag = format!("--app={url}");
    let launched = if cfg!(target_os = "macos") {
        // `open` finds the application by name, and fails when it isn't installed.
        [
            "Google Chrome",
            "Arc",
            "Microsoft Edge",
            "Brave Browser",
            "Chromium",
        ]
        .iter()
        .any(|browser| {
            Command::new("/usr/bin/open")
                .args(["-na", browser, "--args", &app_flag])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
    } else {
        let browsers: &[&str] = if cfg!(target_os = "windows") {
            &["chrome", "msedge", "brave"]
        } else {
            &[
                "google-chrome",
                "google-chrome-stable",
                "chromium",
                "chromium-browser",
                "microsoft-edge",
                "brave-browser",
            ]
        };
        browsers.iter().any(|browser| {
            Command::new(browser)
                .arg(&app_flag)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .is_ok()
        })
    };
    if launched {
        return Ok(());
    }
    tracing::info!("no Chromium browser found for an app window; opening the default browser");
    but_api::open::open_url(url.to_owned())
}

/// The web app manifest that lets a Chromium browser install the panel as a windowed app: one per
/// project, named after it, opening on it. A page without a project gets the one the server
/// started in; only when a project can't be resolved does it install as the plain panel.
fn manifest_json(root: Option<PathBuf>) -> serde_json::Value {
    let (name, start_url) = match root {
        Some(root) => (
            format!("{} · GitButler", repository_name(&root)),
            format!("/?project={}", percent_encode(&root.to_string_lossy())),
        ),
        None => ("GitButler Panel".to_owned(), "/".to_owned()),
    };
    json!({
        "name": name,
        "short_name": "Panel",
        "description": "A live view of the GitButler workspace",
        "start_url": start_url,
        "id": start_url,
        "scope": "/",
        "display": "standalone",
        // Installed, the page may take the title bar too, keeping only the window's buttons.
        "display_override": ["window-controls-overlay"],
        "background_color": "#fbfbfa",
        "theme_color": "#fbfbfa",
        "icons": [{ "src": "/icon.svg", "sizes": "any", "type": "image/svg+xml" }],
    })
}

/// The page for the project at `root`.
fn project_url(port: u16, root: &Path) -> String {
    format!(
        "http://localhost:{port}/?project={}",
        percent_encode(&root.to_string_lossy())
    )
}

/// Run `but panel --foreground` for `root` as a process of its own, in its own process group so
/// the terminal's Ctrl-C and hang-up don't reach it, and wait until it answers.
fn start_in_background(host: IpAddr, at: SocketAddr, root: &Path) -> anyhow::Result<()> {
    use std::process::{Command, Stdio};

    use command_group::CommandGroup as _;

    let but_path = crate::utils::binary_path::current_exe_for_but_exec()
        .context("Could not find the `but` binary to serve the panel with")?;
    Command::new(but_path)
        .arg("-C")
        .arg(root)
        .args(["panel", "--foreground", "--no-open", "--host"])
        .arg(host.to_string())
        .arg("--port")
        .arg(at.port().to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .group_spawn()
        .context("Could not start the panel server")?;

    let started = std::time::Instant::now();
    while started.elapsed() < BACKGROUND_START_TIMEOUT {
        if is_panel(at) {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    anyhow::bail!("The panel server did not start on port {}", at.port())
}

/// Send one request to the server at `at`, as the page would, and return its response.
fn request(at: SocketAddr, method: &str, target: &str) -> io::Result<String> {
    let mut stream = TcpStream::connect(at)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "{method} {target} HTTP/1.1\r\nHost: {at}\r\nOrigin: http://{at}\r\nConnection: close\r\n\r\n"
    )?;
    let mut response = String::new();
    stream
        .take(MAX_REQUEST_BYTES)
        .read_to_string(&mut response)?;
    Ok(response)
}

/// Whether the server at `at` is a panel, which answers `/api/ping`.
fn is_panel(at: SocketAddr) -> bool {
    request(at, "GET", "/api/ping").is_ok_and(|response| response.contains(r#""panel":true"#))
}

/// Ask the panel at `at` to stop serving, and say whether it agreed.
fn stop_panel(at: SocketAddr) -> bool {
    request(at, "POST", "/api/stop").is_ok_and(|response| response.contains(r#""stopped":true"#))
}

/// How other devices reach the panel at `at`, which it tells this machine only. `None` if it
/// listens on this machine alone.
fn network_of(at: SocketAddr) -> Option<Network> {
    let response = request(at, "GET", "/api/network").ok()?;
    let (_, body) = response.split_once("\r\n\r\n")?;
    let body: serde_json::Value = serde_json::from_str(body).ok()?;
    let network = body.get("data")?;
    Some(Network {
        address: network.get("address")?.as_str()?.parse().ok()?,
        token: network.get("token")?.as_str()?.to_owned(),
    })
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

/// Remember the repository at `root` as a GitButler project, as the app's "add project" does.
/// Already being one is fine.
#[cfg(feature = "legacy")]
fn register_project(root: &Path) -> anyhow::Result<()> {
    use gitbutler_project::AddProjectOutcome;

    let outcome = but_api::legacy::projects::add_project_best_effort(root.to_owned())?;
    let refused = match outcome {
        AddProjectOutcome::Added(_) | AddProjectOutcome::AlreadyExists(_) => return Ok(()),
        AddProjectOutcome::PathNotFound => "the path doesn't exist",
        AddProjectOutcome::NotADirectory => "the path isn't a directory",
        AddProjectOutcome::BareRepository => "the repository is bare",
        AddProjectOutcome::NonMainWorktree => "the path is a linked worktree; add its main one",
        AddProjectOutcome::NoWorkdir => "the repository has no working directory",
        AddProjectOutcome::NoDotGitDirectory => "the repository has no .git directory",
        AddProjectOutcome::ReftableRefFormatUnsupported => "reftable repositories aren't supported",
        AddProjectOutcome::NotAGitRepository(_) => "the path isn't a Git repository",
    };
    anyhow::bail!("GitButler can't add {}: {refused}", root.display())
}

#[cfg(not(feature = "legacy"))]
fn register_project(_root: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// How often the app fetches on its own, in minutes, or zero or less when it doesn't. The panel
/// keeps to the same setting, and offers to change it.
fn auto_fetch_minutes() -> isize {
    but_settings::AppSettings::load_from_default_path_creating_without_customization()
        .map_or(-1, |settings| settings.fetch.auto_fetch_interval_minutes)
}

/// Change how often the app and the panel fetch on their own; zero or less turns it off.
fn set_auto_fetch_minutes(minutes: isize) -> anyhow::Result<serde_json::Value> {
    crate::command::config::load_app_settings_sync()?.update_fetch(
        but_settings::api::FetchUpdate {
            auto_fetch_interval_minutes: Some(minutes),
        },
    )?;
    Ok(json!({ "autoFetchMinutes": minutes }))
}

/// Fetch when the last attempt is older than the auto-fetch interval, as the app does. Any
/// process's fetch through the workspace API counts, and so does a failed attempt, so an
/// unreachable remote is tried no more often.
fn auto_fetch_if_due(ctx: &mut Context, minutes: isize) {
    if minutes <= 0 {
        return;
    }
    let last_attempt = but_api::workspace::workspace_fetch_status(ctx)
        .ok()
        .and_then(|status| status.last_attempted_ms);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64);
    let interval = minutes as u64 * 60_000;
    if last_attempt.is_none_or(|last| now.saturating_sub(last) >= interval)
        && let Err(err) = fetch(ctx)
    {
        tracing::debug!(?err, "the panel's automatic fetch failed");
    }
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
    /// project list is readable, and the ones this server has shown.
    fn list(&self) -> serde_json::Value {
        let mut listed: Vec<(String, PathBuf)> = known_projects().unwrap_or_default();
        listed.extend(
            std::iter::once(&self.default_root)
                .chain(self.opened.keys())
                .map(|root| (repository_name(root), root.clone())),
        );
        listed.sort_by_key(|(name, _)| name.to_lowercase());
        listed.dedup_by(|a, b| a.1 == b.1);
        serde_json::Value::Array(
            listed
                .into_iter()
                .map(|(name, path)| json!({ "name": name, "path": path }))
                .collect(),
        )
    }

    /// Open the repository at `path` so the switcher lists it, and remember it as a GitButler
    /// project where the project list is writable, so it stays listed after this server stops.
    /// Returns the project's root, for the page to show.
    fn add(&mut self, path: &str) -> anyhow::Result<PathBuf> {
        let (_, root) = self.get(Some(path))?;
        register_project(&root)?;
        Ok(root)
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
    graphs: &mut HashMap<PathBuf, GraphCache>,
    port: u16,
    network: Option<&Network>,
    stream: TcpStream,
) -> anyhow::Result<Served> {
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let (peer, arrived_at) = (stream.peer_addr()?.ip(), stream.local_addr()?.ip());
    // A connection from this machine comes over loopback, or to its own address from that address.
    let from_this_machine = peer.is_loopback() || peer == arrived_at;
    let mut request = read_request(BufReader::new((&stream).take(MAX_REQUEST_BYTES)))?;
    request.arrived_at = (!arrived_at.is_loopback()).then_some(arrived_at);
    let route = match route(&request, port) {
        // The page holds nothing; its data and what it can do are for those with the token.
        page @ (Route::Index | Route::Script | Route::Icon | Route::ServiceWorker) => page,
        error @ Route::Error { .. } => error,
        Route::Network if !from_this_machine => Route::Error {
            status: "403 Forbidden",
        },
        _ if !from_this_machine && !has_token(&request, network) => Route::Error {
            status: "403 Forbidden",
        },
        route => route,
    };
    let served = if route == Route::Stop {
        Served::Stop
    } else {
        Served::Continue
    };
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
            Route::Icon => Response::Page {
                content_type: "image/svg+xml",
                body: ICON_SVG,
            },
            Route::ServiceWorker => Response::Page {
                content_type: "text/javascript; charset=utf-8",
                body: SERVICE_WORKER_JS,
            },
            Route::Manifest => Response::Manifest(manifest_json(
                projects.get(project.as_deref()).ok().map(|(_, root)| root),
            )),
            Route::Ping => Response::Json(json!({ "panel": true })),
            Route::Stop => Response::Json(json!({ "stopped": true })),
            Route::Network => data_response(Ok(match network {
                Some(network) => json!({
                    "address": network.address,
                    "token": network.token,
                    "url": projects
                        .get(project.as_deref())
                        .ok()
                        .map(|(_, root)| network.project_url(port, &root)),
                }),
                None => serde_json::Value::Null,
            })),
            Route::Projects => data_response(Ok(projects.list())),
            Route::AddProject { path } => {
                data_response(projects.add(&path).map(|root| json!({ "path": root })))
            }
            Route::Workspace => {
                data_response(projects.get(project.as_deref()).and_then(|(ctx, root)| {
                    let minutes = auto_fetch_minutes();
                    auto_fetch_if_due(ctx, minutes);
                    let mut graph = graphs.remove(&root);
                    let result = workspace_json(ctx, &root, minutes, &mut graph);
                    if let Some(graph) = graph {
                        graphs.insert(root, graph);
                    }
                    result
                }))
            }
            Route::Settings { auto_fetch_minutes } => {
                data_response(set_auto_fetch_minutes(auto_fetch_minutes))
            }
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
            Route::Programs { path } => data_response(Ok(programs_json(path.as_deref()))),
            Route::FolderOpeners => data_response(Ok(folder_openers_json())),
            Route::Open {
                paths,
                program,
                worktree,
            } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| open_files(ctx, &program, &paths, worktree.as_deref())),
            ),
            Route::OpenFolder { with } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(_, root)| open_folder(&root, &with)),
            ),
            Route::OpenForgePage { url } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| open_forge_page(ctx, &url)),
            ),
            Route::Upstream => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| upstream_json(ctx)),
            ),
            Route::Fetch => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| fetch(ctx)),
            ),
            Route::Push { branch, force } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| push(ctx, &branch, force)),
            ),
            Route::Pull { check } => data_response(
                projects
                    .get(project.as_deref())
                    .and_then(|(ctx, _)| pull(ctx, check)),
            ),
            Route::Error { status } => Response::Error { status },
        };
    write_response(&stream, response)?;
    Ok(served)
}

/// Whether `request` carries the token a device other than this machine needs.
fn has_token(request: &Request, network: Option<&Network>) -> bool {
    let Some(network) = network else {
        return false;
    };
    request
        .target
        .split_once('?')
        .and_then(|(_, query)| query_params(query))
        .into_iter()
        .flatten()
        .any(|(key, value)| key == "token" && value == network.token)
}

/// Whether the server goes on after a request.
enum Served {
    Continue,
    /// The request was `but panel --stop`'s.
    Stop,
}

/// The workspace graph as last computed for a project, kept while nothing that feeds it changed.
/// Walking the graph is most of a poll's cost, and polls come every few seconds.
struct GraphCache {
    fingerprint: u64,
    taken: std::time::Instant,
    workspace: serde_json::Value,
    branch_names: Vec<String>,
    /// For each branch by name, the lines and files its commits change, against the branch below
    /// it or the target. They only change when the graph does, and cost a diff per branch.
    branch_stats: serde_json::Map<String, serde_json::Value>,
    behind: Option<usize>,
}

/// The cached graph is recomputed at least this often, in case something it depends on changed
/// without touching what the fingerprint watches.
const GRAPH_MAX_AGE: Duration = Duration::from_secs(30);

/// A digest of everything on disk the graph is computed from: `HEAD`, the refs, loose and
/// packed, the linked worktrees' heads, and GitButler's branch metadata. Any ref or metadata
/// write changes a modification time or size in there.
fn graph_fingerprint(ctx: &Context) -> anyhow::Result<u64> {
    use std::hash::{Hash, Hasher};

    fn hash_path(path: &Path, hasher: &mut impl Hasher) {
        let Ok(metadata) = std::fs::metadata(path) else {
            return;
        };
        path.hash(hasher);
        metadata.len().hash(hasher);
        metadata.modified().ok().hash(hasher);
        if metadata.is_dir()
            && let Ok(entries) = std::fs::read_dir(path)
        {
            for entry in entries.flatten() {
                hash_path(&entry.path(), hasher);
            }
        }
    }

    let repo = ctx.repo.get()?;
    let common = repo.common_dir().to_owned();
    let mut hasher = std::hash::DefaultHasher::new();
    for path in [
        repo.git_dir().join("HEAD"),
        common.join("packed-refs"),
        common.join("refs"),
        common.join("worktrees"),
        ctx.project_data_dir.join("virtual_branches.toml"),
    ] {
        hash_path(&path, &mut hasher);
    }
    Ok(hasher.finish())
}

/// Everything the page shows at once: the workspace graph, how far the target has moved on since
/// the last update, the uncommitted changes, the linked worktrees, and what the forge has for each
/// branch. The graph comes from `graph` while its fingerprint holds; the uncommitted changes are
/// read every time, as edits to files leave no trace the fingerprint could see.
fn workspace_json(
    ctx: &mut Context,
    root: &Path,
    auto_fetch_minutes: isize,
    graph: &mut Option<GraphCache>,
) -> anyhow::Result<serde_json::Value> {
    let fingerprint = graph_fingerprint(ctx)?;
    let fresh = graph.as_ref().is_some_and(|cache| {
        cache.fingerprint == fingerprint && cache.taken.elapsed() < GRAPH_MAX_AGE
    });

    let mut guard = ctx.exclusive_worktree_access();
    if !fresh {
        // This command keeps one context for its whole run; drop the cached workspace so the
        // request sees changes made by other processes since the last one.
        ctx.invalidate_workspace(guard.write_permission());
    }
    let perm = guard.read_permission();
    if !fresh {
        let workspace = but_api::workspace::get_workspace(ctx, perm)?;
        // Commits on the target's remote-tracking branch that no stack has yet, as last fetched.
        let behind = ctx
            .workspace_and_db_with_perm(perm)?
            .1
            .target_ref
            .as_ref()
            .map(|target| target.commits_ahead);
        let references = workspace
            .stacks
            .iter()
            .flat_map(|stack| &stack.rows)
            .filter_map(|row| match &row.data {
                DetailedGraphRowData::Reference(reference) => Some(&reference.ref_name),
                DetailedGraphRowData::Commit(_) => None,
            });
        let branch_names = references
            .clone()
            .map(|name| name.display_name.clone())
            .collect();
        let branch_stats = {
            let (repo, ws, _) = ctx.workspace_and_db_with_perm(perm)?;
            references
                .filter_map(|name| {
                    let full_name =
                        gix::refs::FullName::try_from(name.full_name_bytes.clone()).ok()?;
                    let changes =
                        but_workspace::ui::diff::changes_in_branch(&repo, &ws, full_name.as_ref())
                            .ok()?;
                    Some((
                        name.display_name.clone(),
                        serde_json::to_value(changes.stats).ok()?,
                    ))
                })
                .collect()
        };
        *graph = Some(GraphCache {
            fingerprint,
            taken: std::time::Instant::now(),
            workspace: serde_json::to_value(workspace)?,
            branch_names,
            branch_stats,
            behind,
        });
    }
    let cache = graph
        .as_ref()
        .expect("computed above when missing or stale");
    let changes =
        but_api::diff::changes_in_worktree_with_perm(ctx, ChangesSource::Head, false, perm)?;

    Ok(json!({
        "repo": repository_name(root),
        "project": root,
        "workspace": cache.workspace,
        "branchStats": cache.branch_stats,
        "behind": cache.behind,
        "changes": serde_json::to_value(changes.worktree_changes.changes)?,
        "worktrees": worktrees_json(ctx, perm),
        "forge": forge_json(ctx, &cache.branch_names, auto_fetch_minutes),
        "autoFetchMinutes": auto_fetch_minutes,
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

/// What the forge has for this workspace, or `null` when the target's forge is unknown: the forge's
/// name, what it calls a review, the repository's page, the URL a commit ID appends to, and for
/// each branch its compare page and its review with a summary of its CI. Reviews and CI come from
/// the forge cache, refreshed from the forge once they are older than the auto-fetch interval;
/// with auto-fetching off they are read from the cache only, so polling never reaches the network.
#[cfg(feature = "legacy")]
fn forge_json(
    ctx: &Context,
    branch_names: &[String],
    auto_fetch_minutes: isize,
) -> serde_json::Value {
    use but_api::legacy::forge;
    use but_forge::ForgeName;

    let Some(info) = forge::forge_info(ctx).ok().flatten() else {
        return serde_json::Value::Null;
    };
    let name = match info.name {
        ForgeName::GitHub => "GitHub",
        ForgeName::GitLab => "GitLab",
        ForgeName::Bitbucket => "Bitbucket",
        ForgeName::Azure => "Azure DevOps",
    };
    // A compare page is against the target, by its name on the remote.
    let compare = ctx
        .project_meta()
        .ok()
        .zip(ctx.repo.get().ok())
        .and_then(|(meta, repo)| {
            Some((
                forge::remote_url(&meta, &repo).ok()?,
                forge::target_short_name(&meta, &repo).ok()?,
            ))
        });
    let accounts = but_forge::get_all_forge_accounts().unwrap_or_default();
    let cache = Some(if auto_fetch_minutes > 0 {
        but_forge::CacheConfig::CacheWithFallback {
            max_age_seconds: auto_fetch_minutes as u64 * 60,
        }
    } else {
        but_forge::CacheConfig::CacheOnly
    });
    // No account just means no reviews to show.
    let reviews = forge::list_reviews(ctx, cache.clone()).unwrap_or_default();

    let mut branches = serde_json::Map::new();
    for branch in branch_names {
        // GitHub reports a forked pull request's head as `owner:branch`.
        let review = reviews
            .iter()
            .find(|review| review.source_branch.rsplit(':').next() == Some(branch.as_str()));
        let fork = review
            .and_then(|review| review.source_branch.rsplit_once(':'))
            .map(|(owner, _)| owner);
        let url = compare.as_ref().and_then(|(remote_url, base)| {
            but_forge::compare_branch_url(remote_url, base, branch, fork, &accounts)
        });
        let review = review.map(|review| {
            let checks =
                forge::list_ci_checks_for_ref(ctx, branch, cache.clone()).unwrap_or_default();
            json!({
                "number": review.number,
                "url": review.html_url,
                "draft": review.draft,
                "ci": ci_summary(&checks),
            })
        });
        branches.insert(branch.clone(), json!({ "url": url, "review": review }));
    }
    json!({
        "name": name,
        "unit": info.unit,
        "url": info.base_url,
        "commitUrl": format!("{}{}", info.base_url, info.commit_url_path),
        "branches": branches,
    })
}

#[cfg(not(feature = "legacy"))]
fn forge_json(
    _ctx: &Context,
    _branch_names: &[String],
    _auto_fetch_minutes: isize,
) -> serde_json::Value {
    serde_json::Value::Null
}

/// Open `url` in the default browser, which a page shown in an embedded pane can't do by itself:
/// its links open in that pane. Only pages on the project's own forge are opened, so the page
/// can't be made to open anything else.
#[cfg(feature = "legacy")]
fn open_forge_page(ctx: &Context, url: &str) -> anyhow::Result<serde_json::Value> {
    let info =
        but_api::legacy::forge::forge_info(ctx)?.context("The project's forge is unknown")?;
    anyhow::ensure!(
        is_on_forge(url, &info.base_url),
        "'{url}' isn't a page on the project's forge"
    );
    but_api::open::open_url(url.to_owned())?;
    Ok(json!({ "opened": true }))
}

#[cfg(not(feature = "legacy"))]
fn open_forge_page(_ctx: &Context, _url: &str) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("The project's forge is unknown")
}

/// Whether `url` is a web page on the host that serves `forge_base_url`.
#[cfg_attr(not(feature = "legacy"), allow(dead_code))]
fn is_on_forge(url: &str, forge_base_url: &str) -> bool {
    let (Ok(url), Ok(forge)) = (url::Url::parse(url), url::Url::parse(forge_base_url)) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https") && url.host().is_some() && url.host() == forge.host()
}

/// Read every branch's review and CI from the forge again, whatever the cache holds.
#[cfg(feature = "legacy")]
fn refresh_forge(ctx: &Context) -> anyhow::Result<()> {
    but_api::legacy::forge::list_reviews(ctx, Some(but_forge::CacheConfig::NoCache))?;
    but_api::legacy::forge::warm_ci_checks_cache(ctx)
}

#[cfg(not(feature = "legacy"))]
fn refresh_forge(_ctx: &Context) -> anyhow::Result<()> {
    Ok(())
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

/// The changed files and line statistics of every commit in the workspace, by commit ID, for the
/// page's file search and for the size each commit's row shows.
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
                    but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::Yes)?.into();
                let changes = json!({ "changes": details.changes, "stats": details.line_stats });
                cache.insert(commit_id, changes.clone());
                changes
            }
        };
        files.insert(commit_id.to_string(), changes);
    }
    Ok(serde_json::Value::Object(files))
}

/// The GUI programs that suit `path`, by its extension, as `but open` offers them. Without a path,
/// the editors, which can open several files at once.
fn programs_json(path: Option<&str>) -> serde_json::Value {
    let programs = match path {
        Some(path) => but_api::open::list_program_specs_for_file(Path::new(path)),
        None => but_api::open::list_program_specs()
            .into_iter()
            .filter(ProgramSpec::is_gui_editor)
            .collect(),
    };
    programs
        .into_iter()
        .filter(|program| !program.requires_terminal())
        .map(|program| json!({ "id": program.id, "name": program.name }))
        .collect()
}

/// Open the working copies of `paths` with `program`: in the linked `worktree` when given, and
/// otherwise in the main worktree. Files that are gone from that checkout are skipped, and the
/// number opened is returned.
fn open_files(
    ctx: &Context,
    program: &str,
    paths: &[String],
    worktree: Option<&str>,
) -> anyhow::Result<serde_json::Value> {
    let checkout = match worktree {
        None => ctx
            .repo
            .get()?
            .workdir()
            .context("project must have a workdir")?
            .to_owned(),
        Some(name) => {
            let guard = ctx.shared_worktree_access();
            but_api::worktrees::worktrees_list_with_perm(ctx, guard.read_permission())?
                .active
                .into_iter()
                .find(|worktree| worktree.name.as_slice() == name.as_bytes())
                .with_context(|| format!("No active worktree named '{name}'"))?
                .path
        }
    };
    let program = gui_program(program)?;

    let present: Vec<PathBuf> = paths
        .iter()
        .map(|path| checkout.join(path))
        .filter(|path| path.exists())
        .collect();
    let Some(files) = NonEmpty::from_vec(present) else {
        anyhow::bail!(match paths {
            [path] => format!("'{path}' is no longer in the checkout"),
            _ => "None of these files are in the checkout any more".to_owned(),
        });
    };
    let opened = files.len();
    let spec = match files {
        NonEmpty { head, tail } if tail.is_empty() => OpenSpec::File(head),
        files => OpenSpec::Files(files),
    };
    open_in_program_unchecked(&program, spec)?;
    Ok(json!({ "opened": opened }))
}

/// The program with `id`, if it's one the panel may launch: the same check `open_in_program`
/// makes, which only opens one path in the main worktree.
fn gui_program(id: &str) -> anyhow::Result<ProgramSpec> {
    but_api::open::list_program_specs()
        .into_iter()
        .find(|spec| spec.id == id && !spec.requires_terminal())
        .with_context(|| format!("'{id}' is not a program the panel can open"))
}

/// What opens the project's directory.
#[derive(Debug, PartialEq)]
enum FolderOpener {
    /// The platform's file manager.
    FileManager,
    /// A terminal, by the ID `open_in_terminal` knows.
    Terminal(String),
    /// A program, by its ID.
    Program(String),
}

/// What can open the project's directory, each with the query that asks for it: the file manager,
/// a terminal that is installed, and the editors.
fn folder_openers_json() -> serde_json::Value {
    let file_manager = match std::env::consts::OS {
        "macos" => "Finder",
        "windows" => "Explorer",
        _ => "File manager",
    };
    let mut openers = vec![json!({ "with": "", "name": file_manager })];
    if let Some((id, name)) = installed_terminal() {
        openers.push(json!({ "with": format!("terminal={}", percent_encode(&id)), "name": name }));
    }
    for program in but_api::open::list_program_specs()
        .into_iter()
        .filter(ProgramSpec::is_gui_editor)
    {
        openers.push(json!({
            "with": format!("program={}", percent_encode(&program.id)),
            "name": program.name,
        }));
    }
    serde_json::Value::Array(openers)
}

/// The terminal `but` recommends for this platform, if one is installed. The terminal list is only
/// readable through a legacy API today.
#[cfg(feature = "legacy")]
fn installed_terminal() -> Option<(String, String)> {
    but_api::open::terminal::get_recommended_terminal_for_platform(std::env::consts::OS.to_owned())
        .ok()
        .flatten()
        .map(|terminal| (terminal.identifier, terminal.display_name))
}

#[cfg(not(feature = "legacy"))]
fn installed_terminal() -> Option<(String, String)> {
    None
}

/// Open the project's directory at `root` with `with`.
fn open_folder(root: &Path, with: &FolderOpener) -> anyhow::Result<serde_json::Value> {
    match with {
        FolderOpener::FileManager => {
            let url = url::Url::from_directory_path(root)
                .ok()
                .with_context(|| format!("'{}' can't be opened as a URL", root.display()))?;
            but_api::open::open_url(url.to_string())?;
        }
        FolderOpener::Terminal(id) => {
            but_api::open::open_in_terminal(id.clone(), root.to_string_lossy().into_owned())?;
        }
        FolderOpener::Program(id) => {
            open_in_program_unchecked(&gui_program(id)?, OpenSpec::File(root.to_owned()))?;
        }
    }
    Ok(json!({ "opened": true }))
}

/// The target's commits the workspace doesn't have yet, newest first and as last fetched: what a
/// pull would bring in. It follows first parents, so a merge is one commit, carrying the review it
/// landed where the forge cache knows it.
fn upstream_json(ctx: &Context) -> anyhow::Result<serde_json::Value> {
    let page = but_api::target_commits::workspace_target_commits(ctx, None, None)?;
    let upstream: Vec<_> = page
        .commits
        .into_iter()
        .filter(|commit| !commit.in_workspace)
        .collect();
    Ok(serde_json::to_value(upstream)?)
}

/// Fetch from every remote, so the target's new commits and each branch's push status show, and
/// read the reviews and CI again. One refresh at a time across `but` processes, as `but refresh`
/// does. Without a forge account there are no reviews to read, which isn't a failed fetch.
fn fetch(ctx: &mut Context) -> anyhow::Result<serde_json::Value> {
    let _lock = but_core::sync::try_exclusive_inter_process_access(
        &ctx.gitdir,
        but_core::sync::LockScope::BackgroundRefreshOperations,
    )?;
    but_api::workspace::workspace_fetch_from_remotes(ctx, Some("auto".to_owned()))?;
    if let Err(err) = refresh_forge(ctx) {
        tracing::debug!(?err, "the panel could not refresh reviews and CI");
    }
    Ok(json!({ "fetched": true }))
}

/// Push `branch` and the branches below it in its stack, then bring their reviews up to date, as
/// `but push` does. Refused while a commit in that scope is conflicted, since the remote should
/// never see one. Returns the branches that were pushed.
#[cfg(feature = "legacy")]
fn push(ctx: &mut Context, branch: &str, force: bool) -> anyhow::Result<serde_json::Value> {
    let full_name: gix::refs::FullName = branch
        .try_into()
        .with_context(|| format!("'{branch}' is not a branch name"))?;
    let commits =
        crate::legacy::workspace::push_scope_with_expensive_commit_info(ctx, full_name.as_ref())?
            .with_context(|| format!("'{branch}' is not in the workspace"))?;
    let conflicted = commits.iter().filter(|commit| commit.has_conflicts).count();
    if conflicted > 0 {
        anyhow::bail!(
            "{conflicted} conflicted commit{} must be resolved before pushing",
            if conflicted == 1 { "" } else { "s" }
        );
    }
    let outcome = block_on(
        but_api::legacy::workspace::workspace_branch_and_ancestors_push(
            ctx.to_sync(),
            force,
            false,
            full_name.to_string(),
            true,
            Vec::new(),
        ),
    )?;
    let pushed: Vec<&str> = outcome
        .push
        .branch_to_remote
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();
    Ok(json!({ "pushed": pushed }))
}

/// What pulling would do to each stack, or with `check` off, do it: fetch, then rebase every
/// stack onto the target, and report each branch's state after. Records an undo snapshot, so
/// `but undo` reverts it.
#[cfg(feature = "legacy")]
fn pull(ctx: &mut Context, check: bool) -> anyhow::Result<serde_json::Value> {
    use crate::command::legacy::upstream;

    let branches = |statuses: &[upstream::BranchStatusInfo]| -> Vec<serde_json::Value> {
        statuses
            .iter()
            .map(|status| json!({ "name": status.name, "status": status.status.as_str() }))
            .collect()
    };
    if !check {
        fetch(ctx)?;
    }
    let mut guard = ctx.exclusive_worktree_access();
    let perm = guard.write_permission();
    let preview = upstream::dry_run_integration_with_perm(ctx, perm)?;
    if check {
        return Ok(json!({
            "branches": branches(&preview.statuses),
            "worktreeConflicts": preview.outcome.worktree_conflicts,
        }));
    }
    let updates = but_api::workspace::rebase_stack_bottoms(&preview.current);
    let outcome = but_api::workspace::workspace_integrate_upstream_with_perm(
        ctx,
        updates,
        but_core::DryRun::No,
        perm,
    )?;
    let after = upstream::classify(&preview.current, &outcome.workspace_state);
    // The rebase moved the workspace head under this context's cached repository.
    ctx.reload_repo_and_invalidate_workspace(perm)?;
    Ok(json!({ "branches": branches(&after) }))
}

#[cfg(not(feature = "legacy"))]
fn push(_ctx: &mut Context, _branch: &str, _force: bool) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("Pushing needs a build of `but` with the `legacy` feature")
}

#[cfg(not(feature = "legacy"))]
fn pull(_ctx: &mut Context, _check: bool) -> anyhow::Result<serde_json::Value> {
    anyhow::bail!("Pulling needs a build of `but` with the `legacy` feature")
}

/// Wait for `future` on the runtime this command runs in. Requests are handled on its thread, so
/// the runtime must be told this thread is blocking.
#[cfg(feature = "legacy")]
fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
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
    /// This machine's address the request arrived on, unless that is loopback. A page opened on
    /// another device names the server by it.
    arrived_at: Option<IpAddr>,
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
        arrived_at: None,
    })
}

enum Response {
    Page {
        content_type: &'static str,
        body: &'static str,
    },
    Json(serde_json::Value),
    /// A web app manifest, which browsers want served as its own type.
    Manifest(serde_json::Value),
    Error {
        status: &'static str,
    },
}

#[derive(Debug, PartialEq)]
enum Route {
    Index,
    Script,
    /// The icon the browser tab and an installed app show.
    Icon,
    /// The script the browser keeps to show the page while the server is down.
    ServiceWorker,
    /// The web app manifest, for installing the page as an app of its own.
    Manifest,
    /// Lets a second `but panel` recognise a running panel.
    Ping,
    /// `but panel --stop` asks the server to exit.
    Stop,
    /// How other devices reach the server, for `but panel` on this machine to print.
    Network,
    /// The projects the page can switch between.
    Projects,
    /// Open the repository at `path` and list it among the projects, remembering it as a GitButler
    /// project where possible.
    AddProject {
        path: String,
    },
    /// Change how often the app and the panel fetch on their own.
    Settings {
        auto_fetch_minutes: isize,
    },
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
    /// The programs that can open `path`, or without one, the editors that open several files.
    Programs {
        path: Option<String>,
    },
    /// What can open the project's directory.
    FolderOpeners,
    /// Open `paths`, relative to `worktree` or the main worktree, with `program`. A `POST`, like
    /// everything that launches a program.
    Open {
        paths: Vec<String>,
        program: String,
        worktree: Option<String>,
    },
    /// Open the project's directory with `with`.
    OpenFolder {
        with: FolderOpener,
    },
    /// Open a page on the project's forge in the default browser.
    OpenForgePage {
        url: String,
    },
    /// The target's commits a pull would bring in.
    Upstream,
    /// Fetch from every remote. A `POST`, like everything that reaches the network.
    Fetch,
    /// Push `branch` and the branches below it in its stack, with force when asked.
    Push {
        branch: String,
        force: bool,
    },
    /// Rebase every stack onto the target, or with `check`, only say what that would do. The one
    /// route that changes the workspace.
    Pull {
        check: bool,
    },
    Error {
        status: &'static str,
    },
}

fn route(request: &Request, port: u16) -> Route {
    // Only the page itself may call in. Checking `Host` stops another site from reaching this
    // server by pointing its own domain at it (DNS rebinding). The page names the server by
    // localhost, or on another device by the address its request arrived on.
    let names_this_server = |authority: &str| {
        authority == format!("localhost:{port}")
            || authority == format!("127.0.0.1:{port}")
            || request
                .arrived_at
                .is_some_and(|address| authority == SocketAddr::new(address, port).to_string())
    };
    let host_is_local = request.host.as_deref().is_some_and(names_this_server);
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

    // Everything that only reads is a `GET`. Launching a program, reaching the network or changing
    // the workspace is a `POST` from the page itself: another site open in the browser can send a
    // `POST` here, but not with this server's own origin.
    let is_action = matches!(
        path,
        "/api/open"
            | "/api/open-folder"
            | "/api/open-url"
            | "/api/fetch"
            | "/api/push"
            | "/api/pull"
            | "/api/settings"
            | "/api/stop"
    ) || (path == "/api/projects" && request.method == "POST");
    let origin_is_local = request
        .origin
        .as_deref()
        .and_then(|origin| origin.strip_prefix("http://"))
        .is_some_and(names_this_server);
    match (request.method.as_str(), is_action) {
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
        "/icon.svg" => Route::Icon,
        "/sw.js" => Route::ServiceWorker,
        "/manifest.webmanifest" => Route::Manifest,
        "/api/ping" => Route::Ping,
        "/api/stop" => Route::Stop,
        "/api/network" => Route::Network,
        "/api/projects" if is_action => match param("path") {
            Some(path) => Route::AddProject { path },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/projects" => Route::Projects,
        "/api/settings" => match param("autoFetchMinutes").and_then(|value| value.parse().ok()) {
            Some(auto_fetch_minutes) => Route::Settings { auto_fetch_minutes },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
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
        "/api/programs" => Route::Programs {
            path: param("path"),
        },
        "/api/folder-openers" => Route::FolderOpeners,
        "/api/open-folder" => Route::OpenFolder {
            with: match (param("program"), param("terminal")) {
                (Some(id), _) => FolderOpener::Program(id),
                (None, Some(id)) => FolderOpener::Terminal(id),
                (None, None) => FolderOpener::FileManager,
            },
        },
        "/api/open-url" => match param("url") {
            Some(url) => Route::OpenForgePage { url },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/upstream" => Route::Upstream,
        "/api/fetch" => Route::Fetch,
        "/api/push" => match param("branch") {
            Some(branch) => Route::Push {
                branch,
                force: param("force").is_some(),
            },
            None => Route::Error {
                status: "400 Bad Request",
            },
        },
        "/api/pull" => Route::Pull {
            check: param("check").is_some(),
        },
        "/api/open" => {
            let paths: Vec<String> = params
                .iter()
                .filter(|(key, value)| key == "path" && !value.is_empty())
                .map(|(_, value)| value.clone())
                .collect();
            match param("program") {
                Some(program)
                    if !paths.is_empty() && paths.iter().all(|p| is_inside_checkout(p)) =>
                {
                    Route::Open {
                        paths,
                        program,
                        worktree: param("worktree"),
                    }
                }
                _ => Route::Error {
                    status: "400 Bad Request",
                },
            }
        }
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
        Response::Manifest(value) => (
            "200 OK",
            "application/manifest+json",
            serde_json::to_string(&value).context("serializing the panel manifest")?,
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
            arrived_at: None,
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
        assert_eq!(route(&get("/icon.svg", LOCAL), 7789), Route::Icon);
        assert_eq!(route(&get("/sw.js", LOCAL), 7789), Route::ServiceWorker);
        assert_eq!(
            route(&get("/manifest.webmanifest?project=%2Fa", LOCAL), 7789),
            Route::Manifest,
            "the manifest names the project the page carries"
        );
        assert_eq!(route(&get("/api/workspace", LOCAL), 7789), Route::Workspace);
        assert_eq!(route(&get("/api/ping", LOCAL), 7789), Route::Ping);
        assert_eq!(
            route(&post("/api/stop", Some("http://127.0.0.1:7789")), 7789),
            Route::Stop,
            "`but panel --stop` asks as the page would"
        );
        assert_eq!(
            route(&post("/api/stop", Some("https://evil.example")), 7789),
            Route::Error {
                status: "403 Forbidden"
            },
            "another site in the browser can't stop the panel"
        );
        assert_eq!(
            route(&get("/api/stop", LOCAL), 7789),
            Route::Error {
                status: "405 Method Not Allowed"
            },
            "following a link can't stop the panel"
        );
        assert_eq!(route(&get("/api/projects", LOCAL), 7789), Route::Projects);
        assert_eq!(
            route(
                &post(
                    "/api/open-url?url=https%3A%2F%2Fgithub.com%2Fo%2Fr%2Fpull%2F1",
                    Some("http://localhost:7789")
                ),
                7789
            ),
            Route::OpenForgePage {
                url: "https://github.com/o/r/pull/1".into()
            }
        );
        assert_eq!(
            route(
                &get("/api/open-url?url=https%3A%2F%2Fgithub.com", LOCAL),
                7789
            ),
            Route::Error {
                status: "405 Method Not Allowed"
            },
            "following a link can't open the browser"
        );
        assert_eq!(route(&get("/api/upstream", LOCAL), 7789), Route::Upstream);
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
    fn names_the_project_in_the_manifest() {
        let manifest = manifest_json(Some(PathBuf::from("/Users/me/My Repo/fliege")));
        assert_eq!(manifest["name"], "fliege · GitButler");
        assert_eq!(
            manifest["start_url"], "/?project=/Users/me/My%20Repo/fliege",
            "an installed app opens on the project it was installed from"
        );
        assert_eq!(
            manifest["id"], manifest["start_url"],
            "each project is an app of its own"
        );
        assert_eq!(manifest_json(None)["start_url"], "/");
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
                paths: vec!["src/a.rs".into()],
                program: "vscode".into(),
                worktree: None,
            },
        );
        assert_eq!(
            route(
                &post("/api/open?path=a.rs&path=b%2Fc.rs&program=vscode", PAGE),
                7789
            ),
            Route::Open {
                paths: vec!["a.rs".into(), "b/c.rs".into()],
                program: "vscode".into(),
                worktree: None,
            },
            "a commit's files open together, in the order listed"
        );
        assert_eq!(
            route(&post("/api/open?program=vscode", PAGE), 7789),
            Route::Error {
                status: "400 Bad Request"
            },
            "there must be something to open"
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
            assert_eq!(
                route(
                    &post(
                        &format!("/api/open?path=a.rs&path={escape}&program=vscode"),
                        PAGE
                    ),
                    7789
                ),
                Route::Error {
                    status: "400 Bad Request"
                },
                "{escape} would leave the checkout, even alongside a safe path"
            );
        }
    }

    #[test]
    fn opens_the_folder_and_fetches_only_when_the_page_asks() {
        const PAGE: Option<&str> = Some("http://localhost:7789");
        assert_eq!(
            route(&post("/api/open-folder", PAGE), 7789),
            Route::OpenFolder {
                with: FolderOpener::FileManager
            },
            "without a program or terminal, the file manager shows the folder"
        );
        assert_eq!(
            route(&post("/api/open-folder?terminal=iterm2", PAGE), 7789),
            Route::OpenFolder {
                with: FolderOpener::Terminal("iterm2".into())
            },
        );
        assert_eq!(
            route(&post("/api/open-folder?program=vscode", PAGE), 7789),
            Route::OpenFolder {
                with: FolderOpener::Program("vscode".into())
            },
        );
        assert_eq!(route(&post("/api/fetch", PAGE), 7789), Route::Fetch);
        assert_eq!(
            route(&post("/api/push?branch=refs%2Fheads%2Ffeat", PAGE), 7789),
            Route::Push {
                branch: "refs/heads/feat".into(),
                force: false,
            },
        );
        assert_eq!(
            route(
                &post("/api/push?branch=refs%2Fheads%2Ffeat&force=1", PAGE),
                7789
            ),
            Route::Push {
                branch: "refs/heads/feat".into(),
                force: true,
            },
            "force is asked for explicitly"
        );
        assert_eq!(
            route(&post("/api/push", PAGE), 7789),
            Route::Error {
                status: "400 Bad Request"
            },
            "a push needs a branch"
        );
        assert_eq!(
            route(&post("/api/pull?check=1", PAGE), 7789),
            Route::Pull { check: true },
            "a check only previews"
        );
        assert_eq!(
            route(&post("/api/pull", PAGE), 7789),
            Route::Pull { check: false }
        );
        assert_eq!(
            route(&post("/api/projects?path=%2FUsers%2Fme%2Frepo", PAGE), 7789),
            Route::AddProject {
                path: "/Users/me/repo".into()
            },
            "a POST to the project list adds one; a GET lists them"
        );
        assert_eq!(
            route(&post("/api/projects", PAGE), 7789),
            Route::Error {
                status: "400 Bad Request"
            },
            "adding needs a path"
        );
        assert_eq!(
            route(&post("/api/settings?autoFetchMinutes=-1", PAGE), 7789),
            Route::Settings {
                auto_fetch_minutes: -1
            },
            "a negative interval turns auto-fetching off"
        );
        assert_eq!(
            route(&post("/api/settings?autoFetchMinutes=soon", PAGE), 7789),
            Route::Error {
                status: "400 Bad Request"
            },
        );
        assert_eq!(
            route(&get("/api/folder-openers", LOCAL), 7789),
            Route::FolderOpeners,
            "listing what can open the folder only reads"
        );
        for target in [
            "/api/open-folder",
            "/api/fetch",
            "/api/push?branch=refs%2Fheads%2Ffeat",
            "/api/pull",
            "/api/settings?autoFetchMinutes=5",
        ] {
            assert_eq!(
                route(&post(target, Some("https://evil.example")), 7789),
                Route::Error {
                    status: "403 Forbidden"
                },
                "another site in the browser can't {target}"
            );
            assert_eq!(
                route(&get(target, LOCAL), 7789),
                Route::Error {
                    status: "405 Method Not Allowed"
                },
                "a GET never launches anything or reaches the network: {target}"
            );
        }
    }

    #[test]
    fn lists_programs_for_a_file_or_editors_for_many() {
        assert_eq!(
            route(&get("/api/programs?path=src%2Fa.rs", LOCAL), 7789),
            Route::Programs {
                path: Some("src/a.rs".into())
            }
        );
        assert_eq!(
            route(&get("/api/programs", LOCAL), 7789),
            Route::Programs { path: None },
            "without a file, the editors that open a commit's files together"
        );
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

    #[test]
    fn opens_only_pages_on_the_projects_forge() {
        let forge = "https://github.com/gitbutlerapp/gitbutler";
        assert!(is_on_forge(
            "https://github.com/gitbutlerapp/gitbutler/pull/1",
            forge
        ));
        assert!(
            !is_on_forge("https://github.com.evil.example/pull/1", forge),
            "a host that only starts like the forge's is another site"
        );
        assert!(!is_on_forge("https://evil.example/github.com", forge));
        assert!(
            !is_on_forge("file:///etc/passwd", forge),
            "only web pages are opened"
        );
        assert!(!is_on_forge("not a url", forge));
    }

    #[test]
    fn a_page_on_another_device_names_the_server_by_its_network_address() {
        let address: IpAddr = "192.168.1.5".parse().unwrap();
        let on_network = |mut request: Request| {
            request.arrived_at = Some(address);
            request
        };
        assert_eq!(
            route(
                &on_network(get("/api/ping", Some("192.168.1.5:7789"))),
                7789
            ),
            Route::Ping
        );
        assert_eq!(
            route(
                &on_network(Request {
                    host: Some("192.168.1.5:7789".into()),
                    ..post("/api/fetch", Some("http://192.168.1.5:7789"))
                }),
                7789
            ),
            Route::Fetch
        );
        assert_eq!(
            route(
                &on_network(get("/api/ping", Some("evil.example:7789"))),
                7789
            ),
            Route::Error {
                status: "403 Forbidden"
            },
            "a foreign host rebound to the network address is refused too"
        );
        assert_eq!(
            route(&get("/api/ping", Some("192.168.1.5:7789")), 7789),
            Route::Error {
                status: "403 Forbidden"
            },
            "over loopback the server only goes by localhost"
        );
    }

    #[test]
    fn another_device_needs_the_token() {
        let network = Network {
            address: "192.168.1.5".parse().unwrap(),
            token: "s3cret".into(),
        };
        assert!(has_token(
            &get("/api/workspace?project=%2Fa&token=s3cret", LOCAL),
            Some(&network)
        ));
        assert!(!has_token(
            &get("/api/workspace?token=guess", LOCAL),
            Some(&network)
        ));
        assert!(!has_token(&get("/api/workspace", LOCAL), Some(&network)));
        assert!(
            !has_token(&get("/api/workspace?token=", LOCAL), None),
            "a server on this machine alone has no token to match"
        );
        assert_eq!(
            network.project_url(7789, Path::new("/Users/me/fliege")),
            "http://192.168.1.5:7789/?project=/Users/me/fliege&token=s3cret"
        );
    }
}
