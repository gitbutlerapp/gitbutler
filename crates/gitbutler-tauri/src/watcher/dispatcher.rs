mod file_monitor {
    use std::{
        path,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use anyhow::{anyhow, Context, Result};
    use futures::executor::block_on;
    use gitbutler_core::{git, projects::ProjectId};
    use notify::{RecommendedWatcher, Watcher};
    use notify_debouncer_full::{new_debouncer, Debouncer, FileIdMap};
    use tokio::{
        sync::mpsc::{channel, Receiver},
        task,
    };

    use crate::watcher::events::Event;

    #[derive(Debug, Clone)]
    pub struct Dispatcher {
        watcher: Arc<Mutex<Option<Debouncer<RecommendedWatcher, FileIdMap>>>>,
    }

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
            &self,
            project_id: &ProjectId,
            path: &path::Path,
        ) -> Result<Receiver<Event>, anyhow::Error> {
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
                    .map_err(|err| match err.kind {
                        notify::ErrorKind::PathNotFound => backoff::Error::permanent(
                            RunError::from(anyhow!("{} not found", path.display())),
                        ),
                        notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
                            backoff::Error::permanent(RunError::from(anyhow::Error::from(err)))
                        }
                        _ => backoff::Error::transient(RunError::from(anyhow::Error::from(err))),
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
                                            if relative_file_path
                                                .display()
                                                .to_string()
                                                .is_empty() =>
                                        { /* noop */ }
                                        Ok(relative_file_path) => {
                                            let event = if relative_file_path.starts_with(".git") {
                                                tracing::info!(
                                                    %project_id,
                                                    file_path = %relative_file_path.display(),
                                                    "git file change",
                                                );
                                                Event::GitFileChange(
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
                                                Event::ProjectFileChange(
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

    /// Listen to interesting filesystem events of files in `path` that are not `.gitignore`d and pass them to
    /// `tx` as [`Event`] which classifies it, and associates it with `project_id`.
    ///
    /// Configure the channel behind `tx` according to your needs, typically `tokio::sync::mpsc::channel(1)`.
    ///
    /// ### Why is this not an iterator?
    ///
    /// The internal `notify_rx` could be an iterator, which performs all transformations and returns them as item.
    /// However, due to closures being continuously created each time events come in, nested closures need to own
    /// their resources which means they are `Clone` or `Copy`. This isn't the case for `git::Repository`.
    /// Even though `gix::Repository` is `Clone`, an efficient implementation of `is_path_ignored()` requires more state
    /// that ideally is kept between invocations. For that reason, the current channel-based 'worker' architecture
    /// is chosen to allow all this state to live on the stack.
    pub fn spawn(
        project_id: ProjectId,
        path: &path::Path,
        tx: tokio::sync::mpsc::Sender<Event>,
    ) -> Result<(), anyhow::Error> {
        let (notify_tx, notify_rx) = std::sync::mpsc::channel();
        let mut debouncer = new_debouncer(DEBOUNCE_TIMEOUT, None, notify_tx)
            .context("failed to create debouncer")?;

        let policy = backoff::ExponentialBackoffBuilder::new()
            .with_max_elapsed_time(Some(std::time::Duration::from_secs(30)))
            .build();

        // Start the watcher, but retry if there are transient errors.
        backoff::retry(policy, || {
            debouncer
                .watcher()
                .watch(path, notify::RecursiveMode::Recursive)
                .map_err(|err| match err.kind {
                    notify::ErrorKind::PathNotFound => backoff::Error::permanent(RunError::from(
                        anyhow!("{} not found", path.display()),
                    )),
                    notify::ErrorKind::Io(_) | notify::ErrorKind::InvalidConfig(_) => {
                        backoff::Error::permanent(RunError::from(anyhow::Error::from(err)))
                    }
                    _ => backoff::Error::transient(RunError::from(anyhow::Error::from(err))),
                })
        })
        .context("failed to start watcher")?;

        let repo = git::Repository::open(path).context(format!(
            "failed to open project repository: {}",
            path.display()
        ))?;

        tracing::debug!(%project_id, "file watcher started");

        let path = path.to_owned();
        task::spawn_blocking(move || {
            'outer: for result in notify_rx {
                match result {
                    Err(err) => {
                        tracing::error!(?err, "file watcher error");
                    }
                    Ok(events) => {
                        let file_paths = events
                            .into_iter()
                            .filter(|event| is_interesting_kind(event.kind))
                            .flat_map(|event| event.paths.clone())
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
                                        tracing::info!(
                                            %project_id,
                                            file_path = %relative_file_path.display(),
                                            "git file change",
                                        );
                                        Event::GitFileChange(project_id, stripped.to_owned())
                                    } else {
                                        tracing::info!(
                                            %project_id,
                                            file_path = %relative_file_path.display(),
                                            "project file change",
                                        );
                                        Event::ProjectFileChange(
                                            project_id,
                                            relative_file_path.to_path_buf(),
                                        )
                                    };
                                    if block_on(tx.send(event)).is_err() {
                                        // channel closed
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
            notify::EventKind::Create(_)
                | notify::EventKind::Modify(_)
                | notify::EventKind::Remove(_)
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
}

use std::path;
use std::path::Path;

use anyhow::Result;
use gitbutler_core::projects::ProjectId;
use tokio::{
    select,
    sync::mpsc::{channel, Receiver},
    task,
};
use tokio_util::sync::CancellationToken;

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    file_change_dispatcher: file_monitor::Dispatcher,
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            file_change_dispatcher: file_monitor::Dispatcher::new(),
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) {
        self.file_change_dispatcher.stop();
    }

    pub fn run<P: AsRef<path::Path>>(
        &self,
        project_id: &ProjectId,
        path: P,
    ) -> Result<Receiver<events::Event>, anyhow::Error> {
        let path = path.as_ref();

        let mut file_change_rx = self.file_change_dispatcher.run(project_id, path)?;

        let (tx, rx) = channel(1);
        let project_id = *project_id;
        let cancellation_token = self.cancellation_token.clone();
        task::spawn(async move {
            loop {
                select! {
                    () = cancellation_token.cancelled() => {
                        break;
                    }
                    Some(event) = file_change_rx.recv() => {
                        if let Err(error) = tx.send(event).await {
                            tracing::error!(%project_id, ?error,"failed to send file change");
                        }
                    }
                }
            }
            tracing::debug!(%project_id, "dispatcher stopped");
        });

        Ok(rx)
    }
}

/// Return a channel which provides change-events of for `project_id` at `path`.
pub fn new(
    project_id: ProjectId,
    path: impl AsRef<Path>,
) -> Result<Receiver<events::Event>, anyhow::Error> {
    // TODO(ST): is the size of 1 really required? It's unbounded internally, and could be just as unbounded here.
    //           If so, people can call `spawn` directly.
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    file_monitor::spawn(project_id, path.as_ref(), tx)?;
    Ok(rx)
}
