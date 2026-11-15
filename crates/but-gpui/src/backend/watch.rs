//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use std::{collections::HashSet, path::Path, sync::Arc};

use anyhow::Result;
use bstr::{BString, ByteSlice as _};
use gix::notify::{Changes, Outcome, Scope, WorktreeChange, plumbing::PathKind};
use tokio::sync::mpsc;

use super::{Checkout, Project, open, run, runtime};

/// The repository information a watcher invalidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Repository settings, layout and all workspace views.
    Full,
    /// References and Git operation state.
    Commits,
    /// Uncommitted files of one checkout.
    Files,
}

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    rescans: mpsc::UnboundedSender<()>,
    worktrees: mpsc::UnboundedSender<()>,
}

impl Watcher {
    /// Reconcile all monitored checkouts after an explicit refresh.
    /// Return whether the monitor is still running.
    pub fn rescan(&self) -> bool {
        self.rescans.send(()).is_ok()
    }

    /// Re-list active linked worktrees after the app changes their archived state.
    /// Return whether the event pump is still running.
    pub fn refresh_worktrees(&self) -> bool {
        self.worktrees.send(()).is_ok()
    }
}

/// Signal changes on a background task, identifying the checkout and individual file activity.
/// Metadata and broad worktree invalidations reload through the app's existing change channel.
pub fn watch(
    project: &Project,
    on_change: impl Fn(Change, Checkout, Vec<Arc<str>>) + Send + Sync + 'static,
) -> Result<Watcher> {
    let ctx = open(&project.path)?;
    // shortcut: Gix also covers archived worktrees; exclude their native watches when its API permits.
    let monitor = but_filemonitor::FileMonitor::new(&*ctx.repo.get()?)?;
    let mut active = linked_worktrees(&project.path)?;
    let path = project.path.clone();
    let (events, mut received) = mpsc::unbounded_channel();
    let (rescans, requested) = mpsc::unbounded_channel();
    let (worktrees, mut updates) = mpsc::unbounded_channel();
    runtime().spawn(async move {
        if let Err(err) = monitor
            .run(requested, move |outcome| {
                events.send(outcome).ok();
            })
            .await
        {
            tracing::warn!(?err, "file watcher stopped");
        }
    });
    runtime().spawn(async move {
        while let Some(outcome) =
            next_outcome(&path, &mut active, &mut updates, &mut received).await
        {
            tracing::debug!(
                received = outcome.statistics.received,
                rescans = outcome.statistics.rescans,
                added = outcome.statistics.added,
                removed = outcome.statistics.removed,
                "file watcher outcome"
            );
            for err in &outcome.errors {
                tracing::warn!(?err, "file watcher reported an error");
            }
            if outcome.changes.layout {
                refresh_worktrees(&path, &mut active).await;
            }
            forward(outcome, &active, &on_change);
        }
    });
    Ok(Watcher { rescans, worktrees })
}

async fn next_outcome(
    path: &Path,
    active: &mut HashSet<BString>,
    updates: &mut mpsc::UnboundedReceiver<()>,
    received: &mut mpsc::UnboundedReceiver<Outcome>,
) -> Option<Outcome> {
    loop {
        tokio::select! {
            biased;
            Some(()) = updates.recv() => refresh_worktrees(path, active).await,
            outcome = received.recv() => return outcome,
        }
    }
}

async fn refresh_worktrees(path: &Path, active: &mut HashSet<BString>) {
    let path = path.to_owned();
    match run(move || linked_worktrees(&path)).await {
        Ok(listed) => *active = listed,
        Err(err) => tracing::warn!(?err, "failed to refresh linked worktrees for watching"),
    }
}

fn linked_worktrees(path: &Path) -> Result<HashSet<BString>> {
    Ok((open(path)?.worktrees_with_state()?.into_iter())
        .filter(|worktree| !worktree.archived)
        .map(|worktree| worktree.name)
        .collect())
}

fn forward(
    outcome: Outcome,
    active: &HashSet<BString>,
    mut on_change: impl FnMut(Change, Checkout, Vec<Arc<str>>),
) {
    forward_changes(outcome.changes, Checkout::Main, &mut on_change);
    for worktree in outcome.other_worktrees {
        match worktree.kind {
            WorktreeChange::Added | WorktreeChange::Moved { .. } | WorktreeChange::Removed => {
                on_change(Change::Full, Checkout::Main, Vec::new());
            }
            WorktreeChange::Changed(changes) => {
                let Some(name) = worktree
                    .git_dir
                    .file_name()
                    .and_then(|name| gix::path::os_str_into_bstr(name).ok())
                else {
                    continue;
                };
                if active.contains(name) {
                    forward_changes(changes, Checkout::Linked(name.to_owned()), &mut on_change);
                }
            }
        }
    }
}

