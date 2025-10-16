use but_core::RepositoryExt;
use serde::{Deserialize, Serialize};
use std::path;

use crate::{
    default_true::DefaultTrue, ApiProject, AuthKey, CodePushState, FetchResult, ProjectId,
};

/// API-specific project type that can be enriched with computed/derived data
/// while preserving the original project structure for persistence.
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
    /// Force push protection uses safer force push flags instead of doing straight force pushes
    #[serde(default)]
    pub force_push_protection: bool,
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
    #[serde(default)]
    pub forge_override: Option<String>,
    #[serde(default)]
    pub preferred_forge_user: Option<String>,
    /// Gerrit mode enabled for this project, derived from git configuration
    #[serde(default)]
    pub gerrit_mode: bool,
}

impl From<crate::Project> for Project {
    fn from(project: crate::Project) -> Self {
        let gerrit_mode = match gix::open(&project.path) {
            Ok(repo) => repo
                .git_settings()
                .ok()
                .and_then(|s| s.gitbutler_gerrit_mode)
                .unwrap_or(false),
            Err(_) => false,
        };

        Self {
            id: project.id,
            title: project.title,
            description: project.description,
            path: project.path,
            preferred_key: project.preferred_key,
            ok_with_force_push: project.ok_with_force_push,
            force_push_protection: project.force_push_protection,
            api: project.api,
            gitbutler_data_last_fetch: project.gitbutler_data_last_fetch,
            gitbutler_code_push_state: project.gitbutler_code_push_state,
            project_data_last_fetch: project.project_data_last_fetch,
            omit_certificate_check: project.omit_certificate_check,
            snapshot_lines_threshold: project.snapshot_lines_threshold,
            forge_override: project.forge_override,
            preferred_forge_user: project.preferred_forge_user,
            gerrit_mode,
        }
    }
}

impl From<Project> for crate::Project {
    fn from(api_project: Project) -> Self {
        Self {
            id: api_project.id,
            title: api_project.title,
            description: api_project.description,
            path: api_project.path,
            preferred_key: api_project.preferred_key,
            ok_with_force_push: api_project.ok_with_force_push,
            force_push_protection: api_project.force_push_protection,
            api: api_project.api,
            gitbutler_data_last_fetch: api_project.gitbutler_data_last_fetch,
            gitbutler_code_push_state: api_project.gitbutler_code_push_state,
            project_data_last_fetch: api_project.project_data_last_fetch,
            omit_certificate_check: api_project.omit_certificate_check,
            snapshot_lines_threshold: api_project.snapshot_lines_threshold,
            forge_override: api_project.forge_override,
            preferred_forge_user: api_project.preferred_forge_user,
            // Note: gerrit_mode is not included as it's derived, not persisted
        }
    }
}
