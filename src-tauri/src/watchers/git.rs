use crate::{events, projects, sessions};
use anyhow::Result;
use notify::{Config, RecommendedWatcher, Watcher};
use std::{
    collections::HashMap,
    path::Path,
    sync::{mpsc, Arc, Mutex},
};

pub struct GitWatchers {
    watchers: HashMap<String, RecommendedWatcher>,
}

impl GitWatchers {
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
        sender: mpsc::Sender<events::Event>,
        project: projects::Project,
    ) -> Result<()> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(tx, Config::default())?;

        watcher.watch(
            Path::new(&Path::new(&project.path).join(".git")),
            notify::RecursiveMode::Recursive,
        )?;
        self.watchers.insert(project.id.clone(), watcher);

        let project = Arc::new(Mutex::new(project.clone()));

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

                            match on_file_change(&relative_file_path, &project) {
                                Ok(Some(event)) => {
                                    if let Err(e) = sender.send(event) {
                                        log::error!(
                                            "{}: notify event error: {:#}",
                                            project.id.clone(),
                                            e
                                        );
                                    }
                                }
                                Ok(None) => {}
                                Err(e) => log::error!(
                                    "{}: notify event error: {:#}",
                                    project.id.clone(),
                                    e
                                ),
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

        Ok(())
    }
}

fn on_file_change(
    relative_file_path: &Path,
    project: &projects::Project,
) -> Result<Option<events::Event>> {
    if relative_file_path.ne(Path::new("logs/HEAD")) {
        return Ok(None);
    }

    let repo = git2::Repository::open(project.path.clone())?;
    let event = match sessions::Session::current(&repo, &project)? {
        Some(current_session) => Some(events::Event::session(&project, &current_session)),
        None => None,
    };

    Ok(event)
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
