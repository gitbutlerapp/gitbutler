use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use but_project_handle::{
    INVALIDATION_SENTINEL_PATH, ProjectHandleOrLegacyProjectId, REFRESH_SENTINEL_PATH,
};
use gitbutler_notify_debouncer::{Debouncer, NoCache, new_debouncer};
use gix::bstr::ByteSlice as _;
use notify::{RecommendedWatcher, Watcher};
use tokio::task;
use tracing::Level;

use crate::{
    events::{Checkout, InternalEvent, LinkedWorktree},
    watch_plan::{
        build_index_icase_accelerator_if_needed, compute_watch_plan_for_repo, is_tracked_in_index,
        is_watchable_directory, to_repo_relative_path,
    },
};

/// We will collect notifications for up to this amount of time at a very
/// maximum before releasing them. This duration will be hit if e.g. a build
/// is constantly running and producing a lot of file changes, we will process
/// them even if the build is still running.
const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(60);

// The internal rate at which the debouncer will update its state.
// Keeping a higher timeout on Windows because of file-system issues related
// to `virtual_branches.toml`.
const TICK_RATE: Duration = if cfg!(windows) {
    Duration::from_millis(250)
} else {
    Duration::from_millis(100)
};

// The number of TICK_RATE intervals required of "dead air" (i.e. no new events
// arriving) before we will automatically flush pending events. This means that
// after the disk is quiet for TICK_RATE * FLUSH_AFTER_EMPTY, we will process
// the pending events, even if DEBOUNCE_TIMEOUT hasn't expired yet
const FLUSH_AFTER_EMPTY: u32 = 3;

enum Command {
    Flush,
}

/// Handle for a running file monitor spawned with [`spawn()`].
///
/// Dropping this handle will stop the monitor as soon as it tries to send the next event, failing as there is no receiver.
pub struct FileMonitorHandle {
    cmd_tx: std::sync::mpsc::Sender<Command>,
}

impl FileMonitorHandle {
    /// Request that pending filesystem events are emitted immediately.
    pub fn flush(&self) -> Result<()> {
        self.cmd_tx
            .send(Command::Flush)
            .map_err(|_| anyhow!("file monitor stopped"))
    }
}

const ENV_WATCH_MODE: &str = "GITBUTLER_WATCH_MODE";

/// Control how the filesystem watch should be established.
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchMode {
    /// Recursively watch the worktree (and an extra git-dir if the repo uses
    /// a linked worktree with a git-dir outside the worktree), using [`notify::RecursiveMode::Recursive`].
    Legacy,
    /// Ignore-aware watch plan: non-recursive watches of non-ignored worktree directories,
    /// plus explicit git-dir watches and dynamic watch additions for newly created directories.
    /// Each directory is watched with [`notify::RecursiveMode::NonRecursive`].
    Modern,
    /// Automatically pick a mode based on platform heuristics.
    ///
    #[default]
    /// Currently, this enables `Modern` on WSL (Windows Subsystem for Linux.) and `Legacy` elsewhere.
    Auto,
}

impl std::str::FromStr for WatchMode {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s.trim().to_ascii_lowercase().as_str() {
            "legacy" => Self::Legacy,
            "modern" => Self::Modern,
            "auto" => Self::Auto,
            _ => {
                return Err(());
            }
        })
    }
}

impl WatchMode {
    /// Initialise the mode from the environment.
    pub fn from_env() -> Self {
        let Ok(mode) = std::env::var(ENV_WATCH_MODE) else {
            return Self::Auto;
        };

        mode.parse().ok().unwrap_or_else(|| {
            tracing::warn!(
                env = ENV_WATCH_MODE,
                value = mode,
                "unknown watch mode; falling back to auto"
            );
            WatchMode::Auto
        })
    }

