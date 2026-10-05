//! Keeping branches in step between machines, in the background: the branches this machine
//! published are published again as they change, and branches sent to it are pulled.
//!
//! The mesh settings turn each on or off, so Lite and `but mesh` share them. Whichever starts
//! first acts for a repository; the other leaves it alone while that one runs. Nothing happens
//! without an event: a file or branch changing in a repository with published branches, or a
//! machine sending one.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use but_ctx::ProjectHandleOrLegacyProjectId;
use but_settings::AppSettings;
use gitbutler_filemonitor::{FileMonitorHandle, InternalEvent, WatchMode};
use serde::Serialize;

use super::{
    HOSTED_REFS, HostedEvent, HostedListener, LocalHome, LocalHomes, SyncOutcome, git,
    has_uncommitted, hosted_project, local_home, machine_name, rev, snapshot_head, synced_ref,
    worktree_tree,
};

/// Something the follower did, or chose not to, for the person to hear about.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct FollowEvent {
    /// What kind of thing it was.
    pub kind: FollowKind,
    /// What happened, in a sentence that names the repository and branch.
    pub message: String,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FollowEvent);

/// The kinds of [`FollowEvent`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub enum FollowKind {
    /// A branch this machine published changed, and was published again.
    Published,
    /// Another machine sent a branch, and it was pulled here.
    Pulled,
    /// Another machine sent a branch, but pulling it would replace local work.
    Skipped,
    /// Something failed this time; it's tried again on the next change.
    Failed,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FollowKind);

fn event(kind: FollowKind, message: String) -> FollowEvent {
    FollowEvent { kind, message }
}

enum Signal {
    Sent(crate::watcher::WatcherHostedSentPayload),
    /// Reconnected, so anything may have been sent meanwhile.
    Reconnected,
    Files(InternalEvent),
    Stop,
}

/// How often the projects are looked at again, to notice one with a first published branch.
const RESCAN: Duration = Duration::from_secs(30);

/// Follows in the background, holding a connection to the hosted server, until dropped.
pub struct Follower {
    _listener: HostedListener,
    signals: mpsc::Sender<Signal>,
}

impl Drop for Follower {
    fn drop(&mut self) {
        self.signals.send(Signal::Stop).ok();
    }
}

/// Start following, telling `on_event` what was done.
pub fn follow(on_event: impl Fn(FollowEvent) + Send + 'static) -> Result<Follower> {
    let (signals, received) = mpsc::channel();
    let hosted = signals.clone();
    let listener = super::listen_account(move |event| {
        let signal = match event {
            HostedEvent::Sent(sent) => Signal::Sent(sent),
            HostedEvent::Published(None) => Signal::Reconnected,
            HostedEvent::Published(Some(_)) | HostedEvent::Online(_) => return,
        };
        hosted.send(signal).ok();
    })?;
    // File monitors hand their events to a tokio channel, and run their work on a runtime.
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .thread_name("hosted-follower-files")
        .enable_all()
        .build()?;
    let (files, mut file_events) = tokio::sync::mpsc::unbounded_channel();
    let forward = signals.clone();
    runtime.spawn(async move {
        while let Some(event) = file_events.recv().await {
            if forward.send(Signal::Files(event)).is_err() {
                break;
            }
        }
    });
    std::thread::Builder::new()
        .name("hosted-follower".into())
        .spawn(move || {
            Run {
                runtime,
                files,
                locks: HashMap::new(),
                watched: HashMap::new(),
                changed: HashMap::new(),
                rescanned: None,
                // Whatever was sent while nothing was following.
                inbox_due: true,
                reported: HashSet::new(),
            }
            .run(&received, &on_event)
        })?;
    Ok(Follower {
        _listener: listener,
        signals,
    })
}

