//! GitButler's mesh: this machine's connection to the user's other machines, and what they
//! have of a project. It's the `but-mesh` program's, which keeps a daemon reporting this
//! machine's repositories, so everything here runs it and reads its JSON.

use std::ffi::OsStr;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;

use super::CommitId;

/// How long a read of the mesh may take. The CLI waits on the server and the daemon, and a
/// daemon that stopped answering leaves it waiting for minutes.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// How long bringing a branch's commits over may take: a big branch on a slow network, but
/// not so long that a stuck fetch holds the next one for long.
const FETCH_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// How long signing in waits for the browser, as the CLI itself does.
const LOGIN_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// This machine's connection, as `but mesh service status` reports it.
#[derive(Debug, Clone, PartialEq)]
pub struct Connection {
    /// Who it's signed in as.
    pub login: String,
    /// What the mesh calls this machine.
    pub machine: String,
    /// The mesh server, which but.dev links point at.
    pub server: String,
    /// Whether the daemon runs, reporting this machine's work.
    pub daemon_running: bool,
    /// The repositories the daemon reports, by path.
    pub reported: Vec<PathBuf>,
}

/// A project's checkout on another machine: a worktree, with what's unfinished in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkout {
    /// The mesh's id for it, which but.dev links use.
    pub id: String,
    pub machine: String,
    pub online: bool,
    pub path: String,
    /// A linked worktree's name; `None` for the repository's main one.
    pub worktree: Option<String>,
    /// The checkout's folder, when the machine has another checkout of the project with no
    /// worktree name to tell the two apart.
    pub folder: Option<String>,
    pub branches: Vec<Branch>,
    pub uncommitted: Vec<File>,
    /// When it last changed, in seconds since the epoch.
    pub last_activity: Option<i64>,
    /// Why the mesh couldn't read it all, when it couldn't.
    pub error: Option<String>,
}

