use anyhow::Result;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiProject {
    pub name: String,
    pub description: Option<String>,
    pub repository_id: String,
    pub git_url: String,
    pub created_at: String,
    pub updated_at: String,

    pub sync: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub path: String,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub last_fetched_ts: Option<u128>,
}

impl AsRef<Project> for Project {
    fn as_ref(&self) -> &Project {
        self
    }
}

#[derive(Error, Debug)]
pub enum CreateError {
    #[error("{0} does not exist")]
    PathNotFound(String),
    #[error("{0} is not a directory")]
    NotADirectory(String),
    #[error("{0} is not a git repository")]
    NotAGitRepository(String),
}

impl Project {
    pub fn from_path(fpath: String) -> Result<Self, CreateError> {
        // make sure path exists
        let path = std::path::Path::new(&fpath);
        if !path.exists() {
            return Err(CreateError::PathNotFound(fpath).into());
        }

        // make sure path is a directory
        if !path.is_dir() {
            return Err(CreateError::NotADirectory(fpath).into());
        }

        // make sure it's a git repository
        if !path.join(".git").exists() {
            return Err(CreateError::NotAGitRepository(fpath).into());
        };

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .into_iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .unwrap_or_else(|| id.clone());

        let project = Project {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            path: path.to_str().unwrap().to_string(),
            api: None,
            ..Default::default()
        };

        Ok(project)
    }
}
