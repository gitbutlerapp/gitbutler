mod file_change;
mod tick;

use std::time;

use anyhow::{Context, Result};
use tokio::{
    select, spawn,
    sync::mpsc::{channel, Receiver},
};
use tokio_util::sync::CancellationToken;

use crate::projects;

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    project_id: String,
    tick_dispatcher: tick::Dispatcher,
    file_change_dispatcher: file_change::Dispatcher,
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            project_id: project.id.clone(),
            tick_dispatcher: tick::Dispatcher::new(&project.id),
            file_change_dispatcher: file_change::Dispatcher::new(project),
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Err(err) = self.tick_dispatcher.stop() {
            log::error!("{}: failed to stop ticker: {:#}", self.project_id, err);
        }

        if let Err(err) = self.file_change_dispatcher.stop() {
            log::error!(
                "{}: failed to stop file change dispatcher: {:#}",
                self.project_id,
                err
            );
        }
        Ok(())
    }

    pub fn run(self) -> Result<Receiver<events::Event>> {
        let mut tick_rx = self
            .tick_dispatcher
            .run(time::Duration::from_secs(10))
            .context(format!(
                "{}: failed to start tick dispatcher",
                self.project_id
            ))?;

        let mut file_change_rx = self.file_change_dispatcher.run().context(format!(
            "{}: failed to start file change dispatcher",
            self.project_id
        ))?;

        let (tx, rx) = channel(1);
        spawn(async move {
            loop {
                select! {
                    _ = self.cancellation_token.cancelled() => {
                        break;
                    }
                    Some(event) = tick_rx.recv() => {
                        log::warn!("{}: proxying tick", self.project_id);
                        if let Err(e) = tx.send(event).await {
                            log::error!("{}: failed to send tick: {}", self.project_id, e);
                        }
                    }
                    Some(event) = file_change_rx.recv() => {
                        log::warn!("{}: proxying file change", self.project_id);
                        if let Err(e) = tx.send(event).await {
                            log::error!("{}: failed to send file change: {}", self.project_id, e);
                        }
                    }
                }
            }
            log::info!("{}: dispatcher stopped", self.project_id);
        });

        Ok(rx)
    }
}