    /// Initialise the mode from `watch_mode_from_settings`, with environment variable override.
    /// If the environment variable `GITBUTLER_WATCH_MODE` is set, it overrides the feature flag.
    /// Otherwise, the feature flag value is used.
    pub fn from_env_or_settings<F>(watch_mode_from_settings: &str, get_env_var: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        let env_var = get_env_var(ENV_WATCH_MODE);
        env_var
            .as_deref()
            .and_then(|env_var_value| env_var_value.parse().ok())
            .or_else(|| watch_mode_from_settings.parse().ok())
            .unwrap_or_else(|| {
                tracing::warn!(
                    feature_flag = watch_mode_from_settings,
                    env_var = ?env_var,
                    "unknown watch mode from feature flag or environment variable; falling back to auto"
                );
                WatchMode::Auto
            })
    }
}

#[cfg(target_os = "linux")]
fn is_wsl() -> bool {
    if std::env::var_os("WSL_DISTRO_NAME").is_some() || std::env::var_os("WSL_INTEROP").is_some() {
        return true;
    }

    for path in ["/proc/sys/kernel/osrelease", "/proc/version"] {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        let lower = contents.to_ascii_lowercase();
        if lower.contains("microsoft") || lower.contains("wsl") {
            return true;
        }
    }
    false
}

#[cfg(not(target_os = "linux"))]
fn is_wsl() -> bool {
    false
}

fn watch_backoff_policy() -> backoff::ExponentialBackoff {
    backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(30)))
        .build()
}

fn setup_watch_plan(
    debouncer: &mut Debouncer<RecommendedWatcher, NoCache>,
    project_id: ProjectHandleOrLegacyProjectId,
    repo: &gix::Repository,
    worktree_path: &Path,
    git_dir: &Path,
    linked_git_dirs: &[&Path],
) -> Result<()> {
    // Start the watcher, but retry if there are transient errors.
    backoff::retry(watch_backoff_policy(), || {
        let mut paths = debouncer.watcher().paths_mut();
        let mut add_error: Option<(std::path::PathBuf, notify::Error)> = None;
        compute_watch_plan_for_repo(repo, worktree_path, git_dir, linked_git_dirs, |path, mode| {
            if add_error.is_some() {
                return Ok(std::ops::ControlFlow::Break(()));
            }
            match paths.add(path, mode) {
                Ok(()) => Ok(std::ops::ControlFlow::Continue(())),
                Err(err) => match err.kind {
                    notify::ErrorKind::MaxFilesWatch => {
                        tracing::warn!(
                            %project_id,
                            path = %path.display(),
                            "OS file watch limit reached; continuing with partial watches. Monitoring coverage may be incomplete until restart."
                        );
                        Ok(std::ops::ControlFlow::Break(()))
                    }
                    _ => {
                        add_error = Some((path.to_owned(), err));
                        Ok(std::ops::ControlFlow::Break(()))
                    }
                },
            }
        })
        .map_err(|err| backoff::Error::permanent(err.into_boxed_dyn_error()))?;

        if let Some((path, err)) = add_error {
            return Err(into_backoff_err(err, &path));
        }

        match paths.commit() {
            Ok(()) => Ok(()),
            Err(err) => Err(into_backoff_err(err, worktree_path))
        }
    })
        .map_err(backoff_err_to_anyhow).context("Watcher start failed")
}

fn setup_legacy_watch(
    debouncer: &mut Debouncer<RecommendedWatcher, NoCache>,
    worktree_path: &Path,
    git_dir: &Path,
) -> Result<()> {
    let extra_git_dir_to_watch = {
        let mut enclosing_worktree_dir = git_dir.to_owned();
        enclosing_worktree_dir.pop();
        if enclosing_worktree_dir != worktree_path {
            Some(git_dir)
        } else {
            None
        }
    };

    // Start the watcher, but retry if there are transient errors.
    backoff::retry(watch_backoff_policy(), || {
        debouncer
            .watcher()
            .watch(worktree_path, notify::RecursiveMode::Recursive)
            .and_then(|()| {
                if let Some(git_dir) = extra_git_dir_to_watch {
                    debouncer
                        .watcher()
                        .watch(git_dir, notify::RecursiveMode::Recursive)
                } else {
                    Ok(())
                }
            })
            .map_err(|err| into_backoff_err(err, worktree_path))
    })
    .map_err(backoff_err_to_anyhow)
    .context("failed to start watcher")
}

