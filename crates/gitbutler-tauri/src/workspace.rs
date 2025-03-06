use crate::error::Error;
use crate::from_json::HexHash;
use but_hunk_dependency::ui::{
    hunk_dependencies_for_workspace_changes_by_worktree_dir, HunkDependencies,
};
use but_settings::AppSettingsWithDiskSync;
use but_workspace::{commit_engine, StackEntry};
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_stack::StackId;
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn stacks(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<Vec<StackEntry>, Error> {
    let project = projects.get(project_id)?;
    but_workspace::stacks(&project.gb_dir()).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_branches(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: String,
) -> Result<Vec<but_workspace::Branch>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::stack_branches(stack_id, &ctx).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_branch_local_and_remote_commits(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: String,
    branch_name: String,
) -> Result<Vec<but_workspace::Commit>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let repo = ctx.gix_repository()?;
    but_workspace::stack_branch_local_and_remote_commits(stack_id, branch_name, &ctx, &repo)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_branch_upstream_only_commits(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: String,
    branch_name: String,
) -> Result<Vec<but_workspace::UpstreamCommit>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let repo = ctx.gix_repository()?;
    but_workspace::stack_branch_upstream_only_commits(stack_id, branch_name, &ctx, &repo)
        .map_err(Into::into)
}

/// Retrieve all changes in the workspace and associate them with commits in the Workspace of `project_id`.
/// NOTE: right now there is no way to keep track of unassociated hunks.
// TODO: This probably has to change a lot once it's clear how the UI is going to use it.
//       Right now this is only a port from the V2 UI, and that data structure was never used directly.
#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn hunk_dependencies_for_workspace_changes(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
) -> Result<HunkDependencies, Error> {
    let project = projects.get(project_id)?;
    let dependencies =
        hunk_dependencies_for_workspace_changes_by_worktree_dir(&project.path, &project.gb_dir())?;
    Ok(dependencies)
}

/// Create a new commit with `message` on top of `parent_id` that contains all `changes`.
/// If `parent_id` is `None`, this API will infer the parent to be the head of the provided `stack_branch_name`.
/// `stack_id` is the stack that contains the `parent_id`, and it's fatal if that's not the case.
/// All `changes` are meant to be relative to the worktree.
/// Note that submodules *must* be provided as diffspec without hunks, as attempting to generate
/// hunks would fail.
/// `stack_branch_name` is the short name of the reference that the UI knows is present in a given segment.
/// It is needed to insert the new commit into the right bucket.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
#[allow(clippy::too_many_arguments)]
pub fn create_commit_from_worktree_changes(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    parent_id: Option<HexHash>,
    worktree_changes: Vec<commit_engine::ui::DiffSpec>,
    message: String,
    stack_branch_name: String,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = projects.get(project_id)?;
    let repo = but_core::open_repo_for_merging(&project.worktree_path())?;
    // If parent_id was not set but a stack branch name was provided, pick the current head of that branch as parent.
    let parent_commit_id: Option<gix::ObjectId> = match parent_id {
        Some(id) => Some(id.into()),
        None => {
            let reference = repo
                .try_find_reference(&stack_branch_name)
                .map_err(anyhow::Error::from)?;
            if let Some(mut r) = reference {
                Some(r.peel_to_commit().map_err(anyhow::Error::from)?.id)
            } else {
                None
            }
        }
    };
    let guard = project.exclusive_worktree_access();
    Ok(commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::NewCommit {
            parent_commit_id,
            message,
            stack_segment_ref: Some(
                format!("refs/heads/{stack_branch_name}")
                    .try_into()
                    .map_err(anyhow::Error::from)?,
            ),
        },
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        settings.get()?.context_lines,
        guard,
    )?
    .into())
}

/// Amend all `changes` to `commit_id`, keeping its commit message exactly as is.
/// `stack_id` is the stack that contains the `commit_id`, and it's fatal if that's not the case.
/// All `changes` are meant to be relative to the worktree.
/// Note that submodules *must* be provided as diffspec without hunks, as attempting to generate
/// hunks would fail.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn amend_commit_from_worktree_changes(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    commit_id: HexHash,
    worktree_changes: Vec<commit_engine::ui::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = projects.get(project_id)?;
    let mut guard = project.exclusive_worktree_access();
    let repo = but_core::open_repo_for_merging(&project.worktree_path())?;
    Ok(commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::AmendCommit(commit_id.into()),
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        settings.get()?.context_lines,
        guard.write_permission(),
    )?
    .into())
}