struct Run {
    runtime: tokio::runtime::Runtime,
    files: tokio::sync::mpsc::UnboundedSender<InternalEvent>,
    /// Each repository's lock, held while this follower acts for it; `None` where another does.
    locks: HashMap<PathBuf, Option<File>>,
    /// Repositories with branches this machine published, by project id: where each is, and the
    /// file monitors on it and on the worktrees of those branches, by path.
    watched: HashMap<String, (PathBuf, HashMap<PathBuf, FileMonitorHandle>)>,
    /// Repositories whose files or branches changed, and when they last did.
    changed: HashMap<PathBuf, Instant>,
    rescanned: Option<Instant>,
    /// Whether to look for sends that arrived while nothing was listening.
    inbox_due: bool,
    /// Sends already reported as not pulled, by snapshot, so each is reported once.
    reported: HashSet<String>,
}

impl Run {
    fn run(mut self, received: &mpsc::Receiver<Signal>, on_event: &dyn Fn(FollowEvent)) {
        let mut mesh = AppSettings::default().mesh;
        loop {
            // Asleep until something happens, or a change has settled.
            let settle = Duration::from_secs(mesh.publish_after_sec);
            let rescan_in = RESCAN.saturating_sub(self.rescanned.map_or(RESCAN, |at| at.elapsed()));
            let wait = self
                .changed
                .values()
                .map(|at| settle.saturating_sub(at.elapsed()))
                .chain([rescan_in])
                .min()
                .unwrap_or(RESCAN);
            let sent = match received.recv_timeout(wait) {
                Ok(Signal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Signal::Sent(sent)) => Some(sent),
                Ok(Signal::Reconnected) => {
                    self.inbox_due = true;
                    None
                }
                Ok(Signal::Files(event)) => {
                    self.files_changed(event);
                    None
                }
                Err(mpsc::RecvTimeoutError::Timeout) => None,
            };
            if let Ok(settings) =
                AppSettings::load_from_default_path_creating_without_customization()
            {
                mesh = settings.mesh;
            }
            if self.rescanned.is_none_or(|at| at.elapsed() >= RESCAN) {
                self.rescan();
            }

            let settle = Duration::from_secs(mesh.publish_after_sec);
            let settled: Vec<PathBuf> = self
                .changed
                .iter()
                .filter(|(_, at)| at.elapsed() >= settle)
                .map(|(dir, _)| dir.clone())
                .collect();
            for dir in settled {
                self.changed.remove(&dir);
                if mesh.auto_publish {
                    self.publish_changed(&dir, on_event);
                }
            }

            if !mesh.auto_pull {
                continue;
            }
            if let Some(sent) = sent {
                self.pull_sent(&sent.root, &sent.from, &sent.branch, on_event);
            }
            if self.inbox_due {
                self.inbox_due = false;
                self.pull_inbox(on_event);
            }
        }
    }

    /// Note a change in a watched repository, if it can change what a published branch is: its
    /// files, or its branches. The follower's own writes, under `refs/gitbutler`, can't.
    fn files_changed(&mut self, event: InternalEvent) {
        let (project, relevant) = match event {
            InternalEvent::ProjectFilesChange(project, _) => (project, true),
            InternalEvent::GitFilesChange(project, paths) => {
                let relevant = paths.iter().any(|path| {
                    path.to_str().is_some_and(|path| {
                        (path.starts_with(gitbutler_filemonitor::LOCAL_REFS_DIR)
                            && !path.ends_with(".lock"))
                            || path == gitbutler_filemonitor::HEAD
                            || path == "packed-refs"
                    })
                });
                (project, relevant)
            }
        };
        if relevant && let Some((dir, _)) = self.watched.get(&project.to_string()) {
            self.changed.insert(dir.clone(), Instant::now());
        }
    }

