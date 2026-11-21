//! File watching, so the UI follows changes made by editors, agents and other GitButler clients.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Result;
use bstr::ByteSlice as _;
use gix::notify::{Changes, Outcome, Scope, WorktreeChange, plumbing::PathKind};
use tokio::sync::{mpsc, oneshot};

use super::{Checkout, Project, open, runtime};

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

/// A pending cached-status reply awaited by the workspace filesystem worker.
pub type StatusResponse = oneshot::Receiver<Result<but_filemonitor::Snapshot>>;

/// Cached-status requests keyed by the checkout whose files a workspace load reads.
pub type StatusResponses = HashMap<Checkout, StatusResponse>;

/// Keeps a project's watcher alive; dropping it stops watching.
pub struct Watcher {
    requests: mpsc::UnboundedSender<but_filemonitor::Request>,
    git_dir: PathBuf,
    common_dir: PathBuf,
    checkouts: HashSet<Checkout>,
}

impl Watcher {
    /// Checkouts whose cached status is currently requested.
    pub fn checkouts(&self) -> impl Iterator<Item = &Checkout> {
        self.checkouts.iter()
    }

    /// Reconcile all monitored checkouts after an explicit refresh.
    /// Return whether the monitor is still running.
    pub fn rescan(&self) -> bool {
        self.requests.send(but_filemonitor::Request::Rescan).is_ok()
    }

    /// Request cached status only for checkouts whose Uncommitted pages are currently shown.
    pub fn set_checkouts(&mut self, checkouts: Vec<Checkout>) -> bool {
        let checkouts: HashSet<_> = checkouts.into_iter().collect();
        if checkouts == self.checkouts {
            return true;
        }
        let git_dirs = checkouts
            .iter()
            .map(|checkout| self.git_dir(checkout))
            .collect::<Result<_>>();
        let git_dirs = match git_dirs {
            Ok(git_dirs) => git_dirs,
            Err(error) => {
                tracing::warn!(?error, "couldn't identify status checkouts");
                return false;
            }
        };
        if self
            .requests
            .send(but_filemonitor::Request::SetCheckouts(git_dirs))
            .is_err()
        {
            return false;
        }
        self.checkouts = checkouts;
        true
    }

    /// Request an active checkout's cached status beside the workspace load.
    /// Other checkouts use the ordinary filesystem read.
    pub fn request_snapshot(
        &self,
        checkout: &Checkout,
        refresh: bool,
    ) -> Result<Option<StatusResponse>> {
        if !self.checkouts.contains(checkout) {
            return Ok(None);
        }
        let (response, received) = oneshot::channel();
        self.requests
            .send(but_filemonitor::Request::Snapshot {
                git_dir: self.git_dir(checkout)?,
                refresh,
                response,
            })
            .map_err(|_| anyhow::anyhow!("the status monitor stopped"))?;
        Ok(Some(received))
    }

    fn git_dir(&self, checkout: &Checkout) -> Result<PathBuf> {
        Ok(match checkout {
            Checkout::Main => self.git_dir.clone(),
            Checkout::Linked(name) => self
                .common_dir
                .join("worktrees")
                .join(gix::path::from_bstr(name)?),
        })
    }
}

/// Signal changes on a background task, identifying the checkout and individual file activity.
/// Metadata and broad worktree invalidations reload through the app's existing change channel.
pub fn watch(
    project: &Project,
    on_change: impl Fn(Change, Checkout, Vec<Arc<str>>) + Send + Sync + 'static,
) -> Result<Watcher> {
    let ctx = open(&project.path)?;
    let path = project.path.clone();
    let repo = ctx.repo.get()?;
    let git_dir = repo.git_dir().to_owned();
    let common_dir = repo.common_dir().to_owned();
    let monitor = but_filemonitor::FileMonitor::new(
        &repo,
        but_core::diff::status_monitor_options,
        move |repo| worktree_filter(repo, &path),
    )?;
    let (requests, requested) = mpsc::unbounded_channel();
    runtime().spawn(async move {
        if let Err(err) = monitor
            .run(requested, move |outcome| {
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
                forward(outcome, &on_change);
            })
            .await
        {
            tracing::warn!(?err, "file watcher stopped");
        }
    });
    Ok(Watcher {
        requests,
        git_dir,
        common_dir,
        checkouts: HashSet::new(),
    })
}

fn worktree_filter(repo: &gix::Repository, path: &Path) -> Result<Option<Vec<PathBuf>>> {
    let ctx = open(path)?;
    ctx.worktrees_with_state()?;
    let db = ctx.db.get_cache_mut()?;
    let mut selected = Vec::new();
    for proxy in repo.worktrees()? {
        if !db
            .worktree_meta()
            .get(proxy.id()?.as_ref())?
            .is_some_and(|meta| meta.archived)
        {
            selected.push(proxy.git_dir().to_owned());
        }
    }
    Ok(Some(selected))
}

