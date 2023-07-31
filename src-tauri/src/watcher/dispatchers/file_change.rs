use std::{
    path,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use futures::executor::block_on;
use notify::{Config, RecommendedWatcher, Watcher};
use tokio::sync::mpsc;

use crate::{projects, watcher::events};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    project_path: path::PathBuf,
    project_id: String,
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            project_path: path::PathBuf::from(&project.path),
            project_id: project.id.clone(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.lock().unwrap().take() {
            watcher
                .unwatch(std::path::Path::new(&self.project_path))
                .context(format!(
                    "failed to unwatch project path: {}",
                    self.project_path.display()
                ))?;
        }
        Ok(())
    }

    pub async fn run(&self, rtx: mpsc::UnboundedSender<events::Event>) -> Result<()> {
        let repo = git2::Repository::open(&self.project_path).with_context(|| {
            format!(
                "failed to open project repository: {}",
                self.project_path.display()
            )
        })?;

        let (tx, mut rx) = mpsc::channel(1);
        let mut watcher = RecommendedWatcher::new(
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
                            log::warn!("sending file change event: {}", path.display());
                            if let Err(error) = tx.send(path).await {
                                log::error!("failed to send file change event: {:#}", error);
                            }
                        });
                    }
                }
                Err(error) => log::error!("file watcher error: {:#}", error),
            },
            Config::default(),
        )?;

        watcher
            .watch(
                std::path::Path::new(&self.project_path),
                notify::RecursiveMode::Recursive,
            )
            .with_context(|| {
                format!(
                    "failed to watch project path: {}",
                    self.project_path.display()
                )
            })?;
        self.watcher.lock().unwrap().replace(watcher);

        log::info!("{}: file watcher started", self.project_id);

        while let Some(file_path) = rx.recv().await {
            match file_path.strip_prefix(&self.project_path) {
                Ok(relative_file_path) => {
                    let event = if relative_file_path.starts_with(".git") {
                        events::Event::GitFileChange(
                            relative_file_path
                                .strip_prefix(".git")
                                .unwrap()
                                .to_path_buf(),
                        )
                    } else {
                        events::Event::ProjectFileChange(relative_file_path.to_path_buf())
                    };
                    if let Err(e) = rtx.send(event) {
                        log::error!(
                            "{}: failed to send file change event: {:#}",
                            self.project_id,
                            e
                        );
                    }
                }
                Err(err) => {
                    log::error!("{}: failed to strip prefix: {:#}", self.project_id, err)
                }
            }
        }

        log::info!("{}: file watcher stopped", self.project_id);

        Ok(())
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

fn is_interesting_file(git_repo: &git2::Repository, file_path: &path::Path) -> bool {
    if file_path.starts_with(git_repo.path()) {
        file_path.ends_with("FETCH_HEAD")
            || file_path.eq(path::Path::new("logs/HEAD"))
            || file_path.eq(path::Path::new("HEAD"))
            || file_path.eq(path::Path::new("index"))
    } else {
        !git_repo.is_path_ignored(file_path).unwrap_or(false)
    }
}
