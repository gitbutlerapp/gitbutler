use anyhow::{Context, Result};

use crate::{events as app_events, project_repository, projects};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_id: String,
    project_store: projects::Storage,
}

impl Handler {
    pub fn new(project_id: String, project_store: projects::Storage) -> Self {
        Self {
            project_id,
            project_store,
        }
    }

    pub fn handle<P: AsRef<std::path::Path>>(&self, path: P) -> Result<Vec<events::Event>> {
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

        match path.as_ref().to_str().unwrap() {
            "FETCH_HEAD" => {
                log::info!("{}: git fetch", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_fetch(
                    &project.id,
                ))])
            }
            "logs/HEAD" => {
                log::info!("{}: git activity", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_activity(
                    &project.id,
                ))])
            }
            "HEAD" => {
                log::info!("{}: git head changed", project.id);
                let head_ref = project_repository.get_head()?;
                if let Some(head) = head_ref.name() {
                    Ok(vec![events::Event::Emit(app_events::Event::git_head(
                        &project.id,
                        head,
                    ))])
                } else {
                    Ok(vec![])
                }
            }
            "index" => {
                log::info!("{}: git index changed", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_index(
                    &project.id,
                ))])
            }
            _ => Ok(vec![]),
        }
    }
}
