use but_core::RepositoryExt;
use serde::Serialize;

/// API-specific project type that can be enriched with computed/derived data
/// while preserving the original project structure for persistence.
#[derive(Debug, Serialize, Clone)]
pub struct Project {
    #[serde(flatten)]
    inner: crate::Project,
    /// Gerrit mode enabled for this project, derived from git configuration
    #[serde(default)]
    pub gerrit_mode: bool,
    /// Path to the forge review template, if set in git configuration.
    pub forge_review_template_path: Option<String>,
}

impl From<crate::Project> for Project {
    fn from(project: crate::Project) -> Self {
        let gerrit_mode = match project.open_isolated_repo() {
            Ok(repo) => repo
                .git_settings()
                .ok()
                .and_then(|s| s.gitbutler_gerrit_mode)
                .unwrap_or(false),
            Err(_) => false,
        };

        let forge_review_template_path = match project.open_isolated_repo() {
            Ok(repo) => repo
                .git_settings()
                .ok()
                .and_then(|s| s.gitbutler_forge_review_template_path)
                .map(|bstring| bstring.to_string()),
            Err(_) => None,
        };

        Self {
            inner: project,
            gerrit_mode,
            forge_review_template_path,
        }
    }
}

impl From<Project> for crate::Project {
    fn from(api_project: Project) -> Self {
        api_project.inner
    }
}
