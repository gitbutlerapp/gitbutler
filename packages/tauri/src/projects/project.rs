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

#[derive(Debug, Deserialize, Serialize, Copy, Clone)]
pub struct CodePushState {
    pub id: git::Oid,
    pub timestamp: time::SystemTime,
}

pub type ProjectId = Id<Project>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ForcePushPreference {
    Allow,
    Deny,
}

impl Default for ForcePushPreference {
    fn default() -> Self {
        ForcePushPreference::Allow
    }
}

impl From<bool> for ForcePushPreference {
    fn from(value: bool) -> Self {
        if value {
            ForcePushPreference::Allow
        } else {
            ForcePushPreference::Deny
        }
    }
}
impl From<ForcePushPreference> for bool {
    fn from(value: ForcePushPreference) -> Self {
        match value {
            ForcePushPreference::Allow => true,
            ForcePushPreference::Deny => false,
        }
    }
}

impl serde::Serialize for ForcePushPreference {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool((*self).into())
    }
}

impl<'de> serde::Deserialize<'de> for ForcePushPreference {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = bool::deserialize(deserializer)?;
        Ok(value.into())
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub description: Option<String>,
    pub path: path::PathBuf,
    #[serde(default)]
    pub preferred_key: AuthKey,
    /// if ok_with_force_push is true, we'll not try to avoid force pushing
    /// for example, when updating base branch
    #[serde(default)]
    pub ok_with_force_push: ForcePushPreference,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub project_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_code_push_state: Option<CodePushState>,
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
