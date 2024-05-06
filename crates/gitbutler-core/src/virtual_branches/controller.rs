use crate::{
    error::Error,
    snapshots::{
        entry::{OperationType, SnapshotDetails},
        snapshot::Oplog,
    },
};
use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::Context;
use tokio::{sync::Semaphore, task::JoinHandle};

use super::{
    branch::{BranchId, BranchOwnershipClaims},
    errors::{self, FetchFromTargetError},
    target, target_to_base_branch, BaseBranch, RemoteBranchFile, VirtualBranchesHandle,
};
use crate::{
    askpass::AskpassBroker,
    git, keys, project_repository,
    projects::{self, ProjectId},
    users,
};

#[derive(Clone)]
pub struct Controller {
    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
    helper: git::credentials::Helper,

    by_project_id: Arc<tokio::sync::Mutex<HashMap<ProjectId, ControllerInner>>>,
}

impl Controller {
    pub fn new(
        projects: projects::Controller,
        users: users::Controller,
        keys: keys::Controller,
        helper: git::credentials::Helper,
    ) -> Self {
        Self {
            by_project_id: Arc::new(tokio::sync::Mutex::new(HashMap::new())),

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
                ControllerInner::new(&self.projects, &self.users, &self.keys, &self.helper)
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

    pub async fn set_target_push_remote(
        &self,
        project_id: &ProjectId,
        push_remote: &str,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .set_target_push_remote(project_id, push_remote)
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
        commit_oid: git::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
        self.inner(project_id)
            .await
            .amend(project_id, branch_id, commit_oid, ownership)
            .await
    }

    pub async fn move_commit_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        from_commit_oid: git::Oid,
        to_commit_oid: git::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
        self.inner(project_id)
            .await
            .move_commit_file(
                project_id,
                branch_id,
                from_commit_oid,
                to_commit_oid,
                ownership,
            )
            .await
    }

    pub async fn undo_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .undo_commit(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn insert_blank_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        offset: i32,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .insert_blank_commit(project_id, branch_id, commit_oid, offset)
            .await
    }

    pub async fn reorder_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        offset: i32,
    ) -> Result<(), Error> {
        self.inner(project_id)
            .await
            .reorder_commit(project_id, branch_id, commit_oid, offset)
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
    semaphore: Arc<Semaphore>,

    projects: projects::Controller,
    users: users::Controller,
    keys: keys::Controller,
    helper: git::credentials::Helper,
}

impl ControllerInner {
    pub fn new(
        projects: &projects::Controller,
        users: &users::Controller,
        keys: &keys::Controller,
        helper: &git::credentials::Helper,
    ) -> Self {
        Self {
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

        self.with_verify_branch(project_id, |project_repository, user| {
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

            let result = super::commit(
                project_repository,
                branch_id,
                message,
                ownership,
                signing_key.as_ref(),
                user,
                run_hooks,
            )
            .map_err(Into::into);

            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::CreateCommit));
            result
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

    pub async fn list_virtual_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<(Vec<super::VirtualBranch>, Vec<git::diff::FileDiff>), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            super::list_virtual_branches(project_repository).map_err(Into::into)
        })
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: &ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let branch_id = super::create_virtual_branch(project_repository, create)?.id;
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::CreateBranch));
            Ok(branch_id)
        })
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: &ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
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
            let result = super::create_virtual_branch_from_branch(
                project_repository,
                branch,
                signing_key.as_ref(),
                user,
            )?;
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::CreateBranch));
            Ok(result)
        })
    }

    pub fn get_base_branch_data(
        &self,
        project_id: &ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        Ok(super::get_base_branch_data(&project_repository)?)
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
        let project_repository = project_repository::Repository::open(&project)?;
        let result = super::set_base_branch(&project_repository, target_branch)?;
        let _ = project_repository
            .project()
            .create_snapshot(SnapshotDetails::new(OperationType::SetBaseBranch));
        Ok(result)
    }

    pub fn set_target_push_remote(
        &self,
        project_id: &ProjectId,
        push_remote: &str,
    ) -> Result<(), Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::set_target_push_remote(&project_repository, push_remote)?;
        Ok(())
    }

    pub async fn merge_virtual_branch_upstream(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
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

            let result = super::merge_virtual_branch_upstream(
                project_repository,
                branch_id,
                signing_key.as_ref(),
                user,
            )
            .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::MergeUpstream));
            result
        })
    }

    pub async fn update_base_branch(&self, project_id: &ProjectId) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
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

            let result = super::update_base_branch(project_repository, user, signing_key.as_ref())
                .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::UpdateWorkspaceBase));
            result
        })
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let details = if branch_update.ownership.is_some() {
                SnapshotDetails::new(OperationType::MoveHunk)
            } else if branch_update.name.is_some() {
                SnapshotDetails::new(OperationType::UpdateBranchName)
            } else if branch_update.notes.is_some() {
                SnapshotDetails::new(OperationType::UpdateBranchNotes)
            } else if branch_update.order.is_some() {
                SnapshotDetails::new(OperationType::ReorderBranches)
            } else if branch_update.selected_for_changes.is_some() {
                SnapshotDetails::new(OperationType::SelectDefaultVirtualBranch)
            } else if branch_update.upstream.is_some() {
                SnapshotDetails::new(OperationType::UpdateBranchRemoteName)
            } else {
                SnapshotDetails::new(OperationType::GenericBranchUpdate)
            };
            super::update_branch(project_repository, branch_update)?;
            let _ = project_repository.project().create_snapshot(details);
            Ok(())
        })
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            super::delete_branch(project_repository, branch_id)?;
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::DeleteBranch));
            Ok(())
        })
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
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

            let result =
                super::apply_branch(project_repository, branch_id, signing_key.as_ref(), user)
                    .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::ApplyBranch));
            result
        })
    }

    pub async fn unapply_ownership(
        &self,
        project_id: &ProjectId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result =
                super::unapply_ownership(project_repository, ownership).map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::DiscardHunk));
            result
        })
    }

    pub async fn reset_files(
        &self,
        project_id: &ProjectId,
        ownership: &Vec<String>,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::reset_files(project_repository, ownership).map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::DiscardFile));
            result
        })
    }

    pub async fn amend(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::amend(project_repository, branch_id, commit_oid, ownership)
                .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::AmendCommit));
            result
        })
    }

    pub async fn move_commit_file(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        from_commit_oid: git::Oid,
        to_commit_oid: git::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git::Oid, Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::move_commit_file(
                project_repository,
                branch_id,
                from_commit_oid,
                to_commit_oid,
                ownership,
            )
            .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::MoveCommitFile));
            result
        })
    }

    pub async fn undo_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result =
                super::undo_commit(project_repository, branch_id, commit_oid).map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::UndoCommit));
            result
        })
    }

    pub async fn insert_blank_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        offset: i32,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let result =
                super::insert_blank_commit(project_repository, branch_id, commit_oid, user, offset)
                    .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::InsertBlankCommit));
            result
        })
    }

    pub async fn reorder_commit(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
        offset: i32,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::reorder_commit(project_repository, branch_id, commit_oid, offset)
                .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::ReorderCommit));
            result
        })
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        target_commit_oid: git::Oid,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::reset_branch(project_repository, branch_id, target_commit_oid)
                .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::UndoCommit));
            result
        })
    }

    pub async fn unapply_virtual_branch(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result = super::unapply_branch(project_repository, branch_id)
                .map(|_| ())
                .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::UnapplyBranch));
            result
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
        self.with_verify_branch_async(&project_id, move |project_repository, _| {
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

        self.with_verify_branch(project_id, |project_repository, _| {
            let result =
                super::cherry_pick(project_repository, branch_id, commit_oid).map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::CherryPick));
            result
        })
    }

    pub fn list_remote_branches(
        &self,
        project_id: &ProjectId,
    ) -> Result<Vec<super::RemoteBranch>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        Ok(super::list_remote_branches(&project_repository)?)
    }

    pub fn get_remote_branch_data(
        &self,
        project_id: &ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        Ok(super::get_branch_data(&project_repository, refname)?)
    }

    pub async fn squash(
        &self,
        project_id: &ProjectId,
        branch_id: &BranchId,
        commit_oid: git::Oid,
    ) -> Result<(), Error> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let result =
                super::squash(project_repository, branch_id, commit_oid).map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::SquashCommit));
            result
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
        self.with_verify_branch(project_id, |project_repository, _| {
            let result =
                super::update_commit_message(project_repository, branch_id, commit_oid, message)
                    .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::UpdateCommitMessage));
            result
        })
    }

    pub async fn fetch_from_target(
        &self,
        project_id: &ProjectId,
        askpass: Option<(AskpassBroker, String)>,
    ) -> Result<BaseBranch, Error> {
        let project = self.projects.get(project_id)?;
        let mut project_repository = project_repository::Repository::open(&project)?;

        let default_target = default_target(&project_repository.project().gb_dir())
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

        self.with_verify_branch(project_id, |project_repository, user| {
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
            let result = super::move_commit(
                project_repository,
                target_branch_id,
                commit_oid,
                user,
                signing_key.as_ref(),
            )
            .map_err(Into::into);
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationType::MoveCommit));
            result
        })
    }
}

impl ControllerInner {
    fn with_verify_branch<T>(
        &self,
        project_id: &ProjectId,
        action: impl FnOnce(&project_repository::Repository, Option<&users::User>) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        super::integration::verify_branch(&project_repository)?;
        action(&project_repository, user.as_ref())
    }

    fn with_verify_branch_async<T: Send + 'static>(
        &self,
        project_id: &ProjectId,
        action: impl FnOnce(&project_repository::Repository, Option<&users::User>) -> Result<T, Error>
            + Send
            + 'static,
    ) -> Result<JoinHandle<Result<T, Error>>, Error> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        super::integration::verify_branch(&project_repository)?;
        Ok(tokio::task::spawn_blocking(move || {
            action(&project_repository, user.as_ref())
        }))
    }
}

fn default_target(base_path: &Path) -> anyhow::Result<Option<target::Target>> {
    let vb_state = VirtualBranchesHandle::new(base_path);
    match vb_state.get_default_target() {
        Result::Ok(target) => Ok(Some(target)),
        Err(crate::reader::Error::NotFound) => Ok(None),
        Err(err) => Err(err.into()),
    }
}
