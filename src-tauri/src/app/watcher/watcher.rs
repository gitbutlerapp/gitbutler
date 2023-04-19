use std::sync;

use anyhow::Result;
use crossbeam_channel::{select, unbounded};

use crate::{app::gb_repository, events as app_events, projects, search};

use super::{dispatchers, handlers};

pub struct Watcher<'watcher> {
    project_id: String,
    dispatcher: dispatchers::Dispatcher,
    handler: handlers::Handler<'watcher>,
    stop: crossbeam_channel::Receiver<()>,
}

impl<'watcher> Watcher<'watcher> {
    pub fn new(
        project: &projects::Project,
        project_store: projects::Storage,
        gb_repository: &'watcher gb_repository::Repository,
        deltas_searcher: search::Deltas,
        events: sync::mpsc::Sender<app_events::Event>,
        stop: crossbeam_channel::Receiver<()>,
    ) -> Result<Self> {
        Ok(Self {
            project_id: project.id.clone(),
            dispatcher: dispatchers::Dispatcher::new(project.id.clone(), project.path.clone()),
            handler: handlers::Handler::new(
                project.id.clone(),
                project_store,
                gb_repository,
                deltas_searcher,
                events,
            ),
            stop,
        })
    }

    pub fn start(&self) -> Result<()> {
        let (events_tx, events_rx) = unbounded();
        let dispatcher = self.dispatcher.clone();
        let project_id = self.project_id.clone();
        let etx = events_tx.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = dispatcher.start(etx.clone()) {
                log::error!("{}: failed to start dispatcher: {:#}", project_id, e);
            }
        });

        loop {
            select! {
                recv(events_rx) -> event => match event {
                    Ok(events) => {
                        match self.handler.handle(events) {
                            Ok(events) => {
                                for event in events {
                                    if let Err(e) = events_tx.send(event) {
                                        log::error!("{}: failed to post event: {:#}", self.project_id, e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::error!("{}: failed to handle event: {:#}", self.project_id, e);
                            }
                        }
                    },
                    Err(e) => {
                        log::error!("{}: failed to receive event: {:#}", self.project_id, e);
                    }
                },
                recv(self.stop) -> _ => {
                    if let Err(e) = self.dispatcher.stop() {
                        log::error!("{}: failed to stop dispatcher : {:#}", self.project_id, e);
                    }
                    break;
                }
            }
        }

        Ok(())
    }
}
