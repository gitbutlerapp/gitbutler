mod database;
mod file_change;
mod tick;

use std::{path, time};

use anyhow::Result;
use crossbeam_channel::{bounded, select, unbounded, Sender};

use crate::app::{deltas, files, sessions};

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    project_id: String,
    tick_dispatcher: tick::Dispatcher,
    file_change_dispatcher: file_change::Dispatcher,
    database_dispatcher: database::Dispatcher,
    proxy: crossbeam_channel::Receiver<events::Event>,
    stop: (
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>,
    ),
}

impl Dispatcher {
    pub fn new<P: AsRef<path::Path>>(
        project_id: String,
        path: P,
        proxy_chan: crossbeam_channel::Receiver<events::Event>,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        files_database: files::Database,
    ) -> Self {
        Self {
            project_id: project_id.clone(),
            tick_dispatcher: tick::Dispatcher::new(project_id.clone()),
            file_change_dispatcher: file_change::Dispatcher::new(project_id.clone(), path),
            database_dispatcher: database::Dispatcher::new(
                project_id.clone(),
                sessions_database,
                deltas_database,
                files_database,
            ),
            stop: bounded(1),
            proxy: proxy_chan,
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self, sender: Sender<events::Event>) -> Result<()> {
        let (t_tx, t_rx) = unbounded();
        let tick_dispatcher = self.tick_dispatcher.clone();
        let project_id = self.project_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = tick_dispatcher.start(time::Duration::from_secs(10), t_tx) {
                log::error!("{}: failed to start ticker: {:#}", project_id, e);
            }
        });

        let (fw_tx, fw_rx) = unbounded();
        let file_change_dispatcher = self.file_change_dispatcher.clone();
        let project_id = self.project_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = file_change_dispatcher.start(fw_tx) {
                log::error!("{}: failed to start file watcher: {:#}", project_id, e);
            }
        });

        let (db_tx, db_rx) = unbounded();
        let database_dispatcher = self.database_dispatcher.clone();
        let project_id = self.project_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = database_dispatcher.start(db_tx) {
                log::error!("{}: failed to start database listener: {:#}", project_id, e);
            }
        });

        loop {
            select! {
                recv(db_rx) -> event => match event {
                    Ok(event) => {
                        if let Err(e) = sender.send(event) {
                            log::error!("{}: failed to proxy database event: {:#}", self.project_id, e);
                        }
                    },
                    Err(e) => {
                        log::error!("{}: failed to receive database event: {:#}", self.project_id, e);
                    }
                },
                recv(t_rx) -> ts => match ts{
                    Ok(ts) => {
                        if let Err(e) = sender.send(events::Event::Tick(ts)) {
                            log::error!("{}: failed to proxy tick event: {:#}", self.project_id, e);
                        }
                    }
                    Err(e) => {
                        log::error!("{}: failed to receive tick event: {:#}", self.project_id, e);
                    }
                },
                recv(fw_rx) -> path => match path {
                    Ok(path) => {
                        if let Err(e) = sender.send(events::Event::FileChange(path)) {
                            log::error!("{}: failed to proxy path event: {:#}", self.project_id, e);
                        }
                    },
                    Err(e) => {
                        log::error!("{}: failed to receive file change event: {:#}", self.project_id, e);
                    }
                },
                recv(self.proxy) -> event => match event {
                    Ok(event) => {
                        if let Err(e) = sender.send(event) {
                            log::error!("{}: failed to proxy event: {:#}", self.project_id, e);
                        }
                    },
                    Err(e) => {
                        log::error!("{}: failed to receive event: {:#}", self.project_id, e);
                    }
                },
                recv(self.stop.1) -> _ => {
                    if let Err(e) = self.tick_dispatcher.stop() {
                        log::error!("{}: failed to stop ticker: {:#}", self.project_id, e);
                    }
                    if let Err(e) = self.file_change_dispatcher.stop() {
                        log::error!("{}: failed to stop file watcher: {:#}", self.project_id, e);
                    }
                    if let Err(e) = self.database_dispatcher.stop() {
                        log::error!("{}: failed to stop database listener: {:#}", self.project_id, e);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
