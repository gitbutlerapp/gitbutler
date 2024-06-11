use crate::ops::entry::{OperationKind, SnapshotDetails};
use anyhow::Result;
use std::{collections::HashMap, path::Path, sync::Arc};

use anyhow::Context;
use tokio::{sync::Semaphore, task::JoinHandle};

use super::{
    branch::{BranchId, BranchOwnershipClaims},
    target, target_to_base_branch, BaseBranch, Branch, NameConflitResolution, RemoteBranchFile,
    VirtualBranchesHandle,
};
use crate::{
    git, project_repository,
    projects::{self, ProjectId},
    users,
};

#[derive(Clone)]
pub struct Controller {
    projects: projects::Controller,
    users: users::Controller,
    helper: git::credentials::Helper,

    by_project_id: Arc<tokio::sync::Mutex<HashMap<ProjectId, ControllerInner>>>,
}

impl Controller {
    pub fn new(
        projects: projects::Controller,
        users: users::Controller,
        helper: git::credentials::Helper,
    ) -> Self {
        Self {
            by_project_id: Arc::new(tokio::sync::Mutex::new(HashMap::new())),

            projects,
            users,
            helper,
        }
    }

    async fn inner(&self, project_id: ProjectId) -> ControllerInner {
        self.by_project_id
            .lock()
            .await
            .entry(project_id)
            .or_insert_with(|| ControllerInner::new(&self.projects, &self.users, &self.helper))
            .clone()
    }

    pub async fn create_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        message: &str,
        ownership: Option<&BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<git2::Oid> {
        self.inner(project_id)
            .await
            .create_commit(project_id, branch_id, message, ownership, run_hooks)
            .await
    }

    pub async fn can_apply_remote_branch(
        &self,
        project_id: ProjectId,
        branch_name: &git::RemoteRefname,
    ) -> Result<bool> {
        self.inner(project_id)
            .await
            .can_apply_remote_branch(project_id, branch_name)
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: ProjectId,
    ) -> Result<(Vec<super::VirtualBranch>, Vec<git::diff::FileDiff>)> {
        self.inner(project_id)
            .await
            .list_virtual_branches(project_id)
            .await
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId> {
        self.inner(project_id)
            .await
            .create_virtual_branch(project_id, create)
            .await
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId> {
        self.inner(project_id)
            .await
            .create_virtual_branch_from_branch(project_id, branch)
            .await
    }

    pub async fn get_base_branch_data(&self, project_id: ProjectId) -> Result<BaseBranch> {
        self.inner(project_id)
            .await
            .get_base_branch_data(project_id)
    }

    pub async fn list_remote_commit_files(
        &self,
        project_id: ProjectId,
        commit_oid: git2::Oid,
    ) -> Result<Vec<RemoteBranchFile>> {
        self.inner(project_id)
            .await
            .list_remote_commit_files(project_id, commit_oid)
    }

    pub async fn set_base_branch(
        &self,
        project_id: ProjectId,
        target_branch: &git::RemoteRefname,
    ) -> Result<BaseBranch> {
        self.inner(project_id)
            .await
            .set_base_branch(project_id, target_branch)
    }

    pub async fn set_target_push_remote(
        &self,
        project_id: ProjectId,
        push_remote: &str,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .set_target_push_remote(project_id, push_remote)
    }

    pub async fn integrate_upstream_commits(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .integrate_upstream_commits(project_id, branch_id)
            .await
    }

    pub async fn update_base_branch(&self, project_id: ProjectId) -> Result<Vec<Branch>> {
        self.inner(project_id)
            .await
            .update_base_branch(project_id)
            .await
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .update_virtual_branch(project_id, branch_update)
            .await
    }
    pub async fn delete_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .delete_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .apply_virtual_branch(project_id, branch_id)
            .await
    }

    pub async fn unapply_ownership(
        &self,
        project_id: ProjectId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .unapply_ownership(project_id, ownership)
            .await
    }

    pub async fn reset_files(&self, project_id: ProjectId, files: &Vec<String>) -> Result<()> {
        self.inner(project_id)
            .await
            .reset_files(project_id, files)
            .await
    }

    pub async fn amend(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
        self.inner(project_id)
            .await
            .amend(project_id, branch_id, commit_oid, ownership)
            .await
    }

    pub async fn move_commit_file(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        from_commit_oid: git2::Oid,
        to_commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
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
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .undo_commit(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn insert_blank_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .insert_blank_commit(project_id, branch_id, commit_oid, offset)
            .await
    }

    pub async fn reorder_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .reorder_commit(project_id, branch_id, commit_oid, offset)
            .await
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: git2::Oid,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .reset_virtual_branch(project_id, branch_id, target_commit_oid)
            .await
    }

    pub async fn convert_to_real_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        name_conflict_resolution: NameConflitResolution,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .convert_to_real_branch(project_id, branch_id, name_conflict_resolution)
            .await
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        with_force: bool,
        askpass: Option<Option<BranchId>>,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .push_virtual_branch(project_id, branch_id, with_force, askpass)
            .await
    }

