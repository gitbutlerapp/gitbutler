//! GitButler's mesh: this machine's connection to the user's other machines, and what they
//! have of a project. It's the `but-mesh` program's, which keeps a daemon reporting this
//! machine's repositories, so everything here runs it and reads its JSON.

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
    /// The branch checked out, outside a GitButler workspace.
    pub head_branch: Option<String>,
    pub in_workspace: bool,
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
    /// Whether its tip commit is in this repository already.
    pub here: bool,
    pub status: BranchStatus,
    /// Its newest commit, as that machine last reported it.
    pub tip: Option<String>,
    /// Newest first.
    pub commits: Vec<Commit>,
    pub review_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BranchStatus {
    /// Everything pushed.
    Pushed,
    /// Some or all commits only on that machine.
    Unpushed,
    /// Rewritten since it was pushed, so pushing it needs force.
    Diverged,
    /// Merged into the target.
    Integrated,
    /// No upstream and no commits.
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Commit {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
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
        .map(|dir| dir.join("but-mesh"))
        .find(|program| program.is_file())
        .context("GitButler's mesh isn't installed on this machine.")
}

/// Run `but-mesh` with `args` in `dir`, waiting at most `timeout`, and return what it
/// printed. A failure carries what it said on stderr.
fn mesh(dir: Option<&Path>, args: &[&str], timeout: Duration) -> Result<String> {
    let mut command = Command::new(program()?);
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

/// The checkouts of the repository at `path` on the user's other machines, as the mesh last
/// heard of them: the work this machine doesn't have already.
pub fn checkouts(path: &Path) -> Result<Vec<Checkout>> {
    parse_checkouts(&mesh(Some(path), &["status", "--json"], READ_TIMEOUT)?)
}

/// [`checkouts()`] from what `but mesh status --json` printed.
fn parse_checkouts(out: &str) -> Result<Vec<Checkout>> {
    #[derive(Deserialize)]
    struct Status {
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
        head: String,
        head_branch: Option<String>,
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
        review_url: Option<String>,
        tip: Option<String>,
    }
    #[derive(Deserialize)]
    struct CommitJson {
        id: String,
        title: String,
        author: Option<String>,
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
        local: bool,
    }

    let status: Status = serde_json::from_str(out).context("Couldn't read the mesh's status")?;
    let offered = &status.branches;
    // Another machine's, and drawn here: work identical to this machine's, or to another
    // checkout listed, shows once.
    let rows = (status.checkouts.into_iter())
        .filter(|row| !row.this_machine && !row.same_work_as_here && row.same_as.is_none());
    let mut checkouts: Vec<Checkout> = rows
        .map(|row| {
            let checkout = row.checkout;
            let in_workspace = checkout.head == "workspace";
            let branches = (checkout.stacks.into_iter())
                .flat_map(|stack| stack.branches)
                .chain(checkout.branch)
                .map(|branch| {
                    let mesh_branch = offered
                        .iter()
                        .find(|b| b.checkout_id == checkout.id && b.name == branch.name);
                    Branch {
                        mesh_id: mesh_branch.map(|b| b.id.clone()),
                        here: mesh_branch.is_some_and(|b| b.local),
                        status: match branch.status.as_str() {
                            "nothingToPush" => BranchStatus::Pushed,
                            "integrated" => BranchStatus::Integrated,
                            "unpushedCommitsRequiringForce" => BranchStatus::Diverged,
                            "local" => BranchStatus::Empty,
                            _ => BranchStatus::Unpushed,
                        },
                        commits: (branch.commits.into_iter())
                            .map(|commit| Commit {
                                id: commit.id,
                                title: commit.title,
                                author: commit.author,
                                created_at: commit.created_at.map(|ms| ms / 1000),
                                pushed: commit.pushed,
                            })
                            .collect(),
                        review_url: branch.review_url,
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
                head_branch: (!in_workspace).then_some(checkout.head_branch).flatten(),
                in_workspace,
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
    // Two main checkouts on one machine, as two clones, are told apart by their folders.
    let twins: Vec<usize> = (0..checkouts.len())
        .filter(|&ix| {
            let c = &checkouts[ix];
            c.worktree.is_none()
                && (checkouts.iter().enumerate())
                    .any(|(other, o)| other != ix && o.machine == c.machine && o.worktree.is_none())
        })
        .collect();
    for ix in twins {
        let folder = Path::new(&checkouts[ix].path).file_name();
        checkouts[ix].folder = folder.map(|name| name.to_string_lossy().into_owned());
    }
    Ok(checkouts)
}

/// Have the daemon report the repository at `path`, as it does the repositories it found
/// itself. It reports each change within seconds, watching the files.
pub fn report(path: &Path) -> Result<()> {
    mesh(
        None,
        &["add", &path.to_string_lossy(), "--json"],
        READ_TIMEOUT,
    )
    .map(|_| ())
}

/// Bring branch `mesh_id` from another machine and apply it to the workspace at `path`,
/// under its own name or `<name>-from-<machine>`. Returns the name it's applied as.
pub fn apply(path: &Path, mesh_id: &str) -> Result<String> {
    #[derive(Deserialize)]
    struct Applied {
        applied_as: Option<String>,
    }
    // The commits come over the network first, which can take a while.
    let out = mesh(Some(path), &["apply", mesh_id, "--json"], LOGIN_TIMEOUT)?;
    let applied: Applied = serde_json::from_str(&out).unwrap_or(Applied { applied_as: None });
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
    let out = mesh(Some(path), &["fetch", mesh_id, "--json"], LOGIN_TIMEOUT)?;
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
        Some(branch) => format!("{server}/c/{checkout}/b/{branch}"),
        None => format!("{server}/c/{checkout}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{BranchStatus, link, parse_checkouts};

    /// `but mesh status --json`, cut down to what's read: this machine's linked worktree,
    /// work identical to this machine's, and two other machines with what's unfinished.
    const STATUS: &str = r#"{
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
           "commits": [{"id": "b2", "title": "Second", "author": "Ann", "created_at": 2000000,
                        "pushed": false, "rebased": false},
                       {"id": "b1", "title": "First", "author": "Ann", "created_at": 1000000,
                        "pushed": true, "rebased": false}]}]}],
         "branch": null,
         "uncommitted": [{"path": "a.rs", "status": "modified", "additions": 3, "deletions": 1}],
         "last_activity_at": 3000000, "error": null}},
        {"machine": "cloud", "this_machine": false, "online": true, "same_work_as_here": false,
         "same_as": null, "checkout": {"id": "c-cloud", "path": "/home/repo", "worktree_name": null,
         "head": "branch", "head_branch": "claude/fix", "stacks": [],
         "branch": {"name": "claude/fix", "status": "integrated", "review_url": "https://pr/1",
                    "commits": []},
         "uncommitted": [], "last_activity_at": null, "error": null}}
      ],
      "branches": [
        {"id": "abc", "name": "feat", "machine": "studio", "checkout_id": "c-studio", "local": false},
        {"id": "def", "name": "claude/fix", "machine": "cloud", "checkout_id": "c-cloud", "local": true}
      ]
    }"#;

    #[test]
    fn keeps_only_other_machines_work_this_one_lacks() {
        let checkouts = parse_checkouts(STATUS).expect("the sample is valid status JSON");
        let ids: Vec<_> = checkouts.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            ["c-studio", "c-cloud"],
            "this machine's own worktree and work identical to it are left out"
        );

        let studio = &checkouts[0];
        assert!(
            studio.in_workspace && !studio.online,
            "an offline GitButler workspace"
        );
        assert_eq!(studio.head_branch, None, "a workspace isn't on one branch");
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
            BranchStatus::Unpushed,
            "it has a commit only there"
        );
        assert!(!feat.here, "its tip isn't in this repository");
        assert_eq!(
            feat.commits[0].created_at,
            Some(2000),
            "newest first, in seconds"
        );

        let cloud = &checkouts[1];
        assert_eq!(
            cloud.head_branch.as_deref(),
            Some("claude/fix"),
            "a plain branch"
        );
        assert_eq!(cloud.branches.len(), 1, "its checked-out branch is listed");
        assert_eq!(
            cloud.branches[0].status,
            BranchStatus::Integrated,
            "merged upstream"
        );
        assert!(cloud.branches[0].here, "its tip is here already");
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
    }
}
