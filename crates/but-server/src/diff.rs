use anyhow::Context;
use but_core::{Commit, commit::ConflictEntries, ui::TreeChanges};
use but_hunk_assignment::{HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::{ObjectIdExt, OidExt};
use gitbutler_project::ProjectId;
use gitbutler_stack::VirtualBranchesHandle;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::RequestContext;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TreeChangeDiffsParams {
    project_id: ProjectId,
    change: but_core::ui::TreeChange,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommitDetailsParams {
    project_id: ProjectId,
    commit_id: HexHash,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangesInBranchParams {
    project_id: ProjectId,
    stack_id: Option<StackId>,
    branch_name: String,
    remote: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChangesInWorktreeParams {
    project_id: ProjectId,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AssignHunkParams {
    project_id: ProjectId,
    assignments: Vec<HunkAssignmentRequest>,
}

// Helper type for JSON parsing
#[derive(serde::Deserialize, Clone)]
struct HexHash(String);

impl From<HexHash> for git2::Oid {
    fn from(hex: HexHash) -> Self {
        git2::Oid::from_str(&hex.0).expect("Invalid hex hash")
    }
}

impl From<HexHash> for gix::ObjectId {
    fn from(hex: HexHash) -> Self {
        gix::ObjectId::from_hex(hex.0.as_bytes()).expect("Invalid hex hash")
    }
}

pub fn tree_change_diffs(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: TreeChangeDiffsParams = serde_json::from_value(params)?;

    let change: but_core::TreeChange = params.change.into();
    let project = ctx.project_controller.get(params.project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    let diff = change
        .unified_diff(&repo, ctx.app_settings.get()?.context_lines)?
        .context("TODO: Submodules must be handled specifically in the UI")?;
    Ok(serde_json::to_value(diff)?)
}

pub fn commit_details(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: CommitDetailsParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let repo = &gix::open(&project.path).context("Failed to open repo")?;
    let commit = repo
        .find_commit(params.commit_id.clone())
        .context("Failed for find commit")?;
    let changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(repo, params.commit_id.into())?;
    let conflict_entries = Commit::from_id(commit.id())?.conflict_entries()?;

    let details = CommitDetails {
        commit: commit.try_into()?,
        changes,
        conflict_entries,
    };
    Ok(serde_json::to_value(details)?)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetails {
    pub commit: but_workspace::ui::Commit,
    #[serde(flatten)]
    pub changes: but_core::ui::TreeChanges,
    pub conflict_entries: Option<ConflictEntries>,
}

pub fn changes_in_branch(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ChangesInBranchParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let branch_name = params.remote.map_or(params.branch_name.clone(), |r| {
        format!("{r}/{}", params.branch_name)
    });
    let changes = changes_in_branch_inner(command_ctx, branch_name, params.stack_id)?;
    Ok(serde_json::to_value(changes)?)
}

fn changes_in_branch_inner(
    ctx: CommandContext,
    branch_name: String,
    stack_id: Option<StackId>,
) -> anyhow::Result<TreeChanges> {
    let state = VirtualBranchesHandle::new(ctx.project().gb_dir());
    let repo = ctx.gix_repo()?;
    let (start_commit_id, base_commit_id) = if let Some(stack_id) = stack_id {
        commit_and_base_from_stack(&ctx, &state, stack_id, branch_name.clone())
    } else {
        let start_commit_id = repo.find_reference(&branch_name)?.peel_to_commit()?.id;
        let target = state.get_default_target()?;
        let merge_base = ctx
            .repo()
            .merge_base(start_commit_id.to_git2(), target.sha)?;
        Ok((start_commit_id, merge_base.to_gix()))
    }?;

    but_core::diff::ui::changes_in_range(
        ctx.project().path.clone(),
        start_commit_id,
        base_commit_id,
    )
}

fn commit_and_base_from_stack(
    ctx: &CommandContext,
    state: &VirtualBranchesHandle,
    stack_id: StackId,
    branch_name: String,
) -> anyhow::Result<(gix::ObjectId, gix::ObjectId)> {
    let stack = state.get_stack(stack_id)?;

    // Find the branch head and the one before it
    let heads = stack.heads(false);
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
    let repo = ctx.gix_repo()?;

    // Find the head that matches the branch name - the commit contained is our commit_id
    let start_commit_id = repo
        .find_reference(start.with_context(|| format!("Branch {} not found", branch_name))?)?
        .peel_to_commit()?
        .id;

    // Now, find the preceding head in the stack. If it is not present, use the stack merge base
    let base_commit_id = match end {
        Some(end) => repo.find_reference(end)?.peel_to_commit()?.id,
        None => stack.merge_base(ctx)?,
    };
    Ok((start_commit_id, base_commit_id))
}

pub fn changes_in_worktree(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: ChangesInWorktreeParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let mut command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let changes = but_core::diff::worktree_changes(&command_ctx.gix_repo()?)?;

    let project_path = command_ctx.project().path.clone();
    let gb_dir = command_ctx.project().gb_dir().to_path_buf();
    let dependencies = hunk_dependencies_for_workspace_changes_by_worktree_dir(
        &command_ctx,
        &project_path,
        &gb_dir,
        Some(changes.changes.clone()),
    );

    let (assignments, assignments_error) = match &dependencies {
        Ok(dependencies) => but_hunk_assignment::assignments_with_fallback(
            &mut command_ctx,
            false,
            Some(changes.changes.clone()),
            Some(dependencies),
        )?,
        Err(e) => (
            vec![],
            Some(anyhow::anyhow!("failed to get hunk dependencies: {}", e)),
        ),
    };

    let result = WorktreeChanges {
        worktree_changes: changes.into(),
        assignments,
        assignments_error: assignments_error.map(|err| serde_error::Error::new(&*err)),
        dependencies: dependencies.as_ref().ok().cloned(),
        dependencies_error: dependencies
            .as_ref()
            .err()
            .map(|err| serde_error::Error::new(&**err)),
    };
    Ok(serde_json::to_value(result)?)
}

pub fn assign_hunk(ctx: &RequestContext, params: Value) -> anyhow::Result<Value> {
    let params: AssignHunkParams = serde_json::from_value(params)?;

    let project = ctx.project_controller.get(params.project_id)?;
    let mut command_ctx = CommandContext::open(&project, ctx.app_settings.get()?.clone())?;
    let rejections = but_hunk_assignment::assign(&mut command_ctx, params.assignments, None)?;
    Ok(serde_json::to_value(rejections)?)
}