    pub async fn cherry_pick(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<Option<git2::Oid>> {
        self.inner(project_id)
            .await
            .cherry_pick(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn list_remote_branches(
        &self,
        project_id: ProjectId,
    ) -> Result<Vec<super::RemoteBranch>> {
        self.inner(project_id)
            .await
            .list_remote_branches(project_id)
    }

    pub async fn get_remote_branch_data(
        &self,
        project_id: ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData> {
        self.inner(project_id)
            .await
            .get_remote_branch_data(project_id, refname)
    }

    pub async fn squash(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .squash(project_id, branch_id, commit_oid)
            .await
    }

    pub async fn update_commit_message(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        message: &str,
    ) -> Result<()> {
        self.inner(project_id)
            .await
            .update_commit_message(project_id, branch_id, commit_oid, message)
            .await
    }

    pub async fn fetch_from_remotes(
        &self,
        project_id: ProjectId,
        askpass: Option<String>,
    ) -> Result<BaseBranch> {
        self.inner(project_id)
            .await
            .fetch_from_remotes(project_id, askpass)
            .await
    }

    pub async fn move_commit(
        &self,
        project_id: ProjectId,
        target_branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
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
    helper: git::credentials::Helper,
}

impl ControllerInner {
    pub fn new(
        projects: &projects::Controller,
        users: &users::Controller,
        helper: &git::credentials::Helper,
    ) -> Self {
        Self {
            semaphore: Arc::new(Semaphore::new(1)),
            projects: projects.clone(),
            users: users.clone(),
            helper: helper.clone(),
        }
    }

    pub async fn create_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        message: &str,
        ownership: Option<&BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<git2::Oid> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let snapshot_tree = project_repository.project().prepare_snapshot();
            let result = super::commit(
                project_repository,
                branch_id,
                message,
                ownership,
                user,
                run_hooks,
            )
            .map_err(Into::into);
            let _ = snapshot_tree.and_then(|snapshot_tree| {
                project_repository.project().snapshot_commit_creation(
                    snapshot_tree,
                    result.as_ref().err(),
                    message.to_owned(),
                    None,
                )
            });
            result
        })
    }

    pub fn can_apply_remote_branch(
        &self,
        project_id: ProjectId,
        branch_name: &git::RemoteRefname,
    ) -> Result<bool> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::is_remote_branch_mergeable(&project_repository, branch_name).map_err(Into::into)
    }

    pub async fn list_virtual_branches(
        &self,
        project_id: ProjectId,
    ) -> Result<(Vec<super::VirtualBranch>, Vec<git::diff::FileDiff>)> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            super::list_virtual_branches(project_repository).map_err(Into::into)
        })
    }

    pub async fn create_virtual_branch(
        &self,
        project_id: ProjectId,
        create: &super::branch::BranchCreateRequest,
    ) -> Result<BranchId> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let branch_id = super::create_virtual_branch(project_repository, create)?.id;
            Ok(branch_id)
        })
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project_id: ProjectId,
        branch: &git::Refname,
    ) -> Result<BranchId> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            super::create_virtual_branch_from_branch(project_repository, branch, user)
                .map_err(Into::into)
        })
    }

    pub fn get_base_branch_data(&self, project_id: ProjectId) -> Result<BaseBranch> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::get_base_branch_data(&project_repository)
    }

    pub fn list_remote_commit_files(
        &self,
        project_id: ProjectId,
        commit_oid: git2::Oid,
    ) -> Result<Vec<RemoteBranchFile>> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::list_remote_commit_files(project_repository.repo(), commit_oid).map_err(Into::into)
    }

    pub fn set_base_branch(
        &self,
        project_id: ProjectId,
        target_branch: &git::RemoteRefname,
    ) -> Result<BaseBranch> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let _ = project_repository
            .project()
            .create_snapshot(SnapshotDetails::new(OperationKind::SetBaseBranch));
        super::set_base_branch(&project_repository, target_branch)
    }

    pub fn set_target_push_remote(&self, project_id: ProjectId, push_remote: &str) -> Result<()> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::set_target_push_remote(&project_repository, push_remote)
    }

    pub async fn integrate_upstream_commits(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::MergeUpstream));
            super::integrate_upstream_commits(project_repository, branch_id, user)
                .map_err(Into::into)
        })
    }

    pub async fn update_base_branch(&self, project_id: ProjectId) -> Result<Vec<Branch>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::UpdateWorkspaceBase));
            super::update_base_branch(project_repository, user).map_err(Into::into)
        })
    }

    pub async fn update_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_update: super::branch::BranchUpdateRequest,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let snapshot_tree = project_repository.project().prepare_snapshot();
            let old_branch = project_repository
                .project()
                .virtual_branches()
                .get_branch(branch_update.id)?;
            let result = super::update_branch(project_repository, &branch_update);
            let _ = snapshot_tree.and_then(|snapshot_tree| {
                project_repository.project().snapshot_branch_update(
                    snapshot_tree,
                    &old_branch,
                    &branch_update,
                    result.as_ref().err(),
                )
            });
            result?;

            Ok(())
        })
    }

    pub async fn delete_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            super::delete_branch(project_repository, branch_id)
        })
    }

    pub async fn apply_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let snapshot_tree = project_repository.project().prepare_snapshot();
            let result =
                super::apply_branch(project_repository, branch_id, user).map_err(Into::into);

            let _ = snapshot_tree.and_then(|snapshot_tree| {
                project_repository
                    .project()
                    .snapshot_branch_applied(snapshot_tree, result.as_ref())
            });
            result.map(|_| ())
        })
    }

    pub async fn unapply_ownership(
        &self,
        project_id: ProjectId,
        ownership: &BranchOwnershipClaims,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::DiscardHunk));
            super::unapply_ownership(project_repository, ownership).map_err(Into::into)
        })
    }

    pub async fn reset_files(&self, project_id: ProjectId, ownership: &Vec<String>) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::DiscardFile));
            super::reset_files(project_repository, ownership).map_err(Into::into)
        })
    }

    pub async fn amend(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::AmendCommit));
            super::amend(project_repository, branch_id, commit_oid, ownership)
        })
    }

    pub async fn move_commit_file(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        from_commit_oid: git2::Oid,
        to_commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::MoveCommitFile));
            super::move_commit_file(
                project_repository,
                branch_id,
                from_commit_oid,
                to_commit_oid,
                ownership,
            )
            .map_err(Into::into)
        })
    }

    pub async fn undo_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let snapshot_tree = project_repository.project().prepare_snapshot();
            let result: Result<()> =
                super::undo_commit(project_repository, branch_id, commit_oid).map_err(Into::into);
            let _ = snapshot_tree.and_then(|snapshot_tree| {
                project_repository.project().snapshot_commit_undo(
                    snapshot_tree,
                    result.as_ref(),
                    commit_oid,
                )
            });
            result
        })
    }

    pub async fn insert_blank_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::InsertBlankCommit));
            super::insert_blank_commit(project_repository, branch_id, commit_oid, user, offset)
                .map_err(Into::into)
        })
    }

    pub async fn reorder_commit(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::ReorderCommit));
            super::reorder_commit(project_repository, branch_id, commit_oid, offset)
                .map_err(Into::into)
        })
    }

    pub async fn reset_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: git2::Oid,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::UndoCommit));
            super::reset_branch(project_repository, branch_id, target_commit_oid)
                .map_err(Into::into)
        })
    }

    pub async fn convert_to_real_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        name_conflict_resolution: NameConflitResolution,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let snapshot_tree = project_repository.project().prepare_snapshot();
            let result = super::convert_to_real_branch(
                project_repository,
                branch_id,
                name_conflict_resolution,
            )
            .map_err(Into::into);
            let _ = snapshot_tree.and_then(|snapshot_tree| {
                project_repository
                    .project()
                    .snapshot_branch_unapplied(snapshot_tree, result.as_ref())
            });
            result.map(|_| ())
        })
    }

    pub async fn push_virtual_branch(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        with_force: bool,
        askpass: Option<Option<BranchId>>,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;
        let helper = self.helper.clone();
        self.with_verify_branch_async(project_id, move |project_repository, _| {
            super::push(project_repository, branch_id, with_force, &helper, askpass)
        })?
        .await?
    }

    pub async fn cherry_pick(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<Option<git2::Oid>> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::CherryPick));
            super::cherry_pick(project_repository, branch_id, commit_oid).map_err(Into::into)
        })
    }

    pub fn list_remote_branches(&self, project_id: ProjectId) -> Result<Vec<super::RemoteBranch>> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::list_remote_branches(&project_repository)
    }

    pub fn get_remote_branch_data(
        &self,
        project_id: ProjectId,
        refname: &git::Refname,
    ) -> Result<super::RemoteBranchData> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        super::get_branch_data(&project_repository, refname)
    }

    pub async fn squash(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::SquashCommit));
            super::squash(project_repository, branch_id, commit_oid).map_err(Into::into)
        })
    }

    pub async fn update_commit_message(
        &self,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        message: &str,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;
        self.with_verify_branch(project_id, |project_repository, _| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::UpdateCommitMessage));
            super::update_commit_message(project_repository, branch_id, commit_oid, message)
                .map_err(Into::into)
        })
    }

    pub async fn fetch_from_remotes(
        &self,
        project_id: ProjectId,
        askpass: Option<String>,
    ) -> Result<BaseBranch> {
        let project = self.projects.get(project_id)?;
        let mut project_repository = project_repository::Repository::open(&project)?;

        let remotes = project_repository.remotes()?;
        let fetch_results: Vec<Result<(), _>> = remotes
            .iter()
            .map(|remote| project_repository.fetch(remote, &self.helper, askpass.clone()))
            .collect();

        let project_data_last_fetched = if fetch_results.iter().any(Result::is_err) {
            projects::FetchResult::Error {
                timestamp: std::time::SystemTime::now(),
                error: fetch_results
                    .iter()
                    .filter_map(|result| match result {
                        Ok(_) => None,
                        Err(error) => Some(error.to_string()),
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            }
        } else {
            projects::FetchResult::Fetched {
                timestamp: std::time::SystemTime::now(),
            }
        };

        let default_target = default_target(&project_repository.project().gb_dir())?;

        // if we have a push remote, let's fetch from this too
        if let Some(push_remote) = &default_target.push_remote_name {
            if let Err(err) = project_repository.fetch(push_remote, &self.helper, askpass.clone()) {
                tracing::warn!(?err, "fetch from push-remote failed");
            }
        }

        let updated_project = self
            .projects
            .update(&projects::UpdateRequest {
                id: project_id,
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
        project_id: ProjectId,
        target_branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let _permit = self.semaphore.acquire().await;

        self.with_verify_branch(project_id, |project_repository, user| {
            let _ = project_repository
                .project()
                .create_snapshot(SnapshotDetails::new(OperationKind::MoveCommit));
            super::move_commit(project_repository, target_branch_id, commit_oid, user)
                .map_err(Into::into)
        })
    }
}

impl ControllerInner {
    fn with_verify_branch<T>(
        &self,
        project_id: ProjectId,
        action: impl FnOnce(&project_repository::Repository, Option<&users::User>) -> Result<T>,
    ) -> Result<T> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        super::integration::verify_branch(&project_repository)?;
        action(&project_repository, user.as_ref())
    }

    fn with_verify_branch_async<T: Send + 'static>(
        &self,
        project_id: ProjectId,
        action: impl FnOnce(&project_repository::Repository, Option<&users::User>) -> Result<T>
            + Send
            + 'static,
    ) -> Result<JoinHandle<Result<T>>> {
        let project = self.projects.get(project_id)?;
        let project_repository = project_repository::Repository::open(&project)?;
        let user = self.users.get_user()?;
        super::integration::verify_branch(&project_repository)?;
        Ok(tokio::task::spawn_blocking(move || {
            action(&project_repository, user.as_ref())
        }))
    }
}

fn default_target(base_path: &Path) -> anyhow::Result<target::Target> {
    VirtualBranchesHandle::new(base_path).get_default_target()
}
