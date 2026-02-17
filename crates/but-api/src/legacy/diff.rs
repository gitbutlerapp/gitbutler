use anyhow::Result;
use but_api_macros::but_api;
use but_core::ui::TreeChange;
use but_ctx::Context;
use but_hunk_assignment::{AssignmentRejection, HunkAssignmentRequest, WorktreeChanges};
use but_hunk_dependency::ui::{HunkDependencies, hunk_dependencies_for_workspace_changes_by_worktree_dir};
use tracing::instrument;

/// Provide a unified diff for `change`, but fail if `change` is a [type-change](but_core::ModeFlags::TypeChange)
/// or if it involves a change to a [submodule](gix::object::Kind::Commit).
#[but_api]
#[instrument(err(Debug))]
pub fn tree_change_diffs(ctx: &Context, change: TreeChange) -> anyhow::Result<Option<but_core::UnifiedPatch>> {
    let change: but_core::TreeChange = change.into();
    let repo = ctx.repo.get()?;
    change.unified_patch(&repo, ctx.settings.context_lines)
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
#[but_api]
#[instrument(err(Debug))]
pub fn changes_in_worktree(ctx: &mut Context) -> anyhow::Result<WorktreeChanges> {
    let context_lines = ctx.settings.context_lines;
    let (mut guard, repo, ws, mut db) = ctx.workspace_mut_and_db_mut()?;
    let changes = but_core::diff::worktree_changes(&repo)?;

    let dependencies =
        hunk_dependencies_for_workspace_changes_by_worktree_dir(&repo, &ws, Some(changes.changes.clone()));
    let mut trans = db.immediate_transaction()?;

    // If the dependencies calculation failed, we still want to try to get assignments
    // so we pass an empty HunkDependencies in that case.
    let (assignments, assignments_error) = match &dependencies {
        Ok(dependencies) => but_hunk_assignment::assignments_with_fallback(
            trans.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            Some(changes.changes.clone()),
            Some(dependencies),
            context_lines,
        )?,
        Err(_) => but_hunk_assignment::assignments_with_fallback(
            trans.hunk_assignments_mut()?,
            &repo,
            &ws,
            false,
            Some(changes.changes.clone()),
            Some(&HunkDependencies::default()), // empty dependencies on error
            context_lines,
        )?,
    };

    trans.commit()?;
    drop((repo, ws, db));
    but_rules::handler::process_workspace_rules(
        ctx,
        &assignments,
        &dependencies.as_ref().ok().cloned(),
        guard.write_permission(),
    )
    .ok();

    Ok(WorktreeChanges {
        worktree_changes: changes.into(),
        assignments,
        assignments_error: assignments_error.map(|err| serde_error::Error::new(&*err)),
        dependencies: dependencies.as_ref().ok().cloned(),
        dependencies_error: dependencies.as_ref().err().map(|err| serde_error::Error::new(&**err)),
    })
}

#[but_api]
#[instrument(err(Debug))]
pub fn assign_hunk(
    ctx: &mut Context,
    assignments: Vec<HunkAssignmentRequest>,
) -> anyhow::Result<Vec<AssignmentRejection>> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let rejections =
        but_hunk_assignment::assign(db.hunk_assignments_mut()?, &repo, &ws, assignments, None, context_lines)?;
    Ok(rejections)
}
