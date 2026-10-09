#[cfg(target_family = "unix")]
mod spawn {
    use std::{
        path::{Path, PathBuf},
        time::Duration,
    };

    use but_project_handle::{ProjectHandle, ProjectHandleOrLegacyProjectId};
    use gitbutler_filemonitor::{Checkout, InternalEvent, LinkedWorktree, WatchMode};
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
        let monitor = gitbutler_filemonitor::spawn(
            project_id.clone(),
            &workdir,
            || Ok(Vec::new()),
            tx,
            WatchMode::Modern,
        )?;

        std::fs::create_dir(workdir.join("dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, Path::new("dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("dir/new-file"), "hi")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, &Path::new("dir").join("new-file"))
            }
            _ => false,
        })
        .await?;

        std::fs::rename(workdir.join("dir"), workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("old-dir/other-file"), "ho")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, &Path::new("old-dir").join("other-file"))
            }
            _ => false,
        })
        .await?;

        std::fs::remove_dir_all(workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::create_dir(workdir.join("old-dir"))?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, Path::new("old-dir"))
            }
            _ => false,
        })
        .await?;

        std::fs::write(workdir.join("old-dir/other-file"), "")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::ProjectFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, &Path::new("old-dir").join("other-file"))
            }
            _ => false,
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn ignore_rules_do_not_apply_to_the_git_dir() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        let workdir = repo.workdir().expect("non-bare").to_owned();
        std::fs::write(workdir.join(".gitignore"), "HEAD\n")?;
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        let (tx, mut rx) = mpsc::unbounded_channel();
        let monitor = gitbutler_filemonitor::spawn(
            project_id.clone(),
            &workdir,
            || Ok(Vec::new()),
            tx,
            WatchMode::Legacy,
        )?;

        std::fs::write(workdir.join("file"), "")?;
        std::fs::write(workdir.join(".git/HEAD"), "ref: refs/heads/other\n")?;
        monitor.flush()?;
        expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
            InternalEvent::GitFilesChange(id, Checkout::Main, paths) => {
                *id == project_id && contains_path(paths, Path::new("HEAD"))
            }
            _ => false,
        })
        .await?;

        Ok(())
    }

    #[tokio::test]
    async fn git_dir_outside_of_worktree() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (_repo, tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        but_testsupport::invoke_bash_at_dir("git init --separate-git-dir git-dir wt", tmp.path());
        let workdir = tmp.path().join("wt");
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        for watch_mode in [WatchMode::Legacy, WatchMode::Modern] {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let monitor = gitbutler_filemonitor::spawn(
                project_id.clone(),
                &workdir,
                || Ok(Vec::new()),
                tx,
                watch_mode,
            )?;

            std::fs::write(tmp.path().join("git-dir/FETCH_HEAD"), "")?;
            monitor.flush()?;
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::GitFilesChange(id, Checkout::Main, paths) => {
                    *id == project_id && contains_path(paths, Path::new("FETCH_HEAD"))
                }
                _ => false,
            })
            .await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn linked_worktree_changes_name_their_checkout() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        but_testsupport::invoke_bash(
            "git commit --allow-empty -m init && git worktree add nested",
            &repo,
        );
        let workdir = repo.workdir().expect("non-bare").to_owned();
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);
        let nested = Checkout::Linked("nested".into());

        for watch_mode in [WatchMode::Legacy, WatchMode::Modern] {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let monitor = gitbutler_filemonitor::spawn(
                project_id.clone(),
                &workdir,
                {
                    let workdir = workdir.clone();
                    move || {
                        Ok(vec![LinkedWorktree {
                            name: "nested".into(),
                            workdir: workdir.join("nested"),
                        }])
                    }
                },
                tx,
                watch_mode,
            )?;

            std::fs::write(
                workdir.join(".git/worktrees/nested/HEAD"),
                "ref: refs/heads/other\n",
            )?;
            monitor.flush()?;
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::GitFilesChange(id, checkout, paths) => {
                    *id == project_id
                        && *checkout == nested
                        && contains_path(paths, Path::new("HEAD"))
                }
                _ => false,
            })
            .await?;

            std::fs::write(workdir.join("nested/file"), "")?;
            monitor.flush()?;
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::ProjectFilesChange(id, checkout, paths) => {
                    *id == project_id
                        && *checkout == nested
                        && contains_path(paths, Path::new("file"))
                }
                _ => false,
            })
            .await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn linked_worktrees_are_listed_again_once_one_is_registered() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (repo, _tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        but_testsupport::invoke_bash("git commit --allow-empty -m init", &repo);
        let workdir = repo.workdir().expect("non-bare").to_owned();
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        for (watch_mode, name) in [(WatchMode::Legacy, "first"), (WatchMode::Modern, "second")] {
            let (tx, mut rx) = mpsc::unbounded_channel();
            let monitor = gitbutler_filemonitor::spawn(
                project_id.clone(),
                &workdir,
                {
                    let workdir = workdir.clone();
                    move || {
                        Ok(workdir
                            .join(name)
                            .is_dir()
                            .then(|| LinkedWorktree {
                                name: name.into(),
                                workdir: workdir.join(name),
                            })
                            .into_iter()
                            .collect())
                    }
                },
                tx,
                watch_mode,
            )?;

            but_testsupport::invoke_bash(&format!("git worktree add {name}"), &repo);
            monitor.flush()?;
            let registration = Path::new("worktrees").join(name);
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::GitFilesChange(id, Checkout::Main, paths) => {
                    *id == project_id && contains_path(paths, &registration)
                }
                _ => false,
            })
            .await?;

            std::fs::write(
                workdir.join(".git/worktrees").join(name).join("HEAD"),
                "ref: refs/heads/other\n",
            )?;
            monitor.flush()?;
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::GitFilesChange(id, checkout, paths) => {
                    *id == project_id
                        && *checkout == Checkout::Linked(name.into())
                        && contains_path(paths, Path::new("HEAD"))
                }
                _ => false,
            })
            .await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn linked_worktree_files_are_watched() -> anyhow::Result<()> {
        let generous_timeout_for_ci = Duration::from_secs(10);
        let (_repo, tmp) = but_testsupport::writable_scenario("watch-plan-rename-dir");
        but_testsupport::invoke_bash_at_dir(
            "git init main && cd main && git commit --allow-empty -m init",
            tmp.path(),
        );
        let workdir = tmp.path().join("main");
        let project_id =
            ProjectHandleOrLegacyProjectId::ProjectHandle(ProjectHandle::from_path(&workdir)?);

        for (watch_mode, name) in [(WatchMode::Legacy, "first"), (WatchMode::Modern, "second")] {
            let linked_workdir = tmp.path().join(name);
            let spawn = || {
                let (tx, rx) = mpsc::unbounded_channel();
                let linked_workdir = linked_workdir.clone();
                gitbutler_filemonitor::spawn(
                    project_id.clone(),
                    &workdir,
                    move || {
                        Ok(linked_workdir
                            .is_dir()
                            .then(|| LinkedWorktree {
                                name: name.into(),
                                workdir: linked_workdir.clone(),
                            })
                            .into_iter()
                            .collect())
                    },
                    tx,
                    watch_mode,
                )
                .map(|monitor| (monitor, rx))
            };
            let file_changed_in = |file: &'static str| {
                let project_id = project_id.clone();
                move |event: &InternalEvent| match event {
                    InternalEvent::ProjectFilesChange(id, checkout, paths) => {
                        *id == project_id
                            && *checkout == Checkout::Linked(name.into())
                            && contains_path(paths, Path::new(file))
                    }
                    _ => false,
                }
            };

            let (monitor, mut rx) = spawn()?;
            but_testsupport::invoke_bash_at_dir(&format!("git worktree add ../{name}"), &workdir);
            monitor.flush()?;
            let registration = Path::new("worktrees").join(name);
            expect_matching_event(&mut rx, generous_timeout_for_ci, |event| match event {
                InternalEvent::GitFilesChange(_, Checkout::Main, paths) => {
                    contains_path(paths, &registration)
                }
                _ => false,
            })
            .await?;
            std::fs::write(linked_workdir.join("added-while-watching"), "")?;
            monitor.flush()?;
            expect_matching_event(
                &mut rx,
                generous_timeout_for_ci,
                file_changed_in("added-while-watching"),
            )
            .await?;
            drop(monitor);

            let (monitor, mut rx) = spawn()?;
            std::fs::write(linked_workdir.join("present-from-the-start"), "")?;
            monitor.flush()?;
            expect_matching_event(
                &mut rx,
                generous_timeout_for_ci,
                file_changed_in("present-from-the-start"),
            )
            .await?;
        }

        Ok(())
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
