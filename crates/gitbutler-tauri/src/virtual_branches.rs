pub mod commands {
    use anyhow::{anyhow, Context};
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
    use gitbutler_branch_actions::branch_upstream_integration::IntegrationStrategy;
    use gitbutler_branch_actions::internal::{PushResult, StackListResult};
    use gitbutler_branch_actions::upstream_integration::{
        BaseBranchResolution, BaseBranchResolutionApproach, Resolution, StackStatuses,
    };
    use gitbutler_branch_actions::{
        BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, RemoteBranchData,
        RemoteBranchFile, RemoteCommit, StackOrder, VirtualBranchHunkRangeMap, VirtualBranches,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_project as projects;
    use gitbutler_project::{FetchResult, ProjectId};
    use gitbutler_reference::{normalize_branch_name as normalize_name, Refname, RemoteRefname};
    use gitbutler_stack::{BranchOwnershipClaims, StackId};
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
        branch: StackId,
        message: &str,
        ownership: Option<BranchOwnershipClaims>,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let oid =
            gitbutler_branch_actions::create_commit(&project, branch, message, ownership.as_ref())?;
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
            .map(
                |StackListResult {
                     branches,
                     skipped_files,
                     dependency_errors,
                 }| VirtualBranches {
                    branches,
                    skipped_files,
                    dependency_errors,
                },
            )
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn create_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: BranchCreateRequest,
    ) -> Result<StackId, Error> {
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
        pr_number: Option<usize>,
    ) -> Result<StackId, Error> {
        let project = projects.get(project_id)?;
        let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
            &project, &branch, remote, pr_number,
        )?;
        emit_vbranches(&windows, project_id);
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn integrate_upstream_commits(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: StackId,
        series_name: String,
        integration_strategy: Option<IntegrationStrategy>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::integrate_upstream_commits(
            &project,
            branch,
            series_name,
            integration_strategy,
        )?;
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
    pub fn push_base_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::push_base_branch(&project, with_force)?;
        emit_vbranches(&windows, project_id);
        Ok(())
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
    pub fn unapply_without_saving_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::unapply_without_saving_virtual_branch(&project, branch_id)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn save_and_unapply_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch: StackId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::save_and_unapply_virutal_branch(&project, branch)?;
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
    pub fn unapply_lines(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        ownership: BranchOwnershipClaims,
        lines: VirtualBranchHunkRangeMap,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::unapply_lines(&project, &ownership, lines)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn reset_files(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
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
        branch_id: StackId,
        with_force: bool,
    ) -> Result<PushResult, Error> {
        let project = projects.get(project_id)?;
        let upstream_refname = gitbutler_branch_actions::push_virtual_branch(
            &project,
            branch_id,
            with_force,
            Some(Some(branch_id)),
        )?;
        emit_vbranches(&windows, project_id);
        Ok(upstream_refname)
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
    pub fn list_commit_files(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::list_commit_files(&project, commit_oid).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn reset_virtual_branch(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
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
        branch_id: StackId,
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
        branch_id: StackId,
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
        branch_id: StackId,
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
        branch_id: StackId,
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
    pub fn reorder_stack(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
        stack_order: StackOrder,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::reorder_stack(&project, branch_id, stack_order)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn find_git_branches(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_name: &str,
    ) -> Result<Vec<RemoteBranchData>, Error> {
        let project = projects.get(project_id)?;
        let branches = gitbutler_branch_actions::find_git_branches(project, branch_name)?;
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
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn squash_branch_commit(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
        target_commit_oid: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let target_commit_oid = git2::Oid::from_str(&target_commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::squash(&project, branch_id, target_commit_oid)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn fetch_from_remotes(
        windows: State<'_, WindowState>,
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

        emit_vbranches(&windows, project_id);
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
        target_branch_id: StackId,
        source_branch_id: StackId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::move_commit(
            &project,
            target_branch_id,
            commit_oid,
            source_branch_id,
        )?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn update_commit_message(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        branch_id: StackId,
        commit_oid: String,
        message: &str,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::update_commit_message(&project, branch_id, commit_oid, message)?;
        emit_vbranches(&windows, project_id);
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn find_commit(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        commit_oid: String,
    ) -> Result<Option<RemoteCommit>, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = git2::Oid::from_str(&commit_oid).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::find_commit(&project, commit_oid).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn upstream_integration_statuses(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        target_commit_oid: Option<String>,
    ) -> Result<StackStatuses, Error> {
        let project = projects.get(project_id)?;
        let commit_oid = target_commit_oid
            .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
            .transpose()?;
        Ok(gitbutler_branch_actions::upstream_integration_statuses(
            &project, commit_oid,
        )?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, windows), err(Debug))]
    pub fn integrate_upstream(
        windows: State<'_, WindowState>,
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        resolutions: Vec<Resolution>,
        base_branch_resolution: Option<BaseBranchResolution>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        gitbutler_branch_actions::integrate_upstream(
            &project,
            &resolutions,
            base_branch_resolution,
        )?;

        emit_vbranches(&windows, project_id);

        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects), err(Debug))]
    pub fn resolve_upstream_integration(
        projects: State<'_, projects::Controller>,
        project_id: ProjectId,
        resolution_approach: BaseBranchResolutionApproach,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;

        let new_target_id =
            gitbutler_branch_actions::resolve_upstream_integration(&project, resolution_approach)?;
        let commit_id = git2::Oid::to_string(&new_target_id);
        Ok(commit_id)
    }

    pub(crate) fn emit_vbranches(windows: &WindowState, project_id: projects::ProjectId) {
        if let Err(error) = windows.post(gitbutler_watcher::Action::CalculateVirtualBranches(
            project_id,
        )) {
            tracing::error!(?error);
        }
    }
}
