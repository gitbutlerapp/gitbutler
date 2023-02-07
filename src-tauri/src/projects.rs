use crate::{crdt::Delta, fs::list_files, storage};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

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

    fn deltas_path(&self) -> Result<PathBuf, std::io::Error> {
        let path = PathBuf::from(&self.path).join(PathBuf::from(".git/gb/session/deltas"));
        std::fs::create_dir_all(path.clone())?;
        Ok(path)
    }

    pub fn save_file_deltas(
        &self,
        file_path: &Path,
        deltas: &Vec<Delta>,
    ) -> Result<(), std::io::Error> {
        if deltas.is_empty() {
            return Ok(());
        }
        let project_deltas_path = self.deltas_path()?;
        let delta_path = project_deltas_path.join(file_path.to_path_buf());
        log::info!("Writing delta to {}", delta_path.to_str().unwrap());
        let raw_deltas = serde_json::to_string(&deltas).unwrap();
        std::fs::write(delta_path, raw_deltas)?;
        Ok(())
    }

    pub fn get_file_deltas(
        &self,
        file_path: &Path,
    ) -> Result<Option<Vec<Delta>>, Box<dyn std::error::Error>> {
        let project_deltas_path = self.deltas_path()?;
        let delta_path = project_deltas_path.join(file_path.to_path_buf());
        if delta_path.exists() {
            let raw_deltas = std::fs::read_to_string(delta_path.clone())?;
            let deltas: Vec<Delta> = serde_json::from_str(&raw_deltas)?;
            Ok(Some(deltas))
        } else {
            Ok(None)
        }
    }

    pub fn list_deltas(&self) -> Result<HashMap<String, Vec<Delta>>, Box<dyn std::error::Error>> {
        let deltas_path = self.deltas_path()?;
        let file_paths = list_files(&deltas_path)?;
        let mut deltas = HashMap::new();
        for file_path in file_paths {
            let file_path = Path::new(&file_path);
            let file_deltas = self.get_file_deltas(file_path)?;
            let relative_file_path = file_path.strip_prefix(&deltas_path)?;
            if let Some(file_deltas) = file_deltas {
                deltas.insert(
                    relative_file_path.to_str().unwrap().to_string(),
                    file_deltas,
                );
            }
        }
        Ok(deltas)
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