/// Listen to interesting filesystem events of files in `path` that are not `.gitignore`d,
/// classify them, and associate them with `project_id`.
/// These are sent through the passed `out` channel, to indicate either **Git** repository changes
/// or **ProjectWorktree** changes
/// Use `watch_mode` to control how exactly the directory is watched.
/// `linked_worktrees` lists the linked worktrees to watch besides the main one, whose changes are
/// reported under their own [`Checkout`]. The list is obtained again whenever a linked worktree is
/// registered, moved or removed, or when the invalidation sentinel is written, and is left as it
/// was if that fails.
///
/// ### Why is this not an iterator?
///
/// The internal `notify_rx` could be an iterator, which performs all transformations and returns them as item.
/// Due to closures being continuously created each time events come in, nested closures need to own
/// their resources, which means they are `Clone` or `Copy`. This isn't the case for `git::Repository`.
/// Even though `gix::Repository` is `Clone`, an efficient implementation of `is_path_ignored()` requires more state
/// that ideally is kept between invocations. For that reason, the current channel-based 'worker' architecture
/// is chosen to allow all these states to live on the stack.
///
/// Additionally, a channel plays better with how events are handled downstream.
pub fn spawn(
    project_id: ProjectHandleOrLegacyProjectId,
    worktree_path: &std::path::Path,
    linked_worktrees: impl Fn() -> Result<Vec<LinkedWorktree>> + Send + 'static,
    out: tokio::sync::mpsc::UnboundedSender<InternalEvent>,
    watch_mode: WatchMode,
) -> Result<FileMonitorHandle> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(
        DEBOUNCE_TIMEOUT,
        Some(TICK_RATE),
        Some(FLUSH_AFTER_EMPTY),
        notify_tx,
    )
    .context("failed to create debouncer")?;

    let worktree_path = gix::path::realpath(worktree_path)?;
    let repo = gix::open_opts(&worktree_path, gix::open::Options::isolated()).context(format!(
        "failed to open project repository to obtain git-dir: {}",
        worktree_path.display()
    ))?;
    let git_dir = repo.path().to_owned();
    let mut linked = linked_checkouts(&git_dir, &linked_worktrees).unwrap_or_default();
    let linked_git_dirs: Vec<_> = linked
        .iter()
        .map(|checkout| checkout.git_dir.as_path())
        .collect();

    let mut effective_watch_mode = watch_mode;

    match watch_mode {
        WatchMode::Legacy => {
            setup_legacy_watch(&mut debouncer, &worktree_path, &git_dir)?;
        }
        WatchMode::Modern => {
            if let Err(err) = setup_watch_plan(
                &mut debouncer,
                project_id.clone(),
                &repo,
                &worktree_path,
                &git_dir,
                &linked_git_dirs,
            ) {
                tracing::warn!(
                    %project_id,
                    ?err,
                    "watch-plan setup failed; falling back to legacy watch mode"
                );
                effective_watch_mode = WatchMode::Legacy;
                setup_legacy_watch(&mut debouncer, &worktree_path, &git_dir)?;
            }
        }
        WatchMode::Auto => {
            if is_wsl() {
                match setup_watch_plan(
                    &mut debouncer,
                    project_id.clone(),
                    &repo,
                    &worktree_path,
                    &git_dir,
                    &linked_git_dirs,
                ) {
                    Ok(()) => {
                        effective_watch_mode = WatchMode::Modern;
                    }
                    Err(err) => {
                        tracing::warn!(
                            %project_id,
                            ?err,
                            "watch-plan setup failed; falling back to legacy watch mode"
                        );
                        effective_watch_mode = WatchMode::Legacy;
                        setup_legacy_watch(&mut debouncer, &worktree_path, &git_dir)?;
                    }
                }
            } else {
                effective_watch_mode = WatchMode::Legacy;
                setup_legacy_watch(&mut debouncer, &worktree_path, &git_dir)?;
            }
        }
    }
    tracing::debug!(
        %project_id,
        requested = ?watch_mode,
        effective = ?effective_watch_mode,
        "file watcher started"
    );

    let dynamic_watch_enabled = matches!(effective_watch_mode, WatchMode::Modern);
    for checkout in &linked {
        watch_linked_workdir(
            &mut debouncer,
            &project_id,
            &worktree_path,
            checkout,
            dynamic_watch_enabled,
        );
    }
    let main = WatchedCheckout {
        id: Checkout::Main,
        workdir: worktree_path,
        git_dir,
    };
    task::spawn_blocking(move || {
        let _runtime = tracing::span!(Level::INFO, "file monitor", %project_id ).entered();
        tracing::debug!(%project_id, "file watcher started");

        // Even though the watcher cannot 'double-watch', we keep track of newly added watches
        // to avoid it doing any work, and for better traces.
        let mut dynamically_watched_dirs: HashSet<std::path::PathBuf> = HashSet::new();
        'outer: loop {
            // Handle control plane messages.
            loop {
                match cmd_rx.try_recv() {
                    Ok(Command::Flush) => debouncer.flush_nonblocking(),
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => break 'outer,
                }
            }

            let result = match notify_rx.recv_timeout(TICK_RATE) {
                Ok(result) => result,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break 'outer,
            };
            let stats = tracing::span!(
                Level::INFO,
                "handle debounced events",
                ignored = tracing::field::Empty,
                project = tracing::field::Empty,
                git = tracing::field::Empty,
                git_noop = tracing::field::Empty,
                fs_events = tracing::field::Empty,
            )
            .entered();
            match result {
                Err(err) => {
                    tracing::error!(?err, "ignored file watcher error");
                }
                Ok(events) => {
                    let num_events = events.len();
                    let changes_by_checkout = changes_by_checkout(
                        std::iter::once(&main).chain(&linked),
                        events
                            .into_iter()
                            .filter(|event| is_interesting_kind(event.kind))
                            .flat_map(|event| event.event.paths),
                    );
                    let count = |count_of: fn(&Changes) -> usize| -> usize {
                        changes_by_checkout
                            .iter()
                            .map(|(_, changes)| count_of(changes))
                            .sum()
                    };
                    stats.record("fs_events", num_events);
                    stats.record("ignored", count(|changes| changes.ignored));
                    stats.record("git_noop", count(|changes| changes.git_noop));
                    stats.record("git", count(|changes| changes.git.len()));
                    stats.record("project", count(|changes| changes.worktree.len()));
                    let relisted = changes_by_checkout
                        .iter()
                        .any(|(_, changes)| changes.linked_worktrees_may_differ())
                        .then(|| linked_checkouts(&main.git_dir, &linked_worktrees))
                        .flatten();
                    if let Some(relisted) = &relisted {
                        if dynamic_watch_enabled {
                            update_dynamic_watches(
                                &mut debouncer,
                                &mut dynamically_watched_dirs,
                                std::iter::once(main.git_dir.join(LINKED_WORKTREES_DIR)),
                            );
                        }
                        for added in relisted
                            .iter()
                            .filter(|checkout| !linked.contains(checkout))
                        {
                            watch_linked_workdir(
                                &mut debouncer,
                                &project_id,
                                &main.workdir,
                                added,
                                dynamic_watch_enabled,
                            );
                        }
                        for removed in linked
                            .iter()
                            .filter(|checkout| !relisted.contains(checkout))
                        {
                            if removed.has_own_recursive_watch(&main.workdir, dynamic_watch_enabled)
                            {
                                debouncer.watcher().unwatch(&removed.workdir).ok();
                            }
                        }
                    }

                    for (checkout, changes) in changes_by_checkout {
                        if dynamic_watch_enabled && changes.ignore_filtering_ran {
                            update_dynamic_watches(
                                &mut debouncer,
                                &mut dynamically_watched_dirs,
                                changes
                                    .worktree
                                    .iter()
                                    .map(|relative_path| checkout.workdir.join(relative_path)),
                            );
                        }

                        if !changes.git.is_empty() {
                            let event = InternalEvent::GitFilesChange(
                                project_id.clone(),
                                checkout.id.clone(),
                                changes.git.into_iter().collect(),
                            );
                            if out.send(event).is_err() {
                                tracing::info!("channel closed - stopping file watcher");
                                break 'outer;
                            }
                        }
                        if !changes.worktree.is_empty() {
                            let event = InternalEvent::ProjectFilesChange(
                                project_id.clone(),
                                checkout.id.clone(),
                                changes.worktree.into_iter().collect(),
                            );
                            if out.send(event).is_err() {
                                tracing::info!("channel closed - stopping file watcher");
                                break 'outer;
                            }
                        }
                    }

                    if let Some(relisted) = relisted {
                        linked = relisted;
                    }
                }
            }
        }
    });
    Ok(FileMonitorHandle { cmd_tx })
}

