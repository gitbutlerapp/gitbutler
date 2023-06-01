mod file_change;
mod tick;

use std::{path, time};

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::events;

#[derive(Clone)]
pub struct Dispatcher {
    project_id: String,
    tick_dispatcher: tick::Dispatcher,
    file_change_dispatcher: file_change::Dispatcher,
    proxy: crossbeam_channel::Receiver<events::Event>,
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new<P: AsRef<path::Path>>(
        project_id: String,
        path: P,
        proxy_chan: crossbeam_channel::Receiver<events::Event>,
    ) -> Self {
        Self {
            project_id: project_id.clone(),
            tick_dispatcher: tick::Dispatcher::new(project_id.clone()),
            file_change_dispatcher: file_change::Dispatcher::new(project_id, path),
            proxy: proxy_chan,
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.cancellation_token.cancel();
        Ok(())
    }

    pub async fn start(&self, sender: mpsc::UnboundedSender<events::Event>) -> Result<()> {
        let tick_dispatcher = self.tick_dispatcher.clone();
        let s1 = sender.clone();
        let project_id = self.project_id.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = tick_dispatcher
                .start(time::Duration::from_secs(10), s1)
                .await
            {
                log::error!("{}: failed to start ticker: {:#}", project_id, e);
            }
        });

        let file_change_dispatcher = self.file_change_dispatcher.clone();
        let project_id = self.project_id.clone();
        let s2 = sender.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = file_change_dispatcher.start(s2).await {
                log::error!("{}: failed to start file watcher: {:#}", project_id, e);
            }
        });

        let project_id = self.project_id.clone();
        let s3 = sender;
        let proxy = self.proxy.clone();
        tauri::async_runtime::spawn(async move {
            for event in proxy {
                if let Err(e) = s3.send(event) {
                    log::error!("{}: failed to proxy event: {:#}", project_id, e);
                }
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
