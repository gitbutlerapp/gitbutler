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

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProjectError {
    pub message: String,
}

impl Project {
    pub fn from_path(path: String) -> Result<Self, ProjectError> {
        // make sure path exists
        let path = std::path::Path::new(&path);
        if !path.exists() {
            return Err(ProjectError {
                message: "Path does not exist".to_string(),
            });
        }

        // make sure path is a directory
        if !path.is_dir() {
            return Err(ProjectError {
                message: "Path is not a directory".to_string(),
            });
        }

        // make sure it's a git repository
        if !path.join(".git").exists() {
            return Err(ProjectError {
                message: "Path is not a git repository".to_string(),
            });
        };

        // title is the base name of the file
        path.into_iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .map(|title| Self {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                path: path.to_str().unwrap().to_string(),
            })
            .ok_or(ProjectError {
                message: "Could not get title from path".to_string(),
            })
    }
}

const PROJECTS_FILE: &str = "projects.json";

pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StorageError {
    pub message: String,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(e: std::io::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn list_projects(&self) -> Result<Vec<Project>, StorageError> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => serde_json::from_str(&projects).map_err(|e| e.into()),
            None => Ok(vec![]),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Option<Project>, StorageError> {
        let projects = self.list_projects()?;
        Ok(projects.into_iter().find(|p| p.id == id))
    }

    pub fn add_project(&self, project: &Project) -> Result<(), StorageError> {
        let mut projects = self.list_projects()?;
        projects.push(project.clone());
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<(), StorageError> {
        let mut projects = self.list_projects()?;
        projects.retain(|p| p.id != id);
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
