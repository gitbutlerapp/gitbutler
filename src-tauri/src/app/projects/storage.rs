use crate::projects::project;
use crate::storage;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

const PROJECTS_FILE: &str = "projects.json";

#[derive(Debug, Clone)]
pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateRequest {
    pub id: String,
    pub title: Option<String>,
    pub api: Option<project::ApiProject>,
    pub last_fetched_ts: Option<u128>,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn list_projects(&self) -> Result<Vec<project::Project>> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => {
                let all_projects: Vec<project::Project> = serde_json::from_str(&projects)
                    .with_context(|| format!("Failed to parse projects from {}", PROJECTS_FILE))?;
                Ok(all_projects)
            }
            None => Ok(vec![]),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Option<project::Project>> {
        let projects = self.list_projects()?;
        let project = projects.into_iter().find(|p| p.id == id);
        Ok(project)
    }

    pub fn update_project(&self, update_request: &UpdateRequest) -> Result<project::Project> {
        let mut projects = self.list_projects()?;
        let project = projects
            .iter_mut()
            .find(|p| p.id == update_request.id)
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

        if let Some(title) = &update_request.title {
            project.title = title.clone();
        }

        if let Some(api) = &update_request.api {
            project.api = Some(api.clone());
        }

        project.last_fetched_ts = update_request.last_fetched_ts;

        self.storage
            .write(PROJECTS_FILE, &serde_json::to_string(&projects)?)?;

        Ok(projects
            .iter()
            .find(|p| p.id == update_request.id)
            .unwrap()
            .clone())
    }

    pub fn purge(&self, id: &str) -> Result<()> {
        let mut projects = self.list_projects()?;
        if let Some(index) = projects.iter().position(|p| p.id == id) {
            projects.remove(index);
            self.storage
                .write(PROJECTS_FILE, &serde_json::to_string(&projects)?)?;
        }
        Ok(())
    }

    pub fn add_project(&self, project: &project::Project) -> Result<()> {
        let mut projects = self.list_projects()?;
        projects.push(project.clone());
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
