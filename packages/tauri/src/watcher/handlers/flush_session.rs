use anyhow::Result;
use tauri::{AppHandle, Manager};

use crate::{projects::ProjectId, sessions};

use super::events;

#[derive(Clone)]
pub struct Handler {
    session_controller: sessions::Controller,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            session_controller: value.state::<sessions::Controller>().inner().clone(),
        })
    }
}

impl Handler {
    pub fn handle(
        &self,
        project_id: &ProjectId,
        session: &sessions::Session,
    ) -> Result<Vec<events::Event>> {
        let session = self.session_controller.flush(project_id, session)?;

        Ok(vec![
            events::Event::Session(*project_id, session),
            events::Event::PushGitbutlerData(*project_id),
            events::Event::PushProjectToGitbutler(*project_id),
        ])
    }
}