fn forward(outcome: Outcome, mut on_change: impl FnMut(Change, Checkout, Vec<Arc<str>>)) {
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
                forward_changes(changes, Checkout::Linked(name.to_owned()), &mut on_change);
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
    fn native_file_discovery_survives_clean_and_inactive_status_consumers() {
        use crate::backend::ops::tests::{agent_worktree, repo_with, target_origin_main};
        use crate::backend::{Kept, Workspace, load_workspace, reload_uncommitted_with_status};
        use std::time::Duration;

        async fn reload_after_file_event(
            received: &mut mpsc::UnboundedReceiver<(Change, Checkout, Vec<Arc<str>>)>,
            path: &Path,
            shown: Workspace,
            checkout: &Checkout,
        ) -> Workspace {
            tokio::time::timeout(Duration::from_secs(10), async {
                while let Some((change, changed, _)) = received.recv().await {
                    if change == Change::Files && changed == *checkout {
                        return;
                    }
                }
                panic!("the native watcher remains connected");
            })
            .await
            .expect("filesystem changes still reload a checkout without a status consumer");
            let path = path.to_owned();
            let main = *checkout == Checkout::Main;
            let linked = match checkout {
                Checkout::Main => HashSet::new(),
                Checkout::Linked(name) => HashSet::from([name.clone()]),
            };
            super::super::run(move || {
                reload_uncommitted_with_status(&path, shown, main, &linked, Default::default())
            })
            .await
            .expect("file invalidations reload the navigator's uncommitted rows")
        }

        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, linked) = agent_worktree(path);
            super::super::apply(
                path,
                super::super::worktrees::SetArchived {
                    name: "agent".into(),
                    archived: false,
                },
            )
            .expect("the linked checkout is shown in the navigator");
            let mut shown = load_workspace(path, Kept::default())
                .expect("the clean fixture's workspace can be loaded");
            let project = Project {
                id: "project".into(),
                name: "fixture".into(),
                path: path.to_owned(),
                last_active: None,
                last_fetch: None,
            };
            let (events, mut received) = mpsc::unbounded_channel();
            let mut watcher = watch(&project, move |change, checkout, paths| {
                events.send((change, checkout, paths)).ok();
            })
            .expect("GPUI's actual watcher can be constructed");
            runtime().block_on(async {
                tokio::time::timeout(Duration::from_secs(10), received.recv())
                    .await
                    .expect("the watcher establishes initial coverage")
                    .expect("initial coverage emits an outcome");
                // Initial registration invalidations cannot stand in for a later file event.
                while let Ok(Some(_)) =
                    tokio::time::timeout(Duration::from_millis(300), received.recv()).await
                {}
                for (checkout, folder) in [
                    (Checkout::Main, path),
                    (Checkout::Linked("agent".into()), linked.as_path()),
                ] {
                    assert!(
                        shown.uncommitted_in(&checkout).is_some_and(|files| files.is_empty()),
                        "both checkout rows start clean"
                    );
                    assert!(
                        watcher.request_snapshot(&checkout, false)
                            .expect("the monitor is connected").is_none(),
                        "inactive pages do not request status snapshots"
                    );
                    for present in [true, false, true] {
                        if present {
                            std::fs::write(folder.join("untracked"), "new file\n")
                                .expect("the editor creates an untracked file");
                        } else {
                            std::fs::remove_file(folder.join("untracked"))
                                .expect("removing the last untracked file makes the checkout clean");
                        }
                        shown = reload_after_file_event(&mut received, path, shown, &checkout).await;
                        assert_eq!(
                            shown.uncommitted_in(&checkout).is_some_and(|files| {
                                files.iter().any(|file| file.path == "untracked")
                            }),
                            present,
                            "native file events remove and restore the navigator row without refresh"
                        );
                    }
                    assert!(
                        watcher.set_checkouts(vec![checkout.clone()]),
                        "opening Uncommitted starts snapshot consumption"
                    );
                    let response = watcher.request_snapshot(&checkout, false)
                        .expect("the selected snapshot can be requested")
                        .expect("an active Uncommitted page has a status consumer");
                    let snapshot = tokio::time::timeout(Duration::from_secs(10), response).await
                        .expect("the selected status refresh completes")
                        .expect("the status monitor answers")
                        .expect("the selected status snapshot succeeds");
                    assert!(
                        snapshot.items.iter().any(|item| item.location() == "untracked"),
                        "activating the page reads the discovered file"
                    );
                    assert!(watcher.set_checkouts(Vec::new()), "leaving the page stops snapshot consumption");
                    assert!(
                        watcher.request_snapshot(&checkout, false)
                            .expect("the monitor remains connected").is_none(),
                        "leaving Uncommitted keeps status refreshes demand-driven"
                    );
                    std::fs::remove_file(folder.join("untracked"))
                        .expect("the checkout becomes clean after leaving Uncommitted");
                    shown = reload_after_file_event(&mut received, path, shown, &checkout).await;
                    assert!(
                        shown.uncommitted_in(&checkout).is_some_and(|files| files.is_empty()),
                        "the last file disappearing removes the row"
                    );
                    std::fs::write(folder.join("untracked"), "new file again\n")
                        .expect("the editor writes again after the row disappeared");
                    shown = reload_after_file_event(&mut received, path, shown, &checkout).await;
                    assert!(
                        shown.uncommitted_in(&checkout).is_some_and(|files| {
                            files.iter().any(|file| file.path == "untracked")
                        }),
                        "new files restore the row even after its status consumer stopped"
                    );
                }
            });
        });
    }

    #[test]
    fn native_coverage_excludes_archived_broken_worktrees_and_recovers_after_unarchive() {
        use crate::backend::ops::tests::{agent_worktree, repo_with, target_origin_main};

        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (_outside, _linked) = agent_worktree(path);
            crate::backend::apply(
                path,
                super::super::worktrees::SetArchived {
                    name: "agent".into(),
                    archived: true,
                },
            )
            .expect("the linked worktree can be archived");
            let index = path.join(".git/worktrees/agent/index");
            let original_index = std::fs::read(&index).expect("the linked index can be saved");
            std::fs::write(&index, "invalid").expect("the archived index can be made unreadable");
            let repo = gix::open_opts(path, gix::open::Options::isolated())
                .expect("the main fixture repository can be opened");
            let monitor =
                but_filemonitor::FileMonitor::new(&repo, but_core::diff::status_monitor_options, {
                    let path = path.to_owned();
                    move |repo| worktree_filter(repo, &path)
                })
                .expect("the primary monitor can be constructed");
            runtime().block_on(async {
                let (rescans, requested) = mpsc::unbounded_channel();
                let (events, mut received) = mpsc::unbounded_channel();
                let task = runtime().spawn(monitor.run(requested, move |outcome| {
                    events.send(outcome).ok();
                }));
                let initial =
                    tokio::time::timeout(std::time::Duration::from_secs(10), received.recv())
                        .await
                        .expect("initial coverage is established")
                        .expect("initial coverage emits an outcome");
                assert!(
                    initial.errors.is_empty(),
                    "an archived index is never opened: {:?}",
                    initial.errors
                );
                assert!(
                    initial.other_worktrees.is_empty(),
                    "archived worktrees receive no private or checkout coverage"
                );
                assert!(
                    initial.changes.worktree.is_some(),
                    "the selected primary checkout remains monitored"
                );
                std::fs::write(&index, original_index).expect("the linked index can be restored");
                crate::backend::apply(
                    path,
                    super::super::worktrees::SetArchived {
                        name: "agent".into(),
                        archived: false,
                    },
                )
                .expect("the linked worktree can be unarchived");
                assert!(
                    rescans.send(but_filemonitor::Request::Rescan).is_ok(),
                    "the rescan channel remains connected"
                );
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    while let Some(outcome) = received.recv().await {
                        assert!(
                            outcome.errors.is_empty(),
                            "unarchiving establishes native coverage without errors: {:?}",
                            outcome.errors
                        );
                        if outcome.other_worktrees.iter().any(|worktree| {
                            worktree.git_dir.ends_with("worktrees/agent")
                                && matches!(worktree.kind, WorktreeChange::Added)
                        }) {
                            return;
                        }
                    }
                    panic!("unarchived coverage must emit a fresh worktree baseline");
                })
                .await
                .expect("unarchiving establishes coverage without restarting GPUI's watcher");
                drop(rescans);
                task.await
                    .expect("the monitor task finishes")
                    .expect("the monitor shuts down");
            });
        });
    }

    #[test]
    fn selection_keeps_unreadable_and_inaccessible_unarchived_worktrees() {
        use crate::backend::ops::tests::{agent_worktree, git, repo_with, target_origin_main};

        but_testsupport::isolated_app_data_dir(|| {
            let dir = repo_with("one\n");
            let path = dir.path();
            target_origin_main(path);
            let (outside, linked) = agent_worktree(path);
            crate::backend::apply(
                path,
                super::super::worktrees::SetArchived {
                    name: "agent".into(),
                    archived: false,
                },
            )
            .expect("the linked worktree can be made active");
            let repo = gix::open_opts(path, gix::open::Options::isolated())
                .expect("the main fixture repository can be opened");
            std::fs::write(path.join(".git/worktrees/agent/HEAD"), "invalid\n")
                .expect("the private HEAD can be made unreadable");
            git(
                path,
                &[
                    "worktree",
                    "lock",
                    linked.to_str().expect("the test path is UTF-8"),
                ],
            );
            for inaccessible in [false, true] {
                if inaccessible {
                    std::fs::rename(&linked, outside.path().join("temporarily-moved"))
                        .expect("the locked checkout can be made unavailable");
                }
                let selected = worktree_filter(&repo, path)
                    .expect("selection reads registration and archive state")
                    .expect("GPUI always selects explicit native coverage");
                assert!(
                    selected
                        .iter()
                        .any(|git_dir| git_dir.ends_with("worktrees/agent")),
                    "unreadable or unavailable worktrees remain selected for Gix recovery"
                );
            }
        });
    }

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
    fn worktree_lifecycle_and_changes_preserve_checkout_identity() {
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
            |change, checkout, paths| {
                events.push((change, checkout, paths));
            },
        );
        assert_eq!(
            events,
            vec![(Change::Files, Checkout::Linked("linked".into()), Vec::new())],
            "selected worktree changes preserve checkout identity"
        );
    }
}
