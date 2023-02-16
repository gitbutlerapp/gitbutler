use crate::projects::project;
use crate::storage;
use anyhow::Result;
use serde::{Deserialize, Serialize};

const PROJECTS_FILE: &str = "projects.json";

pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateRequest {
    id: String,
    title: Option<String>,
    api: Option<project::ApiProject>,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn list_projects(&self) -> Result<Vec<project::Project>> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => Ok(serde_json::from_str(&projects)?),
            None => Ok(vec![]),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Option<project::Project>> {
        let projects = self.list_projects()?;
        Ok(projects.into_iter().find(|p| p.id == id))
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

        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(self.get_project(&update_request.id)?.unwrap())
    }

    pub fn add_project(&self, project: &project::Project) -> Result<()> {
        let mut projects = self.list_projects()?;
        projects.push(project.clone());
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<()> {
        let mut projects = self.list_projects()?;
        projects.retain(|p| p.id != id);
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
