use crate::{
    base::{
        get_base_branch_data, set_base_branch, set_target_push_remote, update_base_branch,
        BaseBranch,
    },
    branch_manager::BranchManagerExt,
    remote::{get_branch_data, list_remote_branches, RemoteBranch, RemoteBranchData},
    VirtualBranchesExt,
};
use anyhow::Result;
use gitbutler_branch::{
    diff, BranchOwnershipClaims, {BranchCreateRequest, BranchId, BranchUpdateRequest},
};
use gitbutler_command_context::ProjectRepository;
use gitbutler_oplog::{
    entry::{OperationKind, SnapshotDetails},
    OplogExt, SnapshotExt,
};
use gitbutler_project::{FetchResult, Project};
use gitbutler_reference::ReferenceName;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_repo::{credentials::Helper, RepoActionsExt, RepositoryExt};
use tracing::instrument;

use super::r#virtual as branch;

use crate::files::RemoteBranchFile;

#[derive(Clone, Copy, Default)]
pub struct VirtualBranchActions;

impl VirtualBranchActions {
    pub async fn create_commit(
        &self,
        project: &Project,
        branch_id: BranchId,
        message: &str,
        ownership: Option<&BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<git2::Oid> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let snapshot_tree = project_repository
            .project()
            .prepare_snapshot(guard.read_permission());
        let result = branch::commit(
            &project_repository,
            branch_id,
            message,
            ownership,
            run_hooks,
            guard.write_permission(),
        )
        .map_err(Into::into);
        let _ = snapshot_tree.and_then(|snapshot_tree| {
            project_repository.project().snapshot_commit_creation(
                snapshot_tree,
                result.as_ref().err(),
                message.to_owned(),
                None,
                guard.write_permission(),
            )
        });
        result
    }

    pub async fn can_apply_remote_branch(
        &self,
        project: &Project,
        branch_name: &RemoteRefname,
    ) -> Result<bool> {
        let project_repository = ProjectRepository::open(project)?;
        branch::is_remote_branch_mergeable(&project_repository, branch_name).map_err(Into::into)
    }

    pub async fn list_virtual_branches(
        &self,
        project: &Project,
    ) -> Result<(Vec<branch::VirtualBranch>, Vec<diff::FileDiff>)> {
        branch::list_virtual_branches(
            &open_with_verify(project)?,
            project.exclusive_worktree_access().write_permission(),
        )
        .map_err(Into::into)
    }

