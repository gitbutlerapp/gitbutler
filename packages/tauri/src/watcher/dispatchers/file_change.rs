use std::{
    path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use futures::executor::block_on;
use notify::{Config, RecommendedWatcher, Watcher};
use tokio::{
    sync::mpsc::{channel, Receiver},
    task,
};

use crate::{git, watcher::events};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.watcher.lock().unwrap().take();
        Ok(())
    }

    pub fn run(self, project_id: &str, path: &path::Path) -> Result<Receiver<events::Event>> {
        let repo = git::Repository::open(path)
            .with_context(|| format!("failed to open project repository: {}", path.display()))?;

        let (notify_tx, mut notify_rx) = channel(1);
        let mut watcher = RecommendedWatcher::new(
            {
                let project_id = project_id.to_string();
                move |res: notify::Result<notify::Event>| match res {
                    Ok(event) => {
                        if !is_interesting_kind(&event.kind) {
                            return;
                        }
                        for path in event
                            .paths
                            .into_iter()
                            .filter(|file| is_interesting_file(&repo, file))
                        {
                            block_on(async {
                                tracing::info!(
                                    project_id,
                                    path = %path.display(),
                                    "file change detected"
                                );
                                if let Err(error) = notify_tx.send(path).await {
                                    tracing::error!(?error, "failed to send file change event",);
                                }
                            });
                        }
                    }
                    Err(error) => tracing::error!(?error, "file watcher error"),
                }
            },
            Config::default(),
        )?;

        watcher
            .watch(std::path::Path::new(path), notify::RecursiveMode::Recursive)
            .with_context(|| format!("failed to watch project path: {}", path.display()))?;
        self.watcher.lock().unwrap().replace(watcher);

        tracing::debug!(project_id, "file watcher started");

        let (tx, rx) = channel(1);
        let project_id = project_id.to_string();
        task::Builder::new()
            .name(&format!("{} file watcher", project_id))
            .spawn({
                let path = path.to_path_buf();
                let project_id = project_id.clone();
                async move {
                    while let Some(file_path) = notify_rx.recv().await {
                        match file_path.strip_prefix(&path) {
                            Ok(relative_file_path) => {
                                let event = if relative_file_path.starts_with(".git") {
                                    events::Event::GitFileChange(
                                        project_id.to_string(),
                                        relative_file_path
                                            .strip_prefix(".git")
                                            .unwrap()
                                            .to_path_buf(),
                                    )
                                } else {
                                    events::Event::ProjectFileChange(
                                        project_id.to_string(),
                                        relative_file_path.to_path_buf(),
                                    )
                                };
                                if let Err(error) = tx.send(event).await {
                                    tracing::error!(
                                        project_id,
                                        ?error,
                                        "failed to send file change event",
                                    );
                                }
                            }
                            Err(error) => {
                                tracing::error!(project_id, ?error, "failed to strip prefix")
                            }
                        }
                    }
                    tracing::debug!(project_id, "file watcher stopped");
                }
            })?;

        Ok(rx)
    }
}

fn is_interesting_kind(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(notify::event::CreateKind::File)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Remove(notify::event::RemoveKind::File)
    )
}

fn is_interesting_file(git_repo: &git::Repository, file_path: &path::Path) -> bool {
    if file_path.starts_with(git_repo.path()) {
        let check_file_path = file_path.strip_prefix(git_repo.path()).unwrap();
        check_file_path.ends_with("FETCH_HEAD")
            || check_file_path.eq(path::Path::new("logs/HEAD"))
            || check_file_path.eq(path::Path::new("HEAD"))
            || check_file_path.eq(path::Path::new("index"))
    } else {
        !git_repo.is_path_ignored(file_path).unwrap_or(false)
    }
}