fn forward_changes(
    changes: Changes,
    checkout: Checkout,
    on_change: &mut impl FnMut(Change, Checkout, Vec<Arc<str>>),
) {
    if changes.configuration || changes.attributes || changes.layout {
        on_change(Change::Full, checkout.clone(), Vec::new());
    } else if changes.references || changes.operations {
        on_change(Change::Commits, checkout.clone(), Vec::new());
    }
    if changes.index || changes.ignores || changes.worktree.is_some() {
        let paths = match changes.worktree {
            None | Some(Scope::All) => Vec::new(),
            Some(Scope::Paths(paths)) => paths
                .into_iter()
                .filter(|path| path.path_kind == PathKind::File)
                .map(|path| path.path.to_str_lossy().into_owned().into())
                .collect(),
        };
        on_change(Change::Files, checkout, paths);
    }
}

#[cfg(test)]
mod tests {
    use gix::notify::{PathChange, WorktreeChanges, plumbing::EventKind};

    use super::*;

    #[test]
    fn index_and_ignore_invalidations_reload_only_files() {
        for changes in [
            Changes {
                index: true,
                ..Changes::default()
            },
            Changes {
                ignores: true,
                ..Changes::default()
            },
        ] {
            let mut events = Vec::new();
            forward(
                Outcome {
                    changes,
                    ..Outcome::default()
                },
                &HashSet::new(),
                |change, checkout, paths| events.push((change, checkout, paths)),
            );
            assert_eq!(
                events,
                vec![(Change::Files, Checkout::Main, Vec::new())],
                "staging and ignore changes only need to reread checkout files"
            );
        }
    }

    #[test]
    fn references_and_index_preserve_both_invalidations() {
        let mut events = Vec::new();
        forward(
            Outcome {
                changes: Changes {
                    references: true,
                    index: true,
                    ..Changes::default()
                },
                ..Outcome::default()
            },
            &HashSet::new(),
            |change, checkout, paths| events.push((change, checkout, paths)),
        );
        assert_eq!(
            events,
            vec![
                (Change::Commits, Checkout::Main, Vec::new()),
                (Change::Files, Checkout::Main, Vec::new())
            ],
            "a commit and simultaneous index update invalidate both repository views"
        );
    }

    #[test]
    fn metadata_flags_request_the_narrowest_reload() {
        for (changes, expected) in [
            (
                Changes {
                    references: true,
                    ..Changes::default()
                },
                Change::Commits,
            ),
            (
                Changes {
                    operations: true,
                    ..Changes::default()
                },
                Change::Commits,
            ),
            (
                Changes {
                    configuration: true,
                    ..Changes::default()
                },
                Change::Full,
            ),
            (
                Changes {
                    attributes: true,
                    ..Changes::default()
                },
                Change::Full,
            ),
            (
                Changes {
                    layout: true,
                    ..Changes::default()
                },
                Change::Full,
            ),
        ] {
            let mut events = Vec::new();
            forward(
                Outcome {
                    changes,
                    ..Outcome::default()
                },
                &HashSet::new(),
                |change, checkout, paths| events.push((change, checkout, paths)),
            );
            assert_eq!(
                events,
                vec![(expected, Checkout::Main, Vec::new())],
                "metadata invalidations reread only the affected repository views"
            );
        }
    }

