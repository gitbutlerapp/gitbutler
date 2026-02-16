use anyhow::bail;
use bstr::BString;
use but_core::ref_metadata::StackId;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use colored::Colorize;
use gix::ObjectId;

use super::undo::stack_id_by_commit_id;
use crate::{CliId, IdMap, command::legacy::ai, utils::OutputChannel};

pub(crate) fn commits(
    ctx: &mut Context,
    source: &ObjectId,
    destination: &ObjectId,
    custom_message: Option<&str>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Delegate to the shared squashing logic
    squash_commits_internal(ctx, vec![*source], *destination, false, custom_message, None, out)
}

/// Handler for `but squash` command with support for:
/// 1. Multiple commits: `but squash <commit1> <commit2> <commit3>` - squashes 1 and 2 into 3
/// 2. Commit range: `but squash <commit1>..<commit4>` - squashes all in range into last
/// 3. Branch name: `but squash <branch>` - squashes all commits in branch into bottom-most
pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commits: &[String],
    drop_message: bool,
    custom_message: Option<&str>,
    ai: Option<Option<String>>,
) -> anyhow::Result<()> {
    if commits.is_empty() {
        bail!("At least one commit or branch name must be provided");
    }

    let id_map = IdMap::new_from_context(ctx, None)?;

    // If there's only one argument, it could be a branch name or a range
    if commits.len() == 1 {
        let entity_str = &commits[0];

        // First try to resolve as a single entity (branch name or commit)
        let matches = id_map.parse_using_context(entity_str, ctx)?;

        // If we get exactly one match, handle it
        if matches.len() == 1 {
            let entity = &matches[0];

            // Check if it's a branch - if so, squash all commits in the branch
            if let CliId::Branch { name, stack_id, .. } = entity {
                return squash_branch_commits(ctx, out, name, *stack_id, drop_message, custom_message, ai, &id_map);
            }

            // If it's a single commit, error - need at least 2 commits
            if let CliId::Commit { .. } = entity {
                bail!("Need at least 2 commits to squash. To squash all commits in a branch, use the branch name.");
            }

            bail!("'{}' must be a branch name or commit identifier", entity_str);
        }

        // If we get multiple matches, it's ambiguous
        if matches.len() > 1 {
            bail!("'{}' is ambiguous - matches multiple entities", entity_str);
        }

        // No exact match found - try parsing as a range or list if it contains special characters
        if entity_str.contains("..") {
            let sources = parse_commit_range(ctx, &id_map, entity_str)?;
            if sources.len() < 2 {
                bail!("Need at least 2 commits to squash");
            }
            return handle_multi_commit_squash(ctx, out, sources, drop_message, custom_message, ai);
        }

        if entity_str.contains(',') {
            let sources = parse_commit_list(ctx, &id_map, entity_str)?;
            if sources.len() < 2 {
                bail!("Need at least 2 commits to squash");
            }
            return handle_multi_commit_squash(ctx, out, sources, drop_message, custom_message, ai);
        }

        // If it contains a single dash (but not ..), it might be a branch name with a dash
        // This case is already handled above by exact match, so we get here only if no match found

        // Nothing worked
        bail!("No matching branch or commit found for '{}'", entity_str);
    }

    // Multiple separate commit arguments - resolve each one
    let mut sources = Vec::new();
    for commit_str in commits {
        let matches = id_map.parse_using_context(commit_str, ctx)?;
        if matches.is_empty() {
            bail!("No matching commit found for '{}'", commit_str);
        }
        if matches.len() > 1 {
            bail!("'{}' is ambiguous - matches multiple entities", commit_str);
        }
        sources.push(matches[0].clone());
    }

    handle_multi_commit_squash(ctx, out, sources, drop_message, custom_message, ai)
}

/// Helper function to handle squashing multiple commits
fn handle_multi_commit_squash(
    ctx: &mut Context,
    out: &mut OutputChannel,
    sources: Vec<CliId>,
    drop_message: bool,
    custom_message: Option<&str>,
    ai: Option<Option<String>>,
) -> anyhow::Result<()> {
    if sources.len() < 2 {
        bail!("Need at least 2 commits to squash");
    }

    // Extract commit OIDs and validate all are commits
    let mut commit_oids = Vec::new();
    for source in &sources {
        match source {
            CliId::Commit { commit_id, .. } => {
                commit_oids.push(*commit_id);
            }
            other => {
                bail!(
                    "Cannot squash {} - it is {}. All arguments must be commits.",
                    other.to_short_string().blue().underline(),
                    other.kind_for_humans().yellow()
                );
            }
        }
    }

    // The last commit is the target
    let target_oid = commit_oids.pop().expect("We validated sources.len() >= 2");

    // Delegate to the shared squashing logic
    squash_commits_internal(ctx, commit_oids, target_oid, drop_message, custom_message, ai, out)
}