    /// Watch the repositories with branches this machine published, and the worktrees those are
    /// in; stop watching the rest. A repository newly watched counts as changed, so what changed
    /// while nothing was watching is published.
    fn rescan(&mut self) {
        self.rescanned = Some(Instant::now());
        let (Ok(projects), Some(this)) = (super::local_projects(), machine_name()) else {
            return;
        };
        let mut watched = HashMap::new();
        for (id, dir) in projects {
            let published = published_here(&dir, &this);
            if published.is_empty() || !self.acts_for(&dir) {
                continue;
            }
            let mut paths = vec![dir.clone()];
            if let Ok(ctx) = but_ctx::Context::discover(&dir)
                && let Ok(homes) = LocalHomes::read(&ctx, &dir)
            {
                for (branch, _) in &published {
                    if let Ok(LocalHome::Worktree(path)) = homes.of(branch) {
                        paths.push(PathBuf::from(path));
                    }
                }
            }
            let (_, mut monitors) = self.watched.remove(&id).unwrap_or_else(|| {
                self.changed.insert(dir.clone(), Instant::now());
                (dir.clone(), HashMap::new())
            });
            monitors.retain(|path, _| paths.contains(path));
            for path in paths {
                if monitors.contains_key(&path) {
                    continue;
                }
                let Ok(project) = id.parse::<ProjectHandleOrLegacyProjectId>() else {
                    continue;
                };
                let _runtime = self.runtime.enter();
                match gitbutler_filemonitor::spawn(
                    project,
                    &path,
                    self.files.clone(),
                    WatchMode::Auto,
                ) {
                    Ok(monitor) => {
                        monitors.insert(path, monitor);
                    }
                    Err(err) => {
                        tracing::debug!("the follower can't watch {}: {err:#}", path.display())
                    }
                }
            }
            watched.insert(id, (dir, monitors));
        }
        self.watched = watched;
    }

    /// Whether this follower acts for the repository at `dir`: the first of this machine's
    /// clients to lock it does. Per machine name, as one computer may stand in for several.
    fn acts_for(&mut self, dir: &Path) -> bool {
        let lock = self.locks.entry(dir.to_owned()).or_default();
        // Tried again until it's had: the one holding it may have quit.
        if lock.is_none() {
            *lock = (|| {
                let machine = machine_name()?;
                let path = dir.join(format!(".git/gitbutler/mesh-follow-{machine}.lock"));
                std::fs::create_dir_all(path.parent()?).ok()?;
                let file = File::create(path).ok()?;
                file.try_lock().ok()?;
                Some(file)
            })();
        }
        lock.is_some()
    }

    fn publish_changed(&mut self, dir: &Path, on_event: &dyn Fn(FollowEvent)) {
        let Some(this) = machine_name() else {
            return;
        };
        let project = title(dir);
        for (branch, snapshot) in published_here(dir, &this) {
            match publish_if_changed(dir, &branch, &snapshot) {
                Ok(false) => {}
                Ok(true) => on_event(event(
                    FollowKind::Published,
                    format!("Published {branch} in {project}"),
                )),
                Err(err) => on_event(event(
                    FollowKind::Failed,
                    format!("Couldn't publish {branch} in {project}: {err:#}"),
                )),
            }
        }
    }

    /// Pull what `from` sent of `branch` to the project with root commit `root`.
    fn pull_sent(&mut self, root: &str, from: &str, branch: &str, on_event: &dyn Fn(FollowEvent)) {
        let Ok(projects) = super::local_projects() else {
            return;
        };
        let clones: Vec<PathBuf> = projects
            .into_iter()
            .map(|(_, dir)| dir)
            .filter(|dir| hosted_project(dir).is_ok_and(|candidate| candidate == root))
            .collect();
        match clones.as_slice() {
            [] => {}
            [dir] => {
                if self.acts_for(dir) {
                    self.pull(dir, from, branch, on_event);
                }
            }
            // Which one it was meant for can't be told, so it waits to be pulled by hand.
            several => on_event(event(
                FollowKind::Skipped,
                format!(
                    "{from} sent {branch}, but it wasn't pulled: {} projects here are clones of it",
                    several.len()
                ),
            )),
        }
    }

