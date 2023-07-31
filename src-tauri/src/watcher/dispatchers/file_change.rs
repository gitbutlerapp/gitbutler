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
        let (mut watcher, mut rx) = async_watcher()?;
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

        while let Some(event) = rx.recv().await {
            for file_path in event.paths {
                match file_path.strip_prefix(&self.project_path) {
                    Ok(relative_file_path) => {
                        if let Err(e) =
                            rtx.send(events::Event::FileChange(relative_file_path.to_path_buf()))
                        {
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
        }

        log::info!("{}: file watcher stopped", self.project_id);

        Ok(())
    }
}

fn async_watcher() -> notify::Result<(RecommendedWatcher, mpsc::Receiver<notify::Event>)> {
    let (tx, rx) = mpsc::channel(1);

    let watcher = RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| match res {
            Ok(event) => {
                if is_interesting_event(&event.kind) {
                    block_on(async {
                        if let Err(error) = tx.send(event).await {
                            log::error!("failed to send file change event: {:#}", error);
                        }
                    });
                }
            }
            Err(error) => log::error!("file watcher error: {:#}", error),
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}

fn is_interesting_event(kind: &notify::EventKind) -> bool {
    matches!(
        kind,
        notify::EventKind::Create(notify::event::CreateKind::File)
            | notify::EventKind::Modify(notify::event::ModifyKind::Data(_))
            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
            | notify::EventKind::Remove(notify::event::RemoveKind::File)
    )
}