/// Internal shared logic for squashing commits
fn squash_commits_internal(
    ctx: &mut Context,
    source_oids: Vec<ObjectId>,
    target_oid: ObjectId,
    drop_message: bool,
    custom_message: Option<&str>,
    ai: Option<Option<String>>,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Validate all commits are on the same stack
    let target_stack = stack_id_by_commit_id(ctx, &target_oid)?;
    for source_oid in &source_oids {
        let source_stack = stack_id_by_commit_id(ctx, source_oid)?;
        if source_stack != target_stack {
            bail!(
                "Commits must be on the same stack to squash them together. Try squashing commits within a single branch or stack."
            );
        }
    }

    // Collect commit messages if we need them for AI generation
    let (source_messages, destination_message) = if ai.is_some() {
        let repo = ctx.repo.get()?;

        // Get source commit messages
        let mut source_msgs = Vec::new();
        for source_oid in &source_oids {
            let commit = repo.find_commit(*source_oid)?;
            let message_ref = commit.message()?;
            let full_message = if let Some(body) = message_ref.body {
                format!("{}\n\n{}", message_ref.title, body)
            } else {
                message_ref.title.to_string()
            };
            source_msgs.push(full_message);
        }

        // Get destination commit message
        let target_commit = repo.find_commit(target_oid)?;
        let message_ref = target_commit.message()?;
        let dest_message = if let Some(body) = message_ref.body {
            format!("{}\n\n{}", message_ref.title, body)
        } else {
            message_ref.title.to_string()
        };

        (Some(source_msgs), Some(dest_message))
    } else {
        (None, None)
    };

    // If drop_message is set, get the target commit's message BEFORE squashing
    let target_message = if drop_message {
        let repo = ctx.repo.get()?;
        let target_commit = repo.find_commit(target_oid)?;
        let message_ref = target_commit.message()?;
        let full_message = if let Some(body) = message_ref.body {
            format!("{}\n\n{}", message_ref.title, body)
        } else {
            message_ref.title.to_string()
        };
        Some(full_message)
    } else {
        None
    };

    // Perform the squash using the API - it can handle multiple source commits
    let new_commit_oid = gitbutler_branch_actions::squash_commits(
        ctx,
        target_stack,
        source_oids.iter().map(|oid| oid.to_git2()).collect(),
        target_oid.to_git2(),
    )?;

    // Determine the final message and apply if needed
    let final_commit_oid = if let Some(user_summary) = ai {
        // Use AI to generate the commit message
        let ai_message = ai::generate_commit_message_from_multiple_messages(
            out,
            source_messages.unwrap_or_default(),
            destination_message.unwrap_or_default(),
            user_summary,
        )?;
        but_api::commit::commit_reword_only(ctx, new_commit_oid.to_gix(), BString::from(ai_message))?.to_git2()
    } else if let Some(msg) = custom_message {
        but_api::commit::commit_reword_only(ctx, new_commit_oid.to_gix(), BString::from(msg))?.to_git2()
    } else if let Some(target_msg) = target_message {
        but_api::commit::commit_reword_only(ctx, new_commit_oid.to_gix(), BString::from(target_msg))?.to_git2()
    } else {
        new_commit_oid
    };

    // Output message based on context
    if let Some(out) = out.for_human() {
        if source_oids.len() == 1 {
            // Single commit squash (for backwards compatibility with `but rub`)
            writeln!(
                out,
                "Squashed {} → {}",
                source_oids[0].to_string()[..7].blue(),
                final_commit_oid.to_gix().to_string()[..7].blue()
            )?
        } else {
            // Multiple commits squash
            writeln!(
                out,
                "Squashed {} commits → {}",
                source_oids.len(),
                final_commit_oid.to_gix().to_string()[..7].blue()
            )?
        }
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "ok": true,
            "new_commit_id": final_commit_oid.to_gix().to_string(),
            "squashed_count": source_oids.len(),
        }))?;
    }
    Ok(())
}

/// Helper function to squash all commits in a branch into the bottom-most commit
#[allow(clippy::too_many_arguments)]
fn squash_branch_commits(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_name: &str,
    stack_id: Option<StackId>,
    drop_message: bool,
    custom_message: Option<&str>,
    ai: Option<Option<String>>,
    id_map: &IdMap,
) -> anyhow::Result<()> {
    // Find the stack containing this branch
    let stack_id =
        stack_id.ok_or_else(|| anyhow::anyhow!("Branch '{}' is not associated with a stack", branch_name))?;

    // Find all commits in this branch (segment)
    let mut branch_commits: Vec<gix::ObjectId> = Vec::new();
    for stack in id_map.stacks() {
        if stack.id == Some(stack_id) {
            for segment in &stack.segments {
                if let Some(seg_branch_name) = segment.branch_name()
                    && seg_branch_name == branch_name.as_bytes()
                {
                    // Collect all workspace commits in this segment
                    for commit in &segment.workspace_commits {
                        branch_commits.push(commit.commit_id());
                    }
                    break;
                }
            }
            break;
        }
    }

    if branch_commits.is_empty() {
        bail!("No commits found in branch '{}'", branch_name);
    }

    if branch_commits.len() < 2 {
        bail!("Branch '{}' has only one commit, nothing to squash", branch_name);
    }

    // The commits are in order from newest (top) to oldest (bottom)
    // We want to squash all commits into the bottom-most (last in the list)
    let target_oid = branch_commits.pop().expect("We validated len >= 2");
    let source_oids = branch_commits; // These are already ObjectIds

    // Delegate to the shared squashing logic
    squash_commits_internal(ctx, source_oids, target_oid, drop_message, custom_message, ai, out)?;

    // Add branch-specific output message
    if let Some(out) = out.for_human() {
        writeln!(out, "Squashed all commits in branch '{}'", branch_name.blue())?
    }
    Ok(())
}

