use std::path::PathBuf;

use crate::storage;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub path: String,
}

impl AsRef<Project> for Project {
    fn as_ref(&self) -> &Project {
        self
    }
}

impl Project {
    pub fn from_path(path: String) -> Result<Self, String> {
        // make sure path exists
        let path = std::path::Path::new(&path);
        if !path.exists() {
            return Err("Path does not exist".to_string());
        }
        // make sure path is a directory
        if !path.is_dir() {
            return Err("Path is not a directory".to_string());
        }
        // title is the base name of the file
        path.into_iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .map(|title| Self {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                path: path.to_str().unwrap().to_string(),
            })
            .ok_or("Unable to get title".to_string())
    }
}

const PROJECTS_FILE: &str = "projects.json";

pub struct Storage {
    storage: storage::Storage,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, String> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => serde_json::from_str(&projects).map_err(|e| e.to_string()),
            None => Ok(vec![]),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>, String> {
        let projects = self.list_projects()?;
        Ok(projects.into_iter().find(|p| p.id == id))
    }

    pub fn add_project(&self, project: &Project) -> Result<(), String> {
        let mut projects = self.list_projects()?;
        projects.push(project.clone());
        let projects = serde_json::to_string(&projects).map_err(|e| e.to_string())?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<(), String> {
        let mut projects = self.list_projects()?;
        projects.retain(|p| p.id != id);
        let projects = serde_json::to_string(&projects).map_err(|e| e.to_string())?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
