use but_api::commands::workspace::{
    self, AmendCommitFromWorktreeChangesParams, BranchDetailsParams, CannedBranchNameParams,
    CreateCommitFromWorktreeChangesParams, DiscardWorktreeChangesParams,
    MoveChangesBetweenCommitsParams, ShowGraphSvgParams, SplitBranchIntoDependentBranchParams,
    SplitBranchParams, StackDetailsParams, StacksParams, StashIntoBranchParams,
    TargetCommitsParams, UIMoveChangesResult, UncommitChangesParams,
};
use but_api::error::Error;
use but_api::hex_hash::HexHash;
use but_workspace::StacksFilter;
use but_workspace::{commit_engine, ui::StackEntry};
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stacks(
    project_id: ProjectId,
    filter: Option<StacksFilter>,
) -> Result<Vec<StackEntry>, Error> {
    workspace::stacks(StacksParams { project_id, filter })
}

#[cfg(unix)]
#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn show_graph_svg(project_id: ProjectId) -> Result<(), Error> {
    workspace::show_graph_svg(ShowGraphSvgParams { project_id })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stack_details(
    project_id: ProjectId,
    stack_id: Option<StackId>,
) -> Result<but_workspace::ui::StackDetails, Error> {
    workspace::stack_details(StackDetailsParams {
        project_id,
        stack_id,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn branch_details(
    project_id: ProjectId,
    branch_name: String,
    remote: Option<String>,
) -> Result<but_workspace::ui::BranchDetails, Error> {
    workspace::branch_details(BranchDetailsParams {
        project_id,
        branch_name,
        remote,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn create_commit_from_worktree_changes(
    project_id: ProjectId,
    stack_id: StackId,
    parent_id: Option<HexHash>,
    worktree_changes: Vec<but_workspace::DiffSpec>,
    message: String,
    stack_branch_name: String,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    workspace::create_commit_from_worktree_changes(CreateCommitFromWorktreeChangesParams {
        project_id,
        stack_id,
        parent_id,
        worktree_changes,
        message,
        stack_branch_name,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn amend_commit_from_worktree_changes(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    workspace::amend_commit_from_worktree_changes(AmendCommitFromWorktreeChangesParams {
        project_id,
        stack_id,
        commit_id,
        worktree_changes,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn discard_worktree_changes(
    project_id: ProjectId,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<Vec<but_workspace::DiffSpec>, Error> {
    workspace::discard_worktree_changes(DiscardWorktreeChangesParams {
        project_id,
        worktree_changes,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn move_changes_between_commits(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_commit_id: HexHash,
    destination_stack_id: StackId,
    destination_commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
) -> Result<UIMoveChangesResult, Error> {
    workspace::move_changes_between_commits(MoveChangesBetweenCommitsParams {
        project_id,
        source_stack_id,
        source_commit_id,
        destination_stack_id,
        destination_commit_id,
        changes,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn split_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: Vec<String>,
) -> Result<UIMoveChangesResult, Error> {
    workspace::split_branch(SplitBranchParams {
        project_id,
        source_stack_id,
        source_branch_name,
        new_branch_name,
        file_changes_to_split_off,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn split_branch_into_dependent_branch(
    project_id: ProjectId,
    source_stack_id: StackId,
    source_branch_name: String,
    new_branch_name: String,
    file_changes_to_split_off: Vec<String>,
) -> Result<UIMoveChangesResult, Error> {
    workspace::split_branch_into_dependent_branch(SplitBranchIntoDependentBranchParams {
        project_id,
        source_stack_id,
        source_branch_name,
        new_branch_name,
        file_changes_to_split_off,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn uncommit_changes(
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    changes: Vec<but_workspace::DiffSpec>,
    assign_to: Option<StackId>,
) -> Result<UIMoveChangesResult, Error> {
    workspace::uncommit_changes(UncommitChangesParams {
        project_id,
        stack_id,
        commit_id,
        changes,
        assign_to,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn stash_into_branch(
    project_id: ProjectId,
    branch_name: String,
    worktree_changes: Vec<but_workspace::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    workspace::stash_into_branch(StashIntoBranchParams {
        project_id,
        branch_name,
        worktree_changes,
    })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn canned_branch_name(project_id: ProjectId) -> Result<String, Error> {
    workspace::canned_branch_name(CannedBranchNameParams { project_id })
}

#[tauri::command(async)]
#[instrument(err(Debug))]
pub fn target_commits(
    project_id: ProjectId,
    last_commit_id: Option<HexHash>,
    page_size: Option<usize>,
) -> Result<Vec<but_workspace::ui::Commit>, Error> {
    workspace::target_commits(TargetCommitsParams {
        project_id,
        last_commit_id,
        page_size,
    })
}
