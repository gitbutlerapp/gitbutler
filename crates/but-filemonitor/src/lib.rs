//! Repository invalidations and demand-driven status snapshots backed by Gix.
#![deny(unsafe_code)]

use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Instant,
};

use anyhow::{Context as _, Result};
use gix::{
    bstr::ByteSlice as _,
    notify::{Changes, InvalidationPath, Outcome, Scope, WorktreeChange, plumbing},
    status::{
        Item, Monitor,
        monitor::{Options, Update},
    },
};
use tokio::sync::{mpsc, oneshot};

/// Work requested by the application. Dropping every sender stops the monitor.
pub enum Request {
    /// Reconcile subscriptions before rescanning all selected checkouts.
    Rescan,
    /// Request status only for these private Git directories, retaining filesystem subscriptions.
    /// Snapshots for checkouts excluded from native coverage need `refresh: true` after external changes.
    SetCheckouts(Vec<PathBuf>),
    /// Refresh the selected checkout's cached status only when its consumer needs it.
    Snapshot {
        /// The private Git directory of the checkout whose status is requested.
        git_dir: PathBuf,
        /// Invalidate the whole snapshot after application-owned mutations or explicit reloads.
        refresh: bool,
        /// Receives the complete snapshot, or the refresh error without discarding cached status.
        response: oneshot::Sender<Result<Snapshot>>,
    },
}

/// A successful status snapshot with the coverage recomputed for its consumer.
#[derive(Clone, Debug)]
pub struct Snapshot {
    /// The private Git directory identifying the checkout these items describe.
    pub git_dir: PathBuf,
    /// Owned staged and worktree status items for the selected checkout.
    pub items: Vec<Item>,
    /// Content refresh coverage remains significant even if status items compare equal.
    pub update: Update,
    /// The committed HEAD baseline retained by the status monitor.
    pub head: Option<gix::ObjectId>,
}

/// A status monitor plus GitButler's refresh and compatibility signals.
///
/// Construct this off the UI thread, then run it on a Tokio runtime. Notification service
/// continues while status consumers are hidden; status refreshes run only on request.
pub struct FileMonitor<F> {
    status: Monitor,
    selector: F,
    open_options: gix::open::Options,
    status_options: fn(&gix::Repository) -> Result<Options>,
}

struct IdleMonitor {
    monitor: Option<Monitor>,
    runtime: tokio::runtime::Handle,
}

impl Drop for IdleMonitor {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            // Cancellation while idle must not join native watcher threads on the executor.
            self.runtime.spawn_blocking(move || drop(monitor));
        }
    }
}

