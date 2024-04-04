use serde::{Deserialize, Serialize};

use crate::{
    projects::{project, ProjectId},
    storage,
};

const PROJECTS_FILE: &str = "projects.json";

#[derive(Debug, Clone)]
pub struct Storage {
    storage: storage::Storage,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct UpdateRequest {
    pub id: ProjectId,
    pub title: Option<String>,
    pub description: Option<String>,
    pub api: Option<project::ApiProject>,
    pub gitbutler_data_last_fetched: Option<project::FetchResult>,
    pub preferred_key: Option<project::AuthKey>,
    pub ok_with_force_push: Option<bool>,
    pub gitbutler_code_push_state: Option<project::CodePushState>,
    pub project_data_last_fetched: Option<project::FetchResult>,
    pub omit_certificate_check: Option<bool>,
    pub use_diff_context: Option<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Storage(#[from] storage::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("project not found")]
    NotFound,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Storage {
        Storage { storage }
    }

    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Storage {
        Storage::new(storage::Storage::new(path))
    }

    pub fn list(&self) -> Result<Vec<project::Project>, Error> {
        match self.storage.read(PROJECTS_FILE)? {
            Some(projects) => {
                let all_projects: Vec<project::Project> = serde_json::from_str(&projects)?;
                let all_projects: Vec<project::Project> = all_projects
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
                Ok(all_projects)
            }
            None => Ok(vec![]),
        }
    }

    pub fn get(&self, id: &ProjectId) -> Result<project::Project, Error> {
        let projects = self.list()?;
        for project in &projects {
            self.update(&UpdateRequest {
                id: project.id,
                preferred_key: Some(project.preferred_key.clone()),
                ..Default::default()
            })?;
        }
        match projects.into_iter().find(|p| p.id == *id) {
            Some(project) => Ok(project),
            None => Err(Error::NotFound),
        }
    }

    pub fn update(&self, update_request: &UpdateRequest) -> Result<project::Project, Error> {
        let mut projects = self.list()?;
        let project = projects
            .iter_mut()
            .find(|p| p.id == update_request.id)
            .ok_or(Error::NotFound)?;

        if let Some(title) = &update_request.title {
            project.title.clone_from(title);
        }

        if let Some(description) = &update_request.description {
            project.description = Some(description.clone());
        }

        if let Some(api) = &update_request.api {
            project.api = Some(api.clone());
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

        if let Some(omit_certificate_check) = update_request.omit_certificate_check {
            project.omit_certificate_check = Some(omit_certificate_check);
        }

        if let Some(use_diff_context) = update_request.use_diff_context {
            project.use_diff_context = Some(use_diff_context);
        }

        self.storage
            .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;

        Ok(projects
            .iter()
            .find(|p| p.id == update_request.id)
            .unwrap()
            .clone())
    }

    pub fn purge(&self, id: &ProjectId) -> Result<(), Error> {
        let mut projects = self.list()?;
        if let Some(index) = projects.iter().position(|p| p.id == *id) {
            projects.remove(index);
            self.storage
                .write(PROJECTS_FILE, &serde_json::to_string_pretty(&projects)?)?;
        }
        Ok(())
    }

    pub fn add(&self, project: &project::Project) -> Result<(), Error> {
        let mut projects = self.list()?;
        projects.push(project.clone());
        let projects = serde_json::to_string_pretty(&projects)?;
        self.storage.write(PROJECTS_FILE, &projects)?;
        Ok(())
    }
}
