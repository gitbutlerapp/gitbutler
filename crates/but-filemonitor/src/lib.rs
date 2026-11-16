//! Repository invalidations and demand-driven status snapshots backed by Gix.
#![deny(unsafe_code)]

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

use anyhow::{Context as _, Result};
use gix::{
    notify::{Changes, InvalidationPath, Outcome, RepositoryMonitor},
    status::{
        WorktreeStatusHandle,
        monitor::{Options, Update},
    },
};
use tokio::sync::{mpsc, oneshot};

/// Work requested by the application. Dropping every sender stops the monitor.
pub enum Request {
    /// Reconcile subscriptions before rescanning all selected checkouts.
    Rescan,
    /// Keep status for these open panes even when clean. Release handles for panes that closed.
    /// Filesystem subscriptions continue independently.
    SetCheckouts(Vec<PathBuf>),
    /// Read status for a pane or the navigator, retaining the handle if dirty or in use by a pane.
    Snapshot {
        /// The private Git directory of the checkout whose status is requested.
        git_dir: PathBuf,
        /// Extra invalidation supplied by the caller.
        refresh: StatusRefresh,
        /// Receives the complete snapshot, or the refresh error without discarding cached status.
        response: oneshot::Sender<Result<Snapshot>>,
    },
}

/// How to refresh a snapshot in addition to consuming pending filesystem notifications.
pub enum StatusRefresh {
    /// Use the invalidations collected by the monitor.
    Pending,
    /// Include changes known to the caller.
    Scoped(Changes),
    /// Reconcile subscriptions and invalidate the entire snapshot.
    Full,
}

/// A successful status snapshot with the coverage recomputed for its consumer.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// Shared status items with their checkout and baseline HEAD.
    pub status: gix::status::monitor::Snapshot,
    /// Content refresh coverage remains significant even if status items compare equal.
    pub update: Update,
}

/// A project monitor with on-demand status for its checkouts and GitButler's refresh signals.
///
/// Construct this off the UI thread, then run it on a Tokio runtime. Notification service
/// continues while status consumers are hidden; status refreshes run only on request.
pub struct FileMonitor<F> {
    repository: RepositoryMonitor,
    selector: F,
    open_options: gix::open::Options,
    status_options: fn(&gix::Repository) -> Result<Options>,
}

struct IdleMonitor {
    monitors: Option<Monitors>,
    runtime: tokio::runtime::Handle,
}

struct Monitors {
    repository: RepositoryMonitor,
    status: HashMap<PathBuf, WorktreeStatusHandle>,
}

impl Drop for IdleMonitor {
    fn drop(&mut self) {
        if let Some(monitors) = self.monitors.take() {
            // Cancellation must not join native watcher threads on the executor.
            self.runtime.spawn_blocking(move || drop(monitors));
        }
    }
}

