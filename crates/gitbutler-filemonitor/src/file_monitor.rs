use std::{
    collections::{BTreeSet, HashSet},
    path::Path,
    time::Duration,
};

use anyhow::{Context as _, Result, anyhow};
use gitbutler_notify_debouncer::{Debouncer, NoCache, new_debouncer};
use gitbutler_project::ProjectId;
use gix::bstr::BStr;
use notify::{RecommendedWatcher, Watcher};
use tokio::task;
use tracing::Level;

use crate::{
    events::InternalEvent,
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
    #[default]
    Legacy,
    /// Ignore-aware watch plan: non-recursive watches of non-ignored worktree directories,
    /// plus explicit git-dir watches and dynamic watch additions for newly created directories.
    /// Each directory is watched with [`notify::RecursiveMode::NonRecursive`].
    Modern,
    /// Automatically pick a mode based on platform heuristics.
    ///
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
    pub fn from_env_or_settings(watch_mode_from_settings: &str) -> Self {
        std::env::var(ENV_WATCH_MODE)
            .ok()
            .and_then(|env_var_value| env_var_value.parse().ok())
            .or_else(|| watch_mode_from_settings.parse().ok())
            .unwrap_or_else(|| {
                tracing::warn!(
                    feature_flag = watch_mode_from_settings,
                    env_var = ?std::env::var(ENV_WATCH_MODE),
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
    project_id: ProjectId,
    repo: &gix::Repository,
    worktree_path: &Path,
    git_dir: &Path,
) -> Result<()> {
    // Start the watcher, but retry if there are transient errors.
    backoff::retry(watch_backoff_policy(), || {
        let mut paths = debouncer.watcher().paths_mut();
        let mut add_error: Option<(std::path::PathBuf, notify::Error)> = None;
        compute_watch_plan_for_repo(repo, worktree_path, git_dir, |path, mode| {
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
                    debouncer.watcher().watch(git_dir, notify::RecursiveMode::Recursive)
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
    project_id: ProjectId,
    worktree_path: &std::path::Path,
    out: tokio::sync::mpsc::UnboundedSender<InternalEvent>,
    watch_mode: WatchMode,
) -> Result<FileMonitorHandle> {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(DEBOUNCE_TIMEOUT, Some(TICK_RATE), Some(FLUSH_AFTER_EMPTY), notify_tx)
        .context("failed to create debouncer")?;

    let worktree_path = gix::path::realpath(worktree_path)?;
    let repo = gix::open_opts(&worktree_path, gix::open::Options::isolated()).context(format!(
        "failed to open project repository to obtain git-dir: {}",
        worktree_path.display()
    ))?;
    let git_dir = repo.path().to_owned();

    let mut effective_watch_mode = watch_mode;

    match watch_mode {
        WatchMode::Legacy => {
            setup_legacy_watch(&mut debouncer, &worktree_path, &git_dir)?;
        }
        WatchMode::Modern => {
            setup_watch_plan(&mut debouncer, project_id, &repo, &worktree_path, &git_dir)?;
        }
        WatchMode::Auto => {
            if is_wsl() {
                match setup_watch_plan(&mut debouncer, project_id, &repo, &worktree_path, &git_dir) {
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
    let worktree_path = worktree_path.to_owned();
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
                project_dedup = tracing::field::Empty,
                git = tracing::field::Empty,
                git_dedup = tracing::field::Empty,
                git_noop = tracing::field::Empty,
                fs_events = tracing::field::Empty,
            )
            .entered();
            let (mut ignored, mut git_noop) = (0, 0);
            match result {
                Err(err) => {
                    tracing::error!(?err, "ignored file watcher error");
                }
                Ok(events) => {
                    let num_events = events.len();
                    let mut classified_file_paths: Vec<_> = events
                        .into_iter()
                        .filter(|event| is_interesting_kind(event.kind))
                        .flat_map(|event| event.event.paths)
                        .map(|file| {
                            let kind = classify_file(&git_dir, &file);
                            (file, kind)
                        })
                        .collect();
                    let mut ignore_filtering_ran = false;
                    if classified_file_paths.iter().any(|(_, kind)| *kind == FileKind::Project)
                        && let Ok(repo_with_complete_configuration) = gix::open(&worktree_path)
                        && let Ok(index) = repo_with_complete_configuration.index_or_empty()
                        && let Ok(mut excludes) = repo_with_complete_configuration.excludes(
                            &index,
                            None,
                            gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
                        )
                    {
                        ignore_filtering_ran = true;
                        let icase_acc = build_index_icase_accelerator_if_needed(&repo, &index);
                        let is_untracked = |relative_path: &BStr, is_dir: bool| -> bool {
                            !is_tracked_in_index(relative_path, is_dir, &index, icase_acc.as_ref())
                        };
                        for (file_path, kind) in classified_file_paths.iter_mut() {
                            if let Ok(relative_path) = file_path.strip_prefix(&worktree_path) {
                                let is_dir = file_path.is_dir();
                                let is_excluded = excludes
                                    .at_path(relative_path, is_dir.then_some(gix::index::entry::Mode::DIR))
                                    .map(|platform| platform.is_excluded())
                                    .unwrap_or(false);
                                let repo_relative_path = to_repo_relative_path(relative_path);
                                if is_excluded && is_untracked(&repo_relative_path, is_dir) {
                                    *kind = FileKind::ProjectIgnored
                                }
                            }
                        }
                    }

                    #[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
                    enum Mode {
                        AddWatch,
                        RemoveWatch,
                    }
                    let directories_to_watch_or_unwatch = if dynamic_watch_enabled && ignore_filtering_ran {
                        classified_file_paths
                            .iter()
                            .filter_map(|(path, kind)| {
                                if *kind != FileKind::Project {
                                    return None;
                                };
                                let mode = match path.symlink_metadata() {
                                    Ok(md) => (is_watchable_directory(md.file_type())
                                        && !dynamically_watched_dirs.contains(path))
                                    .then_some(Mode::AddWatch)?,
                                    Err(err) => (err.kind() == std::io::ErrorKind::NotFound)
                                        // We don't care if was dynamically watched, it might be watched during initial computation.
                                        .then_some(Mode::RemoveWatch)?,
                                };
                                Some((mode, path.clone()))
                            })
                            .collect()
                    } else {
                        BTreeSet::new()
                    };
                    let (mut stripped_git_paths, mut worktree_relative_paths) = (HashSet::new(), HashSet::new());
                    for (file_path, kind) in classified_file_paths {
                        match kind {
                            FileKind::ProjectIgnored => ignored += 1,
                            FileKind::GitUninteresting => git_noop += 1,
                            FileKind::Project | FileKind::Git => match file_path.strip_prefix(&worktree_path) {
                                Ok(relative_file_path) => {
                                    if relative_file_path.as_os_str().is_empty() {
                                        continue;
                                    }
                                    if let Ok(stripped) = relative_file_path.strip_prefix(".git") {
                                        stripped_git_paths.insert(stripped.to_owned());
                                    } else {
                                        worktree_relative_paths.insert(relative_file_path.to_owned());
                                    };
                                }
                                Err(_) => {
                                    tracing::warn!(%project_id, ?file_path, ?worktree_path, "failed to strip prefix");
                                }
                            },
                        }
                    }

                    stats.record("fs_events", num_events);
                    stats.record("ignored", ignored);
                    stats.record("git_noop", git_noop);
                    stats.record("git", stripped_git_paths.len());
                    stats.record("project", worktree_relative_paths.len());

                    // NOTE: There is an inherent race condition here where files created in the new
                    //       directory before the watch is established will be missed.
                    //       Fortunately that's not a problem right now as we don't really care about the paths.
                    if !directories_to_watch_or_unwatch.is_empty() {
                        tracing::trace!(%project_id, ?directories_to_watch_or_unwatch, "adding or removing dynamic watches");
                        for (mode, path) in directories_to_watch_or_unwatch {
                            let res = match mode {
                                Mode::AddWatch => debouncer.watcher().watch(&path, notify::RecursiveMode::NonRecursive),
                                Mode::RemoveWatch => debouncer.watcher().unwatch(&path),
                            }
                            .inspect_err(|err| {
                                tracing::warn!(
                                    %project_id,
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

                    if !stripped_git_paths.is_empty() {
                        let paths_dedup: Vec<_> = stripped_git_paths.into_iter().collect();
                        stats.record("git_dedup", paths_dedup.len());
                        let event = InternalEvent::GitFilesChange(project_id, paths_dedup);
                        if out.send(event).is_err() {
                            tracing::info!("channel closed - stopping file watcher");
                            break 'outer;
                        }
                    }
                    if !worktree_relative_paths.is_empty() {
                        let paths_dedup: Vec<_> = worktree_relative_paths.into_iter().collect();
                        stats.record("project_dedup", paths_dedup.len());
                        let event = InternalEvent::ProjectFilesChange(project_id, paths_dedup);
                        if out.send(event).is_err() {
                            tracing::info!("channel closed - stopping file watcher");
                            break 'outer;
                        }
                    }
                }
            }
        }
    });
    Ok(FileMonitorHandle { cmd_tx })
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
        notify::ErrorKind::PathNotFound => {
            backoff::Error::permanent(anyhow!("{} not found", path.display()).into_boxed_dyn_error())
        }
        notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
            backoff::Error::permanent(anyhow::Error::from(err).into_boxed_dyn_error())
        }
        _ => backoff::Error::transient(anyhow::Error::from(err).into_boxed_dyn_error()),
    }
}

fn backoff_err_to_anyhow(err: backoff::Error<Box<dyn std::error::Error + Send + Sync + 'static>>) -> anyhow::Error {
    anyhow::Error::from_boxed(Box::from(err.to_string()))
}

pub const LOCAL_REFS_DIR: &str = "refs/heads/";
pub const FETCH_HEAD: &str = "FETCH_HEAD";
pub const HEAD: &str = "HEAD";
pub const HEAD_ACTIVITY: &str = "logs/HEAD";
pub const INDEX: &str = "index";
pub const GB_FLUSH: &str = "GB_FLUSH";

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
            || check_file_path.starts_with(LOCAL_REFS_DIR)
        {
            FileKind::Git
        } else {
            FileKind::GitUninteresting
        }
    } else {
        FileKind::Project
    }
}
