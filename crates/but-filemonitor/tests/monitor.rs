#![cfg(unix)]

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Context as _, Result};
use but_filemonitor::{FileMonitor, Request};
use gix::notify::{Outcome, Scope, WorktreeChange};
use tokio::sync::{mpsc, oneshot};

async fn expect_outcome(
    events: &mut mpsc::UnboundedReceiver<Outcome>,
    matches: impl Fn(&Outcome) -> bool,
) -> Result<Outcome> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(outcome) = events.recv().await {
            assert!(
                outcome.errors.is_empty()
                    || outcome.statistics.rescans > 0
                        && outcome.errors.iter().all(gix::Error::is_conflict),
                "native coverage loss is recoverable; other monitor errors fail: {:?}",
                outcome.errors
            );
            if matches(&outcome) {
                return Ok(outcome);
            }
        }
        anyhow::bail!("monitor stopped before delivering the expected invalidation")
    })
    .await
    .context("waiting for repository invalidation")?
}

async fn settle(events: &mut mpsc::UnboundedReceiver<Outcome>) {
    while let Ok(Some(outcome)) =
        tokio::time::timeout(Duration::from_millis(300), events.recv()).await
    {
        assert!(
            outcome.errors.is_empty(),
            "monitoring settles without failures: {:?}",
            outcome.errors
        );
    }
}

fn git(path: &Path, args: &[&str]) {
    let output = gix_testtools::git_command(path)
        .args(args)
        .output()
        .expect("Git runs");
    assert!(
        output.status.success(),
        "Git fixture operation succeeds: {output:?}"
    );
}

fn repository(path: &Path) -> Result<gix::Repository> {
    git(path, &["init"]);
    std::fs::write(path.join("file"), "initial")?;
    git(path, &["add", "file"]);
    git(
        path,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );
    Ok(gix::open_opts(path, gix::open::Options::isolated())?)
}

#[expect(
    clippy::type_complexity,
    reason = "the test owns the control channel, outcome stream and task"
)]
fn start(
    repo: &gix::Repository,
) -> Result<(
    mpsc::UnboundedSender<Request>,
    mpsc::UnboundedReceiver<Outcome>,
    tokio::task::JoinHandle<Result<()>>,
)> {
    let monitor = FileMonitor::new(repo, |_| Ok(Default::default()), |_| Ok(None))?;
    let (stop, stopped) = mpsc::unbounded_channel();
    let (events, received) = mpsc::unbounded_channel();
    let task = tokio::spawn(monitor.run(stopped, move |outcome| {
        events.send(outcome).ok();
    }));
    Ok((stop, received, task))
}

fn invalidates(scope: &Option<Scope>, path: &str) -> bool {
    match scope {
        Some(Scope::All) => true,
        Some(Scope::Paths(paths)) => paths
            .iter()
            .any(|change| change.path == path || path.starts_with(&format!("{}/", change.path))),
        None => false,
    }
}

#[tokio::test]
async fn files_refresh_sentinel_and_idle_shutdown() -> Result<()> {
    // Cover both the missing default storage directory and a configured external directory.
    for external_storage in [false, true] {
        let tmp = tempfile::tempdir()?;
        let repo = repository(tmp.path())?;
        let external = tempfile::tempdir()?;
        if external_storage {
            git(
                tmp.path(),
                &[
                    "config",
                    but_project_handle::storage_path_config_key(),
                    external.path().to_str().expect("temporary path is UTF-8"),
                ],
            );
        }
        let repo = gix::open_opts(repo.git_dir(), gix::open::Options::isolated())?;
        let storage = but_project_handle::gitbutler_storage_path(&repo)?;
        let (stop, mut events, task) = start(&repo)?;
        expect_outcome(&mut events, |outcome| {
            matches!(outcome.changes.worktree, Some(Scope::All))
        })
        .await?;

        std::fs::write(tmp.path().join("file"), "changed")?;
        expect_outcome(&mut events, |outcome| {
            invalidates(&outcome.changes.worktree, "file")
        })
        .await?;

        std::fs::create_dir_all(&storage)?;
        std::fs::write(storage.join("REFRESH"), "another-process")?;
        expect_outcome(&mut events, |outcome| {
            outcome.changes.references && outcome.changes.layout
        })
        .await?;
        // Once the directory exists, the sentinel itself still triggers later refreshes.
        std::fs::write(storage.join("REFRESH"), "another-process-again")?;
        expect_outcome(&mut events, |outcome| {
            outcome.changes.references && outcome.changes.layout
        })
        .await?;

        std::fs::write(repo.git_dir().join("GB_FLUSH"), "flush")?;
        expect_outcome(&mut events, |outcome| {
            outcome.changes.references && outcome.changes.layout
        })
        .await?;

        drop(stop);
        tokio::time::timeout(Duration::from_secs(10), task).await???;
        assert!(
            events.is_closed(),
            "idle shutdown releases the event sender"
        );
    }
    Ok(())
}