    pub async fn create_virtual_branch(
        &self,
        project: &Project,
        create: &BranchCreateRequest,
    ) -> Result<BranchId> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let branch_manager = project_repository.branch_manager();
        let branch_id = branch_manager
            .create_virtual_branch(create, guard.write_permission())?
            .id;
        Ok(branch_id)
    }

    #[instrument(skip(project), err(Debug))]
    pub async fn get_base_branch_data(project: &Project) -> Result<BaseBranch> {
        let project_repository = ProjectRepository::open(project)?;
        get_base_branch_data(&project_repository)
    }

    pub async fn list_remote_commit_files(
        &self,
        project: &Project,
        commit_oid: git2::Oid,
    ) -> Result<Vec<RemoteBranchFile>> {
        let project_repository = ProjectRepository::open(project)?;
        crate::files::list_remote_commit_files(project_repository.repo(), commit_oid)
            .map_err(Into::into)
    }

    pub async fn set_base_branch(
        &self,
        project: &Project,
        target_branch: &RemoteRefname,
    ) -> Result<BaseBranch> {
        let project_repository = ProjectRepository::open(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::SetBaseBranch),
            guard.write_permission(),
        );
        set_base_branch(&project_repository, target_branch)
    }

    pub async fn set_target_push_remote(&self, project: &Project, push_remote: &str) -> Result<()> {
        let project_repository = ProjectRepository::open(project)?;
        set_target_push_remote(&project_repository, push_remote)
    }

    pub async fn integrate_upstream_commits(
        &self,
        project: &Project,
        branch_id: BranchId,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::MergeUpstream),
            guard.write_permission(),
        );
        branch::integrate_upstream_commits(&project_repository, branch_id).map_err(Into::into)
    }

    pub async fn update_base_branch(&self, project: &Project) -> Result<Vec<ReferenceName>> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::UpdateWorkspaceBase),
            guard.write_permission(),
        );
        update_base_branch(&project_repository, guard.write_permission()).map_err(Into::into)
    }

    pub async fn update_virtual_branch(
        &self,
        project: &Project,
        branch_update: BranchUpdateRequest,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let snapshot_tree = project_repository
            .project()
            .prepare_snapshot(guard.read_permission());
        let old_branch = project_repository
            .project()
            .virtual_branches()
            .get_branch_in_workspace(branch_update.id)?;
        let result = branch::update_branch(&project_repository, &branch_update);
        let _ = snapshot_tree.and_then(|snapshot_tree| {
            project_repository.project().snapshot_branch_update(
                snapshot_tree,
                &old_branch,
                &branch_update,
                result.as_ref().err(),
                guard.write_permission(),
            )
        });
        result?;
        Ok(())
    }
    pub async fn delete_virtual_branch(
        &self,
        project: &Project,
        branch_id: BranchId,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let branch_manager = project_repository.branch_manager();
        let mut guard = project.exclusive_worktree_access();
        branch_manager.delete_branch(branch_id, guard.write_permission())
    }

    pub async fn unapply_ownership(
        &self,
        project: &Project,
        ownership: &BranchOwnershipClaims,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::DiscardHunk),
            guard.write_permission(),
        );
        branch::unapply_ownership(&project_repository, ownership, guard.write_permission())
            .map_err(Into::into)
    }

    pub async fn reset_files(&self, project: &Project, files: &Vec<String>) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::DiscardFile),
            guard.write_permission(),
        );
        branch::reset_files(&project_repository, files).map_err(Into::into)
    }

    pub async fn amend(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::AmendCommit),
            guard.write_permission(),
        );
        branch::amend(
            &project_repository,
            branch_id,
            commit_oid,
            ownership,
            guard.write_permission(),
        )
    }

    pub async fn move_commit_file(
        &self,
        project: &Project,
        branch_id: BranchId,
        from_commit_oid: git2::Oid,
        to_commit_oid: git2::Oid,
        ownership: &BranchOwnershipClaims,
    ) -> Result<git2::Oid> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::MoveCommitFile),
            guard.write_permission(),
        );
        branch::move_commit_file(
            &project_repository,
            branch_id,
            from_commit_oid,
            to_commit_oid,
            ownership,
        )
        .map_err(Into::into)
    }

    pub async fn undo_commit(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let snapshot_tree = project_repository
            .project()
            .prepare_snapshot(guard.read_permission());
        let result: Result<()> =
            branch::undo_commit(&project_repository, branch_id, commit_oid).map_err(Into::into);
        let _ = snapshot_tree.and_then(|snapshot_tree| {
            project_repository.project().snapshot_commit_undo(
                snapshot_tree,
                result.as_ref(),
                commit_oid,
                guard.write_permission(),
            )
        });
        result
    }

    pub async fn insert_blank_commit(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::InsertBlankCommit),
            guard.write_permission(),
        );
        branch::insert_blank_commit(&project_repository, branch_id, commit_oid, offset)
            .map_err(Into::into)
    }

    pub async fn reorder_commit(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        offset: i32,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::ReorderCommit),
            guard.write_permission(),
        );
        branch::reorder_commit(&project_repository, branch_id, commit_oid, offset)
            .map_err(Into::into)
    }

    pub async fn reset_virtual_branch(
        &self,
        project: &Project,
        branch_id: BranchId,
        target_commit_oid: git2::Oid,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::UndoCommit),
            guard.write_permission(),
        );
        branch::reset_branch(&project_repository, branch_id, target_commit_oid).map_err(Into::into)
    }

    pub async fn convert_to_real_branch(
        &self,
        project: &Project,
        branch_id: BranchId,
        name_conflict_resolution: branch::NameConflictResolution,
    ) -> Result<ReferenceName> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let snapshot_tree = project_repository
            .project()
            .prepare_snapshot(guard.read_permission());
        let branch_manager = project_repository.branch_manager();
        let result = branch_manager.convert_to_real_branch(
            branch_id,
            name_conflict_resolution,
            guard.write_permission(),
        );

        let _ = snapshot_tree.and_then(|snapshot_tree| {
            project_repository.project().snapshot_branch_unapplied(
                snapshot_tree,
                result.as_ref(),
                guard.write_permission(),
            )
        });

        result
    }

    pub async fn push_virtual_branch(
        &self,
        project: &Project,
        branch_id: BranchId,
        with_force: bool,
        askpass: Option<Option<BranchId>>,
    ) -> Result<()> {
        let helper = Helper::default();
        let project_repository = open_with_verify(project)?;
        branch::push(&project_repository, branch_id, with_force, &helper, askpass)
    }

    pub async fn list_remote_branches(project: Project) -> Result<Vec<RemoteBranch>> {
        let project_repository = ProjectRepository::open(&project)?;
        list_remote_branches(&project_repository)
    }

    pub async fn get_remote_branch_data(
        &self,
        project: &Project,
        refname: &Refname,
    ) -> Result<RemoteBranchData> {
        let project_repository = ProjectRepository::open(project)?;
        get_branch_data(&project_repository, refname)
    }

    pub async fn squash(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::SquashCommit),
            guard.write_permission(),
        );
        branch::squash(&project_repository, branch_id, commit_oid).map_err(Into::into)
    }

    pub async fn update_commit_message(
        &self,
        project: &Project,
        branch_id: BranchId,
        commit_oid: git2::Oid,
        message: &str,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::UpdateCommitMessage),
            guard.write_permission(),
        );
        branch::update_commit_message(&project_repository, branch_id, commit_oid, message)
            .map_err(Into::into)
    }

    pub async fn fetch_from_remotes(
        &self,
        project: &Project,
        askpass: Option<String>,
    ) -> Result<FetchResult> {
        let project_repository = ProjectRepository::open(project)?;

        let helper = Helper::default();
        let remotes = project_repository.repo().remotes_as_string()?;
        let fetch_results: Vec<Result<(), _>> = remotes
            .iter()
            .map(|remote| project_repository.fetch(remote, &helper, askpass.clone()))
            .collect();

        let project_data_last_fetched = if fetch_results.iter().any(Result::is_err) {
            FetchResult::Error {
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
            FetchResult::Fetched {
                timestamp: std::time::SystemTime::now(),
            }
        };

        Ok(project_data_last_fetched)
    }

    pub async fn move_commit(
        &self,
        project: &Project,
        target_branch_id: BranchId,
        commit_oid: git2::Oid,
    ) -> Result<()> {
        let project_repository = open_with_verify(project)?;
        let mut guard = project.exclusive_worktree_access();
        let _ = project_repository.project().create_snapshot(
            SnapshotDetails::new(OperationKind::MoveCommit),
            guard.write_permission(),
        );
        branch::move_commit(
            &project_repository,
            target_branch_id,
            commit_oid,
            guard.write_permission(),
        )
        .map_err(Into::into)
    }

    pub async fn create_virtual_branch_from_branch(
        &self,
        project: &Project,
        branch: &Refname,
    ) -> Result<BranchId> {
        let project_repository = open_with_verify(project)?;
        let branch_manager = project_repository.branch_manager();
        let mut guard = project.exclusive_worktree_access();
        branch_manager
            .create_virtual_branch_from_branch(branch, guard.write_permission())
            .map_err(Into::into)
    }
}

fn open_with_verify(project: &Project) -> Result<ProjectRepository> {
    let project_repository = ProjectRepository::open(project)?;
    let mut guard = project.exclusive_worktree_access();
    crate::integration::verify_branch(&project_repository, guard.write_permission())?;
    Ok(project_repository)
}
