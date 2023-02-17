use std::path::PathBuf;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

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
pub struct Project {
    pub id: String,
    pub title: String,
    pub path: String,
    pub api: Option<ApiProject>,
}

impl AsRef<Project> for Project {
    fn as_ref(&self) -> &Project {
        self
    }
}

impl Project {
    pub fn refname(&self) -> String {
        format!("refs/gitbutler-{}/current", self.id)
    }

    pub fn session_path(&self) -> PathBuf {
        PathBuf::from(&self.path)
            .join(".git")
            .join(format!("gb-{}", self.id))
            .join("session")
    }

    pub fn deltas_path(&self) -> PathBuf {
        self.session_path().join("deltas")
    }

    pub fn from_path(path: String) -> Result<Self> {
        // make sure path exists
        let path = std::path::Path::new(&path);
        if !path.exists() {
            return Err(anyhow!("path {} does not exist", path.display()));
        }

        // make sure path is a directory
        if !path.is_dir() {
            return Err(anyhow!("path {} is not a directory", path.display()));
        }

        // make sure it's a git repository
        if !path.join(".git").exists() {
            return Err(anyhow!("path {} is not a git repository", path.display()));
        };

        // title is the base name of the file
        path.into_iter()
            .last()
            .map(|p| p.to_str().unwrap().to_string())
            .map(|title| Self {
                id: uuid::Uuid::new_v4().to_string(),
                title,
                path: path.to_str().unwrap().to_string(),
                api: None,
            })
            .ok_or_else(|| anyhow!("failed to get title from path"))
    }
}