impl Checkout {
    /// "macbook", or "macbook · feature" for a linked worktree or another folder.
    pub fn label(&self) -> String {
        match self.worktree.as_ref().or(self.folder.as_ref()) {
            Some(place) => format!("{} · {place}", self.machine),
            None => self.machine.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub name: String,
    /// What `but mesh apply` takes, when the mesh offers this branch to bring here.
    pub mesh_id: Option<String>,
    pub status: BranchStatus,
    /// Its newest commit, as that machine last reported it.
    pub tip: Option<String>,
    /// Newest first.
    pub commits: Vec<Commit>,
}

/// What a branch's glyph tells apart.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchStatus {
    /// Merged into the target.
    Integrated,
    /// Rewritten since it was pushed, so pushing it needs force.
    Diverged,
    /// Pushed, with commits only on that machine, or empty.
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub id: CommitId,
    pub title: String,
    /// In seconds since the epoch.
    pub created_at: Option<i64>,
    pub pushed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct File {
    pub path: String,
    pub additions: usize,
    pub deletions: usize,
}

/// The `but-mesh` program: on the `PATH`, or where its installer puts it, as an app started
/// from the Dock gets a `PATH` without it.
fn program() -> Result<PathBuf> {
    let on_path = std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>());
    let installed = std::env::var_os("HOME").map(|home| Path::new(&home).join(".local/bin"));
    on_path
        .chain(installed)
        .map(|dir| dir.join(format!("but-mesh{}", std::env::consts::EXE_SUFFIX)))
        .find(|program| program.is_file())
        .context("GitButler's mesh isn't installed on this machine.")
}

/// Run `but-mesh` with `args` in `dir`, waiting at most `timeout`, and return what it
/// printed. A failure carries what it said on stderr.
fn mesh(dir: Option<&Path>, args: &[impl AsRef<OsStr>], timeout: Duration) -> Result<String> {
    let mut command = Command::new(program()?);
    // `CREATE_NO_WINDOW`: on Windows a console program started from a window would open a
    // console of its own.
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    if let Some(dir) = dir {
        command.arg("-C").arg(dir);
    }
    let mut child = command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Couldn't start GitButler's mesh")?;
    // Read as it's written: a full pipe would stop it while this waits for it to end.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                pipe.read_to_string(&mut text).ok();
            }
            text
        })
    };
    let stdout = drain(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = drain(child.stderr.take().map(|p| Box::new(p) as _));
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if started.elapsed() > timeout {
            child.kill().ok();
            child.wait().ok();
            bail!("GitButler's mesh didn't answer in time");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let (stdout, stderr) = (
        stdout.join().unwrap_or_default(),
        stderr.join().unwrap_or_default(),
    );
    if !status.success() {
        let said = stderr.trim().trim_start_matches("Error: ");
        bail!(
            "{}",
            if said.is_empty() {
                "GitButler's mesh failed"
            } else {
                said
            }
        );
    }
    Ok(stdout)
}

/// What the CLI says when this machine isn't connected, which isn't a failure to show.
fn not_connected(err: &anyhow::Error) -> bool {
    let text = format!("{err:#}");
    ["not in a mesh", "not signed in", "run `but mesh login`"]
        .iter()
        .any(|said| text.contains(said))
}

/// This machine's connection, or `None` when it isn't in a mesh.
pub fn connection() -> Result<Option<Connection>> {
    #[derive(Deserialize)]
    struct Status {
        login: Option<String>,
        server: Option<String>,
        daemon: Option<String>,
        node: Option<Node>,
        #[serde(default)]
        checkouts: Vec<Reported>,
    }
    #[derive(Deserialize)]
    struct Node {
        name: String,
    }
    #[derive(Deserialize)]
    struct Reported {
        path: PathBuf,
    }
    let out = match mesh(None, &["service", "status", "--json"], READ_TIMEOUT) {
        Ok(out) => out,
        Err(err) if not_connected(&err) => return Ok(None),
        Err(err) => return Err(err),
    };
    let status: Status = serde_json::from_str(&out).context("Couldn't read the mesh's status")?;
    let (Some(login), Some(node)) = (status.login, status.node) else {
        return Ok(None);
    };
    Ok(Some(Connection {
        login,
        machine: node.name,
        server: status
            .server
            .unwrap_or_else(|| "https://but.dev".to_owned()),
        daemon_running: status.daemon.as_deref() == Some("running"),
        reported: status.checkouts.into_iter().map(|c| c.path).collect(),
    }))
}

/// What the mesh has of the repository at `path`.
#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    /// The project, once the mesh has heard of it.
    pub project: Option<Project>,
    /// Its checkouts on the user's other machines, as the mesh last heard of them: the work
    /// this machine doesn't have already.
    pub checkouts: Vec<Checkout>,
}

/// A project as the mesh knows it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Project {
    pub id: String,
    /// The icon picked for it on but.dev or here, as but.dev stores it: an emoji, or
    /// `glyph:<colour>`; empty for none.
    #[serde(default)]
    pub icon: String,
}

/// What the mesh has of the repository at `path`, kept for [`cached_status`] under
/// `project`, the project's id here.
pub fn status(path: &Path, project: &str) -> Result<Status> {
    let out = mesh(Some(path), &["status", "--json"], READ_TIMEOUT)?;
    let status = parse_status(&out)?;
    if let Some(cache) = cache_path(project)
        && std::fs::read_to_string(&cache).ok().as_deref() != Some(out.as_str())
    {
        let written = (cache.parent().map_or(Ok(()), std::fs::create_dir_all))
            .and_then(|()| std::fs::write(&cache, &out));
        if let Err(err) = written {
            tracing::debug!(project, "couldn't keep the mesh's read: {err}");
        }
    }
    Ok(status)
}

/// What [`status`] read of `project` last time, to show until the mesh answers again.
pub fn cached_status(project: &str) -> Option<Status> {
    parse_status(&std::fs::read_to_string(cache_path(project)?).ok()?).ok()
}

