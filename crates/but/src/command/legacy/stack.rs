use anyhow::{Context as _, bail};
use but_ctx::Context;
use colored::Colorize;
use gix::refs::Category;

use crate::{CliId, IdMap, utils::OutputChannel};

/// Stack a branch onto another branch by rebasing it.
///
/// This command rebases the source branch onto the target branch, creating
/// a stacked branch relationship where the source branch depends on the target.
pub fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    source_str: &str,
    target_str: &str,
) -> anyhow::Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve source branch
    let source_matches = id_map.resolve_entity_to_ids(source_str)?;
    if source_matches.is_empty() {
        bail!(
            "Source branch '{}' not found. If you just performed a Git operation, try running 'but status' to refresh.",
            source_str
        );
    }
    if source_matches.len() > 1 {
        bail!(
            "Source '{}' is ambiguous. Try using more characters to disambiguate.",
            source_str
        );
    }

    let source_id = &source_matches[0];
    let source_branch_name = match source_id {
        CliId::Branch { name, .. } => name,
        _ => {
            bail!(
                "Source '{}' must be a branch, but got {}",
                source_str,
                source_id.kind_for_humans()
            );
        }
    };

    // Resolve target branch
    let target_matches = id_map.resolve_entity_to_ids(target_str)?;
    if target_matches.is_empty() {
        bail!(
            "Target branch '{}' not found. If you just performed a Git operation, try running 'but status' to refresh.",
            target_str
        );
    }
    if target_matches.len() > 1 {
        bail!(
            "Target '{}' is ambiguous. Try using more characters to disambiguate.",
            target_str
        );
    }

    let target_id = &target_matches[0];
    let target_branch_name = match target_id {
        CliId::Branch { name, .. } => name,
        _ => {
            bail!(
                "Target '{}' must be a branch, but got {}",
                target_str,
                target_id.kind_for_humans()
            );
        }
    };

    // Validate that source and target are different
    if source_branch_name == target_branch_name {
        bail!("Source and target branches cannot be the same");
    }

    // Perform the stacking operation
    stack_branch_onto_target(ctx, source_branch_name, target_branch_name)?;

    // Output results
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} Stacked branch {} onto {}",
            "✓".green().bold(),
            source_branch_name.blue(),
            target_branch_name.blue()
        )?;
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{}", source_branch_name)?;
    } else if let Some(out) = out.for_json() {
        let value = serde_json::json!({
            "source_branch": source_branch_name,
            "target_branch": target_branch_name,
            "success": true
        });
        out.write_value(value)?;
    }

    Ok(())
}

/// Performs the actual stacking operation by removing and recreating the source branch
/// with an anchor at the target branch.
fn stack_branch_onto_target(
    ctx: &mut Context,
    source_branch_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    // Get repository and workspace
    let mut guard = ctx.exclusive_worktree_access();
    let (mut meta, ws) = ctx.workspace_and_meta_from_head(guard.write_permission())?;
    let repo = ctx.repo.get()?;

    // Convert branch names to full reference names
    let source_ref = Category::LocalBranch
        .to_full_name(source_branch_name)
        .map_err(anyhow::Error::from)?;
    let target_ref = Category::LocalBranch
        .to_full_name(target_branch_name)
        .map_err(anyhow::Error::from)?;

    // Verify both branches exist
    repo.try_find_reference(&source_ref)?
        .with_context(|| format!("Source branch '{}' does not exist", source_branch_name))?;
    repo.try_find_reference(&target_ref)?
        .with_context(|| format!("Target branch '{}' does not exist", target_branch_name))?;

    // Find the source branch's stack
    let (source_stack, _source_segment) = ws
        .find_segment_and_stack_by_refname(source_ref.as_ref())
        .with_context(|| format!("Could not find stack for branch '{}'", source_branch_name))?;

    // Check if it's safe to remove the branch
    // If this is the last branch in the stack, we can't remove it without leaving anonymous segments
    let is_last_branch_in_stack = source_stack
        .segments
        .iter()
        .filter(|seg| seg.ref_info.is_some())
        .count()
        == 1;

    if is_last_branch_in_stack {
        bail!(
            "Cannot stack '{}' as it is the only named branch in its stack. \
             Use 'but branch new --anchor {}' to create a new branch stacked on '{}' instead.",
            source_branch_name,
            target_branch_name,
            target_branch_name
        );
    }

    // Remove the old branch reference
    but_workspace::branch::remove_reference(
        source_ref.as_ref(),
        &repo,
        &ws,
        &mut meta,
        but_workspace::branch::remove_reference::Options {
            avoid_anonymous_stacks: true,
            keep_metadata: true, // Keep metadata so we preserve commit history
        },
    )?;

    // Recreate the branch with the target as anchor
    let anchor = but_workspace::branch::create_reference::Anchor::AtSegment {
        ref_name: std::borrow::Cow::Borrowed(target_ref.as_ref()),
        position: but_workspace::branch::create_reference::Position::Above,
    };

    let stack_id = source_stack.id.unwrap_or_else(but_core::ref_metadata::StackId::generate);

    but_workspace::branch::create_reference(
        source_ref.as_ref(),
        Some(anchor),
        &repo,
        &ws,
        &mut meta,
        |_| stack_id,
        None,
    )?;

    Ok(())
}
