#![cfg(unix)]

use std::{path::Path, time::Duration};

use anyhow::{Context as _, Result};
use but_filemonitor::FileMonitor;
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
    reason = "the test owns the stop signal, outcome stream and task"
)]
fn start(
    repo: &gix::Repository,
) -> Result<(
    oneshot::Sender<()>,
    mpsc::UnboundedReceiver<Outcome>,
    tokio::task::JoinHandle<Result<()>>,
)> {
    let monitor = FileMonitor::new(repo)?;
    let (stop, stopped) = oneshot::channel();
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
        expect_outcome(&mut events, |outcome| outcome.changes.references).await?;

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