impl<F> FileMonitor<F>
where
    F: FnMut(&gix::Repository) -> Result<Option<Vec<PathBuf>>> + Send + 'static,
{
    /// Resolve coverage and GitButler's signal paths, performing filesystem work.
    /// The selector returns private Git directories to monitor, or `None` for all checkouts.
    /// Status options are read again for each requested checkout to follow submodule configuration.
    pub fn new(
        repo: &gix::Repository,
        status_options: fn(&gix::Repository) -> Result<Options>,
        mut selector: F,
    ) -> Result<Self> {
        let mut options = status_options(repo)?;
        let repository = repository_options(repo, selector(repo)?)?;
        options.repository.all_worktrees = true;
        options.repository.worktree_filter = repository.worktree_filter;
        options.repository.additional_paths = repository.additional_paths;
        let mut status = Monitor::new(repo, options)?;
        status.set_loss_debounce(Some(status.options().repository.max_latency))?;
        if status
            .options()
            .repository
            .checkout_filter
            .as_ref()
            .is_some_and(Vec::is_empty)
        {
            status.select_worktree(None)?;
        }
        Ok(Self {
            status,
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
            status: monitor,
            mut selector,
            open_options,
            status_options,
        } = self;
        let git_dir = monitor.repository_monitor().layout().git_dir.clone();
        let retry_interval = monitor.options().repository.retry_interval;
        let mut refresh_options = false;
        let mut rescan = false;
        let mut retry = None;
        let mut request = None;
        let mut checkouts = monitor.options().repository.checkout_filter.clone();
        let mut idle = IdleMonitor {
            monitor: Some(monitor),
            runtime: tokio::runtime::Handle::current(),
        };

        loop {
            let monitor = idle
                .monitor
                .take()
                .context("The monitor is idle between service calls")?;
            let open_at = git_dir.clone();
            let options = open_options.clone_without_trust();
            let now = Instant::now();
            let processed = monitor
                .process_async(move |monitor| {
                    let mut preparation_errors = Vec::new();
                    if refresh_options && retry.is_none_or(|deadline| deadline <= now) {
                        let prepared = (|| -> Result<()> {
                            let repo = gix::open_opts(&open_at, options.clone_without_trust())?;
                            let next = repository_options(&repo, selector(&repo)?)?;
                            monitor.update_subscriptions(
                                next.worktree_filter,
                                next.additional_paths,
                            )?;
                            if checkouts.as_ref().is_some_and(|paths| {
                                monitor
                                    .selected_worktree()
                                    .is_some_and(|selected| !paths.contains(&selected.git_dir))
                            }) {
                                monitor.select_worktree(None)?;
                            }
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
                            monitor.rescan();
                            rescan = false;
                        }
                        monitor.service(now, || {
                            gix::open_opts(&open_at, options.clone_without_trust())
                        })
                    };
                    outcome.errors.extend(preparation_errors);
                    refresh_options |= outcome.changes.configuration || outcome.changes.layout;
                    filter_outcome(&open_at, &options, &mut outcome);
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
                                anyhow::ensure!(
                                    checkouts
                                        .as_ref()
                                        .is_none_or(|paths| paths.contains(&git_dir)),
                                    "the checkout no longer has a status consumer"
                                );
                                let repo = gix::open_opts(&git_dir, options)?;
                                monitor.select_worktree(Some(&repo))?;
                                monitor.set_submodules(status_options(&repo)?.submodules);
                                if refresh {
                                    monitor.invalidate();
                                }
                                let update = monitor.refresh(&repo, &AtomicBool::new(false))?;
                                let items = monitor
                                    .snapshot()
                                    .context("A successful refresh owns status")?
                                    .to_vec();
                                let head = monitor.snapshot_head_id();
                                Ok(Snapshot {
                                    git_dir: gix::path::realpath(repo.git_dir())?,
                                    items,
                                    update,
                                    head,
                                })
                            })();
                            Some((response, snapshot))
                        }
                        _ => None,
                    };
                    outcome.coverage = monitor.coverage_status();
                    outcome.recovery_pending = matches!(
                        outcome.coverage,
                        gix::notify::CoverageStatus::Recovering { .. }
                    );
                    (
                        selector,
                        refresh_options,
                        rescan,
                        retry,
                        checkouts,
                        outcome,
                        response,
                    )
                })
                .await;
            let (next, state) = match processed {
                Ok(result) => result,
                Err(error) => return Err(error.into()),
            };
            idle.monitor = Some(next);
            let (
                next_selector,
                next_refresh,
                next_rescan,
                next_retry,
                next_checkouts,
                outcome,
                response,
            ) = state;
            selector = next_selector;
            refresh_options = next_refresh;
            rescan = next_rescan;
            retry = next_retry;
            checkouts = next_checkouts;
            request = None;
            on_change(outcome);
            if let Some((response, snapshot)) = response {
                response.send(snapshot).ok();
            }
            if refresh_options && retry.is_none() {
                continue;
            }

            let monitor = idle
                .monitor
                .as_ref()
                .context("Successful service returns the monitor")?;
            tokio::select! {
                biased;
                received = requests.recv() => match received {
                    Some(next) => {
                        if matches!(&next, Request::Rescan | Request::SetCheckouts(_) | Request::Snapshot { refresh: true, .. }) {
                            refresh_options = true;
                            retry = None;
                        }
                        rescan |= matches!(&next, Request::Rescan);
                        if let Request::SetCheckouts(paths) = &next {
                            checkouts = Some(paths.clone());
                        }
                        request = Some(next);
                    }
                    None => break,
                },
                _ = monitor.ready_service_async(), if !(refresh_options && retry.is_some()) => {},
                _ = async {
                    match retry {
                        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
                        None => std::future::pending::<()>().await,
                    }
                } => {},
            }
        }
        match idle.monitor.take() {
            Some(monitor) => monitor.shutdown_async().await.map_err(Into::into),
            None => Ok(()),
        }
    }
}

