use crate::projects;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, Watcher};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
};

pub enum Event {
    Change(PathBuf),
}

pub struct FileWatchers {
    watchers: HashMap<String, RecommendedWatcher>,
}

impl FileWatchers {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub fn watch(&mut self, project: projects::Project) -> Result<mpsc::Receiver<Event>> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;
        watcher.watch(Path::new(&project.path), notify::RecursiveMode::Recursive)?;
        self.watchers.insert(project.id.clone(), watcher);

        let project = Arc::new(Mutex::new(project.clone()));

        let (events_sender, events_receiver) = mpsc::channel();
        tauri::async_runtime::spawn_blocking(move || {
            log::info!("{}: watching files", project.lock().unwrap().id);

            let project = project.lock().unwrap().clone();
            let project_path = Path::new(&project.path);

            let repo = git2::Repository::open(&project.path).expect(
                format!(
                    "{}: failed to open repo at \"{}\"",
                    project.id, project.path
                )
                .as_str(),
            );

            while let Ok(event) = rx.recv() {
                if let Err(e) = event {
                    log::error!("{}: notify event error: {:#}", project.id.clone(), e);
                    continue;
                }

                let event = event.unwrap();

                for file_path in event.paths {
                    let relative_file_path = file_path.strip_prefix(&project_path).unwrap();

                    if repo.is_path_ignored(&relative_file_path).unwrap_or(true) {
                        // make sure we're not watching ignored files
                        continue;
                    }

                    match is_interesting_event(&event.kind) {
                        Some(kind_string) => {
                            log::info!(
                                "{}: \"{}\" {}",
                                project.id,
                                relative_file_path.display(),
                                kind_string
                            );

                            if let Err(e) =
                                events_sender.send(Event::Change(relative_file_path.to_path_buf()))
                            {
                                log::error!("{}: failed to send event: {:#}", project.id, e);
                            }
                        }
                        None => {
                            // ignore
                        }
                    }
                }
            }
            log::info!("{}: stopped watching files", project.id);
        });

        Ok(events_receiver)
    }
}

fn is_interesting_event(kind: &notify::EventKind) -> Option<String> {
    match kind {
        notify::EventKind::Create(notify::event::CreateKind::File) => {
            Some("file created".to_string())
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Data(_)) => {
            Some("file modified".to_string())
        }
        notify::EventKind::Modify(notify::event::ModifyKind::Name(_)) => {
            Some("file renamed".to_string())
        }
        notify::EventKind::Remove(notify::event::RemoveKind::File) => {
            Some("file removed".to_string())
        }
        _ => None,
    }
}
