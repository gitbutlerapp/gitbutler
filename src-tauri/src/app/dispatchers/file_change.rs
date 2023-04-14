use std::{
    path::PathBuf,
    sync::{self, Arc, Mutex},
};

use crate::projects;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, Watcher};

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

    pub fn start(&self, rtx: crossbeam_channel::Sender<PathBuf>) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(
            &std::path::Path::new(&self.project_path),
            notify::RecursiveMode::Recursive,
        )?;
        self.watcher.lock().unwrap().replace(watcher);

        log::info!("{}: file watcher started", self.project_id);

        for res in rx {
            match res {
                Ok(event) => {
                    if !is_interesting_event(&event.kind) {
                        continue;
                    }
                    for file_path in event.paths {
                        let relative_file_path =
                            file_path.strip_prefix(&self.project_path).unwrap();
                        if let Err(e) = rtx.send(relative_file_path.to_path_buf()) {
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
