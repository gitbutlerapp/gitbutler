use super::{gb_repository, project_repository};
use crate::{
    app::{dispatchers, listeners},
    events, projects,
};
use anyhow::{Context, Result};
use core::time;
use crossbeam_channel::{bounded, select, unbounded};
use std::sync;

pub struct Watcher<'watcher> {
    project: &'watcher projects::Project,
    gb_repository: &'watcher gb_repository::Repository,
    project_repository: &'watcher project_repository::Repository,

    tick_dispatcher: dispatchers::tick::Dispatcher,
    file_change_dispatcher: dispatchers::file_change::Dispatcher,

    file_change_listener: listeners::file_change::Listener<'watcher>,

    stop: (
        crossbeam_channel::Sender<()>,
        crossbeam_channel::Receiver<()>,
    ),
}

impl<'watcher> Watcher<'watcher> {
    pub fn new(
        project: &'watcher projects::Project,
        gb_repository: &'watcher gb_repository::Repository,
        project_repository: &'watcher project_repository::Repository,
    ) -> Self {
        Self {
            gb_repository,
            project_repository,
            project,
            tick_dispatcher: dispatchers::tick::Dispatcher::new(project),
            file_change_dispatcher: dispatchers::file_change::Dispatcher::new(project),
            file_change_listener: listeners::file_change::Listener::new(
                project,
                project_repository,
                gb_repository,
            ),
            stop: bounded(1),
        }
    }

    pub fn stop(&self) -> anyhow::Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self, events: sync::mpsc::Sender<events::Event>) -> Result<()> {
        let (t_tx, t_rx) = unbounded();
        let tick_dispatcher = self.tick_dispatcher.clone();
        let project_id = self.project.id.clone();

        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = tick_dispatcher.start(time::Duration::from_secs(10), t_tx) {
                log::error!("{}: failed to start ticker: {:#}", project_id, e);
            }
        });

        let (fw_tx, fw_rx) = unbounded();
        let file_change_dispatcher = self.file_change_dispatcher.clone();
        let project_id = self.project.id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = file_change_dispatcher.start(fw_tx) {
                log::error!("{}: failed to start file watcher: {:#}", project_id, e);
            }
        });

        loop {
            select! {
                recv(t_rx) -> ts => {
                    let ts = ts.context("failed to receive tick event")?;
                    log::info!("{}: ticker ticked: {}", self.project.id, ts.elapsed().as_secs());
                }
                recv(fw_rx)-> path => {
                    let path = path.context("failed to receive file change event")?;
                    if !path.starts_with(".git") {
                        if let Err(e) = self.file_change_listener.register(&path) {
                            log::error!("{}: failed to handle file change: {:#}", self.project.id, e);
                        }
                    } else {
                        if let Err(e) = self.on_git_file_change(path.to_str().unwrap(), &events) {
                            log::error!("{}: failed to handle git file change: {:#}", self.project.id, e);
                        }
                    }
                },
                recv(self.stop.1) -> _ => {
                    if let Err(e) = self.tick_dispatcher.stop() {
                        log::error!("{}: failed to stop ticker: {:#}", self.project.id, e);
                    }
                    if let Err(e) = self.file_change_dispatcher.stop() {
                        log::error!("{}: failed to stop file watcher: {:#}", self.project.id, e);
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    fn on_git_file_change(
        &self,
        path: &str,
        events: &sync::mpsc::Sender<events::Event>,
    ) -> Result<()> {
        let event = if path.eq(".git/logs/HEAD") {
            log::info!("{}: git activity", self.project.id);
            Some(events::Event::git_activity(&self.project))
        } else if path.eq(".git/HEAD") {
            log::info!("{}: git head changed", self.project.id);
            let head_ref = self.project_repository.head()?;
            if let Some(head) = head_ref.name() {
                Some(events::Event::git_head(&self.project, &head))
            } else {
                None
            }
        } else if path.eq(".git/index") {
            log::info!("{}: git index changed", self.project.id);
            Some(events::Event::git_index(&self.project))
        } else {
            None
        };

        if let Some(event) = event {
            events
                .send(event)
                .with_context(|| "failed to send git event")?;
        }

        Ok(())
    }
}
