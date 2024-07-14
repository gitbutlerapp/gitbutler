pub mod commands {
    use crate::error::Error;
    use anyhow::{anyhow, Context};
    use gitbutler_branch::branch::{BranchCreateRequest, BranchId, BranchUpdateRequest};
    use gitbutler_branch::ownership::BranchOwnershipClaims;
    use gitbutler_branch_actions::base::BaseBranch;
    use gitbutler_branch_actions::files::RemoteBranchFile;
    use gitbutler_branch_actions::remote::{RemoteBranch, RemoteBranchData};
    use gitbutler_branch_actions::{NameConflitResolution, VirtualBranchActions, VirtualBranches};
    use gitbutler_error::error::Code;
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_reference::normalize_branch_name as normalize_name;
    use gitbutler_reference::ReferenceName;
    use gitbutler_reference::{Refname, RemoteRefname};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::WindowState;

    #[tauri::command(async)]
    #[instrument(err(Debug))]
    pub async fn normalize_branch_name(name: &str) -> Result<String, Error> {
        Ok(normalize_name(name))
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn commit_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: BranchId,
        message: &str,
        ownership: Option<BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<String, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let oid = VirtualBranchActions::default()
            .create_commit(&project, branch, message, ownership.as_ref(), run_hooks)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn list_virtual_branches(
        handle: AppHandle,
        project_id: ProjectId,
    ) -> Result<VirtualBranches, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let (branches, skipped_files) = VirtualBranchActions::default()
            .list_virtual_branches(&project)
            .await?;

        Ok(VirtualBranches {
            branches,
            skipped_files,
        })
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn create_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_id = VirtualBranchActions::default()
            .create_virtual_branch(&project, &branch)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn create_virtual_branch_from_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: Refname,
    ) -> Result<BranchId, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_id = VirtualBranchActions::default()
            .create_virtual_branch_from_branch(&project, &branch)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn integrate_upstream_commits(
        handle: AppHandle,
        project_id: ProjectId,
        branch: BranchId,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .integrate_upstream_commits(&project, branch)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_base_branch_data(
        handle: AppHandle,
        project_id: ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        if let Ok(base_branch) = VirtualBranchActions::default()
            .get_base_branch_data(&project)
            .await
        {
            Ok(Some(base_branch))
        } else {
            Ok(None)
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn set_base_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: &str,
        push_remote: Option<&str>, // optional different name of a remote to push to (defaults to same as the branch)
    ) -> Result<BaseBranch, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_name = format!("refs/remotes/{}", branch)
            .parse()
            .context("Invalid branch name")?;
        let base_branch = VirtualBranchActions::default()
            .set_base_branch(&project, &branch_name)
            .await?;

        // if they also sent a different push remote, set that too
        if let Some(push_remote) = push_remote {
            VirtualBranchActions::default()
                .set_target_push_remote(&project, push_remote)
                .await?;
        }
        emit_vbranches(&handle, project_id).await;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn update_base_branch(
        handle: AppHandle,
        project_id: ProjectId,
    ) -> Result<Vec<ReferenceName>, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let unapplied_branches = VirtualBranchActions::default()
            .update_base_branch(&project)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(unapplied_branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn update_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: BranchUpdateRequest,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .update_virtual_branch(&project, branch)
            .await?;

        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn delete_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .delete_virtual_branch(&project, branch_id)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn convert_to_real_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: BranchId,
        name_conflict_resolution: NameConflitResolution,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .convert_to_real_branch(&project, branch, name_conflict_resolution)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn unapply_ownership(
        handle: AppHandle,
        project_id: ProjectId,
        ownership: BranchOwnershipClaims,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .unapply_ownership(&project, &ownership)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn reset_files(
        handle: AppHandle,
        project_id: ProjectId,
        files: &str,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        // convert files to Vec<String>
        let files = files
            .split('\n')
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>();
        VirtualBranchActions::default()
            .reset_files(&project, &files)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn push_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        VirtualBranchActions::default()
            .push_virtual_branch(&project, branch_id, with_force, Some(Some(branch_id)))
            .await
            .map_err(|err| err.context(Code::Unknown))?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn can_apply_remote_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch: RemoteRefname,
    ) -> Result<bool, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        Ok(VirtualBranchActions::default()
            .can_apply_remote_branch(&project, &branch)
            .await?)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn list_remote_commit_files(
        handle: AppHandle,
        project_id: ProjectId,
        commit_oid: String,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .list_remote_commit_files(&project, commit_oid)
            .await
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn reset_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .reset_virtual_branch(&project, branch_id, target_commit_oid)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn amend_virtual_branch(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        let oid = VirtualBranchActions::default()
            .amend(&project, branch_id, commit_oid, &ownership)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn move_commit_file(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        from_commit_oid: String,
        to_commit_oid: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let from_commit_oid = git2::Oid::from_str(&from_commit_oid).map_err(|e| anyhow!(e))?;
        let to_commit_oid = git2::Oid::from_str(&to_commit_oid).map_err(|e| anyhow!(e))?;
        let oid = VirtualBranchActions::default()
            .move_commit_file(
                &project,
                branch_id,
                from_commit_oid,
                to_commit_oid,
                &ownership,
            )
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn undo_commit(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .undo_commit(&project, branch_id, commit_oid)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn insert_blank_commit(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .insert_blank_commit(&project, branch_id, commit_oid, offset)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn reorder_commit(
        handle: AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .reorder_commit(&project, branch_id, commit_oid, offset)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn list_remote_branches(
        handle: tauri::AppHandle,
        project_id: ProjectId,
    ) -> Result<Vec<RemoteBranch>, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branches = VirtualBranchActions::default()
            .list_remote_branches(project)
            .await?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_remote_branch_data(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        refname: Refname,
    ) -> Result<RemoteBranchData, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_data = VirtualBranchActions::default()
            .get_remote_branch_data(&project, &refname)
            .await?;
        Ok(branch_data)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn squash_branch_commit(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .squash(&project, branch_id, target_commit_oid)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn fetch_from_remotes(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        action: Option<String>,
    ) -> Result<BaseBranch, Error> {
        let projects = handle.state::<projects::Controller>();
        let project = projects.get(project_id)?;

        let project_data_last_fetched = VirtualBranchActions::default()
            .fetch_from_remotes(
                &project,
                Some(action.unwrap_or_else(|| "unknown".to_string())),
            )
            .await?;

        // Updates the project controller with the last fetched timestamp
        //
        // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
        projects
            .update(&projects::UpdateRequest {
                id: project.id,
                project_data_last_fetched: Some(project_data_last_fetched),
                ..Default::default()
            })
            .await
            .context("failed to update project with last fetched timestamp")?;

        let base_branch = VirtualBranchActions::default()
            .get_base_branch_data(&project)
            .await?;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn move_commit(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        commit_oid: String,
        target_branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .move_commit(&project, target_branch_id, commit_oid)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn update_commit_message(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        message: &str,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions::default()
            .update_commit_message(&project, branch_id, commit_oid, message)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    async fn emit_vbranches(handle: &AppHandle, project_id: projects::ProjectId) {
        if let Err(error) = handle
            .state::<WindowState>()
            .post(gitbutler_watcher::Action::CalculateVirtualBranches(
                project_id,
            ))
            .await
        {
            tracing::error!(?error);
        }
    }
}
