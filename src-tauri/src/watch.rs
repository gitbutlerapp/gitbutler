use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

use std::{
    collections::HashMap,
    path::Path,
    sync::{mpsc::channel, Mutex},
    thread::spawn,
};

use crate::Project;

#[derive(Default)]
pub struct WatcherCollection(Mutex<HashMap<String, RecommendedWatcher>>);

pub struct Watchers {
    pub watchers: WatcherCollection,
}

impl Watchers {
    pub fn new() -> Self {
        Self {
            watchers: WatcherCollection::default(),
        }
    }
    pub fn unwatch(&self, project: &Project) -> Result<(), String> {
        let watcher = self.watchers.0.lock().unwrap().remove(&project.id);
        if watcher.is_some() {
            watcher
                .unwrap()
                .unwatch(Path::new(&project.path))
                .expect("Unable to unwatch");
        }
        Ok(())
    }

    pub fn watch(
        &self,
        project: &Project,
        callback: fn(project_id: &str, event: Event),
    ) -> Result<(), String> {
        let (tx, rx) = channel();
        let mut watcher =
            RecommendedWatcher::new(tx, Config::default()).map_err(|e| e.to_string())?;
        watcher
            .watch(Path::new(&project.path), RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;
        let project_id = project.id.clone();
        spawn(move || {
            while let Ok(event) = rx.recv() {
                if let Ok(event) = event {
                    callback(&project_id, event);
                }
            }
        });

        self.watchers
            .0
            .lock()
            .unwrap()
            .insert(project.id.clone(), watcher);

        Ok(())
    }
}
