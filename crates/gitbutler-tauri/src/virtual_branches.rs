use but_api::{commands::virtual_branches, App};
use but_graph::virtual_branches_legacy_types::BranchOwnershipClaims;
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
use gitbutler_project::ProjectId;
use gitbutler_reference::{Refname, RemoteRefname};
use gitbutler_stack::StackId;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn normalize_branch_name(app: State<App>, name: String) -> Result<String, Error> {
    virtual_branches::normalize_branch_name(&app, name)
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn create_virtual_branch(
    app: State<App>,
    project_id: ProjectId,
    branch: BranchCreateRequest,
) -> Result<StackEntryNoOpt, Error> {
    virtual_branches::create_virtual_branch(
        &app,
        virtual_branches::CreateVirtualBranchParams { project_id, branch },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn delete_local_branch(
    app: State<App>,
    project_id: ProjectId,
    refname: Refname,
    given_name: String,
) -> Result<(), Error> {
    virtual_branches::delete_local_branch(
        &app,
        virtual_branches::DeleteLocalBranchParams {
            project_id,
            refname,
            given_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn create_virtual_branch_from_branch(
    app: State<App>,
    project_id: ProjectId,
    branch: Refname,
    remote: Option<RemoteRefname>,
    pr_number: Option<usize>,
) -> Result<StackId, Error> {
    virtual_branches::create_virtual_branch_from_branch(
        &app,
        virtual_branches::CreateVirtualBranchFromBranchParams {
            project_id,
            branch,
            remote,
            pr_number,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn integrate_upstream_commits(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    series_name: String,
    integration_strategy: Option<IntegrationStrategy>,
) -> Result<(), Error> {
    virtual_branches::integrate_upstream_commits(
        &app,
        virtual_branches::IntegrateUpstreamCommitsParams {
            project_id,
            stack_id,
            series_name,
            integration_strategy,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_base_branch_data(
    app: State<App>,
    project_id: ProjectId,
) -> Result<Option<BaseBranch>, Error> {
    virtual_branches::get_base_branch_data(
        &app,
        virtual_branches::GetBaseBranchDataParams { project_id },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn set_base_branch(
    app: State<App>,
    project_id: ProjectId,
    branch: String,
    push_remote: Option<String>,
    stash_uncommitted: Option<bool>,
) -> Result<BaseBranch, Error> {
    virtual_branches::set_base_branch(
        &app,
        virtual_branches::SetBaseBranchParams {
            project_id,
            branch,
            push_remote,
            stash_uncommitted,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn push_base_branch(
    app: State<App>,
    project_id: ProjectId,
    with_force: bool,
) -> Result<(), Error> {
    virtual_branches::push_base_branch(
        &app,
        virtual_branches::PushBaseBranchParams {
            project_id,
            with_force,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_stack_order(
    app: State<App>,
    project_id: ProjectId,
    stacks: Vec<BranchUpdateRequest>,
) -> Result<(), Error> {
    virtual_branches::update_stack_order(
        &app,
        virtual_branches::UpdateStackOrderParams { project_id, stacks },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn unapply_stack(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<(), Error> {
    virtual_branches::unapply_stack(
        &app,
        virtual_branches::UnapplyStackParams {
            project_id,
            stack_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn can_apply_remote_branch(
    app: State<App>,
    project_id: ProjectId,
    branch: RemoteRefname,
) -> Result<bool, Error> {
    virtual_branches::can_apply_remote_branch(
        &app,
        virtual_branches::CanApplyRemoteBranchParams { project_id, branch },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn list_commit_files(
    app: State<App>,
    project_id: ProjectId,
    commit_id: String,
) -> Result<Vec<RemoteBranchFile>, Error> {
    virtual_branches::list_commit_files(
        &app,
        virtual_branches::ListCommitFilesParams {
            project_id,
            commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn amend_virtual_branch(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    worktree_changes: Vec<DiffSpec>,
) -> Result<String, Error> {
    virtual_branches::amend_virtual_branch(
        &app,
        virtual_branches::AmendVirtualBranchParams {
            project_id,
            stack_id,
            commit_id,
            worktree_changes,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn move_commit_file(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    from_commit_id: String,
    to_commit_id: String,
    ownership: BranchOwnershipClaims,
) -> Result<String, Error> {
    virtual_branches::move_commit_file(
        &app,
        virtual_branches::MoveCommitFileParams {
            project_id,
            stack_id,
            from_commit_id,
            to_commit_id,
            ownership,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn undo_commit(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
) -> Result<(), Error> {
    virtual_branches::undo_commit(
        &app,
        virtual_branches::UndoCommitParams {
            project_id,
            stack_id,
            commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn insert_blank_commit(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: Option<String>,
    offset: i32,
) -> Result<(), Error> {
    virtual_branches::insert_blank_commit(
        &app,
        virtual_branches::InsertBlankCommitParams {
            project_id,
            stack_id,
            commit_id,
            offset,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn reorder_stack(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    stack_order: StackOrder,
) -> Result<(), Error> {
    virtual_branches::reorder_stack(
        &app,
        virtual_branches::ReorderStackParams {
            project_id,
            stack_id,
            stack_order,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn find_git_branches(
    app: State<App>,
    project_id: ProjectId,
    branch_name: String,
) -> Result<Vec<RemoteBranchData>, Error> {
    virtual_branches::find_git_branches(
        &app,
        virtual_branches::FindGitBranchesParams {
            project_id,
            branch_name,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn list_branches(
    app: State<App>,
    project_id: ProjectId,
    filter: Option<BranchListingFilter>,
) -> Result<Vec<BranchListing>, Error> {
    virtual_branches::list_branches(
        &app,
        virtual_branches::ListBranchesParams { project_id, filter },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_branch_listing_details(
    app: State<App>,
    project_id: ProjectId,
    branch_names: Vec<String>,
) -> Result<Vec<BranchListingDetails>, Error> {
    virtual_branches::get_branch_listing_details(
        &app,
        virtual_branches::GetBranchListingDetailsParams {
            project_id,
            branch_names,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn squash_commits(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    source_commit_ids: Vec<String>,
    target_commit_id: String,
) -> Result<(), Error> {
    virtual_branches::squash_commits(
        &app,
        virtual_branches::SquashCommitsParams {
            project_id,
            stack_id,
            source_commit_ids,
            target_commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn fetch_from_remotes(
    app: State<App>,
    project_id: ProjectId,
    action: Option<String>,
) -> Result<BaseBranch, Error> {
    virtual_branches::fetch_from_remotes(
        &app,
        virtual_branches::FetchFromRemotesParams { project_id, action },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn move_commit(
    app: State<App>,
    project_id: ProjectId,
    commit_id: String,
    target_stack_id: StackId,
    source_stack_id: StackId,
) -> Result<(), Error> {
    virtual_branches::move_commit(
        &app,
        virtual_branches::MoveCommitParams {
            project_id,
            commit_id,
            target_stack_id,
            source_stack_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn update_commit_message(
    app: State<App>,
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: String,
    message: String,
) -> Result<String, Error> {
    virtual_branches::update_commit_message(
        &app,
        virtual_branches::UpdateCommitMessageParams {
            project_id,
            stack_id,
            commit_id,
            message,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn find_commit(
    app: State<App>,
    project_id: ProjectId,
    commit_id: String,
) -> Result<Option<RemoteCommit>, Error> {
    virtual_branches::find_commit(
        &app,
        virtual_branches::FindCommitParams {
            project_id,
            commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn upstream_integration_statuses(
    app: State<App>,
    project_id: ProjectId,
    target_commit_id: Option<String>,
) -> Result<StackStatuses, Error> {
    virtual_branches::upstream_integration_statuses(
        &app,
        virtual_branches::UpstreamIntegrationStatusesParams {
            project_id,
            target_commit_id,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn integrate_upstream(
    app: State<App>,
    project_id: ProjectId,
    resolutions: Vec<Resolution>,
    base_branch_resolution: Option<BaseBranchResolution>,
) -> Result<IntegrationOutcome, Error> {
    virtual_branches::integrate_upstream(
        &app,
        virtual_branches::IntegrateUpstreamParams {
            project_id,
            resolutions,
            base_branch_resolution,
        },
    )
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn resolve_upstream_integration(
    app: State<App>,
    project_id: ProjectId,
    resolution_approach: BaseBranchResolutionApproach,
) -> Result<String, Error> {
    virtual_branches::resolve_upstream_integration(
        &app,
        virtual_branches::ResolveUpstreamIntegrationParams {
            project_id,
            resolution_approach,
        },
    )
}
