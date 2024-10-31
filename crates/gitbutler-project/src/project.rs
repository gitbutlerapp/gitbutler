use std::{
    path::{self, PathBuf},
    time,
};

use gitbutler_id::id::Id;
use serde::{Deserialize, Serialize};

use crate::default_true::DefaultTrue;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum AuthKey {
    GitCredentialsHelper,
    Local {
        private_key_path: path::PathBuf,
    },
    // There used to be more auth option variants that we are deprecating and replacing with this
    #[serde(other)]
    #[default]
    SystemExecutable,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ApiProject {
    pub name: String,
    pub description: Option<String>,
    pub repository_id: String,
    /// The "gitbuler data, i.e. oplog" URL
    pub git_url: String,
    /// The "project" git URL
    pub code_git_url: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Determines if the project Operations log will be synched with the GitButHub
    pub sync: bool,
    /// Determines if the project code will be synched with the GitButHub
    #[serde(default)]
    pub sync_code: bool,
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
    #[serde(with = "gitbutler_serde::oid")]
    pub id: git2::Oid,
    pub timestamp: time::SystemTime,
}

pub type ProjectId = Id<Project>;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct Project {
    pub id: ProjectId,
    pub title: String,
    pub description: Option<String>,
    /// The worktree directory of the project's repository.
    // TODO(ST): rename this to `worktree_dir` and while at it, add a `git_dir` if it's retrieved from a repo.
    //           Then find `.join(".git")` and use the `git_dir` instead.
    pub path: path::PathBuf,
    #[serde(default)]
    pub preferred_key: AuthKey,
    /// if ok_with_force_push is true, we'll not try to avoid force pushing
    /// for example, when updating base branch
    #[serde(default)]
    pub ok_with_force_push: DefaultTrue,
    pub api: Option<ApiProject>,
    #[serde(default)]
    pub gitbutler_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub gitbutler_code_push_state: Option<CodePushState>,
    #[serde(default)]
    pub project_data_last_fetch: Option<FetchResult>,
    #[serde(default)]
    pub omit_certificate_check: Option<bool>,
    // The number of changed lines that will trigger a snapshot
    pub snapshot_lines_threshold: Option<usize>,
    // Experimental flag for new hunk dependency algorithm
    #[serde(default = "default_true")]
    pub use_experimental_locking: bool,
}

// TODO: Remove after `use_experimental` has been removed.
fn default_true() -> bool {
    true
}

impl Project {
    /// Determines if the project Operations log will be synched with the GitButHub
    pub fn oplog_sync_enabled(&self) -> bool {
        let has_url = self.api.as_ref().map(|api| api.git_url.clone()).is_some();
        self.api.as_ref().map(|api| api.sync).unwrap_or_default() && has_url
    }
    /// Determines if the project code will be synched with the GitButHub
    pub fn code_sync_enabled(&self) -> bool {
        let has_code_url = self
            .api
            .as_ref()
            .and_then(|api| api.code_git_url.clone())
            .is_some();
        self.api
            .as_ref()
            .map(|api| api.sync_code)
            .unwrap_or_default()
            && has_code_url
    }

    pub fn has_code_url(&self) -> bool {
        self.api
            .as_ref()
            .map(|api| api.code_git_url.is_some())
            .unwrap_or_default()
    }

    /// Returns the path to the directory containing the `GitButler` state for this project.
    ///
    /// Normally this is `.git/gitbutler` in the project's repository.
    pub fn gb_dir(&self) -> PathBuf {
        self.path.join(".git").join("gitbutler")
    }

    pub fn snapshot_lines_threshold(&self) -> usize {
        self.snapshot_lines_threshold.unwrap_or(20)
    }

    pub fn worktree_path(&self) -> PathBuf {
        self.path.clone()
    }
}
