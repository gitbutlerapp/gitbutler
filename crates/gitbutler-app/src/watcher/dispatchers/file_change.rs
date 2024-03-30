use std::{
    path,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, Result};
use futures::executor::block_on;
use gitbutler::{git, projects::ProjectId};
use notify::{RecommendedWatcher, Watcher};
use notify_debouncer_full::{new_debouncer, Debouncer, FileIdMap};
use tokio::{
    sync::mpsc::{channel, Receiver},
    task,
};

use crate::watcher::events;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<Debouncer<RecommendedWatcher, FileIdMap>>>>,
}

/// The timeout for debouncing file change events.
/// This is used to prevent multiple events from being sent for a single file change.
static DEBOUNCE_TIMEOUT: Duration = Duration::from_millis(100);

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{0} not found")]
    PathNotFound(path::PathBuf),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    pub fn stop(&self) {
        self.watcher.lock().unwrap().take();
    }

    pub fn run(
        self,
        project_id: &ProjectId,
        path: &path::Path,
    ) -> Result<Receiver<events::Event>, RunError> {
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE_TIMEOUT, None, notify_tx)
            .context("failed to create debouncer")?;

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(std::time::Duration::from_secs(30)))
            .build();

        backoff::retry(policy, || {
            debouncer
                .watcher()
                .watch(path, notify::RecursiveMode::Recursive)
                .map_err(|error| match error.kind {
                    notify::ErrorKind::PathNotFound => {
                        backoff::Error::permanent(RunError::PathNotFound(path.to_path_buf()))
                    }
                    notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
                        backoff::Error::permanent(RunError::Other(error.into()))
                    }
                    _ => backoff::Error::transient(RunError::Other(error.into())),
                })
        })
        .context("failed to start watcher")?;

        let repo = git::Repository::open(path).context(format!(
            "failed to open project repository: {}",
            path.display()
        ))?;

        self.watcher.lock().unwrap().replace(debouncer);

        tracing::debug!(%project_id, "file watcher started");

        let (tx, rx) = channel(1);
        task::spawn_blocking({
            let path = path.to_path_buf();
            let project_id = *project_id;
            move || {
                for result in notify_rx {
                    match result {
                        Err(errors) => {
                            tracing::error!(?errors, "file watcher error");
                        }
                        Ok(events) => {
                            let file_paths = events
                                .into_iter()
                                .filter(|event| is_interesting_kind(event.kind))
                                .flat_map(|event| event.paths.clone())
                                .filter(|file| is_interesting_file(&repo, file));
                            for file_path in file_paths {
                                match file_path.strip_prefix(&path) {
                                    Ok(relative_file_path)
                                        if relative_file_path.display().to_string().is_empty() =>
                                    { /* noop */ }
                                    Ok(relative_file_path) => {
                                        let event = if relative_file_path.starts_with(".git") {
                                            tracing::info!(
                                                %project_id,
                                                file_path = %relative_file_path.display(),
                                                "git file change",
                                            );
                                            events::Event::GitFileChange(
                                                project_id,
                                                relative_file_path
                                                    .strip_prefix(".git")
                                                    .unwrap()
                                                    .to_path_buf(),
                                            )
                                        } else {
                                            tracing::info!(
                                                %project_id,
                                                file_path = %relative_file_path.display(),
                                                "project file change",
                                            );
                                            events::Event::ProjectFileChange(
                                                project_id,
                                                relative_file_path.to_path_buf(),
                                            )
                                        };
                                        if let Err(error) = block_on(tx.send(event)) {
                                            tracing::error!(
                                                %project_id,
                                                ?error,
                                                "failed to send file change event",
                                            );
                                        }
                                    }
                                    Err(error) => {
                                        tracing::error!(%project_id, ?error, "failed to strip prefix");
                                    }
                                }
                            }
                        }
                    }
                }
                tracing::debug!(%project_id, "file watcher stopped");
            }
        });

        Ok(rx)
    }
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

fn is_interesting_file(git_repo: &git::Repository, file_path: &path::Path) -> bool {
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