/// Watch the files of the `linked` worktree: with its own watch plan if `use_watch_plan` is set, and
/// recursively otherwise, unless the recursive watch of `main_workdir` already covers it.
fn watch_linked_workdir(
    debouncer: &mut Debouncer<RecommendedWatcher, NoCache>,
    project_id: &ProjectHandleOrLegacyProjectId,
    main_workdir: &Path,
    linked: &WatchedCheckout,
    use_watch_plan: bool,
) {
    let res = if use_watch_plan {
        gix::open_opts(&linked.workdir, gix::open::Options::isolated())
            .map_err(anyhow::Error::from)
            .and_then(|repo| {
                setup_watch_plan(
                    debouncer,
                    project_id.clone(),
                    &repo,
                    &linked.workdir,
                    &linked.git_dir,
                    &[],
                )
            })
    } else if linked.has_own_recursive_watch(main_workdir, use_watch_plan) {
        debouncer
            .watcher()
            .watch(&linked.workdir, notify::RecursiveMode::Recursive)
            .map_err(Into::into)
    } else {
        Ok(())
    };
    if let Err(err) = res {
        tracing::warn!(
            ?err,
            workdir = ?linked.workdir,
            "failed to watch linked worktree; changes to its files will be missed"
        );
    }
}

/// Watch the directories among the changed `paths` that aren't watched yet, and stop watching those that are gone.
///
/// There is an inherent race condition here where files created in a new directory before the
/// watch is established will be missed. That's not a problem right now as we don't really care
/// about the paths.
fn update_dynamic_watches(
    debouncer: &mut Debouncer<RecommendedWatcher, NoCache>,
    dynamically_watched_dirs: &mut HashSet<PathBuf>,
    paths: impl Iterator<Item = PathBuf>,
) {
    #[derive(Debug)]
    enum Mode {
        AddWatch,
        RemoveWatch,
    }
    for path in paths {
        let mode = match path.symlink_metadata() {
            Ok(md)
                if is_watchable_directory(md.file_type())
                    && !dynamically_watched_dirs.contains(&path) =>
            {
                Mode::AddWatch
            }
            // We don't care if was dynamically watched, it might be watched during initial computation.
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Mode::RemoveWatch,
            _ => continue,
        };
        tracing::trace!(?path, ?mode, "adding or removing dynamic watch");
        let res = match mode {
            Mode::AddWatch => debouncer
                .watcher()
                .watch(&path, notify::RecursiveMode::NonRecursive),
            Mode::RemoveWatch => debouncer.watcher().unwatch(&path),
        }
        .inspect_err(|err| {
            tracing::warn!(
                ?path,
                ?mode,
                ?err,
                "failed to add or remove watch; changes may be missed until restart"
            )
        });
        match mode {
            Mode::AddWatch if res.is_ok() => {
                dynamically_watched_dirs.insert(path);
            }
            _ => {
                // If adding OR removing a watch didn't work, just remove it from our list.
                // On linux, it seems to manage to remove the watch, but fails to communicate it,
                // so our own tracking list would be stale.
                dynamically_watched_dirs.remove(&path);
            }
        }
    }
}

