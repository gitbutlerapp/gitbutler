#[cfg(target_family = "unix")]
mod spawn {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use gitbutler_filemonitor::{InternalEvent, WatchMode};
    use gitbutler_project::ProjectId;
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
        let project_id = ProjectId::from_number_for_testing(1);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor = gitbutler_filemonitor::spawn(project_id, &workdir, tx, WatchMode::Modern)?;

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
}

mod watch_mode {
    use gitbutler_filemonitor::WatchMode;

    #[test]
    fn from_env_or_settings() {
        assert_eq!(WatchMode::from_env_or_settings("auto"), WatchMode::Auto);
        assert_eq!(WatchMode::from_env_or_settings("legacy"), WatchMode::Legacy);
        assert_eq!(WatchMode::from_env_or_settings("modern"), WatchMode::Modern);

        assert_eq!(
            WatchMode::from_env_or_settings("invalid"),
            WatchMode::Auto,
            "Invalid value should fall back to auto"
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
