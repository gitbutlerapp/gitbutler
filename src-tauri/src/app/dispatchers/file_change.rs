use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use crate::projects;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, Watcher};
use tokio::sync;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    project_path: String,
    project_id: String,
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            project_path: project.path.clone(),
            project_id: project.id.clone(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.lock().unwrap().take() {
            watcher.unwatch(&std::path::Path::new(&self.project_path))?;
        }
        Ok(())
    }

    pub async fn start(&self, rtx: sync::mpsc::Sender<PathBuf>) -> Result<()> {
        let (mut watcher, mut rx) = async_watcher()?;

        watcher.watch(
            &std::path::Path::new(&self.project_path),
            notify::RecursiveMode::Recursive,
        )?;
        self.watcher.lock().unwrap().replace(watcher);

        log::info!("{}: file watcher started", self.project_id);

        while let Some(res) = rx.recv().await {
            match res {
                Ok(event) => {
                    if !is_interesting_event(&event.kind) {
                        continue;
                    }
                    for file_path in event.paths {
                        let relative_file_path =
                            file_path.strip_prefix(&self.project_path).unwrap();
                        if let Err(e) = rtx.send(relative_file_path.to_path_buf()).await {
                            log::error!(
                                "{}: failed to send file change event: {:#}",
                                self.project_id,
                                e
                            );
                        }
                    }
                }
                Err(e) => log::error!("{}: file watcher error: {:#}", self.project_id, e),
            }
        }

        log::info!("{}: file watcher stopped", self.project_id);

        Ok(())
    }
}

fn is_interesting_event(kind: &notify::EventKind) -> bool {
    match kind {
        notify::EventKind::Create(notify::event::CreateKind::File) => true,
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => true,
        notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => true,
        notify::EventKind::Remove(notify::event::RemoveKind::File) => true,
        _ => false,
    }
}

fn async_watcher() -> notify::Result<(
    RecommendedWatcher,
    sync::mpsc::Receiver<notify::Result<notify::Event>>,
)> {
    let (tx, rx) = sync::mpsc::channel(1);

    let watcher = RecommendedWatcher::new(
        move |res| {
            futures::executor::block_on(async {
                tx.send(res).await.unwrap();
            })
        },
        Config::default(),
    )?;

    Ok((watcher, rx))
}
