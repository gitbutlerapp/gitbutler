use crate::projects;
use anyhow::Result;
use notify::{RecommendedWatcher, Watcher};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct FileWatchers {
    watchers: HashMap<String, RecommendedWatcher>,
}

#[derive(Debug, Clone)]
pub enum Event {
    FileChange((projects::Project, PathBuf)),
}

impl FileWatchers {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub fn unwatch(&mut self, project: &projects::Project) -> Result<()> {
        if let Some(mut watcher) = self.watchers.remove(&project.id) {
            watcher.unwatch(&Path::new(&Path::new(&project.path).join(".git")))?;
        }
        Ok(())
    }

    pub fn watch(
        &mut self,
        rtx: tokio::sync::mpsc::Sender<Event>,
        project: projects::Project,
    ) -> Result<()> {
        let (tx, mut rx) = tokio::sync::mpsc::channel(32);
        let mut watcher = notify::recommended_watcher(move |res| {
            let _ = tx.try_send(res);
        })?;

        watcher.watch(&Path::new(&project.path), notify::RecursiveMode::Recursive)?;
        self.watchers.insert(project.id.clone(), watcher);

        let project = Arc::new(Mutex::new(project.clone()));

        tauri::async_runtime::spawn(async move {
            log::info!("{}: watching files", project.lock().unwrap().id);

            let project = project.lock().unwrap().clone();
            let project_path = Path::new(&project.path);

            while let Some(event) = rx.recv().await {
                if let Err(e) = event {
                    log::error!("{}: notify event error: {:#}", project.id.clone(), e);
                    continue;
                }

                let event = event.unwrap();

                if is_interesting_event(&event.kind) {
                    for file_path in event.paths {
                        let relative_file_path = file_path.strip_prefix(&project_path).unwrap();
                        if let Err(e) = rtx.send(Event::FileChange((
                            project.clone(),
                            relative_file_path.to_path_buf(),
                        ))).await {
                            log::error!(
                                "{}: failed to send file change event: {:#}",
                                project.id,
                                e
                            );
                        }
                    }
                }
            }
            log::info!("{}: stopped watching files", project.id);
        });

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
