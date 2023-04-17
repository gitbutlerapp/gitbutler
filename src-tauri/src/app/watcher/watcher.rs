use std::{sync, time};

use anyhow::{Context, Result};
use crossbeam_channel::{bounded, select, unbounded};

use crate::{app::gb_repository, events, projects, users};

use super::{dispatchers, listeners};

pub struct Watcher<'watcher> {
    project_id: String,

    tick_dispatcher: dispatchers::tick::Dispatcher,
    file_change_dispatcher: dispatchers::file_change::Dispatcher,

    file_change_listener: listeners::file_change::Listener<'watcher>,
    check_current_session_listener: listeners::check_current_session::Listener<'watcher>,

    stop: (
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>,
    ),
}

impl<'watcher> Watcher<'watcher> {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        gb_repository: &'watcher gb_repository::Repository,
        events: sync::mpsc::Sender<events::Event>,
    ) -> Result<Self> {
        let project = project_store
            .get_project(&project_id)
            .context("failed to get project")?;
        if project.is_none() {
            return Err(anyhow::anyhow!("project not found"));
        }
        let project = project.unwrap();
        Ok(Self {
            project_id: project_id.clone(),

            tick_dispatcher: dispatchers::tick::Dispatcher::new(project_id.clone()),
            file_change_dispatcher: dispatchers::file_change::Dispatcher::new(
                project_id.clone(),
                project.path,
            ),

            file_change_listener: listeners::file_change::Listener::new(
                project_id.clone(),
                project_store.clone(),
                gb_repository,
                events.clone(),
            ),
            check_current_session_listener: listeners::check_current_session::Listener::new(
                project_id,
                project_store,
                gb_repository,
                events,
            ),

            stop: bounded(1),
        })
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self) -> Result<()> {
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

        loop {
            select! {
                recv(t_rx) -> ts => match ts{
                    Ok(ts) => {
                        if let Err(e) = self.check_current_session_listener.register(ts) {
                            log::error!("{}: failed to handle tick event: {:#}", self.project_id, e);
                        }
                    }
                    Err(e) => {
                        log::error!("{}: failed to receive tick event: {:#}", self.project_id, e);
                    }
                },
                recv(fw_rx)-> path => match path {
                    Ok(path) => {
                        if let Err(e) = self.file_change_listener.register(&path) {
                            log::error!("{}: failed to handle file change: {:#}", self.project_id, e);
                        }
                    },
                    Err(e) => {
                        log::error!("{}: failed to receive file change event: {:#}", self.project_id, e);
                    }
                },
                recv(self.stop.1) -> _ => {
                    if let Err(e) = self.tick_dispatcher.stop() {
                        log::error!("{}: failed to stop ticker: {:#}", self.project_id, e);
                    }
                    if let Err(e) = self.file_change_dispatcher.stop() {
                        log::error!("{}: failed to stop file watcher: {:#}", self.project_id, e);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
