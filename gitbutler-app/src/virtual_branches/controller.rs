use std::{collections::HashMap, path, sync::Arc};

use anyhow::Context;
use tauri::{AppHandle, Manager};
use tokio::sync::Semaphore;

use crate::{
    error::Error,
    gb_repository,
    git::{self, diff::SkippedFile},
    keys, project_repository,
    projects::{self, ProjectId},
    users,
};

use super::{
    branch::{BranchId, Ownership},
    errors::{
        self, FetchFromTargetError, GetBaseBranchDataError, GetRemoteBranchDataError,
        IsRemoteBranchMergableError, ListRemoteBranchesError,
    },
    target_to_base_branch, BaseBranch, RemoteBranchFile,
};

#[derive(Clone)]
pub struct Controller {
    local_data_dir: path::PathBuf,
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
    helper: git::credentials::Helper,

    by_project_id: Arc<tokio::sync::Mutex<HashMap<ProjectId, ControllerInner>>>,
}

impl TryFrom<&AppHandle> for Controller {
    type Error = anyhow::Error;

    fn try_from(value: &AppHandle) -> Result<Self, Self::Error> {
        Ok(value.state::<Controller>().inner().clone())
    }
}

impl Controller {
    pub fn new(
        local_data_dir: path::PathBuf,
        projects: projects::Controller,
        users: users::Controller,
        keys: keys::Controller,
        helper: git::credentials::Helper,
    ) -> Self {
        Self {
            by_project_id: Arc::new(tokio::sync::Mutex::new(HashMap::new())),

            local_data_dir,
            projects,
            users,
            keys,
            helper,
        }
    }

    async fn inner(&self, project_id: &ProjectId) -> ControllerInner {
        self.by_project_id
            .lock()
            .await
            .entry(*project_id)
            .or_insert_with(|| {
                ControllerInner::new(
                    &self.local_data_dir,
                    &self.projects,
                    &self.users,
                    &self.keys,
                    &self.helper,
                )
            })
            .clone()
    }

