use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{ApiProject, AuthKey, CodePushState, FetchResult, Project, ProjectId};

const PROJECTS_FILE: &str = "projects.json";

#[derive(Debug, Clone)]
pub(crate) struct Storage {
    inner: gitbutler_storage::Storage,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct UpdateRequest {
    pub id: ProjectId,
    pub title: Option<String>,
    pub description: Option<String>,
    pub path: Option<PathBuf>,
    pub api: Option<ApiProject>,
    #[serde(default = "default_false")]
    pub unset_api: bool,
    pub gitbutler_data_last_fetched: Option<FetchResult>,
    pub preferred_key: Option<AuthKey>,
    pub ok_with_force_push: Option<bool>,
    pub force_push_protection: Option<bool>,
    pub gitbutler_code_push_state: Option<CodePushState>,
    pub project_data_last_fetched: Option<FetchResult>,
    pub omit_certificate_check: Option<bool>,
    pub use_diff_context: Option<bool>,
    pub snapshot_lines_threshold: Option<usize>,
    pub forge_override: Option<String>,
    #[serde(default = "default_false")]
    pub unset_forge_override: bool,
}

fn default_false() -> bool {
    false
}

impl Storage {
    pub fn from_path(path: impl Into<PathBuf>) -> Self {
        Storage {
            inner: gitbutler_storage::Storage::new(path),
        }
    }

    pub fn list(&self) -> Result<Vec<Project>> {
        match self.inner.read(PROJECTS_FILE)? {
            Some(projects) => {
                let all_projects: Vec<Project> = serde_json::from_str(&projects)?;
                let mut all_projects: Vec<_> = all_projects
                    .into_iter()
                    .map(|mut p| {
                        // backwards compatibility for description field
                        if let Some(api_description) =
                            p.api.as_ref().and_then(|api| api.description.as_ref())
                        {
                            p.description = Some(api_description.to_string());
                        }
                        p
                    })
                    .collect();

                all_projects.sort_by_key(|p| p.title.to_lowercase());
                Ok(all_projects)
            }
            None => Ok(vec![]),
        }
    }

    pub fn get(&self, id: ProjectId) -> Result<Project> {
        self.try_get(id)?
            .with_context(|| format!("project {id} not found"))
    }

    pub fn try_get(&self, id: ProjectId) -> Result<Option<Project>> {
        let projects = self.list()?;
        Ok(projects.into_iter().find(|p| p.id == id))
    }

    pub fn update(&self, update_request: &UpdateRequest) -> Result<Project> {
        let mut projects = self.list()?;
        let project = projects
            .iter_mut()
            .find(|p| p.id == update_request.id)
            .with_context(|| "project {id} not found for update")?;

        if let Some(title) = &update_request.title {
            project.title.clone_from(title);
        }

        if let Some(description) = &update_request.description {
            project.description = Some(description.clone());
        }

        if let Some(path) = &update_request.path {
            project.path = path.clone();
        }

        if let Some(api) = &update_request.api {
            project.api = Some(api.clone());
        }

        if update_request.unset_api {
            project.api = None;
        }

        if let Some(forge_override) = &update_request.forge_override {
            project.forge_override = Some(forge_override.clone());
        }

        if update_request.unset_forge_override {
            project.forge_override = None;
        }

        if let Some(preferred_key) = &update_request.preferred_key {
            project.preferred_key = preferred_key.clone();
        }

        if let Some(gitbutler_data_last_fetched) =
            update_request.gitbutler_data_last_fetched.as_ref()
        {
            project.gitbutler_data_last_fetch = Some(gitbutler_data_last_fetched.clone());
        }

        if let Some(project_data_last_fetched) = update_request.project_data_last_fetched.as_ref() {
            project.project_data_last_fetch = Some(project_data_last_fetched.clone());
        }

        if let Some(state) = update_request.gitbutler_code_push_state {
            project.gitbutler_code_push_state = Some(state);
        }

        if let Some(ok_with_force_push) = update_request.ok_with_force_push {
            *project.ok_with_force_push = ok_with_force_push;
        }

        if let Some(force_push_protection) = update_request.force_push_protection {
            project.force_push_protection = force_push_protection;
        }

        if let Some(omit_certificate_check) = update_request.omit_certificate_check {
            project.omit_certificate_check = Some(omit_certificate_check);
        }

        if let Some(snapshot_lines_threshold) = update_request.snapshot_lines_threshold {
            project.snapshot_lines_threshold = Some(snapshot_lines_threshold);
        }

        self.inner
            .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;

        Ok(projects
            .iter()
            .find(|p| p.id == update_request.id)
            .unwrap()
            .clone())
    }

    pub fn purge(&self, id: ProjectId) -> Result<()> {
        let mut projects = self.list()?;
        if let Some(index) = projects.iter().position(|p| p.id == id) {
            projects.remove(index);
            self.inner
                .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;
        }
        Ok(())
    }

    pub fn add(&self, project: &Project) -> Result<()> {
        let mut projects = self.list()?;
        projects.push(project.clone());
        let projects = serde_json::to_string_pretty(&projects)?;
        self.inner.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
