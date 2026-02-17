//! Cherry-pick commits from unapplied branches into applied virtual branches.

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::{cherry_apply, virtual_branches, workspace};
use but_cherry_apply::CherryApplyStatus;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use but_workspace::legacy::{StacksFilter, ui::StackEntry};
use cli_prompts::DisplayPrompt;
use colored::Colorize;
use gitbutler_branch_actions::BranchListingFilter;
use gitbutler_oplog::OplogExt;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};
use gix::{revision::walk::Sorting, traverse::commit::simple::CommitTimeOrder};

use crate::{CliId, IdMap, utils::OutputChannel};

/// Handle the `but pick` command.
///
/// Cherry-picks one or more commits from an unapplied branch into an applied virtual branch.
pub fn handle(ctx: &mut Context, out: &mut OutputChannel, source: &str, target_branch: Option<&str>) -> Result<()> {
    // Get applied stacks first - we'll need them for target resolution
    let stacks = workspace::stacks(ctx, Some(StacksFilter::InWorkspace)).context("Failed to list stacks")?;

    if stacks.is_empty() {
        bail!("No applied stacks in workspace. Apply a branch first with 'but branch apply'.");
    }

    // If no target branch was specified, resolve it once upfront so the user
    // isn't prompted repeatedly when picking multiple commits.
    let target_branch_resolved: Option<String>;
    let effective_target = if target_branch.is_some() {
        target_branch
    } else if stacks.len() > 1 && out.can_prompt() {
        let (_stack_id, branch_name) = select_target_interactively(&stacks)?;
        target_branch_resolved = Some(branch_name);
        target_branch_resolved.as_deref()
    } else {
        None
    };

    // Resolve the source to commit(s) (may involve interactive multi-selection for branches)
    let commit_oids = resolve_source_commits(ctx, out, source)?;

    // Save an oplog snapshot before applying picks so the operation can be undone
    {
        let mut guard = ctx.exclusive_worktree_access();
        let _ = ctx.create_snapshot(
            SnapshotDetails::new(OperationKind::CherryPick),
            guard.write_permission(),
        );
    }

    let mut picked = Vec::new();
    for commit_oid in &commit_oids {
        let commit_hex = commit_oid.to_string();

        // Check cherry-apply status for each commit
        let status = cherry_apply::cherry_apply_status(ctx, commit_hex.clone())
            .context("Failed to check cherry-apply status")?;

        // Resolve the target stack based on status and user input
        let (target_stack_id, target_branch_name) =
            resolve_target_stack(ctx, out, &stacks, effective_target, &status, &commit_hex)?;

        // Execute cherry-apply
        cherry_apply::cherry_apply(ctx, commit_hex.clone(), target_stack_id).context("Failed to cherry-pick commit")?;

        picked.push((commit_hex, target_branch_name, target_stack_id));
    }

    // Output results
    for (commit_hex, target_branch_name, _) in &picked {
        let commit_short = &commit_hex[..7.min(commit_hex.len())];
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} {} {} {}",
                "Picked commit".green(),
                commit_short.yellow(),
                "into branch".green(),
                target_branch_name.cyan()
            )?;
        }
    }

    if let Some(out) = out.for_shell() {
        for (commit_hex, _, _) in &picked {
            writeln!(out, "{commit_hex}")?;
        }
    }

    if let Some(out) = out.for_json() {
        if picked.len() == 1 {
            let (commit_hex, target_branch_name, target_stack_id) = &picked[0];
            out.write_value(serde_json::json!({
                "picked_commit": commit_hex,
                "target_branch": target_branch_name,
                "target_stack_id": target_stack_id.to_string(),
            }))?;
        } else {
            let commits: Vec<_> = picked
                .iter()
                .map(|(commit_hex, target_branch_name, target_stack_id)| {
                    serde_json::json!({
                        "picked_commit": commit_hex,
                        "target_branch": target_branch_name,
                        "target_stack_id": target_stack_id.to_string(),
                    })
                })
                .collect();
            out.write_value(serde_json::json!({
                "picked_commits": commits,
            }))?;
        }
    }

    Ok(())
}

