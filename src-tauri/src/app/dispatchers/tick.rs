use crate::projects;
use anyhow::Result;
use std::sync::{atomic::AtomicBool, Arc};
use tokio::{sync, time};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    project_id: String,
    stop: Arc<AtomicBool>,
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            project_id: project.id.clone(),
            stop: AtomicBool::new(false).into(),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    pub async fn start(
        &self,
        interval: time::Duration,
        rtx: sync::mpsc::Sender<std::time::Instant>,
    ) -> Result<()> {
        let mut interval = time::interval(interval);
        log::info!("{}: ticker started", self.project_id);
        loop {
            if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }

            let tick = interval.tick().await;

            if let Err(e) = rtx.send(tick.into_std()).await {
                log::error!("{}: failed to send tick event: {:#}", self.project_id, e);
            }
        }
        log::info!("{}: ticker stopped", self.project_id);

        Ok(())
    }
}
