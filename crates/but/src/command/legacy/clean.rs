use std::collections::HashSet;

use but_api::WorkspaceState;
use but_core::ref_metadata::StackId;
use nonempty::NonEmpty;

use crate::{
    command::legacy::discard::{self},
    theme::{self, Paint},
    utils::OutputChannel,
};

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
) -> anyhow::Result<Option<WorkspaceState>> {
    let empty_branches = find_empty_branches(ctx, options.include_upstream)?;

    let Some(empty_branches) = NonEmpty::from_vec(empty_branches) else {
        if let Some(out) = out.for_json() {
            out.write_value(&CleanResult {
                deleted: &[],
                failed: &[],
                dry_run: options.dry_run,
            })?;
        } else if let Some(out) = out.for_human() {
            writeln!(out, "No empty branches found.")?;
        }
        return Ok(None);
    };

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
                let t = theme::get();
                writeln!(out, "Would delete branch: {}", t.attention.paint(name))?;
            }
            let count = empty_branches.len();
            let t = theme::get();
            writeln!(
                out,
                "Found {} empty branch(es)",
                t.important.paint(count.to_string())
            )?;
        }
        return Ok(None);
    }

    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (_outcome, ws) = discard::run(
        ctx,
        &mut meta,
        guard.write_permission(),
        discard::DiscardOperation::Branches(
            empty_branches
                .as_ref()
                .try_map(|b| gix::refs::FullName::try_from(format!("refs/heads/{}", b.1)))?,
        ),
        gitbutler_oplog::entry::OperationKind::CleanWorkspace,
    )?;

    let mut deleted = Vec::new();
    for (_stack_id, branch_name) in &empty_branches {
        deleted.push(CleanedBranch {
            name: branch_name.clone(),
        });
    }

    if let Some(out) = out.for_json() {
        out.write_value(&CleanResult {
            deleted: &deleted,
            failed: &[],
            dry_run: false,
        })?;
    } else if let Some(out) = out.for_human() {
        let t = theme::get();
        for branch in &deleted {
            writeln!(
                out,
                "  Deleted branch: {}",
                t.local_branch.paint(&branch.name)
            )?;
        }
        if !deleted.is_empty() {
            writeln!(
                out,
                "{} Deleted {} empty branch(es)",
                t.sym().success,
                t.important.paint(deleted.len().to_string())
            )?;
        }
    }

    Ok(Some(ws))
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

    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;

    let mut empty_branches = Vec::new();

    for stack_entry in &stacks {
        let Some(stack_id) = stack_entry.id else {
            continue;
        };

        // If the stack has assigned worktree changes, none of its branches are empty.
        if stacks_with_changes.contains(&stack_id) {
            continue;
        }

        for branch in &stack_entry.branches {
            let has_local_commits = !branch.commits.is_empty();
            let has_upstream_commits = !branch.upstream_commits.is_empty();

            if has_local_commits {
                continue;
            }

            if has_upstream_commits && !include_upstream {
                continue;
            }

            empty_branches.push((stack_id, branch.name.clone()));
        }
    }

    Ok(empty_branches)
}

/// Returns the set of stack IDs that have at least one worktree change assigned to them.
fn stacks_with_assigned_changes(ctx: &but_ctx::Context) -> anyhow::Result<HashSet<StackId>> {
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
