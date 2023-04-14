use std::time;

use anyhow::Result;
use crossbeam_channel::{bounded, select, tick, Receiver, Sender};

#[derive(Debug, Clone)]
pub struct Dispatcher {
    project_id: String,
    stop: (Sender<()>, Receiver<()>),
}

impl Dispatcher {
    pub fn new(project_id: String) -> Self {
        Self {
            project_id,
            stop: bounded(1),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self, interval: time::Duration, rtx: Sender<time::SystemTime>) -> Result<()> {
        let update = tick(interval);

        log::info!("{}: ticker started", self.project_id);

        loop {
            select! {
                recv(update) -> ts => match ts {
                    Ok(_) => {
                        if let Err(e) = rtx.send(time::SystemTime::now()) {
                            log::error!("{}: failed to send tick event: {:#}", self.project_id, e);
                        }
                    },
                    Err(e) => log::error!("{}: failed to receive tick event: {:#}", self.project_id, e)
                },
                recv(self.stop.1) -> _ => {
                    break;
                }
            }
        }

        log::info!("{}: ticker stopped", self.project_id);

        Ok(())
    }
}