/// Resolve the source argument to one or more commit OIDs.
///
/// Tries in order:
/// 1. Unapplied branch name (shows interactive commit selection if available)
/// 2. CLI ID (e.g., "c5")
/// 3. Full or partial commit SHA (via rev_parse)
fn resolve_source_commits(ctx: &mut Context, out: &mut OutputChannel, source: &str) -> Result<Vec<gix::ObjectId>> {
    // Try as an unapplied branch name first (case-insensitive)
    // This takes priority so branch names trigger interactive commit selection
    let branches = virtual_branches::list_branches(
        ctx,
        Some(BranchListingFilter {
            applied: Some(false),
            local: None,
        }),
    )
    .context("Failed to list branches")?;

    let source_lower = source.to_lowercase();
    if let Some(branch) = branches
        .iter()
        .find(|b| b.name.to_string() == source || b.name.to_string().to_lowercase() == source_lower)
    {
        let branch_name = branch.name.to_string();
        return select_commits_from_branch(ctx, out, branch.head, &branch_name);
    }

    // Try using IdMap for CLI IDs
    let id_map = IdMap::new_from_context(ctx, None)?;
    let cli_ids = id_map.parse_using_context(source, ctx)?;

    for cli_id in &cli_ids {
        if let CliId::Commit { commit_id, .. } = cli_id {
            return Ok(vec![*commit_id]);
        }
    }

    // Fall back to git revision (handles full SHA, short SHA, refs)
    {
        let repo = ctx.repo.get()?;
        if let Ok(oid) = repo.rev_parse_single(source) {
            let object_id: gix::ObjectId = oid.detach();
            if repo.find_commit(object_id).is_ok() {
                return Ok(vec![object_id]);
            }
        }
    }

    bail!(
        "Source '{source}' is not a valid commit ID, CLI ID, or unapplied branch name.\n\
Run 'but status' to see available CLI IDs, or 'but branch list' to see branches."
    );
}

/// Select one or more commits from a branch, either interactively or using the head.
fn select_commits_from_branch(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_head: git2::Oid,
    branch_name: &str,
) -> Result<Vec<gix::ObjectId>> {
    use gix::prelude::ObjectIdExt as _;

    let git2_repo = ctx.git2_repo.get()?;
    let gix_repo = ctx.repo.get()?;

    // Get the target branch to find merge base
    let vb_state = gitbutler_stack::VirtualBranchesHandle::new(ctx.project_data_dir());
    let default_target = vb_state.get_default_target()?;

    let branch_head_gix = branch_head.to_gix();
    let target_oid_gix = default_target.sha.to_gix();

    // Find merge base
    let merge_base = gix_repo
        .merge_base(branch_head_gix, target_oid_gix)
        .context("Failed to find merge base")?;

    // Non-interactive mode: use the branch head directly (most recent commit)
    if !out.can_prompt() {
        // Verify branch_head is not the merge base itself (i.e., there are commits to pick)
        if branch_head_gix == merge_base {
            bail!("No commits found on branch '{branch_name}' that aren't already in target.");
        }
        return Ok(vec![branch_head_gix]);
    }

    // Interactive mode: walk commits from branch head to merge base
    let traversal = branch_head_gix
        .attach(&gix_repo)
        .ancestors()
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .with_hidden(Some(merge_base))
        .all()?;

    // Collect commit OIDs and then look up git2 commits for message display
    let commit_oids: Vec<gix::ObjectId> = traversal
        .filter_map(Result::ok)
        .map(|info| info.id)
        .take(50) // Limit to reasonable number
        .collect();

    // Keep OID paired with each commit so indices stay aligned after filtering.
    let commits: Vec<_> = commit_oids
        .iter()
        .filter_map(|oid| git2_repo.find_commit(oid.to_git2()).ok().map(|c| (*oid, c)))
        .collect();

    if commits.is_empty() {
        bail!("No commits found on branch '{branch_name}' that aren't already in target.");
    }

    // If only one commit, use it directly
    if commits.len() == 1 {
        return Ok(vec![commits[0].0]);
    }

    // Interactive multi-selection
    let options: Vec<String> = commits
        .iter()
        .enumerate()
        .map(|(i, (_oid, c))| {
            let short_id = &c.id().to_string()[..7];
            let message = c.summary().unwrap_or("(no message)");
            let truncated: String = message.chars().take(60).collect();
            let display = if truncated.len() < message.len() {
                format!("{}...", truncated.trim_end())
            } else {
                truncated
            };
            format!("[{}] {} {}", i + 1, short_id, display)
        })
        .collect();

    let prompt =
        cli_prompts::prompts::Multiselect::new(&format!("Pick commits from '{branch_name}':"), options.iter().cloned());

    let selections = prompt
        .display()
        .map_err(|e| anyhow::anyhow!("Selection aborted: {e:?}"))?;

    if selections.is_empty() {
        bail!("No commits selected.");
    }

    // Map selected strings back to indices in the paired `commits` vec
    let selected_indices: Vec<usize> = selections
        .iter()
        .filter_map(|sel| options.iter().position(|opt| opt == sel))
        .collect();

    // Return in oldest-first order so they are applied chronologically
    let mut selected_indices_sorted = selected_indices;
    selected_indices_sorted.sort_unstable();
    selected_indices_sorted.reverse();

    Ok(selected_indices_sorted.into_iter().map(|i| commits[i].0).collect())
}

