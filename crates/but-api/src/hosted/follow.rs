//! Following branches between machines: this machine's followed branches are published as they
//! change, other machines' followed branches pulled as they're published.
//!
//! Which branches are followed lives in each repository's git config, and how often in the mesh
//! settings, so Lite and `but mesh` share both. Whichever starts first acts for a repository; the
//! other leaves it alone while that one runs.

use std::{
    collections::{HashMap, HashSet},
    fs::File,
    path::{Path, PathBuf},
    sync::mpsc,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use but_settings::AppSettings;
use serde::Serialize;

use super::{
    AUTO_PUBLISH, AUTO_PULL, HOSTED_REFS, HostedEvent, HostedListener, LocalHome, SyncOutcome,
    follow_rules, git, hosted_project, local_home, published_name, rev, synced_ref, worktree_tree,
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
    /// This machine's branch changed, and was published.
    Published,
    /// Another machine published a branch, and it was pulled here.
    Pulled,
    /// Another machine published a branch, but pulling it would replace local work.
    Skipped,
    /// Following a branch failed this time; it's tried again.
    Failed,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(FollowKind);

fn event(kind: FollowKind, message: String) -> FollowEvent {
    FollowEvent { kind, message }
}

enum Signal {
    Published(Option<String>),
    Stop,
}

/// Follows branches in the background, holding a connection to the hosted server, until dropped.
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
    let published = signals.clone();
    let listener = super::listen_account(move |event| {
        if let HostedEvent::Published(root) = event {
            published.send(Signal::Published(root)).ok();
        }
    })?;
    std::thread::Builder::new()
        .name("hosted-follower".into())
        .spawn(move || Run::default().run(&received, &on_event))?;
    Ok(Follower {
        _listener: listener,
        signals,
    })
}

#[derive(Default)]
struct Run {
    /// Each repository's lock, held while this follower acts for it; `None` where another does.
    locks: HashMap<PathBuf, Option<File>>,
    last_publish_check: Option<Instant>,
    /// Roots something was published to since they were last looked at.
    published: Option<HashSet<String>>,
    /// Something may have been published anywhere: at the start, and after reconnecting.
    everything_published: bool,
    last_pull: HashMap<String, Instant>,
    /// Snapshots already reported as not pulled, so each is reported once.
    reported: HashSet<String>,
}

impl Run {
    fn run(mut self, received: &mpsc::Receiver<Signal>, on_event: &dyn Fn(FollowEvent)) {
        // Whatever was published while nothing was following.
        self.everything_published = true;
        loop {
            match received.recv_timeout(Duration::from_secs(1)) {
                Ok(Signal::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                Ok(Signal::Published(Some(root))) => {
                    self.published.get_or_insert_default().insert(root);
                }
                Ok(Signal::Published(None)) => self.everything_published = true,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            let Ok(settings) = AppSettings::load_from_default_path_creating_without_customization()
            else {
                continue;
            };
            let Ok(projects) = super::local_projects() else {
                continue;
            };
            let mesh = settings.mesh;
            let publish_due = self.last_publish_check.is_none_or(|last| {
                last.elapsed() >= Duration::from_secs(mesh.publish_interval_sec.max(1))
            });
            if mesh.auto_publish && publish_due {
                self.last_publish_check = Some(Instant::now());
                for (_, dir) in &projects {
                    self.publish_changed(dir, on_event);
                }
            }
            if mesh.auto_pull && (self.everything_published || self.published.is_some()) {
                let interval = Duration::from_secs(mesh.pull_interval_sec);
                let mut waiting = HashSet::new();
                for (_, dir) in &projects {
                    if let Some(root) = self.pull_published(dir, interval, on_event) {
                        waiting.insert(root);
                    }
                }
                self.everything_published = false;
                // A pull held back by the interval is tried again on a later tick.
                self.published = (!waiting.is_empty()).then_some(waiting);
            }
        }
    }

    /// Whether this follower acts for the repository at `dir`: the first to lock it does.
    fn acts_for(&mut self, dir: &Path) -> bool {
        self.locks
            .entry(dir.to_owned())
            .or_insert_with(|| {
                let path = dir.join(".git/gitbutler/mesh-follow.lock");
                std::fs::create_dir_all(path.parent()?).ok()?;
                let file = File::create(path).ok()?;
                file.try_lock().ok()?;
                Some(file)
            })
            .is_some()
    }

    fn publish_changed(&mut self, dir: &Path, on_event: &dyn Fn(FollowEvent)) {
        let branches = follow_rules(dir, AUTO_PUBLISH);
        if branches.is_empty() || !self.acts_for(dir) {
            return;
        }
        let project = title(dir);
        for branch in branches {
            match publish_if_changed(dir, &branch) {
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

    /// Pull what was published of the followed branches in the repository at `dir`. Returns its
    /// root if a pull is held back by `interval`.
    fn pull_published(
        &mut self,
        dir: &Path,
        interval: Duration,
        on_event: &dyn Fn(FollowEvent),
    ) -> Option<String> {
        let rules = follow_rules(dir, AUTO_PULL);
        if rules.is_empty() {
            return None;
        }
        let root = hosted_project(dir).ok()?;
        let relevant = self.everything_published
            || self
                .published
                .as_ref()
                .is_some_and(|roots| roots.contains(&root));
        if !relevant || !self.acts_for(dir) {
            return None;
        }
        if let Err(err) = super::fetch(dir) {
            tracing::debug!("the follower couldn't fetch {}: {err:#}", dir.display());
            return None;
        }
        let project = title(dir);
        let mut held_back = false;
        for rule in rules {
            let Some((machine, branch)) = rule.split_once('/') else {
                continue;
            };
            let key = format!("{}\0{rule}", dir.display());
            if self
                .last_pull
                .get(&key)
                .is_some_and(|last| last.elapsed() < interval)
            {
                held_back = true;
                continue;
            }
            match self.pull_if_published(dir, machine, branch) {
                Ok(None) => {}
                Ok(Some(Ok(message))) => {
                    self.last_pull.insert(key, Instant::now());
                    on_event(event(
                        FollowKind::Pulled,
                        format!("{project}: {message} from {machine}"),
                    ));
                }
                Ok(Some(Err(reason))) => on_event(event(
                    FollowKind::Skipped,
                    format!(
                        "{project}: {machine} published {branch}, but it wasn't pulled. {reason}"
                    ),
                )),
                Err(err) => on_event(event(
                    FollowKind::Failed,
                    format!("Couldn't pull {branch} from {machine} in {project}: {err:#}"),
                )),
            }
        }
        held_back.then_some(root)
    }

    /// Pull `machine`'s `branch` if it published a snapshot this follower hasn't pulled. `None` if
    /// there was nothing new; otherwise what was done, or why not, said once per snapshot.
    fn pull_if_published(
        &mut self,
        dir: &Path,
        machine: &str,
        branch: &str,
    ) -> Result<Option<Result<String, String>>> {
        let name = published_name(branch);
        let Some(snapshot) = rev(dir, &format!("{HOSTED_REFS}/snapshots/{machine}/{name}")) else {
            return Ok(None);
        };
        let followed = format!("{HOSTED_REFS}/followed/{machine}/{name}");
        if rev(dir, &followed).as_ref() == Some(&snapshot) {
            return Ok(None);
        }
        let mut ctx = but_ctx::Context::discover(dir)?;
        // Into a new worktree when it isn't here yet, so the workspace isn't disturbed.
        let outcome = super::hosted_branch_pull(
            &mut ctx,
            machine.to_owned(),
            branch.to_owned(),
            false,
            None,
        )?;
        Ok(match outcome {
            SyncOutcome::Done(message) => {
                git(dir, &[], &["update-ref", &followed, &snapshot])?;
                Some(Ok(message))
            }
            SyncOutcome::NeedsChoice(reason) => {
                self.reported.insert(snapshot).then_some(Err(reason))
            }
        })
    }
}

/// Publish this machine's `branch` if it changed since it was last published or pulled, with
/// its uncommitted changes when it's checked out in a worktree.
fn publish_if_changed(dir: &Path, branch: &str) -> Result<bool> {
    let ctx = but_ctx::Context::discover(dir)?;
    let full = format!("refs/heads/{branch}");
    let tip = rev(dir, &full).with_context(|| format!("there's no local branch {branch}"))?;
    let (tree, uncommitted) = match local_home(&ctx, dir, branch)? {
        LocalHome::Worktree(path) => (worktree_tree(&*ctx.repo.get()?, Path::new(&path))?, true),
        _ => (
            rev(dir, &format!("{tip}^{{tree}}")).context("the branch has no tree")?,
            false,
        ),
    };
    let synced = synced_ref(branch);
    if rev(dir, &format!("{synced}^{{tree}}")) == Some(tree)
        && rev(dir, &format!("{synced}^")) == Some(tip)
    {
        return Ok(false);
    }
    super::publish(&ctx, branch, uncommitted, None)?;
    Ok(true)
}

/// What the repository at `dir` is called, for telling about it.
fn title(dir: &Path) -> String {
    dir.file_name().map_or_else(
        || dir.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}
