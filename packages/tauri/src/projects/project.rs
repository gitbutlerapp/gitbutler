use std::{path, time};

use serde::{Deserialize, Serialize};

use crate::{git, id::Id};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthKey {
    #[default]
    Generated,
    Local {
        private_key_path: path::PathBuf,
        passphrase: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiProject {
    pub name: String,
    pub description: Option<String>,
    pub repository_id: String,
    pub git_url: String,
    pub code_git_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub sync: bool,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub enum FetchResult {
    Fetched {
        timestamp: time::SystemTime,
    },
    Error {
        timestamp: time::SystemTime,
        error: String,
    },
}

impl FetchResult {
    pub fn timestamp(&self) -> &time::SystemTime {
        match self {
            FetchResult::Fetched { timestamp } | FetchResult::Error { timestamp, .. } => timestamp,
        }
    }
}

pub type ProjectId = Id<Project>;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub description: Option<String>,
    pub path: path::PathBuf,
    #[serde(default)]
    pub preferred_key: AuthKey,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub project_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_code_push: Option<git::Oid>,
}

impl AsRef<Project> for Project {
    fn as_ref(&self) -> &Project {
        self
    }
}

impl Project {
    pub fn is_sync_enabled(&self) -> bool {
        self.api.as_ref().map(|api| api.sync).unwrap_or_default()
    }

    pub fn has_code_url(&self) -> bool {
        self.api
            .as_ref()
            .map(|api| api.code_git_url.is_some())
            .unwrap_or_default()
    }
}
