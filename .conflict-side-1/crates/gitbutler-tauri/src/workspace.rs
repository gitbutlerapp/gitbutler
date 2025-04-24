use crate::error::Error;
use crate::from_json::HexHash;
use anyhow::Context;
use but_hunk_dependency::ui::{
    hunk_dependencies_for_workspace_changes_by_worktree_dir, HunkDependencies,
};
use but_settings::AppSettingsWithDiskSync;
use but_workspace::commit_engine::StackSegmentId;
use but_workspace::{commit_engine, StackEntry};
use gitbutler_branch_actions::BranchManagerExt;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
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
    filter: Option<but_workspace::StacksFilter>,
) -> Result<Vec<StackEntry>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let repo = ctx.gix_repo()?;
    but_workspace::stacks(&ctx, &project.gb_dir(), &repo, filter.unwrap_or_default())
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stack_details(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
) -> Result<but_workspace::StackDetails, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::stack_details(&project.gb_dir(), stack_id, &ctx).map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn branch_details(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_name: &str,
    remote: Option<&str>,
) -> Result<but_workspace::BranchDetails, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::branch_details(&project.gb_dir(), branch_name, remote, &ctx).map_err(Into::into)
}

/// Retrieve all changes in the workspace and associate them with commits in the Workspace of `project_id`.
/// NOTE: right now there is no way to keep track of unassociated hunks.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn hunk_dependencies_for_workspace_changes(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> Result<HunkDependencies, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        &ctx,
        &project.path,
        &project.gb_dir(),
    )?;
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
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();
    let snapshot_tree = ctx.prepare_snapshot(guard.read_permission());
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

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &ctx)
        .context("failed to update gitbutler workspace")?;

    let _ = snapshot_tree.and_then(|snapshot_tree| {
        ctx.snapshot_commit_creation(
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
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let mut guard = project.exclusive_worktree_access();

    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::DiscardChanges),
        guard.write_permission(),
    );
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

/// This API allows the user to quickly "stash" a bunch of uncommitted changes - getting them out of the worktree.
/// Unlike the regular stash, the user specifies a new branch where those changes will be 'saved'/committed.
/// Immediatelly after the changes are committed, the branch is unapplied from the workspace, and the "stash" branch can be re-applied at a later time
/// In theory it should be possible to specify an existing "dumping" branch for this, but currently this endpoint expects a new branch.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn stash_into_branch(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    branch_name: String,
    worktree_changes: Vec<commit_engine::ui::DiffSpec>,
) -> Result<commit_engine::ui::CreateCommitOutcome, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let repo = ctx.gix_repo_for_merging()?;

    let mut guard = project.exclusive_worktree_access();
    let perm = guard.write_permission();

    let _ = ctx.snapshot_stash_into_branch(branch_name.clone(), perm);

    let branch_manager = ctx.branch_manager();
    let stack = branch_manager.create_virtual_branch(
        &gitbutler_branch::BranchCreateRequest {
            name: Some(branch_name),
            ..Default::default()
        },
        perm,
    )?;

    let parent_commit_id = stack.head(&repo)?;
    let branch_name = stack.derived_name()?;

    let outcome = commit_engine::create_commit_and_update_refs_with_project(
        &repo,
        &project,
        Some(stack.id),
        commit_engine::Destination::NewCommit {
            parent_commit_id: Some(parent_commit_id),
            message: "Mo-Stashed changes".into(),
            stack_segment: Some(StackSegmentId {
                stack_id: stack.id,
                segment_ref: format!("refs/heads/{branch_name}")
                    .try_into()
                    .map_err(anyhow::Error::from)?,
            }),
        },
        None,
        worktree_changes.into_iter().map(Into::into).collect(),
        settings.get()?.context_lines,
        perm,
    );

    let vb_state = VirtualBranchesHandle::new(project.gb_dir());
    gitbutler_branch_actions::update_workspace_commit(&vb_state, &ctx)
        .context("failed to update gitbutler workspace")?;

    branch_manager.unapply(stack.id, perm, false)?;

    let outcome = outcome?;
    Ok(outcome.into())
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
    let template = gitbutler_stack::canned_branch_name(ctx.repo())?;
    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    gitbutler_stack::Stack::next_available_name(&ctx.gix_repo()?, &state, template, false)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn target_commits(
    projects: State<'_, projects::Controller>,
    settings: State<'_, AppSettingsWithDiskSync>,
    project_id: ProjectId,
    last_commit_id: Option<HexHash>,
    page_size: Option<usize>,
) -> Result<Vec<but_workspace::Commit>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    but_workspace::log_target_first_parent(
        &ctx,
        last_commit_id.map(|id| id.into()),
        page_size.unwrap_or(30),
    )
    .map_err(Into::into)
}
