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
    pub fn new(project_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_ticker() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let dispatcher = Dispatcher::new("test");
        let dispatcher2 = dispatcher.clone();
        let handle = tokio::spawn(async move {
            dispatcher2
                .start(Duration::from_millis(10), tx)
                .await
                .unwrap();
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        dispatcher.stop().unwrap();

        handle.await.unwrap();

        let mut count = 0;
        while let Some(event) = rx.recv().await {
            match event {
                events::Event::Tick(_) => count += 1,
                _ => panic!("unexpected event: {:?}", event),
            }
        }

        assert!(count >= 4);
    }
}