fn repository_options(
    repo: &gix::Repository,
    mut worktree_filter: Option<Vec<PathBuf>>,
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
        ..Default::default()
    })
}

fn filter_outcome(git_dir: &Path, options: &gix::open::Options, outcome: &mut Outcome) {
    if let Err(error) = filter_paths(git_dir, options, &mut outcome.changes) {
        outcome.errors.push(error);
    }
    for worktree in &mut outcome.other_worktrees {
        if let WorktreeChange::Changed(changes) = &mut worktree.kind
            && let Err(error) = filter_paths(&worktree.git_dir, options, changes)
        {
            outcome.errors.push(error);
        }
    }
    outcome
        .other_worktrees
        .retain(|worktree| match &worktree.kind {
            WorktreeChange::Changed(changes) => !changes.is_empty(),
            WorktreeChange::Added | WorktreeChange::Moved { .. } | WorktreeChange::Removed => true,
        });
}

fn filter_paths(
    git_dir: &Path,
    options: &gix::open::Options,
    changes: &mut Changes,
) -> gix::Result<()> {
    let Some(Scope::Paths(paths)) = &mut changes.worktree else {
        return Ok(());
    };
    if !paths
        .iter()
        .any(|path| path.path_kind == plumbing::PathKind::File)
    {
        return Ok(());
    }
    let repo = gix::open_opts(git_dir, options.clone_without_trust())?;
    let index = repo.index_or_empty()?;
    let mut excludes = repo.excludes(
        &index,
        None,
        gix::worktree::stack::state::ignore::Source::WorktreeThenIdMappingIfNotSkipped,
    )?;
    let lookup = repo
        .filesystem_options()?
        .ignore_case
        .then(|| index.prepare_icase_backing());
    let mut retained = Vec::with_capacity(paths.len());
    for path in paths.iter() {
        let tracked = match &lookup {
            Some(lookup) => index
                .entry_by_path_icase(path.path.as_bstr(), true, lookup)
                .is_some(),
            None => index.entry_by_path(path.path.as_bstr()).is_some(),
        };
        if path.path_kind != plumbing::PathKind::File
            || tracked
            || !excludes.at_entry(path.path.as_bstr(), None)?.is_excluded()
        {
            retained.push(path.clone());
        }
    }
    *paths = retained;
    if paths.is_empty() {
        changes.worktree = None;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignore_filter_keeps_tracked_files_and_directory_scopes() -> Result<()> {
        let tmp = tempfile::tempdir()?;
        for args in [vec!["init"], vec!["add", "--force", "tracked"]] {
            if args.first() == Some(&"add") {
                std::fs::write(tmp.path().join(".gitignore"), "*\n")?;
                std::fs::write(tmp.path().join("tracked"), "tracked despite ignore rules")?;
            }
            let output = gix_testtools::git_command(tmp.path()).args(args).output()?;
            assert!(
                output.status.success(),
                "Git prepares the isolated index: {output:?}"
            );
        }
        let repo = gix::open_opts(tmp.path(), gix::open::Options::isolated())?;
        let mut changes = Changes {
            worktree: Some(Scope::Paths(
                [
                    ("ignored", plumbing::PathKind::File),
                    ("tracked", plumbing::PathKind::File),
                    ("directory", plumbing::PathKind::Directory),
                    ("unknown", plumbing::PathKind::Any),
                ]
                .into_iter()
                .map(|(path, path_kind)| gix::notify::PathChange {
                    path: path.into(),
                    kind: plumbing::EventKind::Modify,
                    path_kind,
                })
                .collect(),
            )),
            ..Default::default()
        };
        filter_paths(
            repo.git_dir(),
            &gix::open::Options::isolated(),
            &mut changes,
        )?;
        let Some(Scope::Paths(paths)) = changes.worktree else {
            anyhow::bail!("retained invalidations have path scopes");
        };
        assert_eq!(
            paths
                .iter()
                .map(|path| path.path.as_bstr())
                .collect::<Vec<_>>(),
            [
                b"tracked".as_bstr(),
                b"directory".as_bstr(),
                b"unknown".as_bstr()
            ],
            "ignored untracked files are omitted, while tracked and conservative scopes remain"
        );
        Ok(())
    }
}
