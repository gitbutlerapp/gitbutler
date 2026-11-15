//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use std::{collections::HashSet, path::Path, sync::Arc};

use anyhow::Result;
use bstr::{BString, ByteSlice as _};
use gix::notify::{Changes, Outcome, Scope, WorktreeChange, plumbing::PathKind};

use super::{Checkout, Project, open, run, runtime};

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    _stop: tokio::sync::oneshot::Sender<()>,
}

/// Signal changes on a background task, identifying the checkout and individual file activity.
/// Metadata and broad worktree invalidations reload through the app's existing change channel.
pub fn watch(
    project: &Project,
    on_change: impl Fn(bool, Checkout, Vec<Arc<str>>) + Send + Sync + 'static,
) -> Result<Watcher> {
    let ctx = open(&project.path)?;
    // shortcut: Gix also covers archived worktrees; exclude their native watches when its API permits.
    let monitor = but_filemonitor::FileMonitor::new(&*ctx.repo.get()?)?;
    let mut active = linked_worktrees(&project.path)?;
    let path = project.path.clone();
    let (events, mut received) = tokio::sync::mpsc::unbounded_channel();
    let (stop, stopped) = tokio::sync::oneshot::channel();
    runtime().spawn(async move {
        if let Err(err) = monitor
            .run(stopped, move |outcome| {
                events.send(outcome).ok();
            })
            .await
        {
            tracing::warn!(?err, "file watcher stopped");
        }
    });
    runtime().spawn(async move {
        while let Some(outcome) = received.recv().await {
            for err in &outcome.errors {
                tracing::warn!(?err, "file watcher reported an error");
            }
            if outcome.changes.layout {
                let path = path.clone();
                match run(move || linked_worktrees(&path)).await {
                    Ok(listed) => active = listed,
                    Err(err) => {
                        tracing::warn!(?err, "failed to refresh linked worktrees for watching")
                    }
                }
            }
            forward(outcome, &active, &on_change);
        }
    });
    Ok(Watcher { _stop: stop })
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
    mut on_change: impl FnMut(bool, Checkout, Vec<Arc<str>>),
) {
    forward_changes(outcome.changes, Checkout::Main, &mut on_change);
    for worktree in outcome.other_worktrees {
        match worktree.kind {
            WorktreeChange::Added | WorktreeChange::Moved { .. } | WorktreeChange::Removed => {
                on_change(true, Checkout::Main, Vec::new());
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
    on_change: &mut impl FnMut(bool, Checkout, Vec<Arc<str>>),
) {
    if changes.references
        || changes.index
        || changes.configuration
        || changes.ignores
        || changes.attributes
        || changes.operations
        || changes.layout
    {
        on_change(true, checkout.clone(), Vec::new());
    }
    if let Some(scope) = changes.worktree {
        let paths = match scope {
            Scope::All => Vec::new(),
            Scope::Paths(paths) => paths
                .into_iter()
                .filter(|path| path.path_kind == PathKind::File)
                .map(|path| path.path.to_str_lossy().into_owned().into())
                .collect(),
        };
        on_change(false, checkout, paths);
    }
}

#[cfg(test)]
mod tests {
    use gix::notify::{PathChange, WorktreeChanges, plumbing::EventKind};

    use super::*;

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
                |git, checkout, paths| {
                    events.push((git, checkout, paths));
                },
            );
            assert_eq!(
                events,
                vec![(false, Checkout::Main, Vec::new())],
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
            |git, checkout, paths| events.push((git, checkout, paths)),
        );
        assert_eq!(
            events,
            vec![
                (true, Checkout::Main, Vec::new()),
                (false, Checkout::Main, vec![Arc::from("file")])
            ],
            "metadata reloads everything while simultaneous file activity is preserved"
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
                |git, checkout, paths| {
                    events.push((git, checkout, paths));
                },
            );
            assert_eq!(
                events,
                vec![(true, Checkout::Main, Vec::new())],
                "registration transitions require a fresh workspace listing"
            );
        }
        for (active, expected) in [
            (HashSet::new(), Vec::new()),
            (
                HashSet::from([BString::from("linked")]),
                vec![(false, Checkout::Linked("linked".into()), Vec::new())],
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
                |git, checkout, paths| {
                    events.push((git, checkout, paths));
                },
            );
            assert_eq!(
                events, expected,
                "only active linked worktrees emit incremental changes"
            );
        }
    }
}