/// Parse a commit range like "c1..c3" and return all commits in the range
fn parse_commit_range(ctx: &mut Context, id_map: &IdMap, range_str: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = range_str.split("..").collect();
    if parts.len() != 2 {
        bail!("Range format should be 'start..end', got '{}'", range_str);
    }

    let start_str = parts[0];
    let end_str = parts[1];

    // Resolve start and end to commit IDs
    let start_matches = id_map.parse_using_context(start_str, ctx)?;
    let end_matches = id_map.parse_using_context(end_str, ctx)?;

    if start_matches.len() != 1 {
        bail!("Start of range '{}' must match exactly one commit", start_str);
    }
    if end_matches.len() != 1 {
        bail!("End of range '{}' must match exactly one commit", end_str);
    }

    let start_id = &start_matches[0];
    let end_id = &end_matches[0];

    // Both must be commits
    let (start_commit_oid, end_commit_oid) = match (start_id, end_id) {
        (
            CliId::Commit {
                commit_id: start_oid, ..
            },
            CliId::Commit { commit_id: end_oid, .. },
        ) => (start_oid, end_oid),
        _ => {
            bail!("Range endpoints must be commits, not other types");
        }
    };

    // Verify both commits are on the same stack FIRST
    let start_stack = stack_id_by_commit_id(ctx, start_commit_oid)?;
    let end_stack = stack_id_by_commit_id(ctx, end_commit_oid)?;
    if start_stack != end_stack {
        bail!(
            "Range endpoints must be on the same stack. '{}' and '{}' are on different stacks.",
            start_str,
            end_str
        );
    }

    // Get all commits in order from the SAME stack only
    let mut all_commits_in_order: Vec<(gix::ObjectId, CliId)> = Vec::new();
    for stack in id_map.stacks() {
        // Only process the stack that contains our commits
        if stack.id != Some(start_stack) {
            continue;
        }

        for segment in &stack.segments {
            for commit in &segment.workspace_commits {
                let commit_oid = commit.commit_id();
                // Find the CliId for this commit
                if let Some(cli_id) = id_map
                    .parse_using_context(&commit_oid.to_string(), ctx)
                    .ok()
                    .and_then(|matches| matches.first().cloned())
                {
                    all_commits_in_order.push((commit_oid, cli_id));
                }
            }
        }
        break; // Found our stack, no need to continue
    }

    // Find the positions of start and end commits
    let start_pos = all_commits_in_order.iter().position(|(oid, _)| oid == start_commit_oid);
    let end_pos = all_commits_in_order.iter().position(|(oid, _)| oid == end_commit_oid);

    match (start_pos, end_pos) {
        (Some(start_idx), Some(end_idx)) => {
            let range = if start_idx <= end_idx {
                &all_commits_in_order[start_idx..=end_idx]
            } else {
                &all_commits_in_order[end_idx..=start_idx]
            };

            Ok(range.iter().map(|(_, cli_id)| cli_id.clone()).collect())
        }
        _ => {
            bail!(
                "Could not find range from '{}' to '{}'. Make sure both commits exist in the stack.",
                start_str,
                end_str
            );
        }
    }
}

/// Parse a comma-separated list of commits like "c1,c2,c3"
fn parse_commit_list(ctx: &mut Context, id_map: &IdMap, list_str: &str) -> anyhow::Result<Vec<CliId>> {
    let parts: Vec<&str> = list_str.split(',').collect();
    let mut result = Vec::new();

    for part in parts {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }

        let matches = id_map.parse_using_context(part, ctx)?;
        if matches.is_empty() {
            bail!("Commit '{}' not found", part);
        }
        if matches.len() > 1 {
            bail!("'{}' is ambiguous - matches multiple entities", part);
        }

        // Verify it's a commit
        match &matches[0] {
            CliId::Commit { .. } => result.push(matches[0].clone()),
            other => {
                bail!("'{}' is {} but must be a commit", part, other.kind_for_humans());
            }
        }
    }

    if result.is_empty() {
        bail!("Commit list is empty");
    }

    Ok(result)
}