#[tokio::test]
async fn refresh_sentinel_follows_live_storage_configuration() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let default_storage = but_project_handle::gitbutler_storage_path(&repo)?;
    assert!(
        !default_storage.exists(),
        "monitoring begins before the default storage directory exists"
    );
    let external = tempfile::tempdir()?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| {
        matches!(outcome.changes.worktree, Some(Scope::All))
    })
    .await
    .context("initial coverage before changing storage")?;
    settle(&mut events).await;

    git(
        tmp.path(),
        &[
            "config",
            but_project_handle::storage_path_config_key(),
            external.path().to_str().expect("temporary path is UTF-8"),
        ],
    );
    expect_outcome(&mut events, |outcome| outcome.changes.configuration)
        .await
        .context("configuration change retargets storage")?;
    // A queued configuration invalidation must not satisfy the new sentinel assertion.
    settle(&mut events).await;
    let fresh = gix::open_opts(repo.git_dir(), gix::open::Options::isolated())?;
    let storage = but_project_handle::gitbutler_storage_path(&fresh)?;
    assert!(
        storage.starts_with(external.path()) && !storage.exists(),
        "the reconfigured storage directory is external and initially missing"
    );
    std::fs::create_dir_all(&storage)?;
    settle(&mut events).await;
    let sentinel = storage.join("REFRESH");
    std::fs::write(&sentinel, "after changing storage")?;
    expect_outcome(&mut events, |outcome| {
        outcome.changes.references && outcome.changes.layout
    })
    .await
    .context("REFRESH at the new external storage path")?;

    // An external sentinel can also be published by replacing the file atomically.
    settle(&mut events).await;
    let replacement = storage.join("REFRESH.next");
    std::fs::write(&replacement, "atomic refresh")?;
    settle(&mut events).await;
    std::fs::rename(&replacement, &sentinel)?;
    expect_outcome(&mut events, |outcome| {
        outcome.changes.references && outcome.changes.layout
    })
    .await
    .context("atomic replacement of the external REFRESH sentinel")?;

    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    assert!(
        events.is_closed(),
        "live retargeting keeps orderly shutdown"
    );
    Ok(())
}

