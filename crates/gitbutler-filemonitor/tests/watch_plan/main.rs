#[cfg(target_family = "unix")]
mod unix {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use gitbutler_filemonitor::{InternalEvent, WatchMode};
    use gitbutler_project::ProjectId;
    use tokio::sync::mpsc;

    async fn recv_until(
        rx: &mut mpsc::UnboundedReceiver<InternalEvent>,
        timeout: Duration,
        predicate: impl Fn(&InternalEvent) -> bool,
    ) -> anyhow::Result<InternalEvent> {
        let recv = async move {
            while let Some(event) = rx.recv().await {
                if predicate(&event) {
                    return Ok(event);
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
    async fn watch_plan_tracks_changes_after_directory_rename() -> anyhow::Result<()> {
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        let workdir = repo.workdir().expect("non-bare").to_owned();
        let project_id = ProjectId::from_number_for_testing(1);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor = gitbutler_filemonitor::spawn(project_id, &workdir, tx, WatchMode::Plan)?;

        std::fs::create_dir(workdir.join("dir"))?;
        monitor.flush()?;
        recv_until(&mut rx, Duration::from_secs(10), |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("dir/new-file"), "hi")?;
        monitor.flush()?;
        recv_until(&mut rx, Duration::from_secs(10), |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id
                    && contains_path(paths, Path::new("dir").join("new-file").as_path())
            }
            _ => false,
        })
        .await?;

        std::fs::rename(workdir.join("dir"), workdir.join("old-dir"))?;
        monitor.flush()?;
        recv_until(&mut rx, Duration::from_secs(10), |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("old-dir/other-file"), "ho")?;
        monitor.flush()?;
        recv_until(&mut rx, Duration::from_secs(10), |event| match event {
            InternalEvent::ProjectFilesChange(id, paths) => {
                *id == project_id
                    && contains_path(paths, Path::new("old-dir").join("other-file").as_path())
            }
            _ => false,
        })
        .await?;

        Ok(())
    }
}