    pub async fn create_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        message: &str,
        ownership: Option<&Ownership>,
        run_hooks: bool,
    ) -> Result<git::Oid, ControllerError<errors::CommitError>> {
        self.inner(project_id)
            .await
            .create_commit(project_id, branch_id, message, ownership, run_hooks)
            .await
    }

    pub async fn can_apply_remote_branch(
        &self,
        project_id: &ProjectId,
        branch_name: &git::RemoteRefname,
    ) -> Result<bool, ControllerError<IsRemoteBranchMergableError>> {
        self.inner(project_id)
            .await
            .can_apply_remote_branch(project_id, branch_name)
    }

    pub async fn can_apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<bool, Error> {
        self.inner(project_id)
            .await
            .can_apply_virtual_branch(project_id, branch_id)
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<
        (Vec<super::VirtualBranch>, bool, Vec<SkippedFile>),
        ControllerError<errors::ListVirtualBranchesError>,
    > {
        self.inner(project_id)
            .await
            .list_virtual_branches(project_id)
            .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchError>> {
        self.inner(project_id)
            .await
            .create_virtual_branch(project_id, create)
            .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchFromBranchError>> {
        self.inner(project_id)
            .await
            .create_virtual_branch_from_branch(project_id, branch)
            .await
    }

    pub async fn get_base_branch_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<super::BaseBranch>, ControllerError<GetBaseBranchDataError>> {
        self.inner(project_id)
            .await
            .get_base_branch_data(project_id)
    }

    pub async fn list_remote_commit_files(
        &self,
        project_id: &ProjectId,
        commit_oid: git::Oid,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        self.inner(project_id)
            .await
            .list_remote_commit_files(project_id, commit_oid)
    }

    pub async fn set_base_branch(
        &self,
        project_id: &ProjectId,
        target_branch: &git::RemoteRefname,
    ) -> Result<super::BaseBranch, Error> {
        self.inner(project_id)
            .await
            .set_base_branch(project_id, target_branch)
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::MergeVirtualBranchUpstreamError>> {
        self.inner(project_id)
            .await
            .merge_virtual_branch_upstream(project_id, branch_id)
            .await
    }

    pub async fn update_base_branch(
        &self,
        project_id: &ProjectId,
    ) -> Result<(), ControllerError<errors::UpdateBaseBranchError>> {
        self.inner(project_id)
            .await
            .update_base_branch(project_id)
            .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), ControllerError<errors::UpdateBranchError>> {
        self.inner(project_id)
            .await
            .update_virtual_branch(project_id, branch_update)
            .await
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::DeleteBranchError>> {
        self.inner(project_id)
            .await
            .delete_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::ApplyBranchError>> {
        self.inner(project_id)
            .await
            .apply_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &ProjectId,
        ownership: &Ownership,
    ) -> Result<(), ControllerError<errors::UnapplyOwnershipError>> {
        self.inner(project_id)
            .await
            .unapply_ownership(project_id, ownership)
            .await
    }

    pub async fn reset_files(
        &self,
        project_id: &ProjectId,
        files: &Vec<String>,
    ) -> Result<(), ControllerError<errors::UnapplyOwnershipError>> {
        self.inner(project_id)
            .await
            .reset_files(project_id, files)
            .await
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        ownership: &Ownership,
    ) -> Result<git::Oid, ControllerError<errors::AmendError>> {
        self.inner(project_id)
            .await
            .amend(project_id, branch_id, ownership)
            .await
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        target_commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::ResetBranchError>> {
        self.inner(project_id)
            .await
            .reset_virtual_branch(project_id, branch_id, target_commit_oid)
            .await
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::UnapplyBranchError>> {
        self.inner(project_id)
            .await
            .unapply_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        with_force: bool,
    ) -> Result<(), ControllerError<errors::PushError>> {
        self.inner(project_id)
            .await
            .push_virtual_branch(project_id, branch_id, with_force)
            .await
    }

    pub async fn cherry_pick(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<Option<git::Oid>, ControllerError<errors::CherryPickError>> {
        self.inner(project_id)
            .await
            .cherry_pick(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn list_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::RemoteBranch>, ControllerError<ListRemoteBranchesError>> {
        self.inner(project_id)
            .await
            .list_remote_branches(project_id)
    }

    pub async fn get_remote_branch_data(
        &self,
        project_id: &ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData, ControllerError<GetRemoteBranchDataError>> {
        self.inner(project_id)
            .await
            .get_remote_branch_data(project_id, refname)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::SquashError>> {
        self.inner(project_id)
            .await
            .squash(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn update_commit_message(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        message: &str,
    ) -> Result<(), ControllerError<errors::UpdateCommitMessageError>> {
        self.inner(project_id)
            .await
            .update_commit_message(project_id, branch_id, commit_oid, message)
            .await
    }

    pub async fn fetch_from_target(
        &self,
        project_id: &ProjectId,
    ) -> Result<BaseBranch, ControllerError<errors::FetchFromTargetError>> {
        self.inner(project_id)
            .await
            .fetch_from_target(project_id)
            .await
    }

    pub async fn move_commit(
        &self,
        project_id: &ProjectId,
        target_branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::MoveCommitError>> {
        self.inner(project_id)
            .await
            .move_commit(project_id, target_branch_id, commit_oid)
            .await
    }
}

#[derive(Clone)]
struct ControllerInner {
    local_data_dir: path::PathBuf,
    semaphore: Arc<Semaphore>,

    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
    helper: git::credentials::Helper,
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

impl ControllerInner {
    pub fn new(
        data_dir: &path::Path,
        projects: &projects::Controller,
        users: &users::Controller,
        keys: &keys::Controller,
        helper: &git::credentials::Helper,
    ) -> Self {
        Self {
            local_data_dir: data_dir.to_path_buf(),
            semaphore: Arc::new(Semaphore::new(1)),
            projects: projects.clone(),
            users: users.clone(),
            keys: keys.clone(),
            helper: helper.clone(),
        }
    }

    pub async fn create_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        message: &str,
        ownership: Option<&Ownership>,
        run_hooks: bool,
    ) -> Result<git::Oid, ControllerError<errors::CommitError>> {
        let _permit = self.semaphore.acquire().await;

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
                run_hooks,
            )
            .map_err(Into::into)
        })
    }

    pub fn can_apply_remote_branch(
        &self,
        project_id: &ProjectId,
        branch_name: &git::RemoteRefname,
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
    ) -> Result<
        (Vec<super::VirtualBranch>, bool, Vec<SkippedFile>),
        ControllerError<errors::ListVirtualBranchesError>,
    > {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::list_virtual_branches(gb_repository, project_repository).map_err(Into::into)
        })
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            let branch_id =
                super::create_virtual_branch(gb_repository, project_repository, create)?.id;
            Ok(branch_id)
        })
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId, ControllerError<errors::CreateVirtualBranchFromBranchError>> {
        let _permit = self.semaphore.acquire().await;

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

            super::create_virtual_branch_from_branch(
                gb_repository,
                project_repository,
                branch,
                signing_key.as_ref(),
                user,
            )
        })
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
        let use_context = project_repository
            .project()
            .use_diff_context
            .unwrap_or(false);
        let context_lines = if use_context { 3_u32 } else { 0_u32 };
        super::list_remote_commit_files(
            &project_repository.git_repository,
            commit_oid,
            context_lines,
        )
        .map_err(Into::into)
    }

    pub fn set_base_branch(
        &self,
        project_id: &ProjectId,
        target_branch: &git::RemoteRefname,
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

        super::set_base_branch(&gb_repository, &project_repository, target_branch)
            .map_err(Into::into)
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::MergeVirtualBranchUpstreamError>> {
        let _permit = self.semaphore.acquire().await;

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
    }

    pub async fn update_base_branch(
        &self,
        project_id: &ProjectId,
    ) -> Result<(), ControllerError<errors::UpdateBaseBranchError>> {
        let _permit = self.semaphore.acquire().await;

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

            super::update_base_branch(
                gb_repository,
                project_repository,
                user,
                signing_key.as_ref(),
            )
            .map_err(Into::into)
        })
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), ControllerError<errors::UpdateBranchError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::update_branch(gb_repository, project_repository, branch_update)?;
            Ok(())
        })
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::DeleteBranchError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::delete_branch(gb_repository, project_repository, branch_id)?;
            Ok(())
        })
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::ApplyBranchError>> {
        let _permit = self.semaphore.acquire().await;

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
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &ProjectId,
        ownership: &Ownership,
    ) -> Result<(), ControllerError<errors::UnapplyOwnershipError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::unapply_ownership(gb_repository, project_repository, ownership)
                .map_err(Into::into)
        })
    }

    pub async fn reset_files(
        &self,
        project_id: &ProjectId,
        ownership: &Vec<String>,
    ) -> Result<(), ControllerError<errors::UnapplyOwnershipError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |_, project_repository, _| {
            super::reset_files(project_repository, ownership).map_err(Into::into)
        })
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        ownership: &Ownership,
    ) -> Result<git::Oid, ControllerError<errors::AmendError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::amend(gb_repository, project_repository, branch_id, ownership)
                .map_err(Into::into)
        })
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        target_commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::ResetBranchError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::reset_branch(
                gb_repository,
                project_repository,
                branch_id,
                target_commit_oid,
            )
            .map_err(Into::into)
        })
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), ControllerError<errors::UnapplyBranchError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::unapply_branch(gb_repository, project_repository, branch_id)
                .map(|_| ())
                .map_err(Into::into)
        })
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        with_force: bool,
    ) -> Result<(), ControllerError<errors::PushError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::push(
                project_repository,
                gb_repository,
                branch_id,
                with_force,
                &self.helper,
            )
            .map_err(Into::into)
        })
    }

    pub async fn cherry_pick(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<Option<git::Oid>, ControllerError<errors::CherryPickError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::cherry_pick(gb_repository, project_repository, branch_id, commit_oid)
                .map_err(Into::into)
        })
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

    pub fn get_remote_branch_data(
        &self,
        project_id: &ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData, ControllerError<GetRemoteBranchDataError>> {
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
        super::get_branch_data(&gb_repository, &project_repository, refname)
            .map_err(ControllerError::Action)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::SquashError>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::squash(gb_repository, project_repository, branch_id, commit_oid)
                .map_err(Into::into)
        })
    }

    pub async fn update_commit_message(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        message: &str,
    ) -> Result<(), ControllerError<errors::UpdateCommitMessageError>> {
        let _permit = self.semaphore.acquire().await;
        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::update_commit_message(
                gb_repository,
                project_repository,
                branch_id,
                commit_oid,
                message,
            )
            .map_err(Into::into)
        })
    }

    pub async fn fetch_from_target(
        &self,
        project_id: &ProjectId,
    ) -> Result<BaseBranch, ControllerError<errors::FetchFromTargetError>> {
        let project = self.projects.get(project_id).map_err(Error::from)?;
        let mut project_repository =
            project_repository::Repository::open(&project).map_err(Error::from)?;
        let user = self.users.get_user().map_err(Error::from)?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;

        let default_target = gb_repository
            .default_target()
            .context("failed to get default target")?
            .ok_or(FetchFromTargetError::DefaultTargetNotSet(
                errors::DefaultTargetNotSetError {
                    project_id: *project_id,
                },
            ))
            .map_err(ControllerError::Action)?;

        let project_data_last_fetched = match project_repository
            .fetch(default_target.branch.remote(), &self.helper)
            .map_err(errors::FetchFromTargetError::Remote)
        {
            Ok(()) => projects::FetchResult::Fetched {
                timestamp: std::time::SystemTime::now(),
            },
            Err(error) => projects::FetchResult::Error {
                timestamp: std::time::SystemTime::now(),
                error: error.to_string(),
            },
        };

        let updated_project = self
            .projects
            .update(&projects::UpdateRequest {
                id: *project_id,
                project_data_last_fetched: Some(project_data_last_fetched),
                ..Default::default()
            })
            .await
            .context("failed to update project")?;

        project_repository.set_project(&updated_project);

        let base_branch = target_to_base_branch(&project_repository, &default_target)
            .context("failed to convert target to base branch")?;

        Ok(base_branch)
    }

    pub async fn move_commit(
        &self,
        project_id: &ProjectId,
        target_branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), ControllerError<errors::MoveCommitError>> {
        let _permit = self.semaphore.acquire().await;

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
            super::move_commit(
                gb_repository,
                project_repository,
                target_branch_id,
                commit_oid,
                user,
                signing_key.as_ref(),
            )
            .map_err(Into::into)
        })
    }
}

impl ControllerInner {
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
}
