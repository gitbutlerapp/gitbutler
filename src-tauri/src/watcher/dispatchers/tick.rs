use std::time;

use anyhow::Result;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::watcher::events;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    project_id: String,
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.cancellation_token.cancel();
        Ok(())
    }

    pub async fn start(
        &self,
        interval: time::Duration,
        rtx: mpsc::UnboundedSender<events::Event>,
    ) -> Result<()> {
        let mut ticker = tokio::time::interval(interval);

        log::info!("{}: ticker started", self.project_id);

        loop {
            ticker.tick().await;
            if self.cancellation_token.is_cancelled() {
                break;
            }
            if let Err(e) = rtx.send(events::Event::Tick(time::SystemTime::now())) {
                log::error!("{}: failed to send tick: {}", self.project_id, e);
            }
        }

        log::info!("{}: ticker stopped", self.project_id);

        Ok(())
    }
}
