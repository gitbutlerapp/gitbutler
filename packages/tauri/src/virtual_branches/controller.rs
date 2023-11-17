use std::{collections::HashMap, sync::Arc};

use anyhow::Context;
use tauri::AppHandle;
use tokio::sync::Semaphore;

use crate::{
    error::Error,
    gb_repository, git, keys,
    paths::DataDir,
    project_repository,
    projects::{self, ProjectId},
    users,
};

use super::{
    branch::{BranchId, Ownership},
    errors::{self, GetBaseBranchDataError, IsRemoteBranchMergableError, ListRemoteBranchesError},
    RemoteBranchFile,
};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: DataDir,
    semaphores: Arc<tokio::sync::Mutex<HashMap<String, Semaphore>>>,

    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        DataDir::try_from(value).map(|data_dir| {
            Self::new(
                &data_dir,
                &projects::Controller::from(&data_dir),
                &users::Controller::from(&data_dir),
                &keys::Controller::from(&data_dir),
            )
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ControllerError<E>
where
    E: Into<Error>,
{
    #[error(transparent)]
    VerifyError(#[from] errors::VerifyError),
    #[error(transparent)]
    Action(E),
    #[error(transparent)]
    User(#[from] Error),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<&DataDir> for Controller {
    fn from(value: &DataDir) -> Self {
        Self::new(
            value,
            &projects::Controller::from(value),
            &users::Controller::from(value),
            &keys::Controller::from(value),
        )
    }
}

impl Controller {
    pub fn new(
        data_dir: &DataDir,
        projects: &projects::Controller,
        users: &users::Controller,
        keys: &keys::Controller,
    ) -> Self {
        Self {
            local_data_dir: data_dir.clone(),
            semaphores: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            projects: projects.clone(),
            users: users.clone(),
            keys: keys.clone(),
        }
    }

    pub async fn create_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        message: &str,
        ownership: Option<&Ownership>,
    ) -> Result<git::Oid, ControllerError<errors::CommitError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                    .then(|| {
                        self.keys
                            .get_or_create()
                            .context("failed to get private key")
                    })
                    .transpose()?;

                super::commit(
                    gb_repository,
                    project_repository,
                    branch_id,
                    message,
                    ownership,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Into::into)
            })
        })
        .await
    }

    pub fn can_apply_remote_branch(
        &self,
        project_id: &ProjectId,
        branch_name: &git::BranchName,
    ) -> Result<bool, ControllerError<IsRemoteBranchMergableError>> {
        let project = self.projects.get(project_id).map_err(Error::from)?;
        let project_repository =
            project_repository::Repository::open(&project).map_err(Error::from)?;
        let user = self.users.get_user().map_err(Error::from)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        super::is_remote_branch_mergeable(&gb_repository, &project_repository, branch_name)
            .map_err(ControllerError::Action)
    }

    pub fn can_apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<bool, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user().context("failed to get user")?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        super::is_virtual_branch_mergeable(&gb_repository, &project_repository, branch_id)
            .map_err(Into::into)
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::VirtualBranch>, ControllerError<errors::ListVirtualBranchesError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::list_virtual_branches(gb_repository, project_repository).map_err(Into::into)
            })
        })
        .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                let branch_id =
                    super::create_virtual_branch(gb_repository, project_repository, create)?.id;
                Ok(branch_id)
            })
        })
        .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::BranchName,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchFromBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let branch = super::create_virtual_branch_from_branch(
                    gb_repository,
                    project_repository,
                    branch,
                    None,
                    user,
                )?;

                let signing_key = project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                    .then(|| {
                        self.keys
                            .get_or_create()
                            .context("failed to get private key")
                    })
                    .transpose()?;

                // also apply the branch
                super::apply_branch(
                    gb_repository,
                    project_repository,
                    &branch.id,
                    signing_key.as_ref(),
                    user,
                )
                .context("failed to apply branch")?;

                Ok(branch.id)
            })
        })
        .await
    }

    pub fn get_base_branch_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<super::BaseBranch>, ControllerError<GetBaseBranchDataError>> {
        let project = self.projects.get(project_id).map_err(Error::from)?;
        let project_repository =
            project_repository::Repository::open(&project).map_err(Error::from)?;
        let user = self.users.get_user().map_err(Error::from)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        let base_branch = super::get_base_branch_data(&gb_repository, &project_repository)
            .map_err(ControllerError::Action)?;
        Ok(base_branch)
    }

    pub fn list_remote_commit_files(
        &self,
        project_id: &ProjectId,
        commit_oid: git::Oid,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;

        super::list_remote_commit_files(&project_repository.git_repository, commit_oid)
            .map_err(Into::into)
    }

    pub fn set_base_branch(
        &self,
        project_id: &ProjectId,
        target_branch: &git::RemoteBranchName,
    ) -> Result<super::BaseBranch, Error> {
        let project = self.projects.get(project_id)?;
        let user = self.users.get_user()?;
        let project_repository = project_repository::Repository::open(&project)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;

        let target = super::set_base_branch(
            &gb_repository,
            &project_repository,
            user.as_ref(),
            target_branch,
        )?;

        Ok(target)
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::MergeVirtualBranchUpstreamError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                    .then(|| {
                        self.keys
                            .get_or_create()
                            .context("failed to get private key")
                    })
                    .transpose()?;

                super::merge_virtual_branch_upstream(
                    gb_repository,
                    project_repository,
                    branch_id,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn update_base_branch(
        &self,
        project_id: &ProjectId,
    ) -> Result<(), ControllerError<errors::UpdateBaseBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                super::update_base_branch(gb_repository, project_repository, user)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), ControllerError<errors::UpdateBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::update_branch(gb_repository, project_repository, branch_update)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::DeleteBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::delete_branch(gb_repository, project_repository, branch_id)?;
                Ok(())
            })
        })
        .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::ApplyBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let signing_key = project_repository
                    .config()
                    .sign_commits()
                    .context("failed to get sign commits option")?
                    .then(|| {
                        self.keys
                            .get_or_create()
                            .context("failed to get private key")
                    })
                    .transpose()?;

                super::apply_branch(
                    gb_repository,
                    project_repository,
                    branch_id,
                    signing_key.as_ref(),
                    user,
                )
                .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &ProjectId,
        ownership: &Ownership,
    ) -> Result<(), ControllerError<errors::UnapplyOwnershipError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::unapply_ownership(gb_repository, project_repository, ownership)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        ownership: &Ownership,
    ) -> Result<git::Oid, ControllerError<errors::AmendError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::amend(gb_repository, project_repository, branch_id, ownership)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        target_commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::ResetBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::reset_branch(
                    gb_repository,
                    project_repository,
                    branch_id,
                    target_commit_oid,
                )
                .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::UnapplyBranchError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::unapply_branch(gb_repository, project_repository, branch_id)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        with_force: bool,
    ) -> Result<(), ControllerError<errors::PushError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, user| {
                let credentials = git::credentials::Factory::new(
                    project_repository.project(),
                    self.keys
                        .get_or_create()
                        .context("failed to get or create private key")?,
                    user,
                );

                super::push(
                    project_repository,
                    gb_repository,
                    branch_id,
                    with_force,
                    &credentials,
                )
                .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn flush_vbranches(
        &self,
        project_id: ProjectId,
    ) -> Result<(), ControllerError<errors::FlushAppliedVbranchesError>> {
        self.with_lock(&project_id, || {
            self.with_verify_branch(&project_id, |gb_repository, project_repository, _| {
                super::flush_applied_vbranches(gb_repository, project_repository)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub async fn cherry_pick(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<Option<git::Oid>, ControllerError<errors::CherryPickError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::cherry_pick(gb_repository, project_repository, branch_id, commit_oid)
                    .map_err(Into::into)
            })
        })
        .await
    }

    pub fn list_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::RemoteBranch>, ControllerError<ListRemoteBranchesError>> {
        let project = self.projects.get(project_id).map_err(Error::from)?;
        let project_repository =
            project_repository::Repository::open(&project).map_err(Error::from)?;
        let user = self.users.get_user().map_err(Error::from)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        super::list_remote_branches(&gb_repository, &project_repository)
            .map_err(ControllerError::Action)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::SquashError>> {
        self.with_lock(project_id, || {
            self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
                super::squash(gb_repository, project_repository, branch_id, commit_oid)
                    .map_err(Into::into)
            })
        })
        .await
    }
}

impl Controller {
    fn with_verify_branch<T, E: Into<Error>>(
        &self,
        project_id: &ProjectId,
        action: impl FnOnce(
            &gb_repository::Repository,
            &project_repository::Repository,
            Option<&users::User>,
        ) -> Result<T, E>,
    ) -> Result<T, ControllerError<E>> {
        let project = self.projects.get(project_id).map_err(Error::from)?;
        let project_repository =
            project_repository::Repository::open(&project).map_err(Error::from)?;
        let user = self.users.get_user().map_err(Error::from)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        super::integration::verify_branch(&gb_repository, &project_repository)?;
        action(&gb_repository, &project_repository, user.as_ref()).map_err(ControllerError::Action)
    }

    async fn with_lock<T>(&self, project_id: &ProjectId, action: impl FnOnce() -> T) -> T {
        let mut semaphores = self.semaphores.lock().await;
        let semaphore = semaphores
            .entry(project_id.to_string())
            .or_insert_with(|| Semaphore::new(1));
        let _permit = semaphore.acquire().await;
        action()
    }
}