    /// Pull everything waiting in this machine's inbox, as sent while nothing was listening.
    fn pull_inbox(&mut self, on_event: &dyn Fn(FollowEvent)) {
        let Ok(account) = super::hosted_account() else {
            return;
        };
        let Ok(projects) = super::local_projects() else {
            return;
        };
        // Only where one project is the clone; with several, it waits to be pulled by hand.
        let waiting: HashSet<&String> = account
            .iter()
            .filter(|project| project.machines.iter().any(|machine| machine.sent > 0))
            .filter_map(|project| match project.project_ids.as_slice() {
                [id] => Some(id),
                _ => None,
            })
            .collect();
        for (id, dir) in projects {
            if !waiting.contains(&id) || !self.acts_for(&dir) {
                continue;
            }
            if let Err(err) = super::fetch(&dir) {
                tracing::debug!("the follower couldn't fetch {}: {err:#}", dir.display());
                continue;
            }
            let prefix = format!("{HOSTED_REFS}/inbox/");
            let Ok(refs) = git(&dir, &[], &["for-each-ref", "--format=%(refname)", &prefix]) else {
                continue;
            };
            for sent in refs.lines() {
                let Some((from, _)) = sent.trim_start_matches(&prefix).split_once('/') else {
                    continue;
                };
                let Ok(head) = snapshot_head(&dir, sent) else {
                    continue;
                };
                let branch = head.trim_start_matches("refs/heads/").to_owned();
                self.pull(&dir, from, &branch, on_event);
            }
        }
    }

    fn pull(&mut self, dir: &Path, from: &str, branch: &str, on_event: &dyn Fn(FollowEvent)) {
        let project = title(dir);
        let outcome = but_ctx::Context::discover(dir).and_then(|mut ctx| {
            // Into a new worktree when it isn't here yet, so the workspace isn't disturbed.
            super::hosted_branch_pull(&mut ctx, from.to_owned(), branch.to_owned(), false, None)
        });
        match outcome {
            Ok(SyncOutcome::Done(message)) => on_event(event(
                FollowKind::Pulled,
                format!("{project}: {message}, sent by {from}"),
            )),
            Ok(SyncOutcome::NeedsChoice(reason)) => {
                // Left in the inbox, to pull by hand; said once per snapshot sent.
                let snapshot = rev(
                    dir,
                    &format!(
                        "{HOSTED_REFS}/snapshots/{from}/{}",
                        super::published_name(branch)
                    ),
                )
                .unwrap_or_default();
                if self.reported.insert(snapshot) {
                    on_event(event(
                        FollowKind::Skipped,
                        format!("{project}: {from} sent {branch}, but it wasn't pulled. {reason}"),
                    ));
                }
            }
            Err(err) => on_event(event(
                FollowKind::Failed,
                format!("Couldn't pull {branch} from {from} in {project}: {err:#}"),
            )),
        }
    }
}

/// The branches this machine published from the repository at `dir` that are still local, each
/// with the snapshot it last published.
fn published_here(dir: &Path, this: &str) -> Vec<(String, String)> {
    let prefix = format!("{HOSTED_REFS}/snapshots/{this}/");
    let Ok(refs) = git(dir, &[], &["for-each-ref", "--format=%(refname)", &prefix]) else {
        return Vec::new();
    };
    refs.lines()
        .filter_map(|snapshot| {
            let branch = snapshot_head(dir, snapshot)
                .ok()?
                .trim_start_matches("refs/heads/")
                .to_owned();
            rev(dir, &format!("refs/heads/{branch}"))?;
            Some((branch, snapshot.to_owned()))
        })
        .collect()
}

/// Publish `branch` again if it changed since it was last published or pulled, with its
/// uncommitted changes if `snapshot`, what was last published, had them and it's in a worktree.
fn publish_if_changed(dir: &Path, branch: &str, snapshot: &str) -> Result<bool> {
    let ctx = but_ctx::Context::discover(dir)?;
    let full = format!("refs/heads/{branch}");
    let tip = rev(dir, &full).with_context(|| format!("there's no local branch {branch}"))?;
    let worktree = match local_home(&ctx, dir, branch)? {
        LocalHome::Worktree(path) if has_uncommitted(dir, snapshot) => Some(path),
        _ => None,
    };
    let tree = match &worktree {
        Some(path) => worktree_tree(&*ctx.repo.get()?, Path::new(path))?,
        None => rev(dir, &format!("{tip}^{{tree}}")).context("the branch has no tree")?,
    };
    let synced = synced_ref(branch);
    if rev(dir, &format!("{synced}^{{tree}}")) == Some(tree)
        && rev(dir, &format!("{synced}^")) == Some(tip)
    {
        return Ok(false);
    }
    super::publish(&ctx, branch, worktree.is_some(), None)?;
    Ok(true)
}

/// What the repository at `dir` is called, for telling about it.
fn title(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