#[cfg(target_family = "unix")]
fn is_interesting_kind(kind: notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(
            notify::event::CreateKind::File | notify::event::CreateKind::Folder
        ) | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            // This makes many more events happen, but we won't miss `touch this && rm this` kind of events.
            | notify::EventKind::Modify(notify::event::ModifyKind::Metadata(_))
            | notify::EventKind::Remove(
                notify::event::RemoveKind::File | notify::event::RemoveKind::Folder
            )
    )
}

#[cfg(target_os = "windows")]
fn is_interesting_kind(kind: notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    )
}

fn into_backoff_err(
    err: notify::Error,
    path: &Path,
) -> backoff::Error<Box<dyn std::error::Error + Send + Sync + 'static>> {
    match err.kind {
        notify::ErrorKind::PathNotFound => backoff::Error::permanent(
            anyhow!("{} not found", path.display()).into_boxed_dyn_error(),
        ),
        notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
            backoff::Error::permanent(anyhow::Error::from(err).into_boxed_dyn_error())
        }
        _ => backoff::Error::transient(anyhow::Error::from(err).into_boxed_dyn_error()),
    }
}

fn backoff_err_to_anyhow(
    err: backoff::Error<Box<dyn std::error::Error + Send + Sync + 'static>>,
) -> anyhow::Error {
    anyhow::Error::from_boxed(Box::from(err.to_string()))
}

