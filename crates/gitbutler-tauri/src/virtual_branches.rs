pub mod commands {
    use crate::error::Error;
    use anyhow::{anyhow, Context};
    use gitbutler_branch::BranchOwnershipClaims;
    use gitbutler_branch::{BranchCreateRequest, BranchId, BranchUpdateRequest};
    use gitbutler_branch_actions::RemoteBranchFile;
    use gitbutler_branch_actions::{BaseBranch, BranchListing};
    use gitbutler_branch_actions::{NameConflictResolution, VirtualBranchActions, VirtualBranches};
    use gitbutler_branch_actions::{RemoteBranch, RemoteBranchData};
    use gitbutler_command_context::ProjectRepository;
    use gitbutler_error::error::Code;
    use gitbutler_project as projects;
    use gitbutler_project::{FetchResult, ProjectId};
    use gitbutler_reference::normalize_branch_name as normalize_name;
    use gitbutler_reference::ReferenceName;
    use gitbutler_reference::{Refname, RemoteRefname};
    use tauri::State;
    use tracing::instrument;

    use crate::WindowState;

    #[tauri::command(async)]
    #[instrument(err(Debug))]
    pub async fn normalize_branch_name(name: &str) -> Result<String, Error> {
        Ok(normalize_name(name))
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn commit_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
        message: &str,
        ownership: Option<BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let oid = VirtualBranchActions
            .create_commit(&project, branch, message, ownership.as_ref(), run_hooks)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn list_virtual_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<VirtualBranches, Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .list_virtual_branches(&project)
            .await
            .map_err(Into::into)
            .map(|(branches, skipped_files)| VirtualBranches {
                branches,
                skipped_files,
            })
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn create_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let project = projects.get(project_id)?;
        let branch_id = VirtualBranchActions
            .create_virtual_branch(&project, &branch)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn create_virtual_branch_from_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: Refname,
        remote: Option<RemoteRefname>,
    ) -> Result<BranchId, Error> {
        let project = projects.get(project_id)?;
        let branch_id = VirtualBranchActions
            .create_virtual_branch_from_branch(&project, &branch, remote)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn integrate_upstream_commits(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .integrate_upstream_commits(&project, branch)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn get_base_branch_data(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = projects.get(project_id)?;
        if let Ok(base_branch) = VirtualBranchActions::get_base_branch_data(&project).await {
            Ok(Some(base_branch))
        } else {
            Ok(None)
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn set_base_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: &str,
        push_remote: Option<&str>, // optional different name of a remote to push to (defaults to same as the branch)
    ) -> Result<BaseBranch, Error> {
        let project = projects.get(project_id)?;
        let branch_name = format!("refs/remotes/{}", branch)
            .parse()
            .context("Invalid branch name")?;
        let base_branch = VirtualBranchActions
            .set_base_branch(&project, &branch_name)
            .await?;

        // if they also sent a different push remote, set that too
        if let Some(push_remote) = push_remote {
            VirtualBranchActions
                .set_target_push_remote(&project, push_remote)
                .await?;
        }
        emit_vbranches(&windows, project_id).await;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn update_base_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<ReferenceName>, Error> {
        let project = projects.get(project_id)?;
        let unapplied_branches = VirtualBranchActions.update_base_branch(&project).await?;
        emit_vbranches(&windows, project_id).await;
        Ok(unapplied_branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn update_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchUpdateRequest,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .update_virtual_branch(&project, branch)
            .await?;

        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn delete_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .delete_virtual_branch(&project, branch_id)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn convert_to_real_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
        name_conflict_resolution: NameConflictResolution,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .convert_to_real_branch(&project, branch, name_conflict_resolution)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn unapply_ownership(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        ownership: BranchOwnershipClaims,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .unapply_ownership(&project, &ownership)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn reset_files(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        files: &str,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        // convert files to Vec<String>
        let files = files
            .split('\n')
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>();
        VirtualBranchActions.reset_files(&project, &files).await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn push_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        VirtualBranchActions
            .push_virtual_branch(&project, branch_id, with_force, Some(Some(branch_id)))
            .await
            .map_err(|err| err.context(Code::Unknown))?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn can_apply_remote_branch(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: RemoteRefname,
    ) -> Result<bool, Error> {
        let project = projects.get(project_id)?;
        Ok(VirtualBranchActions
            .can_apply_remote_branch(&project, &branch)
            .await?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn list_remote_commit_files(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .list_remote_commit_files(&project, commit_oid)
            .await
            .map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn reset_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .reset_virtual_branch(&project, branch_id, target_commit_oid)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn amend_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        let oid = VirtualBranchActions
            .amend(&project, branch_id, commit_oid, &ownership)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn move_commit_file(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        from_commit_oid: String,
        to_commit_oid: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let from_commit_oid = git2::Oid::from_str(&from_commit_oid).map_err(|e| anyhow!(e))?;
        let to_commit_oid = git2::Oid::from_str(&to_commit_oid).map_err(|e| anyhow!(e))?;
        let oid = VirtualBranchActions
            .move_commit_file(
                &project,
                branch_id,
                from_commit_oid,
                to_commit_oid,
                &ownership,
            )
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn undo_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .undo_commit(&project, branch_id, commit_oid)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn insert_blank_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .insert_blank_commit(&project, branch_id, commit_oid, offset)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn reorder_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .reorder_commit(&project, branch_id, commit_oid, offset)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn list_remote_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<RemoteBranch>, Error> {
        let project = projects.get(project_id)?;
        let branches = VirtualBranchActions::list_remote_branches(project).await?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn list_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<BranchListing>, Error> {
        let ctx = ProjectRepository::open(&projects.get(project_id)?)?;
        let branches = gitbutler_branch_actions::list_branches(&ctx)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn get_remote_branch_data(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        refname: Refname,
    ) -> Result<RemoteBranchData, Error> {
        let project = projects.get(project_id)?;
        let branch_data = VirtualBranchActions
            .get_remote_branch_data(&project, &refname)
            .await?;
        Ok(branch_data)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn squash_branch_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .squash(&project, branch_id, target_commit_oid)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub async fn fetch_from_remotes(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        action: Option<String>,
    ) -> Result<BaseBranch, Error> {
        let project = projects.get(project_id)?;

        let project_data_last_fetched = VirtualBranchActions
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
                project_data_last_fetched: Some(project_data_last_fetched.clone()),
                ..Default::default()
            })
            .await
            .context("failed to update project with last fetched timestamp")?;

        if let FetchResult::Error { error, .. } = project_data_last_fetched {
            return Err(anyhow!(error).into());
        }

        let base_branch = VirtualBranchActions::get_base_branch_data(&project).await?;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn move_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
        target_branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .move_commit(&project, target_branch_id, commit_oid)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub async fn update_commit_message(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        message: &str,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        VirtualBranchActions
            .update_commit_message(&project, branch_id, commit_oid, message)
            .await?;
        emit_vbranches(&windows, project_id).await;
        Ok(())
    }

    async fn emit_vbranches(windows: &WindowState, project_id: projects::ProjectId) {
        if let Err(error) = windows
            .post(gitbutler_watcher::Action::CalculateVirtualBranches(
                project_id,
            ))
            .await
        {
            tracing::error!(?error);
        }
    }
}
