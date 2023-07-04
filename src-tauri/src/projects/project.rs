use std::time;

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

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FetchResult {
    Fetched {
        timestamp_ms: u128,
    },
    Error {
        timestamp_ms: u128,
        error: String,
        attempt: u32,
    },
}

const TEN_MINUTES: time::Duration = time::Duration::new(10 * 60, 0);

impl FetchResult {
    pub fn should_fetch(&self, now: &time::SystemTime) -> Result<bool> {
        match self {
            FetchResult::Error {
                timestamp_ms,
                attempt,
                ..
            } => {
                // if last fetch errored, wait 10 seconds * 2^attempt, up to 10 minutes
                let last_fetch = time::UNIX_EPOCH
                    + time::Duration::from_millis(TryInto::<u64>::try_into(*timestamp_ms)?);
                Ok(
                    last_fetch + TEN_MINUTES.min(time::Duration::new(10 * 2u64.pow(*attempt), 0))
                        < *now,
                )
            }
            FetchResult::Fetched { timestamp_ms } => {
                // if last fetch was successful, wait 10 minutes
                let last_fetch = time::UNIX_EPOCH
                    + time::Duration::from_millis(TryInto::<u64>::try_into(*timestamp_ms)?);
                Ok(last_fetch + TEN_MINUTES < *now)
            }
        }
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub path: String,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub project_data_last_fetched: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_data_last_fetched: Option<FetchResult>,
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
            return Err(CreateError::PathNotFound(fpath));
        }

        // make sure path is a directory
        if !path.is_dir() {
            return Err(CreateError::NotADirectory(fpath));
        }

        // make sure it's a git repository
        if !path.join(".git").exists() {
            return Err(CreateError::NotAGitRepository(fpath));
        };

        let id = uuid::Uuid::new_v4().to_string();

        // title is the base name of the file
        let title = path
            .iter()
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

impl TryFrom<&git2::Repository> for Project {
    type Error = CreateError;

    fn try_from(repository: &git2::Repository) -> std::result::Result<Self, Self::Error> {
        Project::from_path(
            repository
                .path()
                .parent()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
        )
    }
}
