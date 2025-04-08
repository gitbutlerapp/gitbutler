use crate::error::Error;
use crate::from_json::HexHash;
use anyhow::Context;
use but_hunk_dependency::ui::{
    hunk_dependencies_for_workspace_changes_by_worktree_dir, HunkDependencies,
};
use but_settings::AppSettingsWithDiskSync;
use but_workspace::commit_engine::StackSegmentId;
use but_workspace::{commit_engine, StackEntry};
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{OplogExt, SnapshotExt};
use gitbutler_project as projects;
use gitbutler_project::ProjectId;
use gitbutler_stack::{StackId, VirtualBranchesHandle};
use tauri::State;
use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stacks(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<Vec<StackEntry>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let repo = ctx.gix_repo()?;
    but_workspace::stacks(&project.gb_dir(), &repo).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_info(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<but_workspace::StackDetails, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::stack_info(&project.gb_dir(), stack_id, &ctx).map_err(Into::into)
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
    let repo = ctx.gix_repo()?;
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
    let repo = ctx.gix_repo()?;
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
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = project.prepare_snapshot(guard.read_permission());
    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::NewCommit {
            parent_commit_id,
            message: message.clone(),
            stack_segment: Some(StackSegmentId {
                stack_id,
                segment_ref: format!("refs/heads/{stack_branch_name}")
                    .try_into()
                    .map_err(anyhow::Error::from)?,
            }),
        },
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        settings.get()?.context_lines,
        guard.write_permission(),
    );

    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &ctx)
        .context("failed to update gitbutler workspace")?;

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        project.snapshot_commit_creation(
            snapshot_tree,
            outcome.as_ref().err(),
            message.to_owned(),
            None,
            guard.write_permission(),
        )
    });

    let outcome = outcome?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(outcome.into())
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
    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack_id),
        commit_engine::Destination::AmendCommit(commit_id.into()),
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        settings.get()?.context_lines,
        guard.write_permission(),
    )?;
    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(?outcome.rejected_specs, "Failed to commit at least one hunk");
    }
    Ok(outcome.into())
}

/// Discard all worktree changes that match the specs in `worktree_changes`.
///
/// If whole files should be discarded, be sure to not pass any [hunks](but_workspace::discard::ui::DiscardSpec::hunk_headers)
///
/// Returns the `worktree_changes` that couldn't be applied,
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn discard_worktree_changes(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    worktree_changes: Vec<but_workspace::discard::ui::DiscardSpec>,
) -> Result<Vec<but_workspace::discard::ui::DiscardSpec>, Error> {
    let project = projects.get(project_id)?;
    let repo = but_core::open_repo(&project.worktree_path())?;
    let _guard = project.exclusive_worktree_access();

    let refused = but_workspace::discard_workspace_changes(
        &repo,
        worktree_changes.into_iter().map(|change| {
            but_workspace::discard::DiscardSpec::from(but_workspace::commit_engine::DiffSpec::from(
                change,
            ))
        }),
        settings.get()?.context_lines,
    )?;
    if !refused.is_empty() {
        tracing::warn!(?refused, "Failed to discard at least one hunk");
    }
    Ok(refused
        .into_iter()
        .map(|change| commit_engine::DiffSpec::from(change).into())
        .collect())
}

/// Returns a new available branch name based on a simple template - user_initials-branch-count
/// The main point of this is to be able to provide branch names that are not already taken.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn canned_branch_name(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<String, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    gitbutler_stack::canned_branch_name(ctx.repo()).map_err(Into::into)
}
