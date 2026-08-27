#[cfg(target_family = "unix")]
mod spawn {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use but_project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};
    use but_testsupport::{CommandExt, git_at_dir};
    use gitbutler_filemonitor::{InternalEvent, WatchMode};
    use tokio::sync::mpsc;

    async fn expect_matching_event(
        rx: &mut mpsc::UnboundedReceiver<InternalEvent>,
        timeout: Duration,
        predicate: impl Fn(&InternalEvent) -> bool,
    ) -> anyhow::Result<()> {
        let recv = async move {
            while let Some(event) = rx.recv().await {
                if predicate(&event) {
                    return Ok(());
                }
            }
            anyhow::bail!("event channel closed unexpectedly");
        };
        tokio::time::timeout(timeout, recv)
            .await
            .map_err(|_| anyhow::anyhow!("timeout waiting for matching event"))?
    }

    fn contains_path(paths: &[PathBuf], expected: &Path) -> bool {
        paths.iter().any(|p| p == expected)
    }

    #[tokio::test]
    async fn track_directory_changes_after_rename() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        let workdir = repo.workdir().expect("non-bare").to_owned();
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor =
            gitbutler_filemonitor::spawn(project_id.clone(), &workdir, tx, WatchMode::Modern)?;

        std::fs::create_dir(workdir.join("dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("dir/new-file"), "hi")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, &Path::new("dir").join("new-file"))
            }
            _ => false,
        })
        .await?;

        std::fs::rename(workdir.join("dir"), workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("old-dir/other-file"), "ho")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, &Path::new("old-dir").join("other-file"))
            }
            _ => false,
        })
        .await?;

        std::fs::remove_dir_all(workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::create_dir(workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("old-dir/other-file"), "")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, &Path::new("old-dir").join("other-file"))
            }
            _ => false,
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn emits_repository_config_changes() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        let workdir = repo.workdir().expect("non-bare").to_owned();
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor =
            gitbutler_filemonitor::spawn(project_id.clone(), &workdir, tx, WatchMode::Modern)?;

        let config = repo.common_dir().join("config");
        let mut contents = std::fs::read(&config)?;
        contents.extend_from_slice(b"\n[watcher]\n\tprobe = true\n");
        std::fs::write(config, contents)?;
        monitor.flush()?;

        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::GitFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("config"))
            }
            _ => false,
        })
        .await
    }

    #[tokio::test]
    async fn emits_common_config_changes_for_linked_worktrees() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        let linked_worktree = tmp.path().join("linked-worktree");
        git_at_dir(repo.workdir().expect("non-bare"))
            .args([
                "worktree",
                "add",
                "-b",
                "linked-worktree",
                linked_worktree.to_str().expect("UTF-8 test path"),
            ])
            .run();
        let project_id = ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(
            &linked_worktree,
        )?);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor = gitbutler_filemonitor::spawn(
            project_id.clone(),
            &linked_worktree,
            tx,
            WatchMode::Modern,
        )?;

        let config = repo.common_dir().join("config");
        let mut contents = std::fs::read(&config)?;
        contents.extend_from_slice(b"\n[watcher]\n\tlinkedProbe = true\n");
        std::fs::write(config, contents)?;
        monitor.flush()?;

        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::GitFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("config"))
            }
            _ => false,
        })
        .await
    }
}

mod watch_mode {
    use gitbutler_filemonitor::WatchMode;

    #[test]
    fn from_env_or_settings() {
        assert_eq!(
            WatchMode::from_env_or_settings("auto", |_| None),
            WatchMode::Auto
        );
        assert_eq!(
            WatchMode::from_env_or_settings("legacy", |_| None),
            WatchMode::Legacy
        );
        assert_eq!(
            WatchMode::from_env_or_settings("modern", |_| None),
            WatchMode::Modern
        );

        assert_eq!(
            WatchMode::from_env_or_settings("invalid", |_| None),
            WatchMode::Auto,
            "Invalid value should fall back to auto"
        );
    }

    #[test]
    fn from_env_or_settings_prefers_env() {
        assert_eq!(
            WatchMode::from_env_or_settings("legacy", |_| Some("modern".to_string())),
            WatchMode::Modern
        );
    }

    #[test]
    fn from_str() {
        assert_eq!("auto".parse::<WatchMode>().ok(), Some(WatchMode::Auto));
        assert_eq!("legacy".parse::<WatchMode>().ok(), Some(WatchMode::Legacy));
        assert_eq!("modern".parse::<WatchMode>().ok(), Some(WatchMode::Modern));
        assert_eq!("AUTO".parse::<WatchMode>().ok(), Some(WatchMode::Auto));
        assert_eq!("Legacy".parse::<WatchMode>().ok(), Some(WatchMode::Legacy));
        assert_eq!("MODERN".parse::<WatchMode>().ok(), Some(WatchMode::Modern));
        assert!("invalid".parse::<WatchMode>().is_err());
    }
}