#[tokio::test]
async fn linked_worktree_add_edit_move_remove() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let main = tmp.path().join("main");
    std::fs::create_dir(&main)?;
    let repo = repository(&main)?;
    let (stop, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| {
        matches!(outcome.changes.worktree, Some(Scope::All))
    })
    .await?;

    let linked = tmp.path().join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().expect("temporary path is UTF-8"),
        ],
    );
    let added = expect_outcome(&mut events, |outcome| {
        outcome
            .other_worktrees
            .iter()
            .any(|other| matches!(other.kind, WorktreeChange::Added))
    })
    .await?;
    let identity = added
        .other_worktrees
        .iter()
        .find(|other| matches!(other.kind, WorktreeChange::Added))
        .expect("addition matched")
        .git_dir
        .clone();

    std::fs::write(linked.join("file"), "linked edit")?;
    expect_outcome(&mut events, |outcome| outcome.other_worktrees.iter().any(|other| {
        other.git_dir == identity && matches!(&other.kind, WorktreeChange::Changed(changes) if invalidates(&changes.worktree, "file"))
    })).await?;

    let moved = tmp.path().join("moved");
    git(
        &main,
        &[
            "worktree",
            "move",
            linked.to_str().expect("temporary path is UTF-8"),
            moved.to_str().expect("temporary path is UTF-8"),
        ],
    );
    expect_outcome(&mut events, |outcome| {
        outcome.other_worktrees.iter().any(|other| {
            other.git_dir == identity && matches!(other.kind, WorktreeChange::Moved { .. })
        })
    })
    .await?;

    git(
        &main,
        &[
            "worktree",
            "remove",
            "--force",
            moved.to_str().expect("temporary path is UTF-8"),
        ],
    );
    let removed = expect_outcome(&mut events, |outcome| {
        outcome
            .other_worktrees
            .iter()
            .any(|other| other.git_dir == identity && matches!(other.kind, WorktreeChange::Removed))
    })
    .await?;

    if removed.recovery_pending {
        expect_outcome(&mut events, |outcome| {
            !outcome.recovery_pending && outcome.errors.is_empty()
        })
        .await
        .context("native coverage recovers after worktree removal")?;
    }

    drop(stop);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    Ok(())
}

#[tokio::test]
async fn manual_rescan_refreshes_every_checkout_without_writes() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let main = tmp.path().join("main");
    std::fs::create_dir(&main)?;
    let repo = repository(&main)?;
    let linked = tmp.path().join("linked");
    git(
        &main,
        &[
            "worktree",
            "add",
            "-b",
            "linked",
            linked.to_str().expect("temporary path is UTF-8"),
        ],
    );
    let identity = gix::open_opts(&linked, gix::open::Options::isolated())?
        .git_dir()
        .canonicalize()?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| {
        matches!(outcome.changes.worktree, Some(Scope::All))
            && outcome.other_worktrees.iter().any(|other| {
                other.git_dir == identity && matches!(other.kind, WorktreeChange::Added)
            })
    })
    .await?;
    // Drain startup verification so a queued initial baseline cannot satisfy the rescan assertion.
    settle(&mut events).await;

    control
        .send(Request::Rescan)
        .map_err(|_| anyhow::anyhow!("monitor remains alive"))?;
    expect_outcome(&mut events, |outcome| {
        outcome.changes.references
            && matches!(outcome.changes.worktree, Some(Scope::All))
            && outcome.other_worktrees.iter().any(|other| {
                other.git_dir == identity
                    && matches!(&other.kind, WorktreeChange::Changed(changes) if matches!(changes.worktree, Some(Scope::All)))
            })
    })
    .await?;

    settle(&mut events).await;
    std::fs::write(linked.join("file"), "monitored after rescan")?;
    expect_outcome(&mut events, |outcome| {
        outcome.other_worktrees.iter().any(|other| {
            other.git_dir == identity
                && matches!(&other.kind, WorktreeChange::Changed(changes) if invalidates(&changes.worktree, "file"))
        })
    })
    .await?;

    settle(&mut events).await;
    let storage = but_project_handle::gitbutler_storage_path(&repo)?;
    std::fs::create_dir_all(&storage)?;
    std::fs::write(storage.join("REFRESH"), "after rescan")?;
    expect_outcome(&mut events, |outcome| {
        outcome.changes.references && outcome.changes.layout
    })
    .await?;
    settle(&mut events).await;
    std::fs::write(repo.git_dir().join("GB_FLUSH"), "after rescan")?;
    expect_outcome(&mut events, |outcome| {
        outcome.changes.references && outcome.changes.layout
    })
    .await?;
    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    assert!(
        events.is_closed(),
        "dropping the control sender shuts down the monitor"
    );
    Ok(())
}

