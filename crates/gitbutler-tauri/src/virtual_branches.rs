pub mod commands {
    use anyhow::{anyhow, Context};
    use but_settings::AppSettingsWithDiskSync;
    use but_workspace::ui::StackEntryNoOpt;
    use but_workspace::DiffSpec;
    use gitbutler_branch::{BranchCreateRequest, BranchUpdateRequest};
    use gitbutler_branch_actions::branch_upstream_integration::IntegrationStrategy;
    use gitbutler_branch_actions::upstream_integration::{
        BaseBranchResolution, BaseBranchResolutionApproach, IntegrationOutcome, Resolution,
        StackStatuses,
    };
    use gitbutler_branch_actions::{
        BaseBranch, BranchListing, BranchListingDetails, BranchListingFilter, RemoteBranchData,
        RemoteBranchFile, RemoteCommit, StackOrder,
    };
    use gitbutler_command_context::CommandContext;
    use gitbutler_oxidize::ObjectIdExt;
    use gitbutler_project as projects;
    use gitbutler_project::{FetchResult, ProjectId};
    use gitbutler_reference::{normalize_branch_name as normalize_name, Refname, RemoteRefname};
    use gitbutler_stack::{BranchOwnershipClaims, StackId, VirtualBranchesHandle};
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(err(Debug))]
    pub fn normalize_branch_name(name: &str) -> Result<String, Error> {
        Ok(normalize_name(name)?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn create_virtual_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch: BranchCreateRequest,
    ) -> Result<StackEntryNoOpt, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let stack_entry = gitbutler_branch_actions::create_virtual_branch(
            &ctx,
            &branch,
            ctx.project().exclusive_worktree_access().write_permission(),
        )?;
        Ok(stack_entry)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn delete_local_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        refname: Refname,
        given_name: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        gitbutler_branch_actions::delete_local_branch(&ctx, &refname, given_name)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn create_virtual_branch_from_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch: Refname,
        remote: Option<RemoteRefname>,
        pr_number: Option<usize>,
    ) -> Result<StackId, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let branch_id = gitbutler_branch_actions::create_virtual_branch_from_branch(
            &ctx, &branch, remote, pr_number,
        )?;
        Ok(branch_id)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn integrate_upstream_commits(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        series_name: String,
        integration_strategy: Option<IntegrationStrategy>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        gitbutler_branch_actions::integrate_upstream_commits(
            &ctx,
            stack_id,
            series_name,
            integration_strategy,
        )?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn get_base_branch_data(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
    ) -> Result<Option<BaseBranch>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        if let Ok(base_branch) = gitbutler_branch_actions::base::get_base_branch_data(&ctx) {
            Ok(Some(base_branch))
        } else {
            Ok(None)
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn set_base_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch: &str,
        push_remote: Option<&str>, // optional different name of a remote to push to (defaults to same as the branch)
        stash_uncommitted: Option<bool>,
    ) -> Result<BaseBranch, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let branch_name = format!("refs/remotes/{}", branch)
            .parse()
            .context("Invalid branch name")?;
        let base_branch = gitbutler_branch_actions::set_base_branch(
            &ctx,
            &branch_name,
            stash_uncommitted.unwrap_or_default(),
            ctx.project().exclusive_worktree_access().write_permission(),
        )?;

        // if they also sent a different push remote, set that too
        if let Some(push_remote) = push_remote {
            gitbutler_branch_actions::set_target_push_remote(&ctx, push_remote)?;
        }
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn push_base_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        with_force: bool,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        gitbutler_branch_actions::push_base_branch(&ctx, with_force)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn update_stack_order(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stacks: Vec<BranchUpdateRequest>,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        gitbutler_branch_actions::update_stack_order(&ctx, stacks)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn unapply_stack(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
        let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            Some(but_core::diff::ui::worktree_changes_by_worktree_dir(project.path)?.changes),
            None,
        )?;
        let assigned_diffspec = but_workspace::flatten_diff_specs(
            assignments
                .into_iter()
                .filter(|a| a.stack_id == Some(stack_id))
                .map(|a| a.into())
                .collect::<Vec<DiffSpec>>(),
        );
        gitbutler_branch_actions::unapply_stack(ctx, stack_id, assigned_diffspec)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn can_apply_remote_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch: RemoteRefname,
    ) -> Result<bool, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        Ok(gitbutler_branch_actions::can_apply_remote_branch(
            &ctx, &branch,
        )?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn list_commit_files(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        commit_id: String,
    ) -> Result<Vec<RemoteBranchFile>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::list_commit_files(&ctx, commit_id).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn amend_virtual_branch(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        commit_id: String,
        worktree_changes: Vec<DiffSpec>,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        let oid = gitbutler_branch_actions::amend(&ctx, stack_id, commit_id, worktree_changes)?;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    #[allow(clippy::too_many_arguments)]
    pub fn move_commit_file(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        from_commit_id: String,
        to_commit_id: String,
        ownership: BranchOwnershipClaims,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let from_commit_id = git2::Oid::from_str(&from_commit_id).map_err(|e| anyhow!(e))?;
        let to_commit_id = git2::Oid::from_str(&to_commit_id).map_err(|e| anyhow!(e))?;
        let oid = gitbutler_branch_actions::move_commit_file(
            &ctx,
            stack_id,
            from_commit_id,
            to_commit_id,
            &ownership,
        )?;
        Ok(oid.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn undo_commit(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        commit_id: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::undo_commit(&ctx, stack_id, commit_id)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn insert_blank_commit(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        commit_id: Option<String>,
        offset: i32,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = match commit_id {
            Some(oid) => git2::Oid::from_str(&oid).map_err(|e| anyhow!(e))?,
            None => {
                let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
                let stack = state.get_stack(stack_id)?;
                let gix_repo = ctx.gix_repo()?;
                stack.head_oid(&gix_repo)?.to_git2()
            }
        };
        gitbutler_branch_actions::insert_blank_commit(&ctx, stack_id, commit_id, offset, None)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn reorder_stack(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        stack_order: StackOrder,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        gitbutler_branch_actions::reorder_stack(&ctx, stack_id, stack_order)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn find_git_branches(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch_name: &str,
    ) -> Result<Vec<RemoteBranchData>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let branches = gitbutler_branch_actions::find_git_branches(&ctx, branch_name)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn list_branches(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        filter: Option<BranchListingFilter>,
    ) -> Result<Vec<BranchListing>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let branches = gitbutler_branch_actions::list_branches(&ctx, filter, None)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn get_branch_listing_details(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        branch_names: Vec<String>,
    ) -> Result<Vec<BranchListingDetails>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let branches = gitbutler_branch_actions::get_branch_listing_details(&ctx, branch_names)?;
        Ok(branches)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn squash_commits(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        source_commit_ids: Vec<String>,
        target_commit_id: String,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let source_commit_ids: Vec<git2::Oid> = source_commit_ids
            .into_iter()
            .map(|oid| git2::Oid::from_str(&oid))
            .collect::<Result<_, _>>()
            .map_err(|e| anyhow!(e))?;
        let destination_commit_id =
            git2::Oid::from_str(&target_commit_id).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::squash_commits(
            &ctx,
            stack_id,
            source_commit_ids,
            destination_commit_id,
        )?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn fetch_from_remotes(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        action: Option<String>,
    ) -> Result<BaseBranch, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;

        let project_data_last_fetched = gitbutler_branch_actions::fetch_from_remotes(
            &ctx,
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

        let base_branch = gitbutler_branch_actions::base::get_base_branch_data(&ctx)?;
        Ok(base_branch)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn move_commit(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        commit_id: String,
        target_stack_id: StackId,
        source_stack_id: StackId,
    ) -> Result<(), Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::move_commit(&ctx, target_stack_id, commit_id, source_stack_id)?;
        Ok(())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn update_commit_message(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        stack_id: StackId,
        commit_id: String,
        message: &str,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        let new_commit_id =
            gitbutler_branch_actions::update_commit_message(&ctx, stack_id, commit_id, message)?;
        Ok(new_commit_id.to_string())
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn find_commit(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        commit_id: String,
    ) -> Result<Option<RemoteCommit>, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e))?;
        gitbutler_branch_actions::find_commit(&ctx, commit_id).map_err(Into::into)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn upstream_integration_statuses(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        target_commit_id: Option<String>,
    ) -> Result<StackStatuses, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let commit_id = target_commit_id
            .map(|commit_id| git2::Oid::from_str(&commit_id).map_err(|e| anyhow!(e)))
            .transpose()?;
        Ok(gitbutler_branch_actions::upstream_integration_statuses(
            &ctx, commit_id,
        )?)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn integrate_upstream(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        resolutions: Vec<Resolution>,
        base_branch_resolution: Option<BaseBranchResolution>,
    ) -> Result<IntegrationOutcome, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;
        let outcome = gitbutler_branch_actions::integrate_upstream(
            &ctx,
            &resolutions,
            base_branch_resolution,
        )?;

        Ok(outcome)
    }

    #[tauri::command(async)]
    #[instrument(skip(projects, settings), err(Debug))]
    pub fn resolve_upstream_integration(
        projects: State<'_, projects::Controller>,
        settings: State<'_, AppSettingsWithDiskSync>,
        project_id: ProjectId,
        resolution_approach: BaseBranchResolutionApproach,
    ) -> Result<String, Error> {
        let project = projects.get(project_id)?;
        let ctx = CommandContext::open(&project, settings.get()?.clone())?;

        let new_target_id =
            gitbutler_branch_actions::resolve_upstream_integration(&ctx, resolution_approach)?;
        let commit_id = git2::Oid::to_string(&new_target_id);
        Ok(commit_id)
    }
}