fn cache_path(project: &str) -> Option<PathBuf> {
    Some(
        but_path::app_cache_dir()
            .ok()?
            .join("mesh")
            .join(format!("{project}.json")),
    )
}

/// [`status()`] from what `but mesh status --json` printed.
fn parse_status(out: &str) -> Result<Status> {
    #[derive(Deserialize)]
    struct StatusJson {
        project: Option<Project>,
        checkouts: Vec<Row>,
        #[serde(default)]
        branches: Vec<Offered>,
    }
    #[derive(Deserialize)]
    struct Row {
        machine: String,
        this_machine: bool,
        online: bool,
        #[serde(default)]
        same_work_as_here: bool,
        same_as: Option<serde_json::Value>,
        checkout: CheckoutJson,
    }
    #[derive(Deserialize)]
    struct CheckoutJson {
        id: String,
        path: String,
        worktree_name: Option<String>,
        #[serde(default)]
        stacks: Vec<Stack>,
        branch: Option<BranchJson>,
        #[serde(default)]
        uncommitted: Vec<FileJson>,
        last_activity_at: Option<i64>,
        error: Option<String>,
    }
    #[derive(Deserialize)]
    struct Stack {
        branches: Vec<BranchJson>,
    }
    #[derive(Deserialize)]
    struct BranchJson {
        name: String,
        #[serde(default)]
        commits: Vec<CommitJson>,
        status: String,
        tip: Option<String>,
    }
    #[derive(Deserialize)]
    struct CommitJson {
        id: String,
        title: String,
        created_at: Option<i64>,
        pushed: bool,
    }
    #[derive(Deserialize)]
    struct FileJson {
        path: String,
        additions: Option<usize>,
        deletions: Option<usize>,
    }
    #[derive(Deserialize)]
    struct Offered {
        id: String,
        name: String,
        checkout_id: String,
    }

    let status: StatusJson =
        serde_json::from_str(out).context("Couldn't read the mesh's status")?;
    let offered = &status.branches;
    // Another machine's, and drawn here: work identical to this machine's, or to another
    // checkout listed, shows once.
    let rows = (status.checkouts.into_iter())
        .filter(|row| !row.this_machine && !row.same_work_as_here && row.same_as.is_none());
    let mut checkouts: Vec<Checkout> = rows
        .map(|row| {
            let checkout = row.checkout;
            let branches = (checkout.stacks.into_iter())
                .flat_map(|stack| stack.branches)
                .chain(checkout.branch)
                .map(|branch| {
                    let mesh_branch = offered
                        .iter()
                        .find(|b| b.checkout_id == checkout.id && b.name == branch.name);
                    Branch {
                        mesh_id: mesh_branch.map(|b| b.id.clone()),
                        status: match branch.status.as_str() {
                            "integrated" => BranchStatus::Integrated,
                            "unpushedCommitsRequiringForce" => BranchStatus::Diverged,
                            _ => BranchStatus::Other,
                        },
                        // A commit is shown and fetched by its id, so one without a valid
                        // id is left out.
                        commits: (branch.commits.into_iter())
                            .filter_map(|commit| {
                                Some(Commit {
                                    id: CommitId(
                                        gix::ObjectId::from_hex(commit.id.as_bytes()).ok()?,
                                    ),
                                    title: commit.title,
                                    created_at: commit.created_at.map(|ms| ms / 1000),
                                    pushed: commit.pushed,
                                })
                            })
                            .collect(),
                        tip: branch.tip,
                        name: branch.name,
                    }
                })
                .collect();
            Checkout {
                id: checkout.id,
                machine: row.machine,
                online: row.online,
                path: checkout.path,
                worktree: checkout.worktree_name,
                folder: None,
                branches,
                uncommitted: (checkout.uncommitted.into_iter())
                    .map(|file| File {
                        path: file.path,
                        additions: file.additions.unwrap_or_default(),
                        deletions: file.deletions.unwrap_or_default(),
                    })
                    .collect(),
                last_activity: checkout.last_activity_at.map(|ms| ms / 1000),
                error: checkout.error,
            }
        })
        .collect();
    // A checkout with nothing going on, no uncommitted files and no branch with commits that
    // haven't landed, isn't listed, nor is a machine with only those.
    checkouts.retain(|c| {
        !c.uncommitted.is_empty()
            || (c.branches.iter())
                .any(|b| !b.commits.is_empty() && b.status != BranchStatus::Integrated)
    });
    // Two main checkouts on one machine, as two clones, are told apart by their folders.
    let twins: Vec<usize> = (checkouts.iter().enumerate())
        .filter(|&(ix, c)| {
            c.worktree.is_none()
                && (checkouts.iter().enumerate())
                    .any(|(other, o)| other != ix && o.machine == c.machine && o.worktree.is_none())
        })
        .map(|(ix, _)| ix)
        .collect();
    for ix in twins {
        let Some(c) = checkouts.get_mut(ix) else {
            continue;
        };
        let folder = Path::new(&c.path).file_name();
        c.folder = folder.map(|name| name.to_string_lossy().into_owned());
    }
    Ok(Status {
        project: status.project,
        checkouts,
    })
}

