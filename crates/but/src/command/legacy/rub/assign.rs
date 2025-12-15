use bstr::{BStr, BString};
use but_core::{HunkHeader, ref_metadata::StackId};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignmentRequest;
use colored::Colorize;

use crate::utils::OutputChannel;

pub(crate) fn assign_file_to_branch(
    ctx: &mut Context,
    path: &BStr,
    branch_name: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let assignments = path_to_assignments(ctx, path)?;
    let reqs = to_assignment_request(ctx, assignments, Some(branch_name))?;
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Assigned {} → {}.",
            path.to_string().bold(),
            format!("[{branch_name}]").green()
        )?;
    }
    Ok(())
}

pub(crate) fn assign_hunk_to_branch(
    ctx: &mut Context,
    hunk_header: Option<HunkHeader>,
    path_bytes: &BStr,
    branch_name: &str,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let reqs = to_assignment_request(
        ctx,
        Some((hunk_header, path_bytes.to_owned())).into_iter(),
        Some(branch_name),
    )?;
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Assigned a hunk in {} → {}.",
            path_bytes.to_string().bold(),
            format!("[{branch_name}]").green()
        )?;
    }
    Ok(())
}

pub(crate) fn unassign_file(
    ctx: &mut Context,
    path: &BStr,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let assignments = path_to_assignments(ctx, path)?;
    let reqs = to_assignment_request(ctx, assignments, None)?;
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        writeln!(out, "Unassigned {}", path.to_string().bold())?;
    }
    Ok(())
}

pub(crate) fn assign_all(
    ctx: &mut Context,
    from_branch: Option<&str>,
    to_branch: Option<&str>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let from_stack_id = branch_name_to_stack_id(ctx, from_branch)?;
    let to_stack_id = branch_name_to_stack_id(ctx, to_branch)?;

    // Get all assignment requests from the from_stack_id
    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(
        ctx.legacy_project.worktree_dir()?.into(),
    )?
    .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;

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
    do_assignments(ctx, reqs, out)?;
    if let Some(out) = out.for_human() {
        if to_branch.is_some() {
            writeln!(
                out,
                "Assigned all {} changes to {}.",
                from_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unassigned".to_string().bold()),
                to_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unassigned".to_string().bold())
            )?;
        } else {
            writeln!(
                out,
                "Unassigned all {} changes.",
                from_branch
                    .map(|b| format!("[{b}]").green())
                    .unwrap_or_else(|| "unassigned".to_string().bold())
            )?;
        }
    }
    Ok(())
}

fn do_assignments(
    ctx: &mut Context,
    reqs: Vec<HunkAssignmentRequest>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let rejections = but_hunk_assignment::assign(ctx, reqs, None)?;
    if !rejections.is_empty()
        && let Some(out) = out.for_human()
    {
        writeln!(out, "{rejections:#?}")?;
    }
    Ok(())
}

pub(crate) fn branch_name_to_stack_id(
    ctx: &Context,
    branch_name: Option<&str>,
) -> anyhow::Result<Option<StackId>> {
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

fn path_to_assignments<'path>(
    ctx: &mut Context,
    path: &'path BStr,
) -> anyhow::Result<impl Iterator<Item = (Option<HunkHeader>, BString)> + 'path> {
    let changes = but_core::diff::ui::worktree_changes_by_worktree_dir(
        ctx.legacy_project.worktree_dir()?.into(),
    )?
    .changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;
    Ok(assignments
        .into_iter()
        .filter(move |assignment| assignment.path_bytes == path)
        .map(move |assignment| (assignment.hunk_header, assignment.path_bytes)))
}

fn to_assignment_request(
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
