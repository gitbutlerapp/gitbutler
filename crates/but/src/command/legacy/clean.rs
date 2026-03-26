use std::collections::HashSet;

use but_core::ref_metadata::StackId;
use colored::Colorize;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};

use crate::utils::OutputChannel;

/// Options for the clean command.
pub struct CleanOptions {
    pub dry_run: bool,
    pub include_upstream: bool,
}

/// A branch that was deleted (or would be deleted in dry-run mode).
#[derive(Debug, serde::Serialize)]
struct CleanedBranch {
    name: String,
}

/// A branch deletion that failed.
#[derive(Debug, Clone, serde::Serialize)]
struct FailedBranch {
    name: String,
    error: String,
}

/// JSON output for the clean command.
#[derive(Debug, serde::Serialize)]
struct CleanResult<'a> {
    deleted: &'a [CleanedBranch],
    #[serde(skip_serializing_if = "<[FailedBranch]>::is_empty")]
    failed: &'a [FailedBranch],
    dry_run: bool,
}

pub fn handle(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    options: CleanOptions,
) -> anyhow::Result<()> {
    let empty_branches = find_empty_branches(ctx, options.include_upstream)?;

    if empty_branches.is_empty() {
        if let Some(out) = out.for_json() {
            out.write_value(&CleanResult {
                deleted: &[],
                failed: &[],
                dry_run: options.dry_run,
            })?;
        } else if let Some(out) = out.for_human() {
            writeln!(out, "No empty branches found.")?;
        } else if let Some(_out) = out.for_shell() {
            // No output for shell when nothing to clean.
        }
        return Ok(());
    }

    if options.dry_run {
        let cleaned: Vec<CleanedBranch> = empty_branches
            .iter()
            .map(|(_, name)| CleanedBranch { name: name.clone() })
            .collect();

        if let Some(out) = out.for_json() {
            out.write_value(&CleanResult {
                deleted: &cleaned,
                failed: &[],
                dry_run: true,
            })?;
        } else if let Some(out) = out.for_human() {
            for (_, name) in &empty_branches {
                writeln!(out, "Would delete branch: {}", name.yellow())?;
            }
            let count = empty_branches.len();
            writeln!(out, "Found {} empty branch(es)", count.to_string().bold())?;
        } else if let Some(out) = out.for_shell() {
            for (_, name) in &empty_branches {
                writeln!(out, "{name}")?;
            }
        }
        return Ok(());
    }

    // Create a single oplog snapshot before performing all deletions.
    let mut guard = ctx.exclusive_worktree_access();
    ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CleanWorkspace),
        guard.write_permission(),
    )
    .ok();

    let mut deleted = Vec::new();
    let mut failed = Vec::new();

    for (_stack_id, branch_name) in &empty_branches {
        match but_api::legacy::stack::remove_branch_only(ctx, branch_name, guard.write_permission())
        {
            Ok(()) => {
                deleted.push(CleanedBranch {
                    name: branch_name.clone(),
                });
            }
            Err(err) => {
                failed.push(FailedBranch {
                    name: branch_name.clone(),
                    error: err.to_string(),
                });
            }
        }
    }

    let num_failed = failed.len();

    if let Some(out) = out.for_json() {
        out.write_value(&CleanResult {
            deleted: &deleted,
            failed: &failed,
            dry_run: false,
        })?;
    } else if let Some(out) = out.for_human() {
        for branch in &deleted {
            writeln!(out, "  Deleted branch: {}", branch.name.green())?;
        }
        if !deleted.is_empty() {
            writeln!(
                out,
                "{} Deleted {} empty branch(es)",
                "✓".green().bold(),
                deleted.len().to_string().bold()
            )?;
        }
        for f in &failed {
            writeln!(
                out,
                "{} Failed to delete branch '{}': {}",
                "!".red().bold(),
                f.name,
                f.error
            )?;
        }
    } else if let Some(out) = out.for_shell() {
        for branch in &deleted {
            writeln!(out, "{}", branch.name)?;
        }
    }

    if num_failed == 0 {
        Ok(())
    } else {
        anyhow::bail!("failed to delete {num_failed} branch(es)")
    }
}

/// Find all empty branches in the workspace.
///
/// Returns a list of `(StackId, branch_name)` pairs for branches that are empty.
/// A branch is considered empty if:
/// - It has no local commits
/// - The stack has no assigned worktree changes
/// - It has no upstream-only commits (unless `include_upstream` is true)
fn find_empty_branches(
    ctx: &mut but_ctx::Context,
    include_upstream: bool,
) -> anyhow::Result<Vec<(StackId, String)>> {
    // Get the set of stack IDs that have worktree changes assigned to them.
    let stacks_with_changes = stacks_with_assigned_changes(ctx)?;

    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let mut empty_branches = Vec::new();

    for stack_entry in &stacks {
        let Some(stack_id) = stack_entry.id else {
            continue;
        };

        // If the stack has assigned worktree changes, none of its branches are empty.
        if stacks_with_changes.contains(&stack_id) {
            continue;
        }

        let details = but_api::legacy::workspace::stack_details(ctx, Some(stack_id))?;

        for branch in &details.branch_details {
            let has_local_commits = !branch.commits.is_empty();
            let has_upstream_commits = !branch.upstream_commits.is_empty();

            if has_local_commits {
                continue;
            }

            if has_upstream_commits && !include_upstream {
                continue;
            }

            empty_branches.push((stack_id, branch.name.to_string()));
        }
    }

    Ok(empty_branches)
}

/// Returns the set of stack IDs that have at least one worktree change assigned to them.
fn stacks_with_assigned_changes(ctx: &mut but_ctx::Context) -> anyhow::Result<HashSet<StackId>> {
    let context_lines = ctx.settings.context_lines;
    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _err) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;

    Ok(assignments.into_iter().filter_map(|a| a.stack_id).collect())
}
