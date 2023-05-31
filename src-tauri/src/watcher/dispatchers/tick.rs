use std::time;

use anyhow::Result;
use tokio_util::sync::CancellationToken;

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
        rtx: crossbeam_channel::Sender<time::SystemTime>,
    ) -> Result<()> {
        let mut ticker = tokio::time::interval(interval);

        loop {
            ticker.tick().await;
            if self.cancellation_token.is_cancelled() {
                break;
            }
            println!("{}: tick", self.project_id);
            rtx.send(time::SystemTime::now())?;
        }

        log::info!("{}: ticker stopped", self.project_id);

        Ok(())
    }
}
