use crate::error::Error;
use crate::from_json::HexHash;
use anyhow::Context;
use but_core::{
    commit::ConflictEntries,
    ui::{TreeChange, TreeChanges},
    Commit,
};
use but_hunk_assignment::{AssignmentRejection, HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gix::refs::Category;
use serde::Serialize;
use tracing::instrument;

/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
#[tauri::command(async)]
#[instrument(skip(projects, change, settings), err(Debug))]
pub fn tree_change_diffs(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    change: TreeChange,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    let change: but_core::TreeChange = change.into();
    let project = projects.get(project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    Ok(change
        .unified_diff(&repo, settings.get()?.context_lines)?
        .context("TODO: Submodules must be handled specifically in the UI")?)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn commit_details(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    commit_id: HexHash,
) -> anyhow::Result<CommitDetails, Error> {
    let project = projects.get(project_id)?;
    let repo = &gix::open(&project.path).context("Failed to open repo")?;
    let commit = repo
        .find_commit(commit_id)
        .context("Failed for find commit")?;
    let changes = but_core::diff::ui::commit_changes_by_worktree_dir(repo, commit_id.into())?;
    let conflict_entries = Commit::from_id(commit.id())?.conflict_entries()?;
    Ok(CommitDetails {
        commit: commit.try_into()?,
        changes,
        conflict_entries,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub commit: but_workspace::ui::Commit,
    #[serde(flatten)]
    pub changes: but_core::ui::TreeChanges,
    pub conflict_entries: Option<ConflictEntries>,
}

/// Gets the changes for a given branch.
/// If the branch is part of a stack and if the stack_id is provided, this will include only the changes
/// up to the next branch in the stack.
/// Otherwise, if stack_id is not provided, this will include all changes as compared to the target branch
/// Note that `stack_id` is deprecated in favor of `branch_name`
/// *(which should be a full ref-name as well and make `remote` unnecessary)*
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn changes_in_branch(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    // TODO: remove this, go by name. Ideally, the UI would pass us two commits.
    _stack_id: Option<StackId>,
    branch_name: String,
    remote: Option<String>,
) -> anyhow::Result<TreeChanges, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    let branch_name = remote.map_or(branch_name.clone(), |r| format!("{r}/{branch_name}"));
    changes_in_branch_inner(ctx, branch_name).map_err(Into::into)
}

fn changes_in_branch_inner(
    ctx: CommandContext,
    branch_name: String,
) -> anyhow::Result<TreeChanges> {
    let (repo, _meta, graph) = ctx.graph_and_meta(ctx.gix_repo()?)?;
    let name = Category::LocalBranch.to_full_name(branch_name.as_str())?;
    let ws = graph.to_workspace()?;
    let (stack, segment) = ws.try_find_segment_and_stack_by_refname(name.as_ref())?;

    let base = stack.base();
    let Some((tip, base)) = segment
        .tip()
        .or(base)
        .zip(base)
        .filter(|(tip, base)| tip != base)
    else {
        return Ok(TreeChanges::default());
    };

    but_core::diff::ui::changes_in_range(&repo, tip, base)
}

/// This UI-version of [`but_core::diff::worktree_changes()`] simplifies the `git status` information for display in
/// the user interface as it is right now. From here, it's always possible to add more information as the need arises.
///
/// ### Notable Transformations
/// * There is no notion of an index (`.git/index`) - all changes seem to have happened in the worktree.
/// * Modifications that were made to the index will be ignored *only if* there is a worktree modification to the same file.
/// * conflicts are ignored
///
/// All ignored status changes are also provided so they can be displayed separately.
#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn changes_in_worktree(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
) -> anyhow::Result<WorktreeChanges, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let changes = but_core::diff::worktree_changes(&ctx.gix_repo()?)?;

    let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        ctx,
        &ctx.project().path,
        &ctx.project().gb_dir(),
        Some(changes.changes.clone()),
    );

    let (assignments, assignments_error) = match &dependencies {
        Ok(dependencies) => but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            Some(changes.changes.clone()),
            Some(dependencies),
        )?,
        Err(e) => (
            vec![],
            Some(anyhow::anyhow!("failed to get hunk dependencies: {}", e)),
        ),
    };

    Ok(WorktreeChanges {
        worktree_changes: changes.into(),
        assignments,
        assignments_error: assignments_error.map(|err| serde_error::Error::new(&*err)),
        dependencies: dependencies.as_ref().ok().cloned(),
        dependencies_error: dependencies
            .as_ref()
            .err()
            .map(|err| serde_error::Error::new(&**err)),
    })
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn assign_hunk(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<Vec<AssignmentRejection>, Error> {
    let project = projects.get(project_id)?;
    let ctx = &mut CommandContext::open(&project, settings.get()?.clone())?;
    let rejections = but_hunk_assignment::assign(ctx, assignments, None)?;
    Ok(rejections)
}
