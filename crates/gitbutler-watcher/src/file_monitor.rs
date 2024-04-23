use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;

use crate::events::InternalEvent;
use anyhow::{anyhow, Context, Result};
use gitbutler_core::{git, projects::ProjectId};
use notify::Watcher;
use notify_debouncer_full::new_debouncer;
use tokio::task;
use tracing::Level;

/// The timeout for debouncing file change events.
/// This is used to prevent multiple events from being sent for a single file change.
const DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(100);

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
/// However, due to closures being continuously created each time events come in, nested closures need to own
/// their resources which means they are `Clone` or `Copy`. This isn't the case for `git::Repository`.
/// Even though `gix::Repository` is `Clone`, an efficient implementation of `is_path_ignored()` requires more state
/// that ideally is kept between invocations. For that reason, the current channel-based 'worker' architecture
/// is chosen to allow all this state to live on the stack.
///
/// Additionally, a channel plays better with how events are handled downstream.
pub fn spawn(
    project_id: ProjectId,
    worktree_path: &std::path::Path,
    out: tokio::sync::mpsc::UnboundedSender<InternalEvent>,
) -> Result<()> {
    let (notify_tx, notify_rx) = std::sync::mpsc::channel();
    let mut debouncer =
        new_debouncer(DEBOUNCE_TIMEOUT, None, notify_tx).context("failed to create debouncer")?;

    let policy = backoff::ExponentialBackoffBuilder::new()
        .with_max_elapsed_time(Some(std::time::Duration::from_secs(30)))
        .build();

    // Start the watcher, but retry if there are transient errors.
    backoff::retry(policy, || {
        debouncer
            .watcher()
            .watch(worktree_path, notify::RecursiveMode::Recursive)
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
        tracing::debug!(%project_id, "file watcher started");
        let _debouncer = debouncer;
        let _runtime = tracing::span!(Level::INFO, "file monitor", %project_id ).entered();
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
                    let maybe_repo = git::Repository::open(&worktree_path)
                        .with_context(|| format!("failed to open project repository: {}", worktree_path.display()))
                        .map(Some)
                        .unwrap_or_else(|err| {
                            tracing::error!(
                                ?err,
                                "will consider changes to all files as repository couldn't be opened"
                            );
                            None
                        });

                    let num_events = events.len();
                    let classified_file_paths = events
                        .into_iter()
                        .filter(|event| is_interesting_kind(event.kind))
                        .flat_map(|event| event.event.paths)
                        .map(|file| {
                            let kind = maybe_repo
                                .as_ref()
                                .map_or(FileKind::Project, |repo| classify_file(repo, &file));
                            (file, kind)
                        });
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
    Ok(())
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

fn classify_file(git_repo: &git::Repository, file_path: &Path) -> FileKind {
    if let Ok(check_file_path) = file_path.strip_prefix(git_repo.path()) {
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
    } else if git_repo.is_path_ignored(file_path).unwrap_or(false) {
        FileKind::ProjectIgnored
    } else {
        FileKind::Project
    }
}
