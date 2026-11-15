//! Repository invalidations backed by Gix, with asynchronous waits and blocking filesystem work.
#![deny(unsafe_code)]

use std::{
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context as _, Result};
use gix::{
    bstr::ByteSlice as _,
    notify::{Changes, Outcome, RepositoryMonitor, Scope, WorktreeChange, plumbing},
};
use tokio::sync::oneshot;

/// A repository monitor plus GitButler's refresh and compatibility signals.
///
/// Construct this off the UI thread, then run it on a Tokio runtime. Gix owns repository
/// coverage and recovery; the extra shallow watches only cover signals Gix does not classify.
pub struct FileMonitor {
    repository: RepositoryMonitor,
    signals: plumbing::asynchronous::Watcher,
    paths: Vec<PathBuf>,
    open_options: gix::open::Options,
}

impl FileMonitor {
    /// Resolve repository coverage and GitButler's signal paths, performing filesystem work.
    pub fn new(repo: &gix::Repository) -> Result<Self> {
        Ok(Self {
            repository: repo.monitor(gix::notify::Options {
                all_worktrees: true,
                ..Default::default()
            })?,
            signals: plumbing::asynchronous::Watcher::new(Default::default())?,
            paths: signal_paths(repo)?,
            open_options: repo.open_options().clone_without_trust(),
        })
    }

    /// Establish coverage, publish initial invalidations, and monitor until `stop` is sent or dropped.
    ///
    /// Every outcome is forwarded whole, including simultaneous errors and invalidations. Native
    /// resources are shut down on their workers before this returns. The callback should only signal
    /// its consumer; filesystem registration, inventory and ignore filtering run off the executor.
    pub async fn run(
        self,
        mut stop: oneshot::Receiver<()>,
        mut on_change: impl FnMut(Outcome),
    ) -> Result<()> {
        let Self {
            repository: mut monitor,
            mut signals,
            mut paths,
            open_options,
        } = self;
        let git_dir = monitor.layout().git_dir.clone();
        let retry_interval = monitor.options().retry_interval;
        let mut refresh_signals = true;
        let mut retry = None;

        let (result, monitor) = loop {
            let open_at = git_dir.clone();
            let options = open_options.clone_without_trust();
            let (next, mut outcome) = match monitor
                .service_async(Instant::now(), move || {
                    gix::open_opts(&open_at, options.clone_without_trust())
                })
                .await
            {
                Ok(result) => result,
                Err(err) => break (Err(anyhow::Error::from(err)), None),
            };
            monitor = next;

            let batch = signals.drain(plumbing::Budget::default());
            let signal_changed = batch.loss.is_some()
                || batch
                    .events
                    .iter()
                    .flat_map(|event| &event.paths)
                    .any(|changed| {
                        paths
                            .iter()
                            .any(|signal| changed == signal || signal.starts_with(changed))
                    });
            refresh_signals |=
                signal_changed || outcome.changes.configuration || outcome.changes.layout;
            if signal_changed {
                outcome.changes.references = true;
                outcome.changes.layout = true;
            }
            if let Some(error) = batch.error {
                outcome.errors.push(error);
                refresh_signals = true;
            }

            let open_at = git_dir.clone();
            let options = open_options.clone_without_trust();
            let prepare_signals =
                refresh_signals && retry.is_none_or(|deadline| deadline <= Instant::now());
            let work = tokio::task::spawn_blocking(move || {
                filter_outcome(&open_at, &options, &mut outcome);
                let prepared = prepare_signals.then(|| {
                    let repo = gix::open_opts(&open_at, options)?;
                    let paths = signal_paths(&repo)?;
                    let watches = signal_watches(&paths)?;
                    Ok::<_, anyhow::Error>((paths, watches))
                });
                (outcome, prepared)
            })
            .await;
            let (mut outcome, prepared) = match work {
                Ok(result) => result,
                Err(err) => {
                    break (
                        Err(err).context("Preparing repository invalidations"),
                        Some(monitor),
                    );
                }
            };
            if let Some(prepared) = prepared {
                match prepared {
                    Ok((next_paths, next_watches)) => {
                        paths = next_paths;
                        match async { signals.replace(next_watches)?.await }.await {
                            Ok(()) => {
                                refresh_signals = false;
                                retry = None;
                            }
                            Err(error) => {
                                outcome.errors.push(error);
                                retry = Some(Instant::now() + retry_interval);
                            }
                        }
                    }
                    Err(error) => {
                        outcome
                            .errors
                            .push(gix::Error::from_boxed(error.into_boxed_dyn_error()));
                        retry = Some(Instant::now() + retry_interval);
                    }
                }
            }
            if !outcome.changes.is_empty()
                || !outcome.other_worktrees.is_empty()
                || !outcome.errors.is_empty()
            {
                on_change(outcome);
            }

            tokio::select! {
                biased;
                _ = &mut stop => break (Ok(()), Some(monitor)),
                _ = monitor.ready_async() => {},
                _ = signals.ready() => {},
                _ = async {
                    match retry {
                        Some(deadline) => tokio::time::sleep_until(deadline.into()).await,
                        None => std::future::pending::<()>().await,
                    }
                } => refresh_signals = true,
            }
        };
        let repository_shutdown = match monitor {
            Some(monitor) => monitor.shutdown_async().await.map_err(anyhow::Error::from),
            None => Ok(()),
        };
        let signals_shutdown = signals.shutdown().await.map_err(anyhow::Error::from);
        result.and(repository_shutdown).and(signals_shutdown)
    }
}

fn signal_paths(repo: &gix::Repository) -> Result<Vec<PathBuf>> {
    let sentinel = Path::new(but_project_handle::REFRESH_SENTINEL_PATH)
        .file_name()
        .context("Refresh sentinel has a filename")?;
    [
        but_project_handle::gitbutler_storage_path(repo)?.join(sentinel),
        repo.git_dir().join("GB_FLUSH"),
        repo.git_dir().join("logs/HEAD"),
    ]
    .into_iter()
    .map(|path| gix::path::realpath(path).map_err(anyhow::Error::from))
    .collect()
}

fn signal_watches(paths: &[PathBuf]) -> Result<Vec<plumbing::Watch>> {
    let mut watches = Vec::new();
    for signal in paths {
        let mut parent = signal.parent().context("Signal has a parent directory")?;
        loop {
            match std::fs::metadata(parent) {
                Ok(metadata) if metadata.is_dir() => break,
                Ok(_) => anyhow::bail!("Signal parent is not a directory: {}", parent.display()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                    parent = parent
                        .parent()
                        .context("Signal has an existing ancestor directory")?;
                }
                Err(err) => {
                    return Err(err).with_context(|| format!("Inspecting {}", parent.display()));
                }
            }
        }
        watches.push(plumbing::Watch {
            path: parent.to_owned(),
            recursive: false,
        });
    }
    watches.sort();
    watches.dedup();
    Ok(watches)
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
