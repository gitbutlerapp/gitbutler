use std::sync;

use anyhow::{Context, Result};

use crate::{app::project_repository, events, projects};

pub struct Listener {
    project_id: String,
    project_store: projects::Storage,
    events: sync::mpsc::Sender<events::Event>,
}

impl Listener {
    pub fn new(
        project_id: String,
        project_store: projects::Storage,
        events: sync::mpsc::Sender<events::Event>,
    ) -> Self {
        Self {
            project_id,
            project_store,
            events,
        }
    }

    pub fn register<P: AsRef<std::path::Path>>(&self, path: P) -> Result<()> {
        let project = self
            .project_store
            .get_project(&self.project_id)
            .with_context(|| "failed to get project")?;

        if project.is_none() {
            return Err(anyhow::anyhow!("project not found"));
        }
        let project = project.unwrap();

        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        let path = path.as_ref().to_str().unwrap();
        let event = if path.eq(".git/logs/HEAD") {
            log::info!("{}: git activity", project.id);
            Some(events::Event::git_activity(&project))
        } else if path.eq(".git/HEAD") {
            log::info!("{}: git head changed", project.id);
            let head_ref = project_repository.head()?;
            if let Some(head) = head_ref.name() {
                Some(events::Event::git_head(&project, &head))
            } else {
                None
            }
        } else if path.eq(".git/index") {
            log::info!("{}: git index changed", project.id);
            Some(events::Event::git_index(&project))
        } else {
            None
        };

        if let Some(event) = event {
            self.events
                .send(event)
                .with_context(|| "failed to send git event")?;
        }

        Ok(())
    }
}