/// Give the mesh's project `id` the `icon`, as but.dev stores it, or none when it's empty.
/// The CLI has no command for it, so this asks the server itself, as this machine, with the
/// server and token the CLI keeps in its config.
pub fn set_project_icon(id: &str, icon: &str) -> Result<()> {
    #[derive(Deserialize)]
    struct Config {
        server: String,
        #[serde(default)]
        node_token: String,
    }
    let dir = match std::env::var_os("BUT_MESH_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| Path::new(&home).join(".config")))
            .context("No home folder to find the mesh's config in")?
            .join("but-mesh"),
    };
    let text = std::fs::read_to_string(dir.join("config.toml"))
        .context("This machine isn't signed in to the mesh")?;
    // Its parser's errors can quote the token, so only what went wrong is said.
    let config: Config =
        toml::from_str(&text).map_err(|_| anyhow::anyhow!("Couldn't read the mesh's config"))?;
    if config.node_token.is_empty() {
        bail!("This machine isn't signed in to the mesh");
    }
    // As the CLI: an override of the server for development, else the config's.
    let server = std::env::var("BUT_MESH_SERVER")
        .ok()
        .filter(|server| !server.trim().is_empty())
        .unwrap_or(config.server);
    let url = format!(
        "{}/api/projects/{id}/icon",
        server.trim().trim_end_matches('/')
    );
    let client = reqwest::Client::builder()
        // The token is for the server alone, which never redirects.
        .redirect(reqwest::redirect::Policy::none())
        .timeout(READ_TIMEOUT)
        .build()?;
    let response = super::runtime().block_on(
        client
            .post(url)
            .header("x-but-node-token", config.node_token)
            .json(&serde_json::json!({ "icon": icon }))
            .send(),
    )?;
    if !response.status().is_success() {
        bail!("The mesh refused the icon ({})", response.status());
    }
    Ok(())
}

/// Have the daemon report the repository at `path`, as it does the repositories it found
/// itself. It reports each change within seconds, watching the files.
pub fn report(path: &Path) -> Result<()> {
    mesh(
        None,
        &[OsStr::new("add"), path.as_os_str(), OsStr::new("--json")],
        READ_TIMEOUT,
    )
    .map(|_| ())
}

/// Apply branch `mesh_id` from another machine to the workspace at `path`, under its own name
/// or `<name>-from-<machine>`, once [`fetch()`] has brought its commits here. Returns the name
/// it's applied as.
pub fn apply(path: &Path, mesh_id: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Applied {
        applied_as: Option<String>,
    }
    // It runs in the project's queue: with the commits already here, the CLI only checks with
    // the server, which a read's timeout bounds.
    let out = mesh(Some(path), &["apply", mesh_id, "--json"], READ_TIMEOUT)?;
    let applied: Applied = serde_json::from_str(&out).context("Couldn't read what was applied")?;
    Ok(applied.applied_as.unwrap_or_else(|| mesh_id.to_owned()))
}