#[tokio::test]
async fn manual_rescan_survives_a_selector_retry_without_option_changes() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let fail_next = Arc::new(AtomicBool::new(false));
    let selector_failure = fail_next.clone();
    let monitor = FileMonitor::new(
        &repo,
        |_| Ok(Default::default()),
        move |_| {
            if selector_failure.swap(false, Ordering::SeqCst) {
                anyhow::bail!("fixture selector fails once");
            }
            Ok(None)
        },
    )?;
    let (control, requested) = mpsc::unbounded_channel();
    let (events, mut received) = mpsc::unbounded_channel();
    let task = tokio::spawn(monitor.run(requested, move |outcome| {
        events.send(outcome).ok();
    }));
    expect_outcome(&mut received, |outcome| {
        matches!(outcome.changes.worktree, Some(Scope::All))
    })
    .await?;
    settle(&mut received).await;

    fail_next.store(true, Ordering::SeqCst);
    control
        .send(Request::Rescan)
        .map_err(|_| anyhow::anyhow!("monitor remains alive"))?;
    let failed = tokio::time::timeout(Duration::from_secs(10), received.recv())
        .await
        .context("manual rescan reports the selector failure")?
        .context("selector failure keeps the outcome stream open")?;
    assert!(
        !failed.errors.is_empty() && !fail_next.load(Ordering::SeqCst),
        "the armed selector failure reaches the consumer"
    );
    assert!(
        !task.is_finished(),
        "a selector error retains the main monitor for recovery"
    );
    expect_outcome(&mut received, |outcome| {
        outcome.changes.references && matches!(outcome.changes.worktree, Some(Scope::All))
    })
    .await
    .context("retry completes the pending rescan with unchanged options and no writes")?;

    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    assert!(received.is_closed(), "recovery preserves orderly shutdown");
    Ok(())
}

#[tokio::test]
async fn reflog_changes_refresh_references_without_relisting_worktrees() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| {
        matches!(outcome.changes.worktree, Some(Scope::All))
    })
    .await?;
    settle(&mut events).await;

    std::fs::write(repo.git_dir().join("logs/HEAD"), "only the reflog changed")?;
    let outcome = expect_outcome(&mut events, |outcome| outcome.changes.references).await?;
    assert!(
        !outcome.changes.layout,
        "a reflog edit refreshes references without relisting worktrees"
    );
    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    Ok(())
}

async fn snapshot(
    control: &mpsc::UnboundedSender<Request>,
    repo: &gix::Repository,
    refresh: bool,
) -> Result<but_filemonitor::Snapshot> {
    let (response, received) = oneshot::channel();
    control
        .send(Request::Snapshot {
            git_dir: repo.git_dir().to_owned(),
            refresh,
            response,
        })
        .map_err(|_| anyhow::anyhow!("monitor accepts snapshot requests"))?;
    tokio::time::timeout(Duration::from_secs(10), received).await??
}

#[tokio::test]
async fn status_is_demand_driven_and_refresh_failure_keeps_monitoring() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let head = repo.head_id()?.to_string();
    let object = repo
        .git_dir()
        .join("objects")
        .join(&head[..2])
        .join(&head[2..]);
    let original = std::fs::read(&object)?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| outcome.changes.worktree.is_some()).await?;
    settle(&mut events).await;

    std::fs::remove_file(&object)?;
    settle(&mut events).await;
    assert!(
        snapshot(&control, &repo, false).await.is_err(),
        "only the status request reads the missing HEAD object"
    );
    std::fs::write(&object, original)?;
    std::fs::write(tmp.path().join("file"), "changed")?;
    expect_outcome(&mut events, |outcome| {
        invalidates(&outcome.changes.worktree, "file")
    })
    .await?;
    let recovered = snapshot(&control, &repo, true).await?;
    assert!(
        recovered.items.iter().any(|item| item.location() == "file"),
        "a failed refresh preserves the stream and can recover"
    );
    drop(control);
    task.await??;
    Ok(())
}

