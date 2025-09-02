use crate::hex_hash::HexHash;
use crate::{App, error::Error};
use anyhow::Context;
use but_core::{
    Commit,
    commit::ConflictEntries,
    ui::{TreeChange, TreeChanges},
};
use but_hunk_assignment::{AssignmentRejection, HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::hunk_dependencies_for_workspace_changes_by_worktree_dir;
use but_settings::AppSettings;
use but_workspace::StackId;
use gitbutler_command_context::CommandContext;
use gitbutler_project::ProjectId;
use gitbutler_reference::Refname;
use gix::refs::Category;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TreeChangeDiffsParams {
    pub project_id: ProjectId,
    pub change: TreeChange,
}

/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
pub fn tree_change_diffs(
    app: &App,
    params: TreeChangeDiffsParams,
) -> anyhow::Result<but_core::UnifiedDiff, Error> {
    let change: but_core::TreeChange = params.change.into();
    let project = gitbutler_project::get(params.project_id)?;
    let repo = gix::open(project.path).map_err(anyhow::Error::from)?;
    Ok(change
        .unified_diff(&repo, app.app_settings.get()?.context_lines)?
        .context("TODO: Submodules must be handled specifically in the UI")?)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommitDetailsParams {
    pub project_id: ProjectId,
    pub commit_id: HexHash,
}

pub fn commit_details(
    _app: &App,
    params: CommitDetailsParams,
) -> anyhow::Result<CommitDetails, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let repo = &gix::open(&project.path).context("Failed to open repo")?;
    let commit = repo
        .find_commit(params.commit_id)
        .context("Failed for find commit")?;
    let changes =
        but_core::diff::ui::commit_changes_by_worktree_dir(repo, params.commit_id.into())?;
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesInBranchParams {
    pub project_id: ProjectId,
    // TODO: remove this, go by name. Ideally, the UI would pass us two commits.
    pub _stack_id: Option<StackId>,
    pub branch: Refname,
}

/// Gets the changes for a given branch.
/// If the branch is part of a stack and if the stack_id is provided, this will include only the changes
/// up to the next branch in the stack.
/// Otherwise, if stack_id is not provided, this will include all changes as compared to the target branch
/// Note that `stack_id` is deprecated in favor of `branch_name`
/// *(which should be a full ref-name as well and make `remote` unnecessary)*
pub fn changes_in_branch(
    _app: &App,
    params: ChangesInBranchParams,
) -> anyhow::Result<TreeChanges, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    changes_in_branch_inner(ctx, params.branch).map_err(Into::into)
}

fn changes_in_branch_inner(ctx: CommandContext, branch: Refname) -> anyhow::Result<TreeChanges> {
    let guard = ctx.project().shared_worktree_access();
    let (repo, _meta, graph) = ctx.graph_and_meta(ctx.gix_repo()?, guard.read_permission())?;
    let name = match branch {
        Refname::Virtual(virtual_refname) => {
            Category::LocalBranch.to_full_name(virtual_refname.branch())?
        }
        Refname::Local(local) => Category::LocalBranch.to_full_name(local.branch())?,
        Refname::Other(raw) => Category::LocalBranch.to_full_name(raw.as_str())?,
        Refname::Remote(remote) => {
            Category::RemoteBranch.to_full_name(remote.fullname().as_str())?
        }
    };
    let ws = graph.to_workspace()?;
    but_workspace::ui::diff::changes_in_branch(&repo, &ws, name.as_ref())
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
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesInWorktreeParams {
    pub project_id: ProjectId,
}

pub fn changes_in_worktree(
    _app: &App,
    params: ChangesInWorktreeParams,
) -> anyhow::Result<WorktreeChanges, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
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

    if ctx.app_settings().feature_flags.rules {
        but_rules::handler::process_workspace_rules(
            ctx,
            &assignments,
            &dependencies.as_ref().ok().cloned(),
        )
        .ok();
    }

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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignHunkParams {
    pub project_id: ProjectId,
    pub assignments: Vec<HunkAssignmentRequest>,
}

pub fn assign_hunk(
    _app: &App,
    params: AssignHunkParams,
) -> anyhow::Result<Vec<AssignmentRejection>, Error> {
    let project = gitbutler_project::get(params.project_id)?;
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let rejections = but_hunk_assignment::assign(ctx, params.assignments, None)?;
    Ok(rejections)
}
