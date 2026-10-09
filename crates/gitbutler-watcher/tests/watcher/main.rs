#![cfg(target_family = "unix")]

use std::{path::Path, time::Duration};

use but_ctx::Context;
use but_project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};
use but_settings::AppSettingsWithDiskSync;
use but_testsupport::invoke_bash_at_dir;
use gitbutler_watcher::{Change, Handler, WatchMode, WatcherHandle};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};

const QUIET: Duration = Duration::from_millis(1500);
const GENEROUS_TIMEOUT_FOR_CI: Duration = Duration::from_secs(10);

struct Watched {
    project_id: ProjectHandleOrLegacyProjectId,
    app_settings: AppSettingsWithDiskSync,
    changes: UnboundedReceiver<Change>,
    watcher: WatcherHandle,
}

impl Watched {
    fn ctx(&self) -> anyhow::Result<Context> {
        let mut ctx: Context = self.project_id.clone().try_into()?;
        ctx.settings = self.app_settings.get()?.clone();
        Ok(ctx)
    }

    async fn change_before_quiet(&mut self) -> anyhow::Result<Option<Change>> {
        self.watcher.flush()?;
        Ok(tokio::time::timeout(QUIET, self.changes.recv())
            .await
            .ok()
            .flatten())
    }

    async fn settle(&mut self) -> anyhow::Result<()> {
        while self.change_before_quiet().await?.is_some() {}
        Ok(())
    }

    async fn expect(&mut self, matches: impl Fn(&Change) -> bool) -> anyhow::Result<()> {
        self.watcher.flush()?;
        let matching_change = async {
            while let Some(change) = self.changes.recv().await {
                if matches(&change) {
                    return;
                }
            }
        };
        tokio::time::timeout(GENEROUS_TIMEOUT_FOR_CI, matching_change)
            .await
            .map_err(|_| anyhow::anyhow!("timeout waiting for matching change"))
    }
}

fn watch(main: &Path, config_dir: &Path) -> anyhow::Result<Watched> {
    let app_settings = AppSettingsWithDiskSync::new_with_customization(
        config_dir,
        Some(serde_json::json!({ "featureFlags": { "worktreeManipulation": true } })),
    )?;
    let project_id = ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(main)?);
    let (tx, changes) = unbounded_channel();
    let watcher = gitbutler_watcher::watch_in_background(
        Handler::new(move |change| {
            tx.send(change).ok();
            Ok(())
        }),
        main,
        project_id.clone(),
        app_settings.clone(),
        WatchMode::Legacy,
    )?;
    Ok(Watched {
        project_id,
        app_settings,
        changes,
        watcher,
    })
}

#[tokio::test]
async fn linked_worktree_head_moves_are_workspace_activity() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init main && cd main && git commit --allow-empty -m init && git worktree add ../linked",
        tmp.path(),
    );
    let main = tmp.path().join("main");
    let config_dir = tmp.path().join("config");
    watch(&main, &config_dir)?
        .ctx()?
        .set_worktree_archived("linked".into(), false)?;

    let mut watched = watch(&main, &config_dir)?;
    watched.settle().await?;

    invoke_bash_at_dir("git switch --detach", &tmp.path().join("linked"));
    watched
        .expect(|change| matches!(change, Change::WorkspaceActivity { .. }))
        .await
}

#[tokio::test]
async fn linked_worktrees_added_while_watching_are_watched() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init main && cd main && git commit --allow-empty -m init",
        tmp.path(),
    );
    let main = tmp.path().join("main");
    let mut watched = watch(&main, &tmp.path().join("config"))?;
    watched.ctx()?.worktrees_with_state()?;

    invoke_bash_at_dir("git worktree add --detach ../linked", &main);
    watched
        .expect(|change| matches!(change, Change::WorkspaceActivity { .. }))
        .await?;
    watched.settle().await?;

    invoke_bash_at_dir(
        "git commit --allow-empty -m detached",
        &tmp.path().join("linked"),
    );
    watched
        .expect(|change| matches!(change, Change::WorkspaceActivity { .. }))
        .await
}

#[tokio::test]
async fn linked_worktrees_archived_while_watching_are_not_watched() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init main && cd main && git commit --allow-empty -m init",
        tmp.path(),
    );
    let main = tmp.path().join("main");
    let mut watched = watch(&main, &tmp.path().join("config"))?;
    let ctx = watched.ctx()?;
    ctx.worktrees_with_state()?;

    invoke_bash_at_dir("git worktree add ../linked", &main);
    watched.settle().await?;

    ctx.set_worktree_archived("linked".into(), true)?;
    but_project_handle::write_invalidation_sentinel(&ctx.project_data_dir, &["Worktrees"]);
    watched.settle().await?;

    invoke_bash_at_dir("git switch --detach", &tmp.path().join("linked"));
    let change = watched.change_before_quiet().await?;
    assert!(
        change.is_none(),
        "nothing is reported about an archived worktree, got {change:?}"
    );
    Ok(())
}

#[tokio::test]
async fn linked_worktree_changes_are_reported_for_that_worktree() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    invoke_bash_at_dir(
        "git init main && cd main && git commit --allow-empty -m init && git worktree add ../linked",
        tmp.path(),
    );
    let main = tmp.path().join("main");
    let linked = tmp.path().join("linked");
    let config_dir = tmp.path().join("config");
    watch(&main, &config_dir)?
        .ctx()?
        .set_worktree_archived("linked".into(), false)?;

    let mut watched = watch(&main, &config_dir)?;
    watched.settle().await?;

    let reports_file = |change: &Change, because_of: &[&str]| match change {
        Change::LinkedWorktreeChanges {
            worktree,
            changes,
            changed_paths,
            ..
        } => {
            worktree == "linked"
                && changes
                    .worktree_changes
                    .changes
                    .iter()
                    .any(|change| change.path_bytes == "file")
                && changed_paths.iter().eq(because_of.iter().map(Path::new))
        }
        _ => false,
    };

    std::fs::write(linked.join("file"), "content")?;
    watched
        .expect(|change| reports_file(change, &["file"]))
        .await?;
    watched.settle().await?;

    invoke_bash_at_dir("git add file", &linked);
    watched.expect(|change| reports_file(change, &[])).await
}