#[tokio::test]
async fn repeated_edits_publish_refresh_coverage_without_new_status_deltas() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| outcome.changes.worktree.is_some()).await?;
    snapshot(&control, &repo, false).await?;
    settle(&mut events).await;
    std::fs::write(tmp.path().join("file"), "first change")?;
    expect_outcome(&mut events, |outcome| {
        invalidates(&outcome.changes.worktree, "file")
    })
    .await?;
    let first = snapshot(&control, &repo, false).await?;
    assert!(first.update.changed, "the file first becomes modified");
    settle(&mut events).await;
    std::fs::write(tmp.path().join("file"), "other change")?;
    expect_outcome(&mut events, |outcome| {
        invalidates(&outcome.changes.worktree, "file")
    })
    .await?;
    let second = snapshot(&control, &repo, false).await?;
    assert!(
        !second.update.changed,
        "the second edit keeps the same status classification"
    );
    assert!(
        invalidates(&second.update.unstaged, "file"),
        "content coverage still tells consumers to reread the file"
    );
    drop(control);
    task.await??;
    Ok(())
}

#[tokio::test]
async fn pending_selection_blocks_status_and_native_wakes_until_retry() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let linked = outside.path().join("linked");
    git(
        tmp.path(),
        &[
            "worktree",
            "add",
            "--detach",
            linked.to_str().expect("fixture path is UTF-8"),
        ],
    );
    let private = repo.git_dir().join("worktrees/linked");
    let fail = Arc::new(AtomicBool::new(false));
    let exclude = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let callbacks = Arc::new(AtomicUsize::new(0));
    let monitor = FileMonitor::new(
        &repo,
        |_| {
            let mut options = gix::status::monitor::Options::default();
            options.repository.retry_interval = Duration::from_secs(1);
            Ok(options)
        },
        {
            let fail = fail.clone();
            let exclude = exclude.clone();
            let calls = calls.clone();
            let private = private.clone();
            move |_| {
                calls.fetch_add(1, Ordering::SeqCst);
                anyhow::ensure!(!fail.load(Ordering::SeqCst), "injected selector failure");
                Ok(Some(if exclude.load(Ordering::SeqCst) {
                    Vec::new()
                } else {
                    vec![private.clone()]
                }))
            }
        },
    )?;
    let (control, requested) = mpsc::unbounded_channel();
    let (events, mut received) = mpsc::unbounded_channel();
    let task = tokio::spawn(monitor.run(requested, {
        let callbacks = callbacks.clone();
        move |outcome| {
            callbacks.fetch_add(1, Ordering::SeqCst);
            events.send(outcome).ok();
        }
    }));
    expect_outcome(&mut received, |outcome| {
        outcome
            .other_worktrees
            .iter()
            .any(|wt| matches!(wt.kind, WorktreeChange::Added))
    })
    .await?;
    settle(&mut received).await;
    exclude.store(true, Ordering::SeqCst);
    fail.store(true, Ordering::SeqCst);
    std::fs::write(private.join("index"), "broken archived index")?;
    control
        .send(Request::Rescan)
        .map_err(|_| anyhow::anyhow!("monitor accepts rescan"))?;
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(outcome) = received.recv().await {
            if outcome
                .errors
                .iter()
                .any(|error| error.to_string().contains("injected selector failure"))
            {
                return;
            }
        }
        panic!("selection failure is reported");
    })
    .await?;
    std::fs::write(tmp.path().join("file"), "queued during selection backoff")?;
    let error = snapshot(&control, &repo, false)
        .await
        .expect_err("status waits for selection recovery");
    assert!(
        error.to_string().contains("subscription selection"),
        "an unresolved selection blocks status before the archived index is opened: {error:#}"
    );
    let before_calls = calls.load(Ordering::SeqCst);
    let before_callbacks = callbacks.load(Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        calls.load(Ordering::SeqCst),
        before_calls,
        "selection retries honor backoff"
    );
    assert_eq!(
        callbacks.load(Ordering::SeqCst),
        before_callbacks,
        "queued native events do not spin while selection blocks service"
    );
    fail.store(false, Ordering::SeqCst);
    control
        .send(Request::Rescan)
        .map_err(|_| anyhow::anyhow!("monitor accepts recovery rescan"))?;
    snapshot(&control, &repo, true).await?;
    drop(control);
    task.await??;
    Ok(())
}

