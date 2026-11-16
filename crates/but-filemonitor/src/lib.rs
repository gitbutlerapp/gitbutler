//! Repository invalidations backed by Gix, with asynchronous waits and blocking filesystem work.
#![deny(unsafe_code)]

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context as _, Result};
use gix::{
    bstr::ByteSlice as _,
    notify::{
        Changes, InvalidationPath, Outcome, RepositoryMonitor, Scope, WorktreeChange, plumbing,
    },
};
use tokio::sync::mpsc;

/// A repository monitor plus GitButler's refresh and compatibility signals.
///
/// Construct this off the UI thread, then run it on a Tokio runtime. Gix owns repository
/// coverage, application dependencies and recovery.
pub struct FileMonitor<F> {
    repository: RepositoryMonitor,
    selector: F,
    open_options: gix::open::Options,
}

impl<F> FileMonitor<F>
where
    F: FnMut(&gix::Repository) -> Result<Option<Vec<PathBuf>>> + Send + 'static,
{
    /// Resolve coverage and GitButler's signal paths, performing filesystem work.
    /// The selector returns private Git directories to monitor, or `None` for all checkouts.
    /// It runs again off the executor after configuration/layout changes and explicit rescans.
    pub fn new(repo: &gix::Repository, mut selector: F) -> Result<Self> {
        Ok(Self {
            repository: repository_monitor(repo, selector(repo)?)?,
            selector,
            open_options: repo.open_options().clone_without_trust(),
        })
    }

    /// Establish coverage and monitor until every sender of `rescans` is dropped.
    /// Sending `()` requests a full rescan of every monitored checkout.
    ///
    /// Every outcome is forwarded whole, including simultaneous errors and invalidations. Native
    /// resources are shut down on their workers before this returns. The callback should only signal
    /// its consumer; filesystem registration, inventory and ignore filtering run off the executor.
    pub async fn run(
        self,
        mut rescans: mpsc::UnboundedReceiver<()>,
        mut on_change: impl FnMut(Outcome),
    ) -> Result<()> {
        let Self {
            repository: mut monitor,
            mut selector,
            open_options,
        } = self;
        let git_dir = monitor.layout().git_dir.clone();
        let retry_interval = monitor.options().retry_interval;
        let mut refresh_options = false;
        let mut rescan_requested = false;
        let mut rescan_after_refresh = false;
        let mut retry = None;

        let (result, monitor) = loop {
            let requested = std::mem::take(&mut rescan_requested);
            rescan_after_refresh |= requested;
            let mut outcome = if requested {
                // Resolve the selection before a rescan can read now-excluded indexes.
                Outcome::default()
            } else {
                let open_at = git_dir.clone();
                let options = open_options.clone_without_trust();
                let (next, outcome) = match monitor
                    .service_async(Instant::now(), move || {
                        gix::open_opts(&open_at, options.clone_without_trust())
                    })
                    .await
                {
                    Ok(result) => result,
                    Err(err) => break (Err(anyhow::Error::from(err)), None),
                };
                monitor = next;
                outcome
            };

            refresh_options |= requested || outcome.changes.configuration || outcome.changes.layout;
            let open_at = git_dir.clone();
            let options = open_options.clone_without_trust();
            let current_options = monitor.options().clone();
            let prepare_options =
                refresh_options && retry.is_none_or(|deadline| deadline <= Instant::now());
            let has_files = |changes: &Changes| matches!(&changes.worktree, Some(Scope::Paths(paths)) if paths.iter().any(|path| path.path_kind == plumbing::PathKind::File));
            let filter_needed = has_files(&outcome.changes)
                || outcome.other_worktrees.iter().any(|worktree| {
                    matches!(&worktree.kind, WorktreeChange::Changed(changes) if has_files(changes))
                });
            let (mut outcome, prepared) = if prepare_options || filter_needed {
                match tokio::task::spawn_blocking(move || {
                    filter_outcome(&open_at, &options, &mut outcome);
                    let prepared = prepare_options.then(|| {
                        let repo = gix::open_opts(&open_at, options)?;
                        let candidate = repository_monitor(&repo, selector(&repo)?)?;
                        let next_options = candidate.options();
                        let changed = current_options.additional_paths
                            != next_options.additional_paths
                            || current_options.worktree_filter != next_options.worktree_filter;
                        Ok::<_, anyhow::Error>(changed.then_some(candidate))
                    });
                    (selector, outcome, prepared)
                })
                .await
                {
                    Ok((next_selector, outcome, prepared)) => {
                        selector = next_selector;
                        (outcome, prepared)
                    }
                    Err(err) => {
                        break (
                            Err(err).context("Preparing repository invalidations"),
                            Some(monitor),
                        );
                    }
                }
            } else {
                (outcome, None)
            };
            let mut replacement = None;
            if let Some(prepared) = prepared {
                match prepared {
                    Ok(candidate) => {
                        refresh_options = false;
                        retry = None;
                        replacement = candidate;
                    }
                    Err(error) => {
                        outcome
                            .errors
                            .push(gix::Error::from_boxed(error.into_boxed_dyn_error()));
                        retry = Some(Instant::now() + retry_interval);
                    }
                }
            }
            on_change(outcome);
            if let Some(next) = replacement {
                rescan_after_refresh = false;
                // Candidates have no native coverage until serviced; finish old coverage first.
                let shutdown = monitor.shutdown_async().await;
                monitor = next;
                if let Err(error) = shutdown {
                    break (Err(anyhow::Error::from(error)), Some(monitor));
                }
                continue;
            }
            if rescan_after_refresh && !refresh_options {
                rescan_after_refresh = false;
                monitor.rescan();
                continue;
            }

            tokio::select! {
                biased;
                rescan = rescans.recv() => match rescan {
                    Some(()) => {
                        while rescans.try_recv().is_ok() {}
                        rescan_requested = true;
                        retry = None;
                    }
                    None => break (Ok(()), Some(monitor)),
                },
                _ = monitor.ready_async() => {},
                _ = async {
                    match retry {
                        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
                        None => std::future::pending::<()>().await,
                    }
                } => refresh_options = true,
            }
        };
        let repository_shutdown = match monitor {
            Some(monitor) => monitor.shutdown_async().await.map_err(anyhow::Error::from),
            None => Ok(()),
        };
        result.and(repository_shutdown)
    }
}

fn repository_monitor(
    repo: &gix::Repository,
    mut worktree_filter: Option<Vec<PathBuf>>,
) -> Result<RepositoryMonitor> {
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
    Ok(repo.monitor(gix::notify::Options {
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
        ..gix::notify::Options::default()
    })?)
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
