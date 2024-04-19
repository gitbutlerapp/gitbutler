use crate::error::Error;
use std::{collections::HashMap, path, sync::Arc};

use anyhow::Context;
use tokio::{sync::Semaphore, task::JoinHandle};

use super::{
    branch::{BranchId, BranchOwnershipClaims},
    errors::{self, FetchFromTargetError},
    target_to_base_branch, BaseBranch, RemoteBranchFile,
};
use crate::{
    askpass::AskpassBroker,
    gb_repository, git, keys, project_repository,
    projects::{self, ProjectId},
    users,
    virtual_branches::state::{VirtualBranches, VirtualBranchesHandle},
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
        ownership: Option<&BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<git::Oid, Error> {
        self.inner(project_id)
            .await
            .create_commit(project_id, branch_id, message, ownership, run_hooks)
            .await
    }

    pub async fn can_apply_remote_branch(
        &self,
        project_id: &ProjectId,
        branch_name: &git::RemoteRefname,
    ) -> Result<bool, Error> {
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

    /// Retrieves the virtual branches state from the gitbutler repository (legacy state) and persists it into a flat TOML file
    pub async fn save_vbranches_state(
        &self,
        project_id: &ProjectId,
        branch_ids: Vec<BranchId>,
    ) -> Result<(), Error> {
        let vbranches_state = self
            .inner(project_id)
            .await
            .get_vbranches_state(project_id, branch_ids)?;
        let project = self.projects.get(project_id)?;
        // TODO: this should be constructed somewhere else
        let state_handle = VirtualBranchesHandle::new(project.path.join(".git").as_path());
        if let Some(default_target) = vbranches_state.default_target {
            state_handle.set_default_target(default_target)?;
        }
        for (id, target) in vbranches_state.branch_targets {
            state_handle.set_branch_target(id, target)?;
        }
        for (_, branch) in vbranches_state.branches {
            state_handle.set_branch(branch)?;
        }
        Ok(())
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<(Vec<super::VirtualBranch>, Vec<git::diff::FileDiff>), Error> {
        self.inner(project_id)
            .await
            .list_virtual_branches(project_id)
            .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        self.inner(project_id)
            .await
            .create_virtual_branch(project_id, create)
            .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId, Error> {
        self.inner(project_id)
            .await
            .create_virtual_branch_from_branch(project_id, branch)
            .await
    }

    pub async fn get_base_branch_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
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
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .merge_virtual_branch_upstream(project_id, branch_id)
            .await
    }

    pub async fn update_base_branch(&self, project_id: &ProjectId) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .update_base_branch(project_id)
            .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .update_virtual_branch(project_id, branch_update)
            .await
    }
    pub async fn delete_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .delete_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .apply_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &ProjectId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .unapply_ownership(project_id, ownership)
            .await
    }

    pub async fn reset_files(
        &self,
        project_id: &ProjectId,
        files: &Vec<String>,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .reset_files(project_id, files)
            .await
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
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
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .reset_virtual_branch(project_id, branch_id, target_commit_oid)
            .await
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
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
        askpass: Option<(AskpassBroker, Option<BranchId>)>,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .push_virtual_branch(project_id, branch_id, with_force, askpass)
            .await
    }

    pub async fn cherry_pick(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<Option<git::Oid>, Error> {
        self.inner(project_id)
            .await
            .cherry_pick(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn list_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::RemoteBranch>, Error> {
        self.inner(project_id)
            .await
            .list_remote_branches(project_id)
    }

    pub async fn get_remote_branch_data(
        &self,
        project_id: &ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData, Error> {
        self.inner(project_id)
            .await
            .get_remote_branch_data(project_id, refname)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .update_commit_message(project_id, branch_id, commit_oid, message)
            .await
    }

    pub async fn fetch_from_target(
        &self,
        project_id: &ProjectId,
        askpass: Option<(AskpassBroker, String)>,
    ) -> Result<BaseBranch, Error> {
        self.inner(project_id)
            .await
            .fetch_from_target(project_id, askpass)
            .await
    }

    pub async fn move_commit(
        &self,
        project_id: &ProjectId,
        target_branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
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
        ownership: Option<&BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<git::Oid, Error> {
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
    ) -> Result<bool, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        Ok(super::is_remote_branch_mergeable(
            &project_repository,
            branch_name,
        )?)
    }

    pub fn can_apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<bool, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::is_virtual_branch_mergeable(&project_repository, branch_id).map_err(Into::into)
    }

    /// Retrieves the virtual branches state from the gitbutler repository (legacy state)
    pub fn get_vbranches_state(
        &self,
        project_id: &ProjectId,
        branch_ids: Vec<BranchId>,
    ) -> Result<VirtualBranches, Error> {
        let project = self.projects.get(project_id)?;
        let vb_state = VirtualBranchesHandle::new(&project.gb_dir());

        let default_target = vb_state
            .get_default_target()
            .context("failed to get default target")?;

        let mut branches: HashMap<BranchId, super::Branch> = HashMap::new();
        let mut branch_targets: HashMap<BranchId, super::target::Target> = HashMap::new();

        for branch_id in branch_ids {
            let branch = vb_state
                .get_branch(&branch_id)
                .context("failed to read branch")?;
            branches.insert(branch_id, branch);
            let target = vb_state
                .get_branch_target(&branch_id)
                .context("failed to read target")?;
            branch_targets.insert(branch_id, target);
        }

        Ok(VirtualBranches {
            default_target: Some(default_target),
            branch_targets,
            branches,
        })
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<(Vec<super::VirtualBranch>, Vec<git::diff::FileDiff>), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::list_virtual_branches(gb_repository, project_repository).map_err(Into::into)
        })
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |_, project_repository, _| {
            let branch_id = super::create_virtual_branch(project_repository, create)?.id;
            Ok(branch_id)
        })
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId, Error> {
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

            Ok(super::create_virtual_branch_from_branch(
                gb_repository,
                project_repository,
                branch,
                signing_key.as_ref(),
                user,
            )?)
        })
    }

    pub fn get_base_branch_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        Ok(super::get_base_branch_data(
            &gb_repository,
            &project_repository,
        )?)
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

        Ok(super::set_base_branch(
            &gb_repository,
            &project_repository,
            target_branch,
        )?)
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
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

    pub async fn update_base_branch(&self, project_id: &ProjectId) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
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
        ownership: &BranchOwnershipClaims,
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |_, project_repository, _| {
            super::reset_files(project_repository, ownership).map_err(Into::into)
        })
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
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
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
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
        askpass: Option<(AskpassBroker, Option<BranchId>)>,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;
        let helper = self.helper.clone();
        let project_id = *project_id;
        let branch_id = *branch_id;
        self.with_verify_branch_async(&project_id, move |_, project_repository, _| {
            Ok(super::push(
                project_repository,
                &branch_id,
                with_force,
                &helper,
                askpass,
            )?)
        })?
        .await
        .map_err(Error::from_err)?
    }

    pub async fn cherry_pick(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<Option<git::Oid>, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |gb_repository, project_repository, _| {
            super::cherry_pick(gb_repository, project_repository, branch_id, commit_oid)
                .map_err(Into::into)
        })
    }

    pub fn list_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::RemoteBranch>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        Ok(super::list_remote_branches(
            &gb_repository,
            &project_repository,
        )?)
    }

    pub fn get_remote_branch_data(
        &self,
        project_id: &ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        Ok(super::get_branch_data(
            &gb_repository,
            &project_repository,
            refname,
        )?)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
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
    ) -> Result<(), Error> {
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
        askpass: Option<(AskpassBroker, String)>,
    ) -> Result<BaseBranch, Error> {
        let project = self.projects.get(project_id)?;
        let mut project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
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
                errors::DefaultTargetNotSet {
                    project_id: *project_id,
                },
            ))?;

        let project_data_last_fetched = match project_repository
            .fetch(default_target.branch.remote(), &self.helper, askpass)
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
    ) -> Result<(), Error> {
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
    fn with_verify_branch<T>(
        &self,
        project_id: &ProjectId,
        action: impl FnOnce(
            &gb_repository::Repository,
            &project_repository::Repository,
            Option<&users::User>,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository = gb_repository::Repository::open(
            &self.local_data_dir,
            &project_repository,
            user.as_ref(),
        )
        .context("failed to open gitbutler repository")?;
        super::integration::verify_branch(&project_repository)?;
        action(&gb_repository, &project_repository, user.as_ref())
    }

    fn with_verify_branch_async<T: Send + 'static>(
        &self,
        project_id: &ProjectId,
        action: impl FnOnce(
                &gb_repository::Repository,
                &project_repository::Repository,
                Option<&users::User>,
            ) -> Result<T, Error>
            + Send
            + 'static,
    ) -> Result<JoinHandle<Result<T, Error>>, Error> {
        let local_data_dir = self.local_data_dir.clone();
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        let gb_repository =
            gb_repository::Repository::open(&local_data_dir, &project_repository, user.as_ref())
                .context("failed to open gitbutler repository")?;
        super::integration::verify_branch(&project_repository)?;
        Ok(tokio::task::spawn_blocking(move || {
            action(&gb_repository, &project_repository, user.as_ref())
        }))
    }
}
