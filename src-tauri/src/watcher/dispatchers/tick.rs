use std::time;

use anyhow::Result;
use tauri::async_runtime::{channel, spawn, Receiver};
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

    pub fn run(self, interval: time::Duration) -> Result<Receiver<events::Event>> {
        let (tx, rx) = channel(1);
        let mut ticker = tokio::time::interval(interval);

        spawn(async move {
            log::info!("{}: ticker started", self.project_id);
            loop {
                ticker.tick().await;
                if self.cancellation_token.is_cancelled() {
                    break;
                }
                log::warn!("{}: sending tick", self.project_id);
                if let Err(e) = tx.send(events::Event::Tick(time::SystemTime::now())).await {
                    log::error!("{}: failed to send tick: {}", self.project_id, e);
                }
            }
            log::info!("{}: ticker stopped", self.project_id);
        });

        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_ticker() {
        let dispatcher = Dispatcher::new("test");
        let dispatcher2 = dispatcher.clone();
        let mut rx = dispatcher2.run(Duration::from_millis(10)).unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;

        dispatcher.stop().unwrap();

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
