use but_core::{DiffSpec, ref_metadata::StackId};
use but_ctx::{Context, access::RepoExclusive};
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
    let outcome = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?;
    if let Some(out) = out.for_human() {
        let new_commit = outcome
            .new_commit
            .map(|c| {
                let s = c.to_string();
                format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
            })
            .unwrap_or_default();
        writeln!(out, "Amended {} → {}", description, new_commit)?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "ok": true,
            "new_commit_id": outcome.new_commit.map(|c| c.to_string()),
        }))?;
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
    let outcome = amend_diff_specs(ctx, diff_specs, stack_id, *oid, guard.write_permission())?;
    if let Some(out) = out.for_human() {
        let new_commit = outcome
            .new_commit
            .map(|c| {
                let s = c.to_string();
                format!("{}{}", s[..2].blue().underline(), s[2..7].blue())
            })
            .unwrap_or_default();
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
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "ok": true,
            "new_commit_id": outcome.new_commit.map(|c| c.to_string()),
        }))?;
    }
    Ok(())
}

fn wt_assignments(ctx: &mut Context) -> anyhow::Result<Vec<HunkAssignment>> {
    let changes = but_core::diff::ui::worktree_changes(&*ctx.repo.get()?)?.changes;
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        Some(changes.clone()),
        None,
        context_lines,
    )?;
    Ok(assignments)
}

fn amend_diff_specs(
    ctx: &mut Context,
    diff_specs: Vec<DiffSpec>,
    stack_id: Option<StackId>,
    oid: ObjectId,
    perm: &mut RepoExclusive,
) -> anyhow::Result<CreateCommitOutcome> {
    but_workspace::legacy::commit_engine::create_commit_and_update_refs_with_project(
        &*ctx.repo.get()?,
        &ctx.project_data_dir(),
        stack_id,
        commit_engine::Destination::AmendCommit {
            commit_id: oid,
            new_message: None,
        },
        but_workspace::flatten_diff_specs(diff_specs),
        ctx.settings.context_lines,
        perm,
    )
}
