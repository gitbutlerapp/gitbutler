use crate::error::Error;
use crate::from_json::HexHash;
use anyhow::Context;
use but_core::ui::{TreeChange, WorktreeChanges};
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::OidExt;
use gitbutler_project::ProjectId;
use gitbutler_stack::{stack_context::CommandContextExt, VirtualBranchesHandle};
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
    change
        .unified_diff(&repo, settings.get()?.context_lines)
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects), err(Debug))]
pub fn changes_in_commit(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
    commit_id: HexHash,
) -> anyhow::Result<Vec<TreeChange>, Error> {
    let project = projects.get(project_id)?;
    but_core::diff::ui::commit_changes_by_worktree_dir(project.path, commit_id.into())
        .map_err(Into::into)
}

#[tauri::command(async)]
#[instrument(skip(projects, settings), err(Debug))]
pub fn changes_in_branch(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    settings: tauri::State<'_, but_settings::AppSettingsWithDiskSync>,
    project_id: ProjectId,
    stack_id: StackId,
    branch_name: String,
) -> anyhow::Result<Vec<TreeChange>, Error> {
    let project = projects.get(project_id)?;
    let ctx = CommandContext::open(&project, settings.get()?.clone())?;
    changes_in_branch_inner(ctx, branch_name, stack_id).map_err(Into::into)
}

fn changes_in_branch_inner(
    ctx: CommandContext,
    branch_name: String,
    stack_id: StackId,
) -> anyhow::Result<Vec<TreeChange>> {
    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let stack = state.get_stack(stack_id)?;

    // Find the branch head and the one before it
    let heads = stack.heads();
    let (start, end) = heads
        .iter()
        .rev()
        .fold((None, None), |(start, end), branch| {
            if start.is_some() && end.is_none() {
                (start, Some(branch))
            } else if branch == &branch_name {
                (Some(branch), None)
            } else {
                (start, end)
            }
        });
    let repo = ctx.gix_repository()?;

    // Find the head that matches the branch name - the commit contained is our commit_id
    let start_commit_id = repo
        .find_reference(start.with_context(|| format!("Branch {} not found", branch_name))?)?
        .peel_to_commit()?
        .id;

    // Now, find the preceding head in the stack. If it is not present, use the stack merge base
    let base_commit_id = match end {
        Some(end) => repo.find_reference(end)?.peel_to_commit()?.id,
        None => stack.merge_base(&ctx.to_stack_context()?)?.to_gix(),
    };

    but_core::diff::ui::changes_in_commit_range(
        ctx.project().path.clone(),
        start_commit_id,
        base_commit_id,
    )
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
#[instrument(skip(projects), err(Debug))]
pub fn changes_in_worktree(
    projects: tauri::State<'_, gitbutler_project::Controller>,
    project_id: ProjectId,
) -> anyhow::Result<WorktreeChanges, Error> {
    let project = projects.get(project_id)?;
    Ok(but_core::diff::ui::worktree_changes_by_worktree_dir(
        project.path,
    )?)
}