#[tokio::test]
async fn clean_checkout_keeps_native_file_events_without_status_consumers() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| outcome.changes.worktree.is_some()).await?;
    let clean = snapshot(&control, &repo, false).await?;
    assert!(
        clean.items.is_empty(),
        "the Uncommitted row can disappear after a clean snapshot"
    );
    control.send(Request::SetCheckouts(Vec::new()))?;
    assert!(
        snapshot(&control, &repo, false).await.is_err(),
        "no status consumer remains after its row disappears"
    );
    settle(&mut events).await;

    std::fs::write(tmp.path().join("hi"), "new untracked file")?;
    expect_outcome(&mut events, |outcome| {
        invalidates(&outcome.changes.worktree, "hi")
    })
    .await
    .context("a new file must be discovered without a status consumer")?;
    settle(&mut events).await;
    std::fs::remove_file(tmp.path().join("hi"))?;
    expect_outcome(&mut events, |outcome| {
        invalidates(&outcome.changes.worktree, "hi")
    })
    .await
    .context("removing the new file remains observable without a status consumer")?;

    control.send(Request::SetCheckouts(vec![repo.git_dir().to_owned()]))?;
    let clean = snapshot(&control, &repo, false).await?;
    assert!(
        clean.items.is_empty(),
        "returning to the checkout reads its current clean state"
    );
    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    Ok(())
}

#[tokio::test]
async fn checkout_demand_switches_status_and_preserves_metadata_watches() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let outside = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let monitor = FileMonitor::new(&repo, |_| Ok(Default::default()), |_| Ok(None))?;
    let (control, requested) = mpsc::unbounded_channel();
    let (events, mut received) = mpsc::unbounded_channel();
    let task = tokio::spawn(monitor.run(requested, move |outcome| {
        events.send(outcome).ok();
    }));
    expect_outcome(&mut received, |outcome| outcome.changes.worktree.is_some()).await?;
    settle(&mut received).await;
    let linked_path = outside.path().join("linked");
    git(
        tmp.path(),
        &[
            "worktree",
            "add",
            "--detach",
            linked_path.to_str().expect("fixture path is UTF-8"),
        ],
    );
    let linked = gix::open_opts(&linked_path, gix::open::Options::isolated())?;
    expect_outcome(&mut received, |outcome| {
        outcome
            .other_worktrees
            .iter()
            .any(|worktree| matches!(worktree.kind, WorktreeChange::Added))
    })
    .await?;
    settle(&mut received).await;
    std::fs::write(tmp.path().join("main-only"), "main")?;
    expect_outcome(&mut received, |outcome| {
        invalidates(&outcome.changes.worktree, "main-only")
    })
    .await?;
    settle(&mut received).await;
    std::fs::write(linked_path.join("linked-only"), "linked")?;
    expect_outcome(&mut received, |outcome| {
        outcome.other_worktrees.iter().any(|worktree| {
            matches!(&worktree.kind,
                WorktreeChange::Changed(changes) if invalidates(&changes.worktree, "linked-only"))
        })
    })
    .await?;
    control.send(Request::SetCheckouts(vec![
        repo.git_dir().to_owned(),
        linked.git_dir().to_owned(),
    ]))?;
    let main = snapshot(&control, &repo, false).await?;
    assert!(
        main.items.iter().any(|item| item.location() == "main-only"),
        "the main snapshot includes its own new file"
    );
    assert!(
        !main
            .items
            .iter()
            .any(|item| item.location() == "linked-only"),
        "the main snapshot cannot inherit linked checkout status"
    );
    let other = snapshot(&control, &linked, false).await?;
    assert!(
        other
            .items
            .iter()
            .any(|item| item.location() == "linked-only"),
        "the linked snapshot includes its own new file"
    );
    assert!(
        !other
            .items
            .iter()
            .any(|item| item.location() == "main-only"),
        "the linked snapshot cannot inherit main checkout status"
    );
    let again = snapshot(&control, &repo, false).await?;
    assert_eq!(
        again.git_dir, main.git_dir,
        "returning to main restores its target identity"
    );
    assert!(
        again
            .items
            .iter()
            .any(|item| item.location() == "main-only"),
        "returning to main rebuilds its own snapshot"
    );
    settle(&mut received).await;
    control.send(Request::SetCheckouts(vec![linked.git_dir().to_owned()]))?;
    assert!(
        snapshot(&control, &repo, false).await.is_err(),
        "inactive status requests are refused"
    );
    settle(&mut received).await;
    std::fs::write(tmp.path().join("main-only"), "main again")?;
    expect_outcome(&mut received, |outcome| {
        invalidates(&outcome.changes.worktree, "main-only")
    })
    .await
    .context("the main checkout remains observable with only a linked status consumer")?;
    settle(&mut received).await;
    std::fs::write(linked_path.join("linked-only"), "linked again")?;
    let outcome = expect_outcome(&mut received, |outcome| {
        outcome.other_worktrees.iter().any(|worktree| {
            matches!(&worktree.kind,
            WorktreeChange::Changed(changes) if invalidates(&changes.worktree, "linked-only"))
        })
    })
    .await?;
    assert!(
        outcome
            .other_worktrees
            .iter()
            .all(|worktree| matches!(worktree.kind, WorktreeChange::Changed(_))),
        "a status-consumer change does not invent worktree membership changes"
    );
    control.send(Request::SetCheckouts(Vec::new()))?;
    settle(&mut received).await;
    git(tmp.path(), &["branch", "metadata-only"]);
    expect_outcome(&mut received, |outcome| outcome.changes.references).await?;
    assert!(
        snapshot(&control, &linked, false).await.is_err(),
        "clearing every consumer also rejects linked status requests"
    );
    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    Ok(())
}

