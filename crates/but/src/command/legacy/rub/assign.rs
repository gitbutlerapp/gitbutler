use bstr::BString;
use but_core::{HunkHeader, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignmentRequest;
use colored::Colorize;

use crate::{id::UncommittedCliId, utils::OutputChannel};

pub(crate) fn assign_uncommitted_to_branch(
    ctx: &mut Context,
    uncommitted_cli_id: UncommittedCliId,
    branch_name: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let description = uncommitted_cli_id.describe();

    let assignments = uncommitted_cli_id
        .hunk_assignments
        .into_iter()
        .map(|hunk_assignment| (hunk_assignment.hunk_header, hunk_assignment.path_bytes));
    let reqs = to_assignment_request(ctx, assignments, Some(branch_name))?;
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Staged {} → {}.", description, format!("[{branch_name}]").green())?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

pub(crate) fn assign_uncommitted_to_stack(
    ctx: &mut Context,
    uncommitted_cli_id: UncommittedCliId,
    stack_id: &StackId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let description = uncommitted_cli_id.describe();

    let assignments = uncommitted_cli_id
        .hunk_assignments
        .into_iter()
        .map(|hunk_assignment| (hunk_assignment.hunk_header, hunk_assignment.path_bytes));
    let reqs = to_assignment_request(ctx, assignments, None)?
        .into_iter()
        .map(|mut req| {
            req.stack_id = Some(*stack_id);
            req
        })
        .collect();
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Staged {} → stack {}.",
            description,
            format!("[{stack_id}]").green()
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

pub(crate) fn unassign_uncommitted(
    ctx: &mut Context,
    uncommitted_cli_id: UncommittedCliId,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let description = uncommitted_cli_id.describe();

    let assignments = uncommitted_cli_id
        .hunk_assignments
        .into_iter()
        .map(|hunk_assignment| (hunk_assignment.hunk_header, hunk_assignment.path_bytes));
    let reqs = to_assignment_request(ctx, assignments, None)?;
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Unstaged {description}")?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

/// Target for hunk assignment operations.
///
/// This enum identifies where hunks should be assigned or moved to/from:
/// either a branch, referenced by its name, or a stack, referenced by its [`StackId`].
pub enum AssignTarget<'a> {
    /// A branch, identified by its name.
    Branch(&'a str),
    /// A stack, identified by its [`StackId`].
    Stack(&'a StackId),
}

pub(crate) fn assign_all(
    ctx: &mut Context,
    from: Option<AssignTarget>,
    to: Option<AssignTarget>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let (from_branch, from_stack_id) = match from {
        Some(AssignTarget::Branch(name)) => (Some(name.to_string()), branch_name_to_stack_id(ctx, Some(name))?),
        Some(AssignTarget::Stack(stack_id)) => (stack_id_to_branch_name(ctx, stack_id), Some(*stack_id)),

        None => (None, None),
    };
    let (to_branch, to_stack_id) = match to {
        Some(AssignTarget::Branch(name)) => (Some(name.to_string()), branch_name_to_stack_id(ctx, Some(name))?),
        Some(AssignTarget::Stack(stack_id)) => (stack_id_to_branch_name(ctx, stack_id), Some(*stack_id)),
        None => (None, None),
    };
    assign_all_inner(ctx, from_branch, from_stack_id, to_branch, to_stack_id, out)
}

fn assign_all_inner(
    ctx: &mut Context,
    from_branch: Option<String>,
    from_stack_id: Option<StackId>,
    to_branch: Option<String>,
    to_stack_id: Option<StackId>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Get all assignment requests from the from_stack_id
    let changes = but_core::diff::ui::worktree_changes(&*ctx.repo.get()?)?.changes;

    let context_lines = ctx.settings.context_lines;
    let (_, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        false,
        Some(changes),
        None,
        context_lines,
    )?;

    let mut reqs = Vec::new();
    for assignment in assignments {
        if assignment.stack_id == from_stack_id {
            reqs.push(HunkAssignmentRequest {
                hunk_header: assignment.hunk_header,
                path_bytes: assignment.path_bytes,
                stack_id: to_stack_id,
            });
        }
    }
    drop((repo, ws, db));
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        if to_branch.is_some() {
            writeln!(
                out,
                "Staged all {} changes to {}.",
                from_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unstaged".to_string().bold()),
                to_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unstaged".to_string().bold())
            )?;
        } else {
            writeln!(
                out,
                "Unstaged all {} changes.",
                from_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unstaged".to_string().bold())
            )?;
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({"ok": true}))?;
    }
    Ok(())
}

pub(crate) fn do_assignments(
    ctx: &mut Context,
    reqs: Vec<HunkAssignmentRequest>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let rejections = but_hunk_assignment::assign(db.hunk_assignments_mut()?, &repo, &ws, reqs, None, context_lines)?;
    if !rejections.is_empty()
        && let Some(out) = out.for_human()
    {
        writeln!(out, "{rejections:#?}")?;
    }
    Ok(())
}

pub(crate) fn branch_name_to_stack_id(ctx: &Context, branch_name: Option<&str>) -> anyhow::Result<Option<StackId>> {
    let stack_id = if let Some(branch_name) = branch_name {
        crate::legacy::commits::stacks(ctx)?
            .iter()
            .find(|s| s.heads.iter().any(|h| h.name == branch_name))
            .and_then(|s| s.id)
    } else {
        None
    };
    Ok(stack_id)
}

fn stack_id_to_branch_name(ctx: &Context, stack_id: &StackId) -> Option<String> {
    crate::legacy::commits::stacks(ctx)
        .ok()?
        .into_iter()
        .find(|s| s.id.as_ref() == Some(stack_id))
        .and_then(|s| s.heads.first().map(|h| h.name.to_string()))
}

pub(crate) fn to_assignment_request(
    ctx: &mut Context,
    assignments: impl Iterator<Item = (Option<HunkHeader>, BString)>,
    branch_name: Option<&str>,
) -> anyhow::Result<Vec<HunkAssignmentRequest>> {
    let stack_id = branch_name_to_stack_id(ctx, branch_name)?;

    let mut reqs = Vec::new();
    for (hunk_header, path_bytes) in assignments {
        reqs.push(HunkAssignmentRequest {
            hunk_header,
            path_bytes,
            stack_id,
        });
    }
    Ok(reqs)
}
