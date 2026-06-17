//! Cherry-pick commits from unapplied branches into applied virtual branches.

use crate::legacy::workspace::HeadInfoStack;
use crate::theme::{self, Paint};
use anyhow::{Context as _, Result, bail};
use but_api::legacy::{cherry_apply, virtual_branches};
use but_cherry_apply::CherryApplyStatus;
use but_core::{RepositoryExt, ref_metadata::StackId, sync::RepoShared};
use but_ctx::Context;
use gitbutler_branch_actions::BranchListingFilter;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gix::{revision::walk::Sorting, traverse::commit::simple::CommitTimeOrder};

use crate::{
    CliId, IdMap,
    command::legacy::workspace_target,
    utils::{OutputChannel, WriteWithUtils, shorten_hex_object_id, shorten_object_id},
};

/// Handle the `but pick` command.
///
/// Cherry-picks one or more commits from an unapplied branch into an applied virtual branch.
pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source: &str,
    target_branch: Option<&str>,
) -> Result<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    // Get applied stacks first - we'll need them for target resolution
    let stacks = crate::legacy::workspace::applied_stacks(ctx).context("Failed to list stacks")?;

    if stacks.is_empty() {
        bail!("No applied stacks in workspace. Apply a branch first with 'but branch apply'.");
    }

    // If no target branch was specified, resolve it once upfront so the user
    // isn't prompted repeatedly when picking multiple commits.
    let target_branch_resolved: Option<String>;
    let effective_target = if target_branch.is_some() {
        target_branch
    } else if stacks.len() > 1 && out.can_prompt() {
        let mut input = out
            .prepare_for_terminal_input()
            .context("Human input required - run this in a terminal")?;
        let (_stack_id, branch_name) = select_target_interactively(&mut input, &stacks)?;
        target_branch_resolved = Some(branch_name);
        target_branch_resolved.as_deref()
    } else {
        None
    };

    // Resolve the source to commit(s) (may involve interactive multi-selection for branches)
    let commit_oids = resolve_source_commits(ctx, guard.read_permission(), out, &id_map, source)?;

    // Save an oplog snapshot before applying picks so the operation can be undone
    let _ = ctx.create_snapshot(
        SnapshotDetails::new(OperationKind::CherryPick),
        guard.write_permission(),
    );

    let mut picked = Vec::new();
    for commit_oid in &commit_oids {
        let commit_hex = commit_oid.to_string();

        // Check cherry-apply status for each commit
        let status =
            cherry_apply::cherry_apply_status_with_perm(ctx, *commit_oid, guard.read_permission())
                .context("Failed to check cherry-apply status")?;

        // Resolve the target stack based on status and user input
        let (target_stack_id, target_branch_name) = resolve_target_stack(
            ctx,
            out,
            &id_map,
            &stacks,
            effective_target,
            &status,
            &commit_hex,
        )?;

        // Execute cherry-apply
        cherry_apply::cherry_apply_with_perm(
            ctx,
            *commit_oid,
            target_stack_id,
            guard.write_permission(),
        )
        .context("Failed to cherry-pick commit")?;

        picked.push((commit_hex, target_branch_name, target_stack_id));
    }

    // Output results
    let t = theme::get();
    let repo = ctx.repo.get()?.clone().for_commit_shortening();
    for (commit_hex, target_branch_name, _) in &picked {
        let commit_short = shorten_hex_object_id(&repo, commit_hex);
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} {} {} {}",
                t.success.paint("Picked commit"),
                t.commit_id.paint(&commit_short),
                t.success.paint("into branch"),
                t.local_branch.paint(target_branch_name)
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
fn resolve_source_commits(
    ctx: &Context,
    perm: &RepoShared,
    out: &mut OutputChannel,
    id_map: &IdMap,
    source: &str,
) -> Result<Vec<gix::ObjectId>> {
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
        return select_commits_from_branch(ctx, perm, out, branch.head, &branch_name);
    }

    // Try using IdMap for CLI IDs
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
    ctx: &Context,
    perm: &RepoShared,
    out: &mut OutputChannel,
    branch_head: gix::ObjectId,
    branch_name: &str,
) -> Result<Vec<gix::ObjectId>> {
    use gix::prelude::ObjectIdExt as _;

    let repo = ctx.repo.get()?;

    // Find merge base
    let (merge_base, _) =
        workspace_target::merge_base_with_target_with_perm(ctx, perm, branch_head)?;

    // Non-interactive mode: use the branch head directly (most recent commit)
    if !out.can_prompt() {
        // Verify branch_head is not the merge base itself (i.e., there are commits to pick)
        if branch_head == merge_base {
            bail!("No commits found on branch '{branch_name}' that aren't already in target.");
        }
        return Ok(vec![branch_head]);
    }

    // Interactive mode: walk commits from branch head to merge base
    let traversal = branch_head
        .attach(&repo)
        .ancestors()
        .sorting(Sorting::ByCommitTime(CommitTimeOrder::NewestFirst))
        .with_hidden(Some(merge_base))
        .all()?;

    // Collect commit OIDs and then decode them for message display
    let commit_oids: Vec<gix::ObjectId> = traversal
        .filter_map(Result::ok)
        .map(|info| info.id)
        .take(50) // Limit to reasonable number
        .collect();

    // Keep OID paired with each commit summary so indices stay aligned after filtering.
    let commits: Vec<_> = commit_oids
        .iter()
        .filter_map(|oid| {
            repo.find_commit(*oid).ok().and_then(|commit| {
                let commit = commit.decode().ok()?;
                (*oid, super::commit_summary(&commit)).into()
            })
        })
        .collect();

    if commits.is_empty() {
        bail!("No commits found on branch '{branch_name}' that aren't already in target.");
    }

    // If only one commit, use it directly
    if commits.len() == 1 {
        return Ok(vec![commits[0].0]);
    }

    // Interactive multi-selection
    let options = commits
        .iter()
        .enumerate()
        .map(|(i, (oid, message))| {
            let short_id = shorten_object_id(&repo, *oid);
            let display = out.truncate_if_unpaged(message, 60);
            (format!("[{}] {} {}", i + 1, short_id, display), i)
        })
        .collect::<Vec<_>>();
    let options = nonempty::NonEmpty::from_vec(options).context("No commits available")?;
    let mut input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;

    let selections = input
        .prompt_multi_select(format!("Pick commits from '{branch_name}'"), &options)?
        .ok_or_else(|| anyhow::anyhow!("Selection aborted"))?;

    if selections.is_empty() {
        bail!("No commits selected.");
    }

    // Return in oldest-first order so they are applied chronologically
    let mut selected_indices_sorted = selections.into_iter().copied().collect::<Vec<_>>();
    selected_indices_sorted.sort_unstable();
    selected_indices_sorted.reverse();

    Ok(selected_indices_sorted
        .into_iter()
        .map(|i| commits[i].0)
        .collect())
}

