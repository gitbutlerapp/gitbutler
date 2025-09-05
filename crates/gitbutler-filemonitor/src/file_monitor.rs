use anyhow::{anyhow, Context, Result};
use gitbutler_notify_debouncer::{new_debouncer, Debouncer, NoCache};
use gitbutler_project::ProjectId;
use notify::{RecommendedWatcher, Watcher};
use std::{collections::HashSet, path::Path, time::Duration};
use tokio::task;
use tracing::Level;

use crate::events::InternalEvent;

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
    Duration::from_millis(50)
};

// The number of TICK_RATE intervals required of "dead air" (i.e. no new events
// arriving) before we will automatically flush pending events. This means that
// after the disk is quiet for TICK_RATE * FLUSH_AFTER_EMPTY, we will process
// the pending events, even if DEBOUNCE_TIMEOUT hasn't expired yet
const FLUSH_AFTER_EMPTY: u32 = 3;

/// This error is required only because `anyhow::Error` isn't implementing `std::error::Error`, and [`spawn()`]
/// needs to wrap it into a `backoff::Error` which also has to implement the `Error` trait.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
struct RunError {
    #[from]
    source: anyhow::Error,
}

/// Listen to interesting filesystem events of files in `path` that are not `.gitignore`d,
/// turn them into [`Events`](Event) which classifies it, and associates it with `project_id`.
/// These are sent through the passed `out` channel, to indicate either **Git** repository changes
/// or **ProjectWorktree** changes
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
) -> Result<Debouncer<RecommendedWatcher, NoCache>> {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(
        DEBOUNCE_TIMEOUT,
        Some(TICK_RATE),
        Some(FLUSH_AFTER_EMPTY),
        notify_tx,
    )
    .context("failed to create debouncer")?;

    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(30)))
        .build();

    let worktree_path = gix::path::realpath(worktree_path)?;
    let git_dir = gix::open_opts(&worktree_path, gix::open::Options::isolated())
        .context(format!(
            "failed to open project repository to obtain git-dir: {}",
            worktree_path.display()
        ))?
        .path()
        .to_owned();
    let extra_git_dir_to_watch = {
        let mut enclosing_worktree_dir = git_dir.clone();
        enclosing_worktree_dir.pop();
        if enclosing_worktree_dir != worktree_path {
            Some(git_dir.as_path())
        } else {
            None
        }
    };

    // Start the watcher, but retry if there are transient errors.
    backoff::retry(policy, || {
        debouncer
            .watcher()
            .watch(&worktree_path, notify::RecursiveMode::Recursive)
            .and_then(|()| {
                if let Some(git_dir) = extra_git_dir_to_watch {
                    debouncer
                        .watcher()
                        .watch(git_dir, notify::RecursiveMode::Recursive)
                } else {
                    Ok(())
                }
            })
            .map_err(|err| match err.kind {
                notify::ErrorKind::PathNotFound => backoff::Error::permanent(RunError::from(
                    anyhow!("{} not found", worktree_path.display()),
                )),
                notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
                    backoff::Error::permanent(RunError::from(anyhow::Error::from(err)))
                }
                _ => backoff::Error::transient(RunError::from(anyhow::Error::from(err))),
            })
    })
    .context("failed to start watcher")?;

    let worktree_path = worktree_path.to_owned();
    task::spawn_blocking(move || {
        let _runtime = tracing::span!(Level::INFO, "file monitor", %project_id ).entered();
        tracing::debug!(%project_id, "file watcher started");

        'outer: for result in notify_rx {
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
                    if classified_file_paths
                        .iter()
                        .any(|(_, kind)| *kind == FileKind::Project)
                    {
                        if let Ok(repo) = gix::open(&worktree_path) {
                            if let Ok(index) = repo.index_or_empty() {
                                if let Ok(mut excludes) = repo.excludes(
                                    &index,
                                    None,
                                    gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
                                ) {
                                    for (file_path, kind) in classified_file_paths.iter_mut() {
                                        if let Ok(relative_path) = file_path.strip_prefix(&worktree_path) {
                                            let is_excluded = excludes
                                                .at_path(relative_path, None)
                                                .map(|platform| platform.is_excluded())
                                                .unwrap_or(false);
                                            let is_untracked =
                                                || index.entry_by_path(&gix::path::to_unix_separators_on_windows(gix::path::into_bstr(relative_path))).is_none();
                                            if is_excluded && is_untracked() {
                                                *kind = FileKind::ProjectIgnored
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    let (mut stripped_git_paths, mut worktree_relative_paths) =
                        (HashSet::new(), HashSet::new());
                    for (file_path, kind) in classified_file_paths {
                        match kind {
                            FileKind::ProjectIgnored => ignored += 1,
                            FileKind::GitUninteresting => git_noop += 1,
                            FileKind::Project | FileKind::Git => match file_path
                                .strip_prefix(&worktree_path)
                            {
                                Ok(relative_file_path) => {
                                    if relative_file_path.as_os_str().is_empty() {
                                        continue;
                                    }
                                    if let Ok(stripped) = relative_file_path.strip_prefix(".git") {
                                        stripped_git_paths.insert(stripped.to_owned());
                                    } else {
                                        worktree_relative_paths
                                            .insert(relative_file_path.to_owned());
                                    };
                                }
                                Err(err) => {
                                    tracing::error!(%project_id, ?err, "failed to strip prefix");
                                }
                            },
                        }
                    }

                    stats.record("fs_events", num_events);
                    stats.record("ignored", ignored);
                    stats.record("git_noop", git_noop);
                    stats.record("git", stripped_git_paths.len());
                    stats.record("project", worktree_relative_paths.len());

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
    Ok(debouncer)
}

#[cfg(target_family = "unix")]
fn is_interesting_kind(kind: notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(notify::event::CreateKind::File)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Remove(notify::event::RemoveKind::File)
    )
}

#[cfg(target_os = "windows")]
fn is_interesting_kind(kind: notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(_) | notify::EventKind::Modify(_) | notify::EventKind::Remove(_)
    )
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
        if check_file_path == Path::new("FETCH_HEAD")
            || check_file_path == Path::new("logs/HEAD")
            || check_file_path == Path::new("HEAD")
            || check_file_path == Path::new("GB_FLUSH")
            || check_file_path == Path::new("index")
        {
            FileKind::Git
        } else {
            FileKind::GitUninteresting
        }
    } else {
        FileKind::Project
    }
}
