use super::gb_repository;
use crate::{
    app::{
        dispatchers::{file_change, tick},
        reader,
    },
    git, projects,
};
use anyhow::Result;
use core::time;
use std::path::PathBuf;
use tokio::sync;

pub struct Watcher<'watcher> {
    project: &'watcher projects::Project,
    gb_repository: &'watcher gb_repository::Repository,

    ticker: tick::Dispatcher,
    file_watcher: file_change::Dispatcher,

    stop: tokio_util::sync::CancellationToken,
}

impl<'watcher> Watcher<'watcher> {
    pub fn new(
        gb_repository: &'watcher gb_repository::Repository,
        project: &'watcher projects::Project
        ) -> Self {
        Self {
            gb_repository,
            project,
            ticker: tick::Dispatcher::new(project),
            file_watcher: file_change::Dispatcher::new(project),
            stop: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.stop.cancel();
        Ok(())
    }

    pub async fn start(&self) -> Result<()> {
        let (t_tx, mut t_rx) = sync::mpsc::channel(128);
        let ticker = self.ticker.clone();
        let project_id = self.project.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = ticker.start(time::Duration::from_secs(10), t_tx).await {
                log::error!("{}: failed to start ticker: {:#}", project_id, e);
            }
        });

        let (fw_tx, mut fw_rx) = sync::mpsc::channel(128);
        let file_watcher = self.file_watcher.clone();
        let project_id = self.project.id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = file_watcher.start(fw_tx).await {
                log::error!("{}: failed to start file watcher: {:#}", project_id, e);
            }
        });

        let reader = reader::DirReader::open(&std::path::Path::new(&self.project.path));
        loop {
            tokio::select! {
                Some(ts) = t_rx.recv() => {
                    log::info!("{}: ticker ticked: {}", self.project.id, ts.elapsed().as_secs());
                }
                Some(path) = fw_rx.recv() => {
                    log::info!("{}: file changed: {}", self.project.id, path.display());
                },
                _ = self.stop.cancelled() => {
                    if let Err(e) = self.ticker.stop() {
                        log::error!("{}: failed to stop ticker: {:#}", self.project.id, e);
                    }
                    if let Err(e) = self.file_watcher.stop() {
                        log::error!("{}: failed to stop file watcher: {:#}", self.project.id, e);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
