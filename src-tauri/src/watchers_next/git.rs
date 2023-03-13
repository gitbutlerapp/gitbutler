use crate::projects;
use anyhow::Result;
use notify::{Config, RecommendedWatcher, Watcher};
use std::{
    collections::HashMap,
    path::Path,
    sync::{mpsc, Arc, Mutex},
};

pub enum Event {
    Head,
}

pub struct GitWatchers {
    watchers: HashMap<String, RecommendedWatcher>,
}

impl GitWatchers {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    pub fn watch(&mut self, project: projects::Project) -> Result<mpsc::Receiver<Event>> {
        let (tx, rx) = mpsc::channel();
        let watcher = Arc::new(Mutex::new(RecommendedWatcher::new(tx, Config::default())?));

        watcher.lock().unwrap().watch(
            Path::new(&Path::new(&project.path).join(".git")),
            notify::RecursiveMode::Recursive,
        )?;

        let project = Arc::new(Mutex::new(project.clone()));

        let (events_sender, events_receiver) = mpsc::channel();
        tauri::async_runtime::spawn_blocking(move || {
            log::info!("{}: watching git", project.lock().unwrap().id);

            let project = project.lock().unwrap().clone();
            let project_path = Path::new(&project.path);

            while let Ok(event) = rx.recv() {
                if let Err(e) = event {
                    log::error!("{}: notify event error: {:#}", project.id.clone(), e);
                    continue;
                }

                let event = event.unwrap();

                for file_path in event.paths {
                    let relative_file_path = file_path
                        .strip_prefix(Path::new(&project_path.join(".git")))
                        .unwrap();

                    match is_interesting_event(&event.kind) {
                        Some(kind_string) => {
                            log::info!(
                                "{}: \"{}\" {}",
                                project.id,
                                relative_file_path.display(),
                                kind_string
                            );

                            if relative_file_path == Path::new("HEAD") {
                                if let Err(e) = events_sender.send(Event::Head) {
                                    log::error!("{}: failed to send event: {:#}", project.id, e);
                                }
                            }
                        }
                        None => {
                            // ignore
                        }
                    }
                }
            }
            log::info!("{}: stopped watching git", project.id);
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