/// Have branch `mesh_id` from another machine in the repository at `path`, its tip `reported`
/// as that machine last reported it: already here, as a pushed branch fetched from its remote
/// often is, or copied from the mesh, under a ref of the mesh's own; nothing is checked out or
/// applied. Returns its tip here, which a copy can have moved past `reported`.
pub fn fetch(path: &Path, mesh_id: &str, reported: Option<&str>) -> Result<CommitId> {
    let reported = reported.and_then(|tip| gix::ObjectId::from_hex(tip.as_bytes()).ok());
    if let Some(tip) = reported
        && gix::open(path).is_ok_and(|repo| repo.has_object(tip))
    {
        return Ok(CommitId(tip));
    }
    #[derive(Deserialize)]
    struct Fetched {
        tip: String,
    }
    let out = mesh(Some(path), &["fetch", mesh_id, "--json"], FETCH_TIMEOUT)?;
    let fetched: Fetched = serde_json::from_str(&out).context("Couldn't read what was fetched")?;
    let tip =
        gix::ObjectId::from_hex(fetched.tip.as_bytes()).context("The mesh named no commit")?;
    Ok(CommitId(tip))
}

/// Connect this machine to the mesh: the CLI opens the browser to approve it and waits, then
/// starts the daemon.
pub fn log_in() -> Result<()> {
    mesh(None, &["login", "--json"], LOGIN_TIMEOUT).map(|_| ())
}

/// Stop the daemon and take this machine out of the mesh. Its work stays on disk.
pub fn log_out() -> Result<()> {
    mesh(None, &["off", "--json"], READ_TIMEOUT).map(|_| ())
}