/// Resolve the target stack based on user input and cherry-apply status.
fn resolve_target_stack(
    ctx: &Context,
    out: &mut OutputChannel,
    id_map: &IdMap,
    stacks: &[HeadInfoStack],
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
            let repo = ctx.repo.get()?;
            let commit_short = shorten_hex_object_id(&repo, commit_hex);
            bail!(
                "Commit {commit_short} would cause conflicts with multiple stacks. \
                 Resolve workspace conflicts first."
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
        return find_stack_by_target(ctx, id_map, stacks, target);
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

    let mut input = out
        .prepare_for_terminal_input()
        .context("Human input required - run this in a terminal")?;
    select_target_interactively(&mut input, stacks)
}

/// Handle the case where a commit is locked to a specific stack.
fn handle_locked_to_stack(
    out: &mut OutputChannel,
    stacks: &[HeadInfoStack],
    target_branch: Option<&str>,
    locked_stack_id: StackId,
) -> Result<(StackId, String)> {
    let locked_stack = stacks
        .iter()
        .find(|stack| stack.id == Some(locked_stack_id))
        .context("Locked stack not found in workspace")?;

    let locked_branch_name = locked_stack
        .top_branch_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| locked_stack_id.to_string());

    // Warn if user specified a different target
    if let Some(target) = target_branch {
        let target_lower = target.to_lowercase();
        let target_matches = locked_stack
            .branches
            .iter()
            .any(|branch| branch.name == target || branch.name.to_lowercase() == target_lower);

        let t = theme::get();
        if !target_matches && let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} Commit is locked to '{}' due to conflicts. Ignoring specified target.",
                t.attention.paint("Warning:"),
                t.local_branch.paint(&locked_branch_name)
            )?;
        }
    }

    Ok((locked_stack_id, locked_branch_name))
}

/// Find a stack by CLI ID or branch name (case-insensitive).
fn find_stack_by_target(
    ctx: &Context,
    id_map: &IdMap,
    stacks: &[HeadInfoStack],
    target: &str,
) -> Result<(StackId, String)> {
    // Try parsing as CLI ID first
    if let Ok(cli_ids) = id_map.parse_using_context(target, ctx) {
        for cli_id in &cli_ids {
            if let CliId::Branch {
                stack_id: Some(stack_id),
                name,
                ..
            } = cli_id
            {
                // Verify the stack is in our list of applied stacks
                if stacks.iter().any(|stack| stack.id == Some(*stack_id)) {
                    return Ok((*stack_id, name.clone()));
                }
            }
        }
    }

    // Fall back to name-based matching (case-insensitive)
    let target_lower = target.to_lowercase();
    for stack in stacks {
        for branch in &stack.branches {
            if branch.name == target || branch.name.to_lowercase() == target_lower {
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
fn get_stack_info(stack: &HeadInfoStack) -> Result<(StackId, String)> {
    let stack_id = stack.id.context("Stack has no ID")?;
    let branch_name = stack
        .top_branch_name()
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| stack_id.to_string());
    Ok((stack_id, branch_name))
}

/// Format stack names for display in error messages.
fn format_stack_names(stacks: &[HeadInfoStack]) -> String {
    stacks
        .iter()
        .filter_map(|stack| stack.top_branch_name().map(ToOwned::to_owned))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Interactive selection of target stack.
fn select_target_interactively(
    input: &mut crate::utils::InputOutputChannel<'_>,
    stacks: &[HeadInfoStack],
) -> Result<(StackId, String)> {
    let options = stacks
        .iter()
        .filter_map(|stack| {
            let stack_id = stack.id?;
            let branch_name = stack.top_branch_name()?.to_owned();
            Some((branch_name.clone(), (stack_id, branch_name)))
        })
        .collect::<Vec<_>>();
    let options =
        nonempty::NonEmpty::from_vec(options).context("No branches available for selection")?;

    input
        .prompt_select("Select target branch", &options)?
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Selection aborted"))
}