pub const LOCAL_REFS_DIR: &str = "refs/heads/";
pub const REMOTE_REFS_DIR: &str = "refs/remotes/";
pub const FETCH_HEAD: &str = "FETCH_HEAD";
pub const HEAD: &str = "HEAD";
pub const HEAD_ACTIVITY: &str = "logs/HEAD";
pub const INDEX: &str = "index";
pub const GB_FLUSH: &str = "GB_FLUSH";
pub const LINKED_WORKTREES_DIR: &str = "worktrees";
pub const LINKED_WORKTREE_GITDIR: &str = "gitdir";

#[derive(PartialEq)]
struct WatchedCheckout {
    id: Checkout,
    workdir: PathBuf,
    git_dir: PathBuf,
}

#[derive(Default)]
struct Changes {
    git: HashSet<PathBuf>,
    worktree: HashSet<PathBuf>,
    ignored: usize,
    git_noop: usize,
    ignore_filtering_ran: bool,
}

fn changes_by_checkout<'a>(
    checkouts: impl Iterator<Item = &'a WatchedCheckout>,
    paths: impl IntoIterator<Item = PathBuf>,
) -> Vec<(&'a WatchedCheckout, Changes)> {
    let mut paths_by_checkout: Vec<_> = checkouts.map(|checkout| (checkout, Vec::new())).collect();
    for path in paths {
        let innermost = paths_by_checkout
            .iter_mut()
            .filter_map(|(checkout, paths)| {
                Some((checkout.depth_of_root_containing(&path)?, paths))
            })
            .max_by_key(|(depth, _)| *depth);
        if let Some((_, paths)) = innermost {
            paths.push(path);
        }
    }
    paths_by_checkout
        .into_iter()
        .map(|(checkout, paths)| (checkout, checkout.changes(paths)))
        .collect()
}

