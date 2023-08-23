mod file_change;
mod tick;

use std::{path, time};

use anyhow::{Context, Result};
use tokio::{
    select, spawn,
    sync::mpsc::{channel, Receiver},
};
use tokio_util::sync::CancellationToken;

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    tick_dispatcher: tick::Dispatcher,
    file_change_dispatcher: file_change::Dispatcher,
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            tick_dispatcher: tick::Dispatcher::new(),
            file_change_dispatcher: file_change::Dispatcher::new(),
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        if let Err(err) = self.tick_dispatcher.stop() {
            tracing::error!("failed to stop ticker: {:#}", err);
        }

        if let Err(err) = self.file_change_dispatcher.stop() {
            tracing::error!("failed to stop file change dispatcher: {:#}", err);
        }
        Ok(())
    }

    pub fn run<P: AsRef<path::Path>>(
        self,
        project_id: &str,
        path: P,
    ) -> Result<Receiver<events::Event>> {
        let path = path.as_ref();

        let mut tick_rx = self
            .tick_dispatcher
            .run(project_id, time::Duration::from_secs(10))
            .context(format!("{}: failed to start tick dispatcher", project_id))?;

        let mut file_change_rx =
            self.file_change_dispatcher
                .run(project_id, path)
                .context(format!(
                    "{}: failed to start file change dispatcher",
                    project_id
                ))?;

        let (tx, rx) = channel(1);
        let project_id = project_id.to_owned();
        spawn(async move {
            loop {
                select! {
                    _ = self.cancellation_token.cancelled() => {
                        break;
                    }
                    Some(event) = tick_rx.recv() => {
                        if let Err(e) = tx.send(event).await {
                            tracing::error!("{}: failed to send tick: {}", project_id, e);
                        }
                    }
                    Some(event) = file_change_rx.recv() => {
                        if let Err(e) = tx.send(event).await {
                            tracing::error!("{}: failed to send file change: {}", project_id, e);
                        }
                    }
                }
            }
            tracing::info!("{}: dispatcher stopped", project_id);
        });

        Ok(rx)
    }
}