/// Resolve the target stack based on user input and cherry-apply status.
fn resolve_target_stack(
    ctx: &mut Context,
    out: &mut OutputChannel,
    stacks: &[StackEntry],
    target_branch: Option<&str>,
    status: &CherryApplyStatus,
    commit_hex: &str,
) -> Result<(StackId, String)> {
    // Handle status-based constraints
    match status {
        CherryApplyStatus::NoStacks => {
            // This shouldn't happen since we check earlier, but handle it gracefully
            bail!("No applied stacks in workspace. Apply a branch first with 'but branch apply'.");
        }
        CherryApplyStatus::CausesWorkspaceConflict => {
            bail!(
                "Commit {} would cause conflicts with multiple stacks. \
                 Resolve workspace conflicts first.",
                &commit_hex[..7.min(commit_hex.len())]
            );
        }
        CherryApplyStatus::LockedToStack(locked_stack_id) => {
            return handle_locked_to_stack(out, stacks, target_branch, *locked_stack_id);
        }
        CherryApplyStatus::ApplicableToAnyStack => {
            // Can apply to any stack, continue with target resolution
        }
    }

    // If target is specified, find matching stack (by CLI ID or name)
    if let Some(target) = target_branch {
        return find_stack_by_target(ctx, stacks, target);
    }

    // If only one stack, use it automatically
    if stacks.len() == 1 {
        return get_stack_info(&stacks[0]);
    }

    // Multiple stacks and no target specified - need interactive selection
    if !out.can_prompt() {
        bail!(
            "Multiple stacks in workspace. Specify target branch explicitly.\n\
Available stacks: {}",
            format_stack_names(stacks)
        );
    }

    select_target_interactively(stacks)
}

/// Handle the case where a commit is locked to a specific stack.
fn handle_locked_to_stack(
    out: &mut OutputChannel,
    stacks: &[StackEntry],
    target_branch: Option<&str>,
    locked_stack_id: StackId,
) -> Result<(StackId, String)> {
    let locked_stack = stacks
        .iter()
        .find(|s| s.id == Some(locked_stack_id))
        .context("Locked stack not found in workspace")?;

    let locked_branch_name = locked_stack
        .heads
        .first()
        .map(|h| h.name.to_string())
        .unwrap_or_else(|| locked_stack_id.to_string());

    // Warn if user specified a different target
    if let Some(target) = target_branch {
        let target_lower = target.to_lowercase();
        let target_matches = locked_stack
            .heads
            .iter()
            .any(|h| h.name.to_str_lossy() == target || h.name.to_string().to_lowercase() == target_lower);

        if !target_matches && let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} Commit is locked to '{}' due to conflicts. Ignoring specified target.",
                "Warning:".yellow(),
                locked_branch_name.cyan()
            )?;
        }
    }

    Ok((locked_stack_id, locked_branch_name))
}

/// Find a stack by CLI ID or branch name (case-insensitive).
fn find_stack_by_target(ctx: &mut Context, stacks: &[StackEntry], target: &str) -> Result<(StackId, String)> {
    // Try parsing as CLI ID first
    if let Ok(id_map) = IdMap::new_from_context(ctx, None)
        && let Ok(cli_ids) = id_map.parse_using_context(target, ctx)
    {
        for cli_id in &cli_ids {
            if let CliId::Branch {
                stack_id: Some(stack_id),
                name,
                ..
            } = cli_id
            {
                // Verify the stack is in our list of applied stacks
                if stacks.iter().any(|s| s.id == Some(*stack_id)) {
                    return Ok((*stack_id, name.clone()));
                }
            }
        }
    }

    // Fall back to name-based matching (case-insensitive)
    let target_lower = target.to_lowercase();
    for stack in stacks {
        for head in &stack.heads {
            if head.name.to_str_lossy() == target || head.name.to_string().to_lowercase() == target_lower {
                return get_stack_info(stack);
            }
        }
    }

    bail!(
        "Target branch '{}' not found among applied stacks.\n\
Available stacks: {}",
        target,
        format_stack_names(stacks)
    );
}

/// Extract stack ID and branch name from a stack entry.
fn get_stack_info(stack: &StackEntry) -> Result<(StackId, String)> {
    let stack_id = stack.id.context("Stack has no ID")?;
    let branch_name = stack
        .heads
        .first()
        .map(|h| h.name.to_string())
        .unwrap_or_else(|| stack_id.to_string());
    Ok((stack_id, branch_name))
}

/// Format stack names for display in error messages.
fn format_stack_names(stacks: &[StackEntry]) -> String {
    stacks
        .iter()
        .filter_map(|s| s.heads.first().map(|h| h.name.to_string()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Interactive selection of target stack.
fn select_target_interactively(stacks: &[StackEntry]) -> Result<(StackId, String)> {
    let options: Vec<String> = stacks
        .iter()
        .filter_map(|s| s.heads.first().map(|h| h.name.to_string()))
        .collect();

    if options.is_empty() {
        bail!("No branches available for selection.");
    }

    let prompt = cli_prompts::prompts::Selection::new("Select target branch:", options.iter().cloned());

    let selection = prompt
        .display()
        .map_err(|e| anyhow::anyhow!("Selection aborted: {e:?}"))?;

    // Find the selected stack
    for stack in stacks {
        for head in &stack.heads {
            if head.name.to_str_lossy() == selection {
                let stack_id = stack.id.context("Stack has no ID")?;
                return Ok((stack_id, head.name.to_string()));
            }
        }
    }

    bail!("Selected branch not found.");
}
