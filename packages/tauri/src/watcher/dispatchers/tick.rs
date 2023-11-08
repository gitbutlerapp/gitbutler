use std::time;

use anyhow::Result;
use tokio::{
    sync::mpsc::{channel, Receiver},
    task,
};
use tokio_util::sync::CancellationToken;

use crate::{projects::ProjectId, watcher::events};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    cancellation_token: CancellationToken,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            cancellation_token: CancellationToken::new(),
        }
    }

    pub fn stop(&self) {
        self.cancellation_token.cancel();
    }

    pub fn run(
        self,
        project_id: &ProjectId,
        interval: time::Duration,
    ) -> Result<Receiver<events::Event>> {
        let (tx, rx) = channel(1);
        let mut ticker = tokio::time::interval(interval);

        task::Builder::new()
            .name(&format!("{} ticker", project_id))
            .spawn({
                let project_id = *project_id;
                async move {
                    tracing::debug!(%project_id, "ticker started");
                    loop {
                        ticker.tick().await;
                        if self.cancellation_token.is_cancelled() {
                            break;
                        }
                        if let Err(error) = tx.send(events::Event::Tick(project_id)).await {
                            tracing::error!(%project_id, ?error, "failed to send tick");
                        }
                    }
                    tracing::debug!(%project_id, "ticker stopped");
                }
            })?;

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn test_ticker() {
        let dispatcher = Dispatcher::new();
        let dispatcher2 = dispatcher.clone();
        let mut rx = dispatcher2
            .run(&ProjectId::generate(), Duration::from_millis(10))
            .unwrap();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            dispatcher.stop();
        });

        let mut count = 0_i32;
        while let Some(event) = rx.recv().await {
            if let events::Event::Tick(_) = event {
                count += 1_i32;
            }
        }

        assert!(count >= 4_i32);
    }
}
