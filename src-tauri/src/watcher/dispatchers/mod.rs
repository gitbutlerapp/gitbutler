mod file_change;
mod tick;

use std::time;

use anyhow::Result;
use tokio::sync::mpsc;
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
        self.cancellation_token.cancel();
        Ok(())
    }

    pub async fn run(&self, sender: mpsc::UnboundedSender<events::Event>) -> Result<()> {
        let tick_dispatcher = self.tick_dispatcher.clone();
        let s1 = sender.clone();
        let project_id = self.project_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = tick_dispatcher
                .run(time::Duration::from_secs(10), s1)
                .await
            {
                log::error!("{}: failed to start ticker: {:#}", project_id, e);
            }
        });

        let file_change_dispatcher = self.file_change_dispatcher.clone();
        let project_id = self.project_id.clone();
        let s2 = sender.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = file_change_dispatcher.run(s2).await {
                log::error!("{}: failed to start file watcher: {:#}", project_id, e);
            }
        });

        self.cancellation_token.cancelled().await;

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

        log::info!("{}: dispatcher stopped", self.project_id);

        Ok(())
    }
}
