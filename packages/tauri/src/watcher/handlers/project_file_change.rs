use crate::projects::ProjectId;
use anyhow::Result;
use std::vec;

use super::events;

#[derive(Clone, Default)]
pub struct Handler {}

impl Handler {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &ProjectId,
    ) -> Result<Vec<events::Event>> {
        let path = path.as_ref().to_path_buf();
        Ok(vec![
            events::Event::SessionProcessing(project_id.clone(), path),
            // TODO: throttle this event to max 1 per 30ms
            events::Event::VirtualBranch(project_id.clone()),
        ])
    }
}
