use crate::projects::project;
use crate::storage;

const PROJECTS_FILE: &str = "projects.json";

pub struct Storage {
    storage: storage::Storage,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn list_projects(&self) -> Result<Vec<project::Project>, Error> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => Ok(serde_json::from_str(&projects)?),
            None => Ok(vec![]),
        }
    }

    pub fn get_project(&self, id: &str) -> Result<Option<project::Project>, Error> {
        let projects = self.list_projects()?;
        Ok(projects.into_iter().find(|p| p.id == id))
    }

    pub fn add_project(&self, project: &project::Project) -> Result<(), Error> {
        let mut projects = self.list_projects()?;
        projects.push(project.clone());
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }

    pub fn delete_project(&self, id: &str) -> Result<(), Error> {
        let mut projects = self.list_projects()?;
        projects.retain(|p| p.id != id);
        let projects = serde_json::to_string(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum Error {
    StorageError(storage::Error),
    JSONError(serde_json::Error),
}

impl From<storage::Error> for Error {
    fn from(err: storage::Error) -> Self {
        Error::StorageError(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::JSONError(err)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::StorageError(err) => write!(f, "Storage error: {}", err),
            Error::JSONError(err) => write!(f, "JSON error: {}", err),
        }
    }
}