fn linked_checkouts(
    main_git_dir: &Path,
    linked_worktrees: &impl Fn() -> Result<Vec<LinkedWorktree>>,
) -> Option<Vec<WatchedCheckout>> {
    let worktrees = linked_worktrees()
        .inspect_err(|err| tracing::warn!(?err, "failed to list linked worktrees"))
        .ok()?;
    Some(
        worktrees
            .into_iter()
            .map(|worktree| WatchedCheckout::linked(main_git_dir, worktree))
            .collect(),
    )
}

impl Changes {
    fn linked_worktrees_may_differ(&self) -> bool {
        self.git.iter().any(|path| {
            path.starts_with(LINKED_WORKTREES_DIR)
                || path == Path::new(LINKED_WORKTREE_GITDIR)
                || path == Path::new(INVALIDATION_SENTINEL_PATH)
        })
    }
}

impl WatchedCheckout {
    fn linked(main_git_dir: &Path, worktree: LinkedWorktree) -> Self {
        WatchedCheckout {
            git_dir: main_git_dir
                .join(LINKED_WORKTREES_DIR)
                .join(gix::path::from_bstr(worktree.name.as_bstr())),
            workdir: gix::path::realpath(&worktree.workdir).unwrap_or(worktree.workdir),
            id: Checkout::Linked(worktree.name),
        }
    }

    fn has_own_recursive_watch(&self, main_workdir: &Path, use_watch_plan: bool) -> bool {
        !use_watch_plan && !self.workdir.starts_with(main_workdir)
    }

    fn depth_of_root_containing(&self, path: &Path) -> Option<usize> {
        [&self.git_dir, &self.workdir]
            .into_iter()
            .filter(|root| {
                path.strip_prefix(root)
                    .is_ok_and(|relative_path| !relative_path.as_os_str().is_empty())
            })
            .map(|root| root.components().count())
            .max()
    }

    fn changes(&self, paths: impl IntoIterator<Item = PathBuf>) -> Changes {
        let mut classified: Vec<_> = paths
            .into_iter()
            .map(|path| {
                let kind = classify_file(&self.git_dir, &path);
                (path, kind)
            })
            .collect();
        let mut changes = Changes {
            ignore_filtering_ran: self.mark_ignored(&mut classified),
            ..Default::default()
        };
        for (path, kind) in classified {
            match kind {
                FileKind::ProjectIgnored => changes.ignored += 1,
                FileKind::GitUninteresting => changes.git_noop += 1,
                FileKind::Git => {
                    if let Ok(relative_path) = path.strip_prefix(&self.git_dir) {
                        changes.git.insert(relative_path.to_owned());
                    }
                }
                FileKind::Project => {
                    if let Ok(relative_path) = path.strip_prefix(&self.workdir) {
                        changes.worktree.insert(relative_path.to_owned());
                    }
                }
            }
        }
        changes
    }

    fn mark_ignored(&self, classified: &mut [(PathBuf, FileKind)]) -> bool {
        if classified
            .iter()
            .any(|(_, kind)| *kind == FileKind::Project)
            && let Ok(repo) = gix::open(&self.workdir)
            && let Ok(index) = repo.index_or_empty()
            && let Ok(mut excludes) = repo.excludes(
                &index,
                None,
                gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
            )
        {
            let icase_acc = build_index_icase_accelerator_if_needed(&repo, &index);
            for (path, kind) in classified
                .iter_mut()
                .filter(|(_, kind)| *kind == FileKind::Project)
            {
                if let Ok(relative_path) = path.strip_prefix(&self.workdir) {
                    let is_dir = path.is_dir();
                    let is_excluded = excludes
                        .at_path(
                            relative_path,
                            is_dir.then_some(gix::index::entry::Mode::DIR),
                        )
                        .map(|platform| platform.is_excluded())
                        .unwrap_or(false);
                    if is_excluded
                        && !is_tracked_in_index(
                            to_repo_relative_path(relative_path).as_ref(),
                            is_dir,
                            &index,
                            icase_acc.as_ref(),
                        )
                    {
                        *kind = FileKind::ProjectIgnored
                    }
                }
            }
            true
        } else {
            false
        }
    }
}

