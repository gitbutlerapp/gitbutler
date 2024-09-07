pub mod commands {
    use anyhow::{anyhow, Context};
    use gitbutler_branch::{
        BranchCreateRequest, BranchId, BranchOwnershipClaims, BranchUpdateRequest,
    };
    use gitbutler_branch_actions::{
        BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, RemoteBranch,
        RemoteBranchData, RemoteBranchFile, VirtualBranches,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_error::error::Code;
    use gitbutler_project as projects;
    use gitbutler_project::{FetchResult, ProjectId};
    use gitbutler_reference::{
        normalize_branch_name as normalize_name, ReferenceName, Refname, RemoteRefname,
    };
    use std::path::PathBuf;
    use tauri::State;
    use tracing::instrument;

    use crate::{error::Error, WindowState};

    #[tauri::command(async)]
    #[instrument(err(Debug))]
    pub fn normalize_branch_name(name: &str) -> Result<String, Error> {
        Ok(normalize_name(name)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn commit_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
        message: &str,
        ownership: Option<BranchOwnershipClaims>,
        run_hooks: bool,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let oid = gitbutler_branch_actions::create_commit(
            &project,
            branch,
            message,
            ownership.as_ref(),
            run_hooks,
        )?;
        emit_vbranches(&windows, project_id);
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn list_virtual_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<VirtualBranches, Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::list_virtual_branches(&project)
            .map_err(Into::into)
            .map(|(branches, skipped_files)| VirtualBranches {
                branches,
                skipped_files,
            })
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn create_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchCreateRequest,
    ) -> Result<BranchId, Error> {
        let project = projects.get(project_id)?;
        let branch_id = gitbutler_branch_actions::create_virtual_branch(&project, &branch)?;
        emit_vbranches(&windows, project_id);
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn delete_local_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        refname: Refname,
        given_name: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::delete_local_branch(&project, &refname, given_name)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn create_virtual_branch_from_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: Refname,
        remote: Option<RemoteRefname>,
    ) -> Result<BranchId, Error> {
        let project = projects.get(project_id)?;
        let branch_id =
            gitbutler_branch_actions::create_virtual_branch_from_branch(&project, &branch, remote)?;
        emit_vbranches(&windows, project_id);
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn integrate_upstream_commits(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::integrate_upstream_commits(&project, branch)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn get_base_branch_data(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = projects.get(project_id)?;
        if let Ok(base_branch) = gitbutler_branch_actions::get_base_branch_data(&project) {
            Ok(Some(base_branch))
        } else {
            Ok(None)
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn set_base_branch(
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
        let base_branch = gitbutler_branch_actions::set_base_branch(&project, &branch_name)?;

        // if they also sent a different push remote, set that too
        if let Some(push_remote) = push_remote {
            gitbutler_branch_actions::set_target_push_remote(&project, push_remote)?;
        }
        emit_vbranches(&windows, project_id);
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_base_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<ReferenceName>, Error> {
        let project = projects.get(project_id)?;
        let unapplied_branches = gitbutler_branch_actions::update_base_branch(&project)?;
        emit_vbranches(&windows, project_id);
        Ok(unapplied_branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchUpdateRequest,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::update_virtual_branch(&project, branch)?;

        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_branch_order(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branches: Vec<BranchUpdateRequest>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::update_branch_order(&project, branches)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn delete_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::delete_virtual_branch(&project, branch_id)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn convert_to_real_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::convert_to_real_branch(&project, branch)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn unapply_ownership(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        ownership: BranchOwnershipClaims,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::unapply_ownership(&project, &ownership)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn reset_files(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        files: Vec<PathBuf>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::reset_files(&project, branch_id, &files)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn push_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::push_virtual_branch(
            &project,
            branch_id,
            with_force,
            Some(Some(branch_id)),
        )
        .map_err(|err| err.context(Code::Unknown))?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn can_apply_remote_branch(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: RemoteRefname,
    ) -> Result<bool, Error> {
        let project = projects.get(project_id)?;
        Ok(gitbutler_branch_actions::can_apply_remote_branch(
            &project, &branch,
        )?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn list_remote_commit_files(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::list_remote_commit_files(&project, commit_oid).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn reset_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::reset_virtual_branch(&project, branch_id, target_commit_oid)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn amend_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        let oid = gitbutler_branch_actions::amend(&project, branch_id, commit_oid, &ownership)?;
        emit_vbranches(&windows, project_id);
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn move_commit_file(
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
        let oid = gitbutler_branch_actions::move_commit_file(
            &project,
            branch_id,
            from_commit_oid,
            to_commit_oid,
            &ownership,
        )?;
        emit_vbranches(&windows, project_id);
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn undo_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::undo_commit(&project, branch_id, commit_oid)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn insert_blank_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::insert_blank_commit(&project, branch_id, commit_oid, offset)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn create_change_reference(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        name: ReferenceName,
        change_id: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::create_change_reference(&project, branch_id, name, change_id)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn push_change_reference(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        name: ReferenceName,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::push_change_reference(&project, branch_id, name, with_force)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_change_reference(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        name: ReferenceName,
        new_change_id: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::update_change_reference(
            &project,
            branch_id,
            name,
            new_change_id,
        )?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn reorder_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        offset: i32,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::reorder_commit(&project, branch_id, commit_oid, offset)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn list_local_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
    ) -> Result<Vec<RemoteBranch>, Error> {
        let project = projects.get(project_id)?;
        let branches = gitbutler_branch_actions::list_local_branches(project)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn list_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        filter: Option<BranchListingFilter>,
    ) -> Result<Vec<BranchListing>, Error> {
        let ctx = CommandContext::open(&projects.get(project_id)?)?;
        let branches = gitbutler_branch_actions::list_branches(&ctx, filter, None)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn get_branch_listing_details(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_names: Vec<String>,
    ) -> Result<Vec<BranchListingDetails>, Error> {
        let ctx = CommandContext::open(&projects.get(project_id)?)?;
        let branches = gitbutler_branch_actions::get_branch_listing_details(&ctx, branch_names)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn get_remote_branch_data(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        refname: Refname,
    ) -> Result<RemoteBranchData, Error> {
        let project = projects.get(project_id)?;
        let branch_data = gitbutler_branch_actions::get_remote_branch_data(&project, &refname)?;
        Ok(branch_data)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn squash_branch_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::squash(&project, branch_id, target_commit_oid)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn fetch_from_remotes(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        action: Option<String>,
    ) -> Result<BaseBranch, Error> {
        let project = projects.get(project_id)?;

        let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
            &project,
            Some(action.unwrap_or_else(|| "unknown".to_string())),
        )?;

        // Updates the project controller with the last fetched timestamp
        //
        // TODO: This cross dependency likely indicates that last_fetched is stored in the wrong place - value is coupled with virtual branches state
        projects
            .update(&projects::UpdateRequest {
                id: project.id,
                project_data_last_fetched: Some(project_data_last_fetched.clone()),
                ..Default::default()
            })
            .context("failed to update project with last fetched timestamp")?;

        if let FetchResult::Error { error, .. } = project_data_last_fetched {
            return Err(anyhow!(error).into());
        }

        let base_branch = gitbutler_branch_actions::get_base_branch_data(&project)?;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn move_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
        target_branch_id: BranchId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::move_commit(&project, target_branch_id, commit_oid)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_commit_message(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: BranchId,
        commit_oid: String,
        message: &str,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::update_commit_message(&project, branch_id, commit_oid, message)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    fn emit_vbranches(windows: &WindowState, project_id: projects::ProjectId) {
        if let Err(error) = windows.post(gitbutler_watcher::Action::CalculateVirtualBranches(
            project_id,
        )) {
            tracing::error!(?error);
        }
    }
}