/// The but.dev page of `checkout`, or of its branch `branch`, as `but mesh link` prints it.
pub fn link(server: &str, checkout: &str, branch: Option<&str>) -> String {
    let server = server.trim_end_matches('/');
    match branch {
        // Git allows `#` and `%` in a branch name, which a URL would take as a fragment or an
        // escape.
        Some(branch) => {
            let branch = branch.replace('%', "%25").replace('#', "%23");
            format!("{server}/c/{checkout}/b/{branch}")
        }
        None => format!("{server}/c/{checkout}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{BranchStatus, link, parse_status};

    /// `but mesh status --json`, cut down to what's read: this machine's linked worktree,
    /// work identical to this machine's, and two other machines with what's unfinished.
    const STATUS: &str = r#"{
      "project": {"id": "p1", "name": "repo", "root_commit": "r", "icon": "glyph:pop"},
      "checkouts": [
        {"machine": "here", "this_machine": true, "online": true, "same_work_as_here": false,
         "same_as": null, "checkout": {"id": "c-here", "path": "/w/repo-wt", "worktree_name": "wt",
         "head": "detached", "head_branch": null, "stacks": [], "branch": null, "uncommitted": [],
         "last_activity_at": 1000, "error": null}},
        {"machine": "twin", "this_machine": false, "online": true, "same_work_as_here": true,
         "same_as": null, "checkout": {"id": "c-twin", "path": "/w/repo", "worktree_name": null,
         "head": "workspace", "head_branch": "gitbutler/workspace", "stacks": [], "branch": null,
         "uncommitted": [], "last_activity_at": null, "error": null}},
        {"machine": "studio", "this_machine": false, "online": false, "same_work_as_here": false,
         "same_as": null, "checkout": {"id": "c-studio", "path": "/s/repo", "worktree_name": null,
         "head": "workspace", "head_branch": "gitbutler/workspace",
         "stacks": [{"branches": [{"name": "feat", "status": "unpushedCommits", "review_url": null,
           "commits": [{"id": "b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2", "title": "Second",
                        "author": "Ann", "created_at": 2000000, "pushed": false, "rebased": false},
                       {"id": "not a sha", "title": "Garbled", "created_at": 1500000,
                        "pushed": true, "rebased": false},
                       {"id": "b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1", "title": "First",
                        "author": "Ann", "created_at": 1000000, "pushed": true, "rebased": false}]}]}],
         "branch": null,
         "uncommitted": [{"path": "a.rs", "status": "modified", "additions": 3, "deletions": 1}],
         "last_activity_at": 3000000, "error": null}},
        {"machine": "cloud", "this_machine": false, "online": true, "same_work_as_here": false,
         "same_as": null, "checkout": {"id": "c-cloud", "path": "/home/repo", "worktree_name": null,
         "head": "branch", "head_branch": "claude/fix", "stacks": [],
         "branch": {"name": "claude/fix", "status": "integrated", "review_url": "https://pr/1",
                    "commits": []},
         "uncommitted": [{"path": "b.rs", "status": "added", "additions": 1, "deletions": 0}],
         "last_activity_at": null, "error": null}},
        {"machine": "laptop", "this_machine": false, "online": true, "same_work_as_here": false,
         "same_as": null, "checkout": {"id": "c-idle", "path": "/l/repo", "worktree_name": null,
         "head": "branch", "head_branch": "done", "stacks": [],
         "branch": {"name": "done", "status": "integrated", "review_url": null, "commits": []},
         "uncommitted": [], "last_activity_at": null, "error": null}}
      ],
      "branches": [
        {"id": "abc", "name": "feat", "machine": "studio", "checkout_id": "c-studio", "local": false},
        {"id": "def", "name": "claude/fix", "machine": "cloud", "checkout_id": "c-cloud", "local": true}
      ]
    }"#;

    #[test]
    fn keeps_only_other_machines_work_this_one_lacks() {
        let status = parse_status(STATUS).expect("the sample is valid status JSON");
        let project = status.project.expect("the sample names its project");
        assert_eq!(
            (project.id.as_str(), project.icon.as_str()),
            ("p1", "glyph:pop"),
            "the project's id and icon are read"
        );
        let checkouts = status.checkouts;
        let ids: Vec<_> = checkouts.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            ["c-studio", "c-cloud"],
            "this machine's own worktree, work identical to it, and a checkout with nothing \
             going on are left out"
        );

        let studio = &checkouts[0];
        assert!(!studio.online, "it's offline");
        assert_eq!(
            studio.last_activity,
            Some(3000),
            "milliseconds become seconds"
        );
        assert_eq!(
            studio.uncommitted[0].additions, 3,
            "its uncommitted file is read"
        );
        let feat = &studio.branches[0];
        assert_eq!(
            feat.mesh_id.as_deref(),
            Some("abc"),
            "the id `but mesh apply` takes"
        );
        assert_eq!(
            feat.status,
            BranchStatus::Other,
            "it has a commit only there"
        );
        assert_eq!(
            feat.commits[0].created_at,
            Some(2000),
            "newest first, in seconds"
        );
        let titles: Vec<_> = feat.commits.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            ["Second", "First"],
            "a commit without a valid id is left out"
        );

        let cloud = &checkouts[1];
        assert_eq!(cloud.branches.len(), 1, "its checked-out branch is listed");
        assert_eq!(
            cloud.branches[0].status,
            BranchStatus::Integrated,
            "merged upstream"
        );
    }

    #[test]
    fn links_to_but_dev_as_the_cli_does() {
        assert_eq!(
            link("https://but.dev/", "c1", None),
            "https://but.dev/c/c1",
            "a checkout's page, with no doubled slash"
        );
        assert_eq!(
            link("https://but.dev", "c1", Some("feat")),
            "https://but.dev/c/c1/b/feat",
            "a branch of it"
        );
        assert_eq!(
            link("https://but.dev", "c1", Some("fix/#12-100%")),
            "https://but.dev/c/c1/b/fix/%2312-100%25",
            "a branch whose name has what a URL reads otherwise"
        );
    }
}