impl<F> FileMonitor<F>
where
    F: FnMut(&gix::Repository) -> Result<Option<Vec<PathBuf>>> + Send + 'static,
{
    /// Resolve coverage and GitButler's signal paths, performing filesystem work.
    /// Coverage and timing use `notifications`. The selector supplies the worktree filter, and
    /// GitButler supplies the additional signal paths.
    /// Status options are read again for each requested checkout to follow submodule configuration.
    pub fn new(
        repo: &gix::Repository,
        notifications: gix::notify::Options,
        status_options: fn(&gix::Repository) -> Result<Options>,
        mut selector: F,
    ) -> Result<Self> {
        let options = repository_options(repo, selector(repo)?, notifications)?;
        let mut repository = repo.monitor(options)?;
        repository.set_loss_debounce(Some(repository.options().max_latency))?;
        Ok(Self {
            repository,
            selector,
            open_options: repo.open_options().clone_without_trust(),
            status_options,
        })
    }

    /// Establish coverage and monitor until every request sender is dropped.
    /// Every outcome is forwarded whole, including simultaneous errors and invalidations.
    /// Filesystem work and native teardown remain on Tokio blocking workers.
    pub async fn run(
        self,
        mut requests: mpsc::UnboundedReceiver<Request>,
        mut on_change: impl FnMut(Outcome),
    ) -> Result<()> {
        let Self {
            repository,
            mut selector,
            open_options,
            status_options,
        } = self;
        let git_dir = repository.layout().git_dir.clone();
        let retry_interval = repository.options().retry_interval;
        let mut refresh_options = false;
        let mut rescan = false;
        let mut retry = None;
        let mut request = None;
        let mut checkouts = HashSet::new();
        let mut idle = IdleMonitor {
            monitors: Some(Monitors {
                repository,
                status: HashMap::new(),
            }),
            runtime: tokio::runtime::Handle::current(),
        };

        loop {
            let open_at = git_dir.clone();
            let options = open_options.clone_without_trust();
            let mut work = idle;
            let (next, outcome, response) = tokio::task::spawn_blocking(move || {
                let now = Instant::now();
                let monitors = work
                    .monitors
                    .as_mut()
                    .expect("the worker owns the monitors");
                let project = &mut monitors.repository;
                if let Some(Request::SetCheckouts(paths)) = &request {
                    let next: HashSet<_> = paths
                        .iter()
                        .map(|path| gix::path::realpath(path).unwrap_or_else(|_| path.clone()))
                        .collect();
                    monitors.status.retain(|git_dir, _| {
                        !checkouts.contains(git_dir) || next.contains(git_dir)
                    });
                    checkouts = next;
                }
                let mut preparation_errors = Vec::new();
                if refresh_options && retry.is_none_or(|deadline| deadline <= now) {
                    let prepared = (|| -> Result<()> {
                        let repo = gix::open_opts(&open_at, options.clone_without_trust())?;
                        let next = repository_options(&repo, selector(&repo)?, Default::default())?;
                        project
                            .update_subscriptions(next.worktree_filter, next.additional_paths)?;
                        Ok(())
                    })();
                    match prepared {
                        Ok(()) => {
                            refresh_options = false;
                            retry = None;
                        }
                        Err(error) => {
                            preparation_errors
                                .push(gix::Error::from_boxed(error.into_boxed_dyn_error()));
                            retry = Some(now + retry_interval);
                        }
                    }
                }
                // Excluded indexes must not be opened by a rescan before selection is updated.
                let blocked = refresh_options && retry.is_some();
                let mut outcome = if blocked {
                    Outcome::default()
                } else {
                    if rescan {
                        project.rescan();
                        for handle in monitors.status.values_mut() {
                            handle.rescan();
                        }
                        rescan = false;
                    }
                    project.service(now, || {
                        gix::open_opts(&open_at, options.clone_without_trust())
                    })
                };
                outcome.errors.extend(preparation_errors);
                refresh_options |= outcome.changes.configuration || outcome.changes.layout;
                monitors.status.retain(|_, handle| !handle.is_closed());
                if !blocked {
                    project.service_status_handles(now, &mut outcome, monitors.status.values_mut());
                }
                let response = match request {
                    Some(Request::Snapshot {
                        git_dir,
                        refresh,
                        response,
                    }) if !response.is_closed() => {
                        let snapshot = (|| -> Result<Snapshot> {
                            anyhow::ensure!(
                                !blocked,
                                "status refresh waits for subscription selection to recover"
                            );
                            let git_dir = gix::path::realpath(&git_dir)?;
                            let repo = project.open_worktree(&git_dir)?;
                            let status_options = status_options(&repo)?;
                            let handle = match monitors.status.entry(git_dir.clone()) {
                                std::collections::hash_map::Entry::Occupied(entry) => {
                                    let handle = entry.into_mut();
                                    handle.set_submodules(status_options.submodules);
                                    handle
                                }
                                std::collections::hash_map::Entry::Vacant(entry) => {
                                    entry.insert(project.worktree_status(&repo, status_options)?)
                                }
                            };
                            match refresh {
                                StatusRefresh::Pending => {}
                                StatusRefresh::Scoped(changes) => {
                                    handle.invalidate_changes(&changes);
                                }
                                StatusRefresh::Full => handle.invalidate(),
                            }
                            let result = handle
                                .refresh(&repo, &AtomicBool::new(false))
                                .map_err(anyhow::Error::from)
                                .and_then(|update| {
                                    Ok(Snapshot {
                                        status: handle
                                            .shared_snapshot()
                                            .context("A successful refresh owns status")?,
                                        update,
                                    })
                                });
                            if !checkouts.contains(&git_dir)
                                && handle.snapshot().is_none_or(<[_]>::is_empty)
                            {
                                monitors.status.remove(&git_dir);
                            }
                            result
                        })();
                        Some((response, snapshot))
                    }
                    _ => None,
                };
                outcome.coverage = project.coverage_with_status(monitors.status.values());
                (
                    (work, selector, refresh_options, rescan, retry, checkouts),
                    outcome,
                    response,
                )
            })
            .await
            .context("servicing GitButler's repository monitor")?;
            (idle, selector, refresh_options, rescan, retry, checkouts) = next;
            request = None;
            on_change(outcome);
            if let Some((response, snapshot)) = response {
                response.send(snapshot).ok();
            }
            if refresh_options && retry.is_none() {
                continue;
            }

            let monitors = idle
                .monitors
                .as_ref()
                .context("service returns the monitors")?;
            tokio::select! {
                biased;
                received = requests.recv() => match received {
                    Some(next) => {
                        if matches!(&next, Request::Rescan | Request::Snapshot { refresh: StatusRefresh::Full, .. }) {
                            refresh_options = true;
                            retry = None;
                        }
                        rescan |= matches!(&next, Request::Rescan);
                        request = Some(next);
                    }
                    None => break,
                },
                _ = monitors.repository.ready_with_status_async(monitors.status.values()), if retry.is_none() => {},
                _ = async {
                    match retry {
                        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {},
            }
        }
        let monitors = idle.monitors.take();
        tokio::task::spawn_blocking(move || drop(monitors))
            .await
            .context("shutting down GitButler's repository monitor")
    }
}

fn repository_options(
    repo: &gix::Repository,
    mut worktree_filter: Option<Vec<PathBuf>>,
    options: gix::notify::Options,
) -> Result<gix::notify::Options> {
    let sentinel = Path::new(but_project_handle::REFRESH_SENTINEL_PATH)
        .file_name()
        .context("Refresh sentinel has a filename")?;
    if let Some(paths) = &mut worktree_filter {
        paths.sort();
        paths.dedup();
    }
    let full = Changes {
        references: true,
        layout: true,
        ..Changes::default()
    };
    Ok(gix::notify::Options {
        all_worktrees: true,
        worktree_filter,
        additional_paths: vec![
            InvalidationPath {
                path: but_project_handle::gitbutler_storage_path(repo)?.join(sentinel),
                changes: full.clone(),
            },
            InvalidationPath {
                path: repo.git_dir().join("GB_FLUSH"),
                changes: full,
            },
            InvalidationPath {
                path: repo.git_dir().join("logs/HEAD"),
                changes: Changes {
                    references: true,
                    ..Changes::default()
                },
            },
        ],
        ..options
    })
}
