use std::path::Path;
use std::{path, time::Duration};

use crate::watcher::events::InternalEvent;
use anyhow::{anyhow, Context, Result};
use gitbutler_core::{git, projects::ProjectId};
use notify::Watcher;
use notify_debouncer_full::new_debouncer;
use tokio::task;

/// The timeout for debouncing file change events.
/// This is used to prevent multiple events from being sent for a single file change.
static DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(100);

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

    let repo = git::Repository::open(worktree_path).context(format!(
        "failed to open project repository: {}",
        worktree_path.display()
    ))?;

    tracing::debug!(%project_id, "file watcher started");

    let path = worktree_path.to_owned();
    task::spawn_blocking(move || {
        let _debouncer = debouncer;
        'outer: for result in notify_rx {
            match result {
                Err(err) => {
                    tracing::error!(?err, "file watcher error");
                }
                Ok(events) => {
                    let file_paths = events
                        .into_iter()
                        .filter(|event| is_interesting_kind(event.kind))
                        .flat_map(|event| event.event.paths)
                        .filter(|file| is_interesting_file(&repo, file));
                    for file_path in file_paths {
                        match file_path.strip_prefix(&path) {
                            Ok(relative_file_path) => {
                                if relative_file_path.as_os_str().is_empty() {
                                    continue;
                                }
                                let event = if let Ok(stripped) =
                                    relative_file_path.strip_prefix(".git")
                                {
                                    InternalEvent::GitFileChange(project_id, stripped.to_owned())
                                } else {
                                    InternalEvent::ProjectFileChange(
                                        project_id,
                                        relative_file_path.to_path_buf(),
                                    )
                                };
                                if out.send(event).is_err() {
                                    tracing::info!("channel closed - stopping file watcher");
                                    break 'outer;
                                }
                            }
                            Err(err) => {
                                tracing::error!(%project_id, ?err, "failed to strip prefix");
                            }
                        }
                    }
                }
            }
        }
        tracing::debug!(%project_id, "file watcher stopped");
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

fn is_interesting_file(git_repo: &git::Repository, file_path: &Path) -> bool {
    if file_path.starts_with(git_repo.path()) {
        let check_file_path = file_path.strip_prefix(git_repo.path()).unwrap();
        check_file_path.ends_with("FETCH_HEAD")
            || check_file_path.eq(path::Path::new("logs/HEAD"))
            || check_file_path.eq(path::Path::new("HEAD"))
            || check_file_path.eq(path::Path::new("GB_FLUSH"))
            || check_file_path.eq(path::Path::new("index"))
    } else {
        !git_repo.is_path_ignored(file_path).unwrap_or(false)
    }
}
