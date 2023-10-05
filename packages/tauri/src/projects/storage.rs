use std::path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{projects::project, storage};

const PROJECTS_FILE: &str = "projects.json";

#[derive(Debug, Clone)]
pub struct Storage {
    storage: storage::Storage,
}

impl From<&storage::Storage> for Storage {
    fn from(storage: &storage::Storage) -> Self {
        Storage {
            storage: storage.clone(),
        }
    }
}

impl From<&path::PathBuf> for Storage {
    fn from(value: &path::PathBuf) -> Self {
        Self::from(&storage::Storage::from(value))
    }
}

impl From<&AppHandle> for Storage {
    fn from(value: &AppHandle) -> Self {
        Self::from(value.state::<storage::Storage>().inner())
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateRequest {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub api: Option<project::ApiProject>,
    pub project_data_last_fetched: Option<project::FetchResult>,
    pub gitbutler_data_last_fetched: Option<project::FetchResult>,
    pub preferred_key: Option<project::AuthKey>,
}

impl Storage {
    pub fn list(&self) -> Result<Vec<project::Project>> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => {
                let all_projects: Vec<project::Project> = serde_json::from_str(&projects)
                    .with_context(|| format!("Failed to parse projects from {}", PROJECTS_FILE))?;
                let all_projects: Vec<project::Project> = all_projects
                    .into_iter()
                    .map(|mut p| {
                        // backwards compatibility for description field
                        if let Some(api_description) =
                            p.api.as_ref().and_then(|api| api.description.as_ref())
                        {
                            p.description = Some(api_description.to_string());
                        }
                        p
                    })
                    .collect();
                Ok(all_projects)
            }
            None => Ok(vec![]),
        }
    }

    pub fn get(&self, id: &str) -> Result<project::Project> {
        let projects = self.list()?;
        match projects.into_iter().find(|p| p.id == id) {
            Some(project) => Ok(project),
            None => Err(anyhow::anyhow!("{}: project not found", id)),
        }
    }

    pub fn update(&self, update_request: &UpdateRequest) -> Result<project::Project> {
        let mut projects = self.list()?;
        let project = projects
            .iter_mut()
            .find(|p| p.id == update_request.id)
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

        if let Some(title) = &update_request.title {
            project.title = title.clone();
        }

        if let Some(description) = &update_request.description {
            project.description = Some(description.clone());
        }

        if let Some(api) = &update_request.api {
            project.api = Some(api.clone());
        }

        if let Some(preferred_key) = &update_request.preferred_key {
            project.preferred_key = preferred_key.clone();
        }

        if let Some(project_data_last_fetched) = update_request.project_data_last_fetched.as_ref() {
            project.project_data_last_fetched = Some(project_data_last_fetched.clone());
        }

        if let Some(gitbutler_data_last_fetched) =
            update_request.gitbutler_data_last_fetched.as_ref()
        {
            project.gitbutler_data_last_fetched = Some(gitbutler_data_last_fetched.clone());
        }

        self.storage
            .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;

        Ok(projects
            .iter()
            .find(|p| p.id == update_request.id)
            .unwrap()
            .clone())
    }

    pub fn purge(&self, id: &str) -> Result<()> {
        let mut projects = self.list()?;
        if let Some(index) = projects.iter().position(|p| p.id == id) {
            projects.remove(index);
            self.storage
                .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;
        }
        Ok(())
    }

    pub fn add(&self, project: &project::Project) -> Result<()> {
        let mut projects = self.list()?;
        projects.push(project.clone());
        let projects = serde_json::to_string_pretty(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