/// A classification for a changed file.
#[derive(Debug, Eq, PartialEq)]
enum FileKind {
    /// A file in the `.git` repository of the current project itself.
    Git,
    /// Like `Git`, but shouldn't have any effect.
    GitUninteresting,
    /// A file in the worktree of the current project.
    Project,
    /// A file that was ignored in the project, and thus shouldn't trigger a computation.
    ProjectIgnored,
}

fn classify_file(git_dir: &Path, file_path: &Path) -> FileKind {
    if let Ok(check_file_path) = file_path.strip_prefix(git_dir) {
        if check_file_path == Path::new(FETCH_HEAD)
            || check_file_path == Path::new(HEAD_ACTIVITY)
            || check_file_path == Path::new(HEAD)
            || check_file_path == Path::new(GB_FLUSH)
            || check_file_path == Path::new(INDEX)
            || check_file_path == Path::new(REFRESH_SENTINEL_PATH)
            || check_file_path == Path::new(INVALIDATION_SENTINEL_PATH)
            || check_file_path == Path::new(LINKED_WORKTREE_GITDIR)
            || check_file_path.starts_with(LOCAL_REFS_DIR)
            || check_file_path.starts_with(REMOTE_REFS_DIR)
            || is_linked_worktree_registration(check_file_path)
        {
            FileKind::Git
        } else {
            FileKind::GitUninteresting
        }
    } else {
        FileKind::Project
    }
}

fn is_linked_worktree_registration(git_dir_relative_path: &Path) -> bool {
    git_dir_relative_path
        .strip_prefix(LINKED_WORKTREES_DIR)
        .is_ok_and(|name_and_file| {
            let mut components = name_and_file.components();
            components.next();
            let file = components.as_path();
            file.as_os_str().is_empty() || file == Path::new(LINKED_WORKTREE_GITDIR)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn git_dir() -> &'static Path {
        Path::new("/repo/.git")
    }

    #[test]
    fn classify_local_ref() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/refs/heads/main")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_remote_ref() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/refs/remotes/origin/main")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_nested_remote_ref() {
        assert_eq!(
            classify_file(
                git_dir(),
                Path::new("/repo/.git/refs/remotes/origin/feature/branch")
            ),
            FileKind::Git
        );
    }

    #[test]
    fn classify_head() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/HEAD")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_objects_as_uninteresting() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/objects/ab/cdef1234567890")),
            FileKind::GitUninteresting
        );
    }

    #[test]
    fn classify_worktree_file() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/src/main.rs")),
            FileKind::Project
        );
    }

    #[test]
    fn classify_refresh_sentinel() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/gitbutler/REFRESH")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_invalidation_sentinel() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/gitbutler/INVALIDATE")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_linked_worktree_registration() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/worktrees/name")),
            FileKind::Git
        );
    }

    #[test]
    fn classify_linked_worktree_location() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/worktrees/name/gitdir")),
            FileKind::Git,
            "seen from the main git dir, for a linked worktree that isn't watched"
        );
        assert_eq!(
            classify_file(
                Path::new("/repo/.git/worktrees/name"),
                Path::new("/repo/.git/worktrees/name/gitdir")
            ),
            FileKind::Git,
            "seen from the git dir of a watched linked worktree"
        );
    }

    #[test]
    fn classify_linked_worktree_contents_as_uninteresting() {
        assert_eq!(
            classify_file(git_dir(), Path::new("/repo/.git/worktrees/name/HEAD")),
            FileKind::GitUninteresting
        );
    }

    #[test]
    fn classify_metadata_store_as_uninteresting() {
        // The metadata store is uninteresting — the sentinel is what signals refreshes.
        assert_eq!(
            classify_file(
                git_dir(),
                Path::new("/repo/.git/gitbutler/virtual_branches.toml")
            ),
            FileKind::GitUninteresting
        );
    }
}
