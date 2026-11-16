#![cfg(unix)]

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context as _, Result};
use but_filemonitor::FileMonitor;
use gix::notify::{CoverageStatus, Outcome, Scope, WorktreeChange};
use tokio::sync::mpsc;

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
    mpsc::UnboundedSender<()>,
    mpsc::UnboundedReceiver<Outcome>,
    tokio::task::JoinHandle<Result<()>>,
)> {
    let monitor = FileMonitor::new(repo, |_| Ok(None))?;
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

    if removed.coverage != CoverageStatus::Healthy {
        expect_outcome(&mut events, |outcome| {
            outcome.coverage == CoverageStatus::Healthy && outcome.errors.is_empty()
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

    control.send(())?;
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
    let monitor = FileMonitor::new(&repo, move |_| {
        if selector_failure.swap(false, Ordering::SeqCst) {
            anyhow::bail!("fixture selector fails once");
        }
        Ok(None)
    })?;
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
    control.send(())?;
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
