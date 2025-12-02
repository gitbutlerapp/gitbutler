use anyhow::{Context as _, Result};
use but_api_macros::api_cmd_tauri;
use but_core::{
    Commit,
    ref_metadata::StackId,
    ui::{TreeChange, TreeChanges},
};
use but_ctx::Context;
use but_hunk_assignment::{AssignmentRejection, HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::{
    HunkDependencies, hunk_dependencies_for_workspace_changes_by_worktree_dir,
};
use but_settings::AppSettings;
use gitbutler_project::ProjectId;
use gitbutler_reference::Refname;
use gix::refs::Category;
use tracing::instrument;

use crate::json::HexHash;

/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn tree_change_diffs(
    project_id: ProjectId,
    change: TreeChange,
) -> anyhow::Result<Option<but_core::UnifiedPatch>> {
    let change: but_core::TreeChange = change.into();
    let project = gitbutler_project::get(project_id)?;
    let app_settings = AppSettings::load_from_default_path_creating()?;
    let repo = project.open_repo()?;
    change.unified_patch(&repo, app_settings.context_lines)
}

pub mod json {
    use but_core::commit::ConflictEntries;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct CommitDetails {
        pub commit: but_workspace::ui::Commit,
        #[serde(flatten)]
        pub changes: but_core::ui::TreeChanges,
        pub conflict_entries: Option<ConflictEntries>,
    }
}

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn commit_details(
    project_id: ProjectId,
    commit_id: HexHash,
) -> anyhow::Result<json::CommitDetails> {
    let project = gitbutler_project::get(project_id)?;
    let repo = project.open_repo()?;
    let commit = repo
        .find_commit(commit_id)
        .context("Failed for find commit")?;
    let changes = but_core::diff::ui::commit_changes_with_line_stats_by_worktree_dir(
        &repo,
        commit_id.into(),
    )?;
    let conflict_entries = Commit::from_id(commit.id())?.conflict_entries()?;
    Ok(json::CommitDetails {
        commit: commit.try_into()?,
        changes,
        conflict_entries,
    })
}

/// Gets the changes for a given branch.
/// If the branch is part of a stack and if the stack_id is provided, this will include only the changes
/// up to the next branch in the stack.
/// Otherwise, if stack_id is not provided, this will include all changes as compared to the target branch
/// Note that `stack_id` is deprecated in favor of `branch_name`
/// *(which should be a full ref-name as well and make `remote` unnecessary)*
#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn changes_in_branch(
    project_id: ProjectId,
    _stack_id: Option<StackId>,
    branch: Refname,
) -> anyhow::Result<TreeChanges> {
    let ctx = Context::new_from_legacy_project_id(project_id)?;
    changes_in_branch_inner(ctx, branch)
}

fn changes_in_branch_inner(ctx: Context, branch: Refname) -> anyhow::Result<TreeChanges> {
    let guard = ctx.shared_worktree_access();
    let (repo, _meta, graph) =
        ctx.graph_and_meta_and_repo_from_head(ctx.repo.get()?.clone(), guard.read_permission())?;
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
#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn changes_in_worktree(project_id: ProjectId) -> anyhow::Result<WorktreeChanges> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let changes = but_core::diff::worktree_changes(&*ctx.repo.get()?)?;

    let dependencies =
        hunk_dependencies_for_workspace_changes_by_worktree_dir(ctx, Some(changes.changes.clone()));

    // If the dependencies calculation failed, we still want to try to get assignments
    // so we pass an empty HunkDependencies in that case.
    let (assignments, assignments_error) = match &dependencies {
        Ok(dependencies) => but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            Some(changes.changes.clone()),
            Some(dependencies),
        )?,
        Err(_) => but_hunk_assignment::assignments_with_fallback(
            ctx,
            false,
            Some(changes.changes.clone()),
            Some(&HunkDependencies::default()), // empty dependencies on error
        )?,
    };

    if ctx.settings().feature_flags.rules {
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

#[api_cmd_tauri]
#[instrument(err(Debug))]
pub fn assign_hunk(
    project_id: ProjectId,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<Vec<AssignmentRejection>> {
    let project = gitbutler_project::get(project_id)?;
    let ctx = &mut Context::new_from_legacy_project(project.clone())?;
    let rejections = but_hunk_assignment::assign(ctx, assignments, None)?;
    Ok(rejections)
}
