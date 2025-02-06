use crate::error::Error;
use crate::from_json::HexHash;
use but_hunk_dependency::ui::{
    hunk_dependencies_for_workspace_changes_by_worktree_dir, HunkDependencies,
};
use but_workspace::commit_engine::RefHandling;
use but_workspace::{commit_engine, StackEntry};
use gitbutler_command_context::CommandContext;
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_settings::AppSettingsWithDiskSync;
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
/// If `parent_id` is `None`, there is not a single commit as the repository is unborn.
/// All `changes` are meant to be relative to the worktree.
/// Note that submodules *must* be provided as diffspec without hunks, as attempting to generate
/// hunks would fail.
#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn create_commit_from_worktree_changes(
    projects: State<'_, projects::Controller>,
    project_id: ProjectId,
    parent_id: Option<HexHash>,
    worktree_changes: Vec<commit_engine::ui::DiffSpec>,
    message: String,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = projects.get(project_id)?;
    let repo = gix::open(project.worktree_path()).map_err(anyhow::Error::from)?;
    let out = commit_engine::create_commit(
        &repo,
        commit_engine::Destination::ParentForNewCommit(parent_id.map(Into::into)),
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        &message,
        3, /* context-lines */
        RefHandling::UpdateHEADRefForTipCommits,
    )?;
    Ok(out.into())
}