#[tokio::test]
async fn status_consumers_preserve_explicit_metadata_only_native_coverage() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let monitor = FileMonitor::new(
        &repo,
        |_| {
            let mut options = gix::status::monitor::Options::default();
            options.repository.checkout_filter = Some(Vec::new());
            Ok(options)
        },
        |_| Ok(None),
    )?;
    let (control, requested) = mpsc::unbounded_channel();
    let (events, mut received) = mpsc::unbounded_channel();
    let task = tokio::spawn(monitor.run(requested, move |outcome| {
        events.send(outcome).ok();
    }));
    expect_outcome(&mut received, |_| true).await?;
    settle(&mut received).await;

    control.send(Request::SetCheckouts(vec![repo.git_dir().to_owned()]))?;
    let clean = snapshot(&control, &repo, false).await?;
    assert!(
        clean.items.is_empty(),
        "an explicit status request can read a clean checkout"
    );
    std::fs::write(tmp.path().join("hi"), "not covered by native policy")?;
    while let Ok(Some(outcome)) =
        tokio::time::timeout(Duration::from_millis(300), received.recv()).await
    {
        assert!(
            outcome.errors.is_empty(),
            "metadata-only service succeeds: {:?}",
            outcome.errors
        );
        assert!(
            outcome.changes.worktree.is_none(),
            "requesting status cannot enable checkout coverage excluded by the original options"
        );
    }
    assert!(
        snapshot(&control, &repo, false).await?.items.is_empty(),
        "an uncovered checkout has no native invalidation to refresh its cached status"
    );
    assert!(
        snapshot(&control, &repo, true)
            .await?
            .items
            .iter()
            .any(|item| item.location() == "hi"),
        "callers with excluded checkout coverage can request an explicit status refresh"
    );
    git(tmp.path(), &["branch", "metadata-still-watched"]);
    expect_outcome(&mut received, |outcome| outcome.changes.references).await?;
    drop(control);
    tokio::time::timeout(Duration::from_secs(10), task).await???;
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn aborting_an_idle_monitor_releases_its_task() -> Result<()> {
    let tmp = tempfile::tempdir()?;
    let repo = repository(tmp.path())?;
    let (control, mut events, task) = start(&repo)?;
    expect_outcome(&mut events, |outcome| outcome.changes.worktree.is_some()).await?;
    settle(&mut events).await;
    task.abort();
    let error = tokio::time::timeout(Duration::from_secs(1), task)
        .await?
        .expect_err("the idle task is cancelled");
    assert!(error.is_cancelled());
    assert!(
        events.is_closed(),
        "cancellation releases the outcome callback"
    );
    drop(control);
    Ok(())
}
