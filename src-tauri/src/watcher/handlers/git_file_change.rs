use anyhow::{Context, Result};
use tauri::AppHandle;

use crate::{events as app_events, project_repository, projects};

use super::events;

#[derive(Clone)]
pub struct Handler {
    project_store: projects::Storage,
}

impl TryFrom<&AppHandle> for Handler {
    type Error = anyhow::Error;
    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(Self {
            project_store: projects::Storage::try_from(value)?,
        })
    }
}

impl Handler {
    pub fn handle<P: AsRef<std::path::Path>>(
        &self,
        path: P,
        project_id: &str,
    ) -> Result<Vec<events::Event>> {
        let project = self
            .project_store
            .get_project(project_id)
            .context("failed to get project")?;

        if project.is_none() {
            return Err(anyhow::anyhow!("project not found"));
        }
        let project = project.unwrap();

        let project_repository = project_repository::Repository::open(&project)
            .with_context(|| "failed to open project repository for project")?;

        match path.as_ref().to_str().unwrap() {
            "FETCH_HEAD" => {
                tracing::info!("{}: git fetch", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_fetch(
                    &project.id,
                ))])
            }
            "logs/HEAD" => {
                tracing::info!("{}: git activity", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_activity(
                    &project.id,
                ))])
            }
            "HEAD" => {
                tracing::info!("{}: git head changed", project.id);
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
                tracing::info!("{}: git index changed", project.id);
                Ok(vec![events::Event::Emit(app_events::Event::git_index(
                    &project.id,
                ))])
            }
            _ => Ok(vec![]),
        }
    }
}
