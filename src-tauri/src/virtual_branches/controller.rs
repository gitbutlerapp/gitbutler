use std::{collections::HashMap, path, sync::Arc};

use anyhow::Context;
use tauri::AppHandle;
use tokio::sync::Semaphore;

use crate::{gb_repository, projects, users};

pub struct Controller {
    local_data_dir: path::PathBuf,
    semaphores: Arc<tokio::sync::Mutex<HashMap<String, Semaphore>>>,

    projects_storage: projects::Storage,
    users_storage: users::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    LockError(#[from] tokio::sync::AcquireError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl TryFrom<&AppHandle> for Controller {
    type Error = Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        let local_data_dir = value
            .path_resolver()
            .app_local_data_dir()
            .context("Failed to get local data dir")?;
        Ok(Self {
            local_data_dir,
            semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            projects_storage: projects::Storage::from(value),
            users_storage: users::Storage::from(value),
        })
    }
}

impl Controller {
    pub async fn create_commit(
        &self,
        project_id: &str,
        branch: &str,
        message: &str,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = 
            project.as_ref()
            .try_into()
            .context("failed to open project repository")?;
        let gb_repository = self.open_gb_repository(project_id)?;

        let mut semaphores = self.semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await?;

        super::commit(&gb_repository, &project_repository, branch, message)?;

        Ok(())
    }

    fn open_gb_repository(&self, project_id: &str) -> Result<gb_repository::Repository, Error> {
        gb_repository::Repository::open(
            self.local_data_dir.clone(),
            project_id,
            self.projects_storage.clone(),
            self.users_storage.clone(),
        )
        .context("failed to open repository")
        .map_err(Error::Other)
    }
}
