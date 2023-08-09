use std::{collections::HashMap, path, sync::Arc};

use anyhow::Context;
use tauri::AppHandle;
use tokio::sync::Semaphore;

use crate::{
    gb_repository, keys,
    project_repository::{self, conflicts},
    projects, users,
};

pub struct Controller {
    local_data_dir: path::PathBuf,
    semaphores: Arc<tokio::sync::Mutex<HashMap<String, Semaphore>>>,

    projects_storage: projects::Storage,
    users_storage: users::Storage,
    keys_storage: keys::Storage,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("detached head detected, go back to gitbutler/integration to continue")]
    DetachedHead,
    #[error("unexpected head {0}, go back to gitbutler/integration to continue")]
    InvalidHead(String),
    #[error("failed to open project repository")]
    PushError(#[from] project_repository::Error),
    #[error("project is in a conflicted state")]
    Conflicting,
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
            keys_storage: keys::Storage::from(value),
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

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;

            super::commit(&gb_repository, &project_repository, branch, message)
                .map_err(Error::Other)
        })
        .await
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &str,
    ) -> Result<Vec<super::VirtualBranch>, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")
                .map_err(Error::Other)?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;
            super::list_virtual_branches(&gb_repository, &project_repository).map_err(Error::Other)
        })
        .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &str,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;

            if conflicts::is_resolving(&project_repository) {
                return Err(Error::Conflicting);
            }

            super::create_virtual_branch(&gb_repository, create).map_err(Error::Other)?;
            Ok(())
        })
        .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &str,
        branch: &project_repository::branch::Name,
    ) -> Result<String, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock::<Result<String, Error>>(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;

            let branch_id = super::create_virtual_branch_from_branch(
                &gb_repository,
                &project_repository,
                branch,
                None,
            )
            .map_err(Error::Other)?;

            // also apply the branch
            super::apply_branch(&gb_repository, &project_repository, &branch_id)
                .map_err(Error::Other)?;
            Ok(branch_id)
        })
        .await
    }

    pub fn get_base_branch_data(
        &self,
        project_id: &str,
    ) -> Result<Option<super::BaseBranch>, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;
        let project_repository = project
            .as_ref()
            .try_into()
            .context("failed to open project repository")?;
        self.verify_branch(&project_repository)?;
        let gb_repository = self.open_gb_repository(project_id)?;
        super::get_base_branch_data(&gb_repository, &project_repository).map_err(Error::Other)
    }

    pub async fn set_base_branch(
        &self,
        project_id: &str,
        target_branch: &str,
    ) -> Result<super::BaseBranch, Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            let gb_repository = self.open_gb_repository(project_id)?;

            super::set_base_branch(&gb_repository, &project_repository, target_branch)
                .map_err(Error::Other)
        })
        .await
    }

    pub async fn update_base_branch(&self, project_id: &str) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;

            super::update_base_branch(&gb_repository, &project_repository).map_err(Error::Other)
        })
        .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &str,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock::<Result<(), Error>>(project_id, || {
            let gb_repository = self.open_gb_repository(project_id)?;
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;

            self.verify_branch(&project_repository)?;
            super::update_branch(&gb_repository, branch_update).map_err(Error::Other)?;
            Ok(())
        })
        .await
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock::<Result<(), Error>>(project_id, || {
            let gb_repository = self.open_gb_repository(project_id)?;
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            super::delete_branch(&gb_repository, branch_id)?;
            Ok(())
        })
        .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;
            super::apply_branch(&gb_repository, &project_repository, branch_id)
                .map_err(Error::Other)
        })
        .await
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;
            super::unapply_branch(&gb_repository, &project_repository, branch_id)
                .map_err(Error::Other)
        })
        .await
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: &str,
        branch_id: &str,
    ) -> Result<(), Error> {
        let project = self
            .projects_storage
            .get_project(project_id)
            .context("failed to get project")?
            .context("project not found")?;

        let private_key = self
            .keys_storage
            .get_or_create()
            .context("failed to get or create private key")?;

        self.with_lock(project_id, || {
            let project_repository = project
                .as_ref()
                .try_into()
                .context("failed to open project repository")?;
            self.verify_branch(&project_repository)?;
            let gb_repository = self.open_gb_repository(project_id)?;

            super::push(&project_repository, &gb_repository, branch_id, &private_key).map_err(|e| {
                match e {
                    super::PushError::Repository(e) => Error::PushError(e),
                    super::PushError::Other(e) => Error::Other(e),
                }
            })
        })
        .await
    }

    async fn with_lock<T>(&self, project_id: &str, action: impl FnOnce() -> T) -> T {
        let mut semaphores = self.semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await;
        action()
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

    fn verify_branch(
        &self,
        project_repository: &project_repository::Repository,
    ) -> Result<(), Error> {
        match project_repository
            .get_head()
            .map_err(|e| Error::Other(e))?
            .name()
        {
            None => Err(Error::DetachedHead),
            Some(super::vbranch::GITBUTLER_INTEGRATION_REFERENCE) => Ok(()),
            Some(head_name) => Err(Error::InvalidHead(head_name.to_string())),
        }
    }
}
