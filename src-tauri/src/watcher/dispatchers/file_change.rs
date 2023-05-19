use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use notify::{Config, RecommendedWatcher, Watcher};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    watcher: Arc<Mutex<Option<RecommendedWatcher>>>,
    project_path: PathBuf,
    project_id: String,
}

impl Dispatcher {
    pub fn new<P: AsRef<std::path::Path>>(project_id: String, path: P) -> Self {
        Self {
            watcher: Arc::new(Mutex::new(None)),
            project_path: path.as_ref().to_path_buf(),
            project_id,
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Some(mut watcher) = self.watcher.lock().unwrap().take() {
            watcher
                .unwatch(&std::path::Path::new(&self.project_path))
                .context(format!(
                    "failed to unwatch project path: {}",
                    self.project_path.display()
                ))?;
        }
        Ok(())
    }

    pub fn start(&self, rtx: crossbeam_channel::Sender<PathBuf>) -> Result<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher
            .watch(
                &std::path::Path::new(&self.project_path),
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

        for res in rx {
            match res {
                Ok(event) => {
                    if !is_interesting_event(&event.kind) {
                        continue;
                    }
                    for file_path in event.paths {
                        if let Err(e) = file_path
                            .strip_prefix(&self.project_path)
                            .with_context(|| {
                                format!(
                                    "failed to striprefix from file path: {}",
                                    file_path.display()
                                )
                            })
                            .map(|relative_file_path| rtx.send(relative_file_path.to_path_buf()))
                        {
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