    #[test]
    fn archive_refreshes_precede_events_and_preserve_unreadable_worktrees() {
        use crate::backend::ops::tests::{agent_worktree, repo_with, target_origin_main};

        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            crate::backend::apply(
                path,
                super::super::worktrees::SetArchived {
                    name: "agent".into(),
                    archived: false,
                },
            )
            .expect("the linked worktree can be made active");
            runtime().block_on(async {
                let (worktrees, mut updates) = mpsc::unbounded_channel();
                let (events, mut received) = mpsc::unbounded_channel();
                let mut active = linked_worktrees(path).expect("the initial active set can be read");
                let changed = || Outcome {
                    other_worktrees: vec![WorktreeChanges {
                        git_dir: path.join(".git/worktrees/agent"), worktree: linked.clone(),
                        kind: WorktreeChange::Changed(Changes {
                            worktree: Some(Scope::All), ..Changes::default()
                        }),
                    }], ..Outcome::default()
                };
                for (archived, unreadable) in [(true, false), (false, false), (false, true)] {
                    crate::backend::apply(path, super::super::worktrees::SetArchived {
                        name: "agent".into(), archived,
                    }).expect("the archive intent succeeds");
                    if unreadable {
                        std::fs::write(path.join(".git/worktrees/agent/HEAD"), "invalid\n")
                            .expect("the private HEAD can be made unreadable");
                    }
                    assert!(worktrees.send(()).is_ok(), "the relist command receiver is alive");
                    assert!(events.send(changed()).is_ok(), "the outcome receiver is alive");
                    let outcome = tokio::time::timeout(std::time::Duration::from_secs(10),
                        next_outcome(path, &mut active, &mut updates, &mut received)).await
                        .expect("queued re-lists and events finish without native watcher activity")
                        .expect("the outcome sender is still alive");
                    let mut forwarded = Vec::new();
                    forward(outcome, &active,
                        |change, checkout, paths| forwarded.push((change, checkout, paths)));
                    let expected = if archived { Vec::new() } else {
                        vec![(Change::Files, Checkout::Linked("agent".into()), Vec::new())]
                    };
                    assert_eq!(forwarded, expected,
                        "archive commands apply before outcomes and unreadable HEAD does not archive a checkout");
                }
                drop(worktrees);
                drop(events);
                let ended = tokio::time::timeout(std::time::Duration::from_secs(1),
                    next_outcome(path, &mut active, &mut updates, &mut received)).await
                    .expect("closed command channels do not spin or prevent shutdown");
                assert!(ended.is_none(), "closing both channels ends the event pump");
            });
        });
    }

    #[test]
    fn broad_scopes_reload_without_file_activity() {
        for scope in [
            Scope::All,
            Scope::Paths(vec![PathChange {
                path: "dir".into(),
                kind: EventKind::Any,
                path_kind: PathKind::Directory,
            }]),
            Scope::Paths(vec![PathChange {
                path: "unknown".into(),
                kind: EventKind::Any,
                path_kind: PathKind::Any,
            }]),
        ] {
            let mut events = Vec::new();
            forward(
                Outcome {
                    changes: Changes {
                        worktree: Some(scope),
                        ..Changes::default()
                    },
                    ..Outcome::default()
                },
                &HashSet::new(),
                |change, checkout, paths| {
                    events.push((change, checkout, paths));
                },
            );
            assert_eq!(
                events,
                vec![(Change::Files, Checkout::Main, Vec::new())],
                "broad invalidations reload the checkout without counting a directory as a save"
            );
        }
    }

    #[test]
    fn metadata_and_files_both_reach_the_reload_channel() {
        let mut events = Vec::new();
        forward(
            Outcome {
                changes: Changes {
                    references: true,
                    worktree: Some(Scope::Paths(vec![PathChange {
                        path: "file".into(),
                        kind: EventKind::Any,
                        path_kind: PathKind::File,
                    }])),
                    ..Changes::default()
                },
                ..Outcome::default()
            },
            &HashSet::new(),
            |change, checkout, paths| events.push((change, checkout, paths)),
        );
        assert_eq!(
            events,
            vec![
                (Change::Commits, Checkout::Main, Vec::new()),
                (Change::Files, Checkout::Main, vec![Arc::from("file")])
            ],
            "commit reloads preserve simultaneous checkout invalidations and file activity"
        );
    }

    #[test]
    fn lifecycle_reloads_even_after_a_worktree_is_removed_from_the_active_set() {
        for kind in [
            WorktreeChange::Added,
            WorktreeChange::Moved {
                previous_worktree: "old".into(),
            },
            WorktreeChange::Removed,
        ] {
            let mut events = Vec::new();
            forward(
                Outcome {
                    other_worktrees: vec![WorktreeChanges {
                        git_dir: ".git/worktrees/linked".into(),
                        worktree: "linked".into(),
                        kind,
                    }],
                    ..Outcome::default()
                },
                &HashSet::new(),
                |change, checkout, paths| {
                    events.push((change, checkout, paths));
                },
            );
            assert_eq!(
                events,
                vec![(Change::Full, Checkout::Main, Vec::new())],
                "registration transitions require a fresh workspace listing"
            );
        }
        for (active, expected) in [
            (HashSet::new(), Vec::new()),
            (
                HashSet::from([BString::from("linked")]),
                vec![(Change::Files, Checkout::Linked("linked".into()), Vec::new())],
            ),
        ] {
            let mut events = Vec::new();
            forward(
                Outcome {
                    other_worktrees: vec![WorktreeChanges {
                        git_dir: ".git/worktrees/linked".into(),
                        worktree: "linked".into(),
                        kind: WorktreeChange::Changed(Changes {
                            worktree: Some(Scope::All),
                            ..Changes::default()
                        }),
                    }],
                    ..Outcome::default()
                },
                &active,
                |change, checkout, paths| {
                    events.push((change, checkout, paths));
                },
            );
            assert_eq!(
                events, expected,
                "only active linked worktrees emit incremental changes"
            );
        }
    }
}
