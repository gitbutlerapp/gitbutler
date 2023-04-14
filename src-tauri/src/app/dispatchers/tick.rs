use crate::projects;
use anyhow::{Result, Context};
use crossbeam_channel::{bounded, select, tick, Receiver, Sender};
use std::time;

#[derive(Debug, Clone)]
pub struct Dispatcher {
    project_id: String,
    stop: (Sender<()>, Receiver<()>),
}

impl Dispatcher {
    pub fn new(project: &projects::Project) -> Self {
        Self {
            project_id: project.id.clone(),
            stop: bounded(1),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self, interval: time::Duration, rtx: Sender<time::Instant>) -> Result<()> {
        log::info!("{}: ticker started", self.project_id);
        let update = tick(interval);

        loop {
            select! {
                recv(update) -> ts => {
                    let ts = ts.context("failed to receive tick event")?;
                    if let Err(e) = rtx.send(ts) {
                        log::error!("{}: failed to send tick event: {:#}", self.project_id, e);
                    }

                }
                recv(self.stop.1) -> _ => {
                    break;
                }
            }
        }

        log::info!("{}: ticker stopped", self.project_id);

        Ok(())
    }
}
