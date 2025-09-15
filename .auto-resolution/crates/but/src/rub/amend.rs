use but_hunk_assignment::HunkAssignment;
use but_workspace::{
    DiffSpec, StackId,
    commit_engine::{self, CreateCommitOutcome},
};
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::access::WorktreeWritePermission;
use gix::ObjectId;

use super::assign::branch_name_to_stack_id;

pub(crate) fn file_to_commit(
    ctx: &mut CommandContext,
    path: &str,
    stack_id: Option<StackId>,
    oid: &ObjectId,
) -> anyhow::Result<()> {
    let diff_specs: Vec<DiffSpec> = wt_assignments(ctx)?
        .into_iter()
        .filter(|assignment| assignment.stack_id == stack_id && assignment.path == path)
        .map(|assignment| assignment.into())
        .collect();

    let mut guard = ctx.project().exclusive_worktree_access();
    let new_commit = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?
        .new_commit
        .map(|c| {
            let s = c.to_string();
            format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
        })
        .unwrap_or_default();
    println!("Amended {} → {}", path.bold(), new_commit);
    Ok(())
}

pub(crate) fn assignments_to_commit(
    ctx: &mut CommandContext,
    branch_name: Option<&str>,
    oid: &ObjectId,
) -> anyhow::Result<()> {
    let stack_id = branch_name_to_stack_id(ctx, branch_name)?;
    let diff_specs: Vec<DiffSpec> = wt_assignments(ctx)?
        .into_iter()
        .filter(|assignment| assignment.stack_id == stack_id)
        .map(|assignment| assignment.into())
        .collect();
    let mut guard = ctx.project().exclusive_worktree_access();
    let new_commit = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?
        .new_commit
        .map(|c| {
            let s = c.to_string();
            format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
        })
        .unwrap_or_default();

    if let Some(branch_name) = branch_name {
        println!(
            "Amended assigned files {} → {}",
            format!("[{branch_name}]").green(),
            new_commit,
        );
    } else {
        println!("Amended unassigned files → {new_commit}",)
    }
    Ok(())
}

fn wt_assignments(ctx: &mut CommandContext) -> anyhow::Result<Vec<HunkAssignment>> {
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;
    Ok(assignments)
}

fn amend_diff_specs(
    ctx: &mut CommandContext,
    diff_specs: Vec<DiffSpec>,
    stack_id: Option<StackId>,
    oid: ObjectId,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    commit_engine::create_commit_and_update_refs_with_project(
        &ctx.gix_repo_for_merging()?,
        ctx.project(),
        stack_id,
        commit_engine::Destination::AmendCommit {
            commit_id: oid,
            new_message: None,
        },
        None,
        but_workspace::flatten_diff_specs(diff_specs),
        ctx.app_settings().context_lines,
        perm,
    )
}
