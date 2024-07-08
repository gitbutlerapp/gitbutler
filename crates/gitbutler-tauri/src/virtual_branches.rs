pub mod commands {
    use crate::error::Error;
    use anyhow::{anyhow, Context};
    use gitbutler_core::{
        error::Code,
        git,
        types::ReferenceName,
        virtual_branches::branch::{self, BranchId, BranchOwnershipClaims},
    };
    use gitbutler_project as projects;
    use gitbutler_project::ProjectId;
    use gitbutler_virtual::assets;
    use gitbutler_virtual::base::BaseBranch;
    use gitbutler_virtual::files::RemoteBranchFile;
    use gitbutler_virtual::remote::{RemoteBranch, RemoteBranchData};
    use gitbutler_virtual::{Controller, NameConflitResolution, VirtualBranches};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::watcher;

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
        let oid = handle
            .state::<Controller>()
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
        let (branches, skipped_files) = handle
            .state::<Controller>()
            .list_virtual_branches(&project)
            .await?;

        let proxy = handle.state::<assets::Proxy>().inner().clone();
        let branches = proxy.proxy_virtual_branches(branches).await;
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
        branch: branch::BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_id = handle
            .state::<Controller>()
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
        branch: git::Refname,
    ) -> Result<BranchId, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_id = handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        if let Ok(base_branch) = handle
            .state::<Controller>()
            .get_base_branch_data(&project)
            .await
        {
            let proxy = handle.state::<assets::Proxy>().inner().clone();
            let base_branch = proxy.proxy_base_branch(base_branch).await;
            return Ok(Some(base_branch));
        }
        Ok(None)
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
        let base_branch = handle
            .state::<Controller>()
            .set_base_branch(&project, &branch_name)
            .await?;

        let proxy = handle.state::<assets::Proxy>().inner().clone();
        let base_branch = proxy.proxy_base_branch(base_branch).await;

        // if they also sent a different push remote, set that too
        if let Some(push_remote) = push_remote {
            handle
                .state::<Controller>()
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
        let unapplied_branches = handle
            .state::<Controller>()
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
        branch: branch::BranchUpdateRequest,
    ) -> Result<(), Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        branch: git::RemoteRefname,
    ) -> Result<bool, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        Ok(handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        let oid = handle
            .state::<Controller>()
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
        let oid = handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        let branches = handle
            .state::<Controller>()
            .list_remote_branches(project)
            .await?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_remote_branch_data(
        handle: tauri::AppHandle,
        project_id: ProjectId,
        refname: git::Refname,
    ) -> Result<RemoteBranchData, Error> {
        let project = handle.state::<projects::Controller>().get(project_id)?;
        let branch_data = handle
            .state::<Controller>()
            .get_remote_branch_data(&project, &refname)
            .await?;

        let proxy = handle.state::<assets::Proxy>().inner().clone();
        let branch_data = proxy.proxy_remote_branch_data(branch_data).await;
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
        handle
            .state::<Controller>()
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

        let project_data_last_fetched = handle
            .state::<Controller>()
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

        let base_branch = handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
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
        handle
            .state::<Controller>()
            .update_commit_message(&project, branch_id, commit_oid, message)
            .await?;
        emit_vbranches(&handle, project_id).await;
        Ok(())
    }

    async fn emit_vbranches(handle: &AppHandle, project_id: projects::ProjectId) {
        if let Err(error) = handle
            .state::<watcher::Watchers>()
            .post(gitbutler_watcher::Action::CalculateVirtualBranches(
                project_id,
            ))
            .await
        {
            tracing::error!(?error);
        }
    }
}
