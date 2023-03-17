use super::repository;
use crate::{projects, users};
use anyhow::{Context, Result};
use std::collections::HashMap;

pub struct Store {
    by_project_id: HashMap<String, repository::Repository>,
    projects_storage: projects::Storage,
    users_storage: users::Storage,
}

impl Store {
    pub fn new(projects_storage: projects::Storage, users_storage: users::Storage) -> Store {
        Store {
            by_project_id: HashMap::new(),
            users_storage,
            projects_storage,
        }
    }

    pub fn get(&mut self, project_id: &str) -> Result<repository::Repository> {
        if let Some(repository) = self.by_project_id.get(project_id) {
            return Ok(repository.clone());
        }

        let repository = self.open(project_id)?;
        self.by_project_id
            .insert(project_id.to_string(), repository.clone());

        return Ok(repository);
    }

    fn open(&self, project_id: &str) -> Result<repository::Repository> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .with_context(|| "failed to get project")?
            .ok_or_else(|| anyhow::anyhow!("project {} not found", project_id))?;

        let user = self
            .users_storage
            .get()
            .with_context(|| "failed to get user for project")?;

        let repository = repository::Repository::new(project, user)?;

        Ok(repository)
    }
}
