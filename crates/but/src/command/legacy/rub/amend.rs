use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::{Context, access::WorktreeWritePermission};
use but_hunk_assignment::HunkAssignment;
use but_workspace::commit_engine::{self, CreateCommitOutcome};
use colored::Colorize;
use gix::ObjectId;

use super::assign::branch_name_to_stack_id;
use crate::{id::UncommittedCliId, utils::OutputChannel};

pub(crate) fn uncommitted_to_commit(
    ctx: &mut Context,
    uncommitted_cli_id: UncommittedCliId,
    oid: &ObjectId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let description = uncommitted_cli_id.describe();

    let first_hunk_assignment = uncommitted_cli_id.hunk_assignments.first();
    let stack_id = first_hunk_assignment.stack_id;

    let diff_specs: Vec<DiffSpec> = uncommitted_cli_id
        .hunk_assignments
        .into_iter()
        .map(|assignment: HunkAssignment| assignment.into())
        .collect();

    let mut guard = ctx.exclusive_worktree_access();
    let new_commit = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?
        .new_commit
        .map(|c| {
            let s = c.to_string();
            format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
        })
        .unwrap_or_default();
    if let Some(out) = out.for_human() {
        writeln!(out, "Amended {} → {}", description, new_commit)?;
    }
    Ok(())
}

pub(crate) fn assignments_to_commit(
    ctx: &mut Context,
    branch_name: Option<&str>,
    oid: &ObjectId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let stack_id = branch_name_to_stack_id(ctx, branch_name)?;
    let diff_specs: Vec<DiffSpec> = wt_assignments(ctx)?
        .into_iter()
        .filter(|assignment| assignment.stack_id == stack_id)
        .map(|assignment| assignment.into())
        .collect();
    let mut guard = ctx.exclusive_worktree_access();
    let new_commit = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?
        .new_commit
        .map(|c| {
            let s = c.to_string();
            format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
        })
        .unwrap_or_default();

    if let Some(out) = out.for_human() {
        if let Some(branch_name) = branch_name {
            writeln!(
                out,
                "Amended assigned files {} → {}",
                format!("[{branch_name}]").green(),
                new_commit,
            )?;
        } else {
            writeln!(out, "Amended unassigned files → {new_commit}")?;
        }
    }
    Ok(())
}

fn wt_assignments(ctx: &mut Context) -> anyhow::Result<Vec<HunkAssignment>> {
    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(
        ctx.legacy_project.worktree_dir()?.into(),
    )?
    .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;
    Ok(assignments)
}

fn amend_diff_specs(
    ctx: &mut Context,
    diff_specs: Vec<DiffSpec>,
    stack_id: Option<StackId>,
    oid: ObjectId,
    perm: &mut WorktreeWritePermission,
) -> anyhow::Result<CreateCommitOutcome> {
    but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &ctx.clone_repo_for_merging()?,
        &ctx.project_data_dir(),
        stack_id,
        commit_engine::Destination::AmendCommit {
            commit_id: oid,
            new_message: None,
        },
        but_workspace::flatten_diff_specs(diff_specs),
        ctx.settings().context_lines,
        perm,
    )
}
