mod files;
mod git;

use crate::projects;
use anyhow::Result;
use std::sync::{mpsc, Arc, Mutex};

enum Event {
    Files(files::Event),
    Git(git::Event),
}

pub struct Watchers {
    file_watchers: files::FileWatchers,
    git_watchers: git::GitWatchers,
}

impl Watchers {
    pub fn new() -> Self {
        Self {
            file_watchers: files::FileWatchers::new(),
            git_watchers: git::GitWatchers::new(),
        }
    }

    pub fn watch(&mut self, project: projects::Project) -> Result<()> {
        let (tx, rx) = mpsc::channel::<Event>();

        let shared_tx = Arc::new(Mutex::new(tx.clone()));
        let tx_clone = shared_tx.clone();

        let files_rx = self.file_watchers.watch(project.clone())?;
        tauri::async_runtime::spawn_blocking(move || {
            while let Ok(event) = files_rx.recv() {
                if let Err(e) = tx_clone.lock().unwrap().send(Event::Files(event)) {
                    log::error!("failed to send event: {:#}", e);
                }
            }
        });

        let git_rx = self.git_watchers.watch(project.clone())?;
        tauri::async_runtime::spawn_blocking(move || {
            while let Ok(event) = git_rx.recv() {
                if let Err(e) = tx.send(Event::Git(event)) {
                    log::error!("failed to send event: {:#}", e);
                }
            }
        });

        tauri::async_runtime::spawn_blocking(move || {
            while let Ok(event) = rx.recv() {
                match event {
                    Event::Files(event) => match event {
                        files::Event::Change(path) => {
                            println!("file changed: {}", path.display());
                        }
                    },
                    Event::Git(event) => match event {
                        git::Event::Head => {
                            println!("HEAD changed");
                        }
                    },
                }
            }
        });

        Ok(())
    }
}
