use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};

use crate::app::{deltas, files, sessions, watcher::events};

#[derive(Clone)]
pub struct Dispatcher {
    project_id: String,
    sessions_database: sessions::Database,
    deltas_database: deltas::Database,
    files_database: files::Database,
    stop: (Sender<()>, Receiver<()>),
}

impl Dispatcher {
    pub fn new(
        project_id: String,
        sessions_database: sessions::Database,
        deltas_database: deltas::Database,
        files_database: files::Database,
    ) -> Self {
        Self {
            project_id,
            sessions_database,
            deltas_database,
            files_database,
            stop: bounded(1),
        }
    }

    pub fn stop(&self) -> Result<()> {
        self.stop.0.send(())?;
        Ok(())
    }

    pub fn start(&self, rtx: crossbeam_channel::Sender<events::Event>) -> Result<()> {
        log::info!("{}: database listener started", self.project_id);

        let project_id = self.project_id.clone();
        let boxed_rtx = Box::new(rtx.clone());
        self.sessions_database.on(move |session| {
            if let Err(err) = boxed_rtx.send(events::Event::Session(session)) {
                log::error!("{}: failed to send db session event: {:#}", project_id, err);
            }
        })?;

        let project_id = self.project_id.clone();
        let boxed_rtx = Box::new(rtx.clone());
        self.deltas_database
            .on(move |session_id, file_path, delta| {
                if let Err(err) = boxed_rtx.send(events::Event::Deltas((
                    session_id.to_string(),
                    file_path.into(),
                    vec![delta],
                ))) {
                    log::error!("{}: failed to send db delta event: {:#}", project_id, err);
                }
            })?;

        let project_id = self.project_id.clone();
        let boxed_rtx = Box::new(rtx.clone());
        self.files_database
            .on(move |session_id, file_path, contents| {
                if let Err(err) = boxed_rtx.send(events::Event::File((
                    session_id.to_string(),
                    file_path.into(),
                    contents.to_string(),
                ))) {
                    log::error!("{}: failed to send db file event: {:#}", project_id, err);
                }
            })?;

        self.stop
            .1
            .recv()
            .context("Failed to receive stop signal")?;

        log::info!("{}: database listener stopped", self.project_id);

        Ok(())
    }
}
