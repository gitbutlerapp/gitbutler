use std::io::IsTerminal;

use but_core::{RepositoryExt, ref_metadata::StackId};
use but_ctx::Context;
use cli_prompts::DisplayPrompt;
use colored::Colorize;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_project::Project;

use crate::{
    CliId, IdMap,
    args::{push, push::Command},
    utils::OutputChannel,
};

pub fn handle(
    args: push::Command,
    ctx: &mut Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let mut id_map = IdMap::new_from_context(ctx)?;
    id_map.add_file_info_from_context(ctx, None)?;

    // Check gerrit mode early
    let gerrit_mode = {
        let repo = ctx.repo.get()?;
        repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false)
    };

    // If no branch_id is provided, show all branches and prompt or push all
    let branch_name = if let Some(ref branch_id) = args.branch_id {
        // Resolve branch_id to actual branch name
        resolve_branch_name(ctx, &id_map, branch_id)?
    } else {
        handle_no_branch_specified(ctx, &ctx.legacy_project, out)?
    };

    // If we have multiple branches to push (from "all" selection)
    if branch_name == "__all__" {
        return push_all_branches(ctx, &ctx.legacy_project, &args, gerrit_mode, out);
    }

    // Single branch push
    push_single_branch(ctx, &ctx.legacy_project, &branch_name, &args, gerrit_mode, out)
}

fn push_single_branch(
    _ctx: &Context,
    project: &Project,
    branch_name: &str,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(args, branch_name, gerrit_mode)?;

    // Call push_stack
    let result: PushResult = but_api::legacy::stack::push_stack(
        project.id,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.to_string(),
        args.run_hooks,
        gerrit_flags,
    )?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Push completed successfully")?;
        writeln!(out, "Pushed to remote: {}", result.remote)?;
        if !gerrit_mode && !result.branch_to_remote.is_empty() {
            for (branch, remote_ref) in &result.branch_to_remote {
                writeln!(out, "  {} -> {}", branch, remote_ref)?;
            }
        }

        // The PushResult struct doesn't have a commits_pushed field,
        // so we'll skip showing the count for now
    }

    Ok(())
}

fn push_all_branches(
    ctx: &Context,
    project: &Project,
    args: &Command,
    gerrit_mode: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let branches_with_info = get_branches_with_unpushed_info(ctx, project)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Pushing all branches with unpushed commits...")?;
        writeln!(out)?;
    }

    let mut total_commits_pushed = 0;
    let mut pushed_branches = Vec::new();

    for (branch_name, unpushed_count, _) in branches_with_info {
        if unpushed_count > 0 {
            if let Some(out) = out.for_human() {
                writeln!(out, "Pushing branch '{}'...", branch_name.bold())?;
            }

            match push_single_branch(ctx, project, &branch_name, args, gerrit_mode, out) {
                Ok(_) => {
                    pushed_branches.push((branch_name.clone(), unpushed_count));
                    total_commits_pushed += unpushed_count;
                }
                Err(e) => {
                    if let Some(out) = out.for_human() {
                        writeln!(out, "  Failed to push '{}': {}", branch_name, e)?;
                    }
                }
            }
        }
    }

    if let Some(out) = out.for_human() {
        writeln!(out)?;
        if pushed_branches.is_empty() {
            writeln!(out, "No branches had unpushed commits.")?;
        } else {
            writeln!(out, "Push completed. Summary:")?;
            for (branch, count) in pushed_branches {
                writeln!(out, "  {}: {} commit{} pushed",
                    branch.bold(),
                    count,
                    if count == 1 { "" } else { "s" }
                )?;
            }
            writeln!(out, "Total commits pushed: {}", total_commits_pushed)?;
        }
    }

    Ok(())
}

fn handle_no_branch_specified(
    ctx: &Context,
    project: &Project,
    out: &mut OutputChannel,
) -> anyhow::Result<String> {
    let branches_with_info = get_branches_with_unpushed_info(ctx, project)?;

    if branches_with_info.is_empty() {
        anyhow::bail!("No branches found in the workspace");
    }

    // Check if we're in an interactive terminal
    let is_interactive = std::io::stdin().is_terminal() && out.for_human().is_some();

    if !is_interactive {
        // Non-interactive mode: push all branches with unpushed commits
        if let Some(out) = out.for_human() {
            writeln!(out, "Non-interactive mode detected. Pushing all branches with unpushed commits...")?;
        }
        return Ok("__all__".to_string());
    }

    // Interactive mode: show branches and prompt for selection
    if let Some(out) = out.for_human() {
        writeln!(out, "Applied branches and unpushed commits:")?;
        writeln!(out)?;

        let mut has_unpushed = false;
        for (branch_name, unpushed_count, stack_name) in &branches_with_info {
            if *unpushed_count > 0 {
                has_unpushed = true;
                writeln!(out, "  {} ({}): {} unpushed commit{}",
                    branch_name.bold(),
                    stack_name.dimmed(),
                    unpushed_count,
                    if *unpushed_count == 1 { "" } else { "s" }
                )?;
            } else {
                writeln!(out, "  {} ({}): up to date",
                    branch_name.dimmed(),
                    stack_name.dimmed()
                )?;
            }
        }

        if !has_unpushed {
            writeln!(out)?;
            writeln!(out, "All branches are up to date with the remote.")?;
            return Err(anyhow::anyhow!("No branches to push"));
        }

        writeln!(out)?;

        // Create selection options
        let mut options = vec!["all - Push all branches with unpushed commits".to_string()];
        for (branch_name, unpushed_count, _) in &branches_with_info {
            if *unpushed_count > 0 {
                options.push(format!("{} - {} unpushed commit{}",
                    branch_name,
                    unpushed_count,
                    if *unpushed_count == 1 { "" } else { "s" }
                ));
            }
        }

        let prompt = cli_prompts::prompts::Selection::new(
            "Which branch(es) would you like to push?",
            options.clone().into_iter(),
        );

        let selection = prompt.display()
            .map_err(|e| anyhow::anyhow!("Selection aborted: {:?}", e))?;

        // Parse the selection
        if selection.starts_with("all ") {
            Ok("__all__".to_string())
        } else {
            // Extract branch name from the selection
            let branch_name = selection.split(" - ").next()
                .ok_or_else(|| anyhow::anyhow!("Invalid selection"))?;
            Ok(branch_name.to_string())
        }
    } else {
        Err(anyhow::anyhow!("Human output required for interactive prompt"))
    }
}

fn get_branches_with_unpushed_info(
    _ctx: &Context,
    project: &Project,
) -> anyhow::Result<Vec<(String, usize, String)>> {
    let stacks = but_api::legacy::workspace::stacks(
        project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    let mut branches_info = Vec::new();

    for stack in stacks {
        if let Some(stack_id) = stack.id {
            let stack_details = but_api::legacy::workspace::stack_details(project.id, Some(stack_id))?;
            let stack_name = stack.name()
                .map(|n| n.to_string())
                .unwrap_or_else(|| "unnamed".to_string());

            // Get branch names from the heads
            for head in &stack.heads {
                let branch_name = head.name.to_string();

                // Find the corresponding branch details to count unpushed commits
                let unpushed_count = if let Some(branch_detail) = stack_details.branch_details.iter()
                    .find(|b| b.name == head.name) {
                    // Count unpushed commits - if there's a remote tracking branch,
                    // compare with it, otherwise all commits are unpushed
                    if branch_detail.remote_tracking_branch.is_some() {
                        // Count the local commits not on the remote
                        branch_detail.commits.len()
                    } else {
                        // No remote tracking branch means all commits are unpushed
                        branch_detail.commits.len()
                    }
                } else {
                    // If no detailed branch info found, assume no unpushed commits
                    0
                };

                branches_info.push((branch_name, unpushed_count, stack_name.clone()));
            }
        }
    }

    // Sort by stack name and then by branch name for consistent ordering
    branches_info.sort_by(|a, b| {
        a.2.cmp(&b.2).then(a.0.cmp(&b.0))
    });

    Ok(branches_info)
}

pub fn get_gerrit_flags(
    args: &Command,
    branch_name: &str,
    gerrit_mode: bool,
) -> anyhow::Result<Vec<but_gerrit::PushFlag>> {
    let has_gerrit_flag = args.wip
        || args.ready
        || !args.hashtag.is_empty()
        || args.topic.is_some()
        || args.topic_from_branch
        || args.private;

    if has_gerrit_flag && !gerrit_mode {
        return Err(anyhow::anyhow!(
            "Gerrit push flags (--wip, --ready, --hashtag/--tag, --topic, --topic-from-branch, --private) can only be used when gerrit_mode is enabled for this repository"
        ));
    }

    if !gerrit_mode {
        return Ok(vec![]);
    }

    let mut flags = Vec::new();

    // Handle Wip/Ready - Ready is default if neither is specified
    if args.wip {
        flags.push(but_gerrit::PushFlag::Wip);
    } else {
        // Default to Ready, or explicit Ready
        flags.push(but_gerrit::PushFlag::Ready);
    }

    // Handle hashtags - can be multiple
    for hashtag in &args.hashtag {
        if hashtag.trim().is_empty() {
            return Err(anyhow::anyhow!("Hashtag cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Hashtag(hashtag.clone()));
    }

    // Handle topic - at most one
    if let Some(topic) = &args.topic {
        if topic.trim().is_empty() {
            return Err(anyhow::anyhow!("Topic cannot be empty"));
        }
        flags.push(but_gerrit::PushFlag::Topic(topic.clone()));
    } else if args.topic_from_branch {
        flags.push(but_gerrit::PushFlag::Topic(branch_name.to_string()));
    }

    // Handle private flag
    if args.private {
        flags.push(but_gerrit::PushFlag::Private);
    }

    Ok(flags)
}

fn resolve_branch_name(
    ctx: &mut Context,
    id_map: &IdMap,
    branch_id: &str,
) -> anyhow::Result<String> {
    // Try to resolve as CliId first
    let cli_ids = id_map.resolve_entity_to_ids(branch_id)?;

    if cli_ids.is_empty() {
        // If no CliId matches, treat as literal branch name but validate it exists
        let available_branches = get_available_branch_names(ctx)?;
        if !available_branches.contains(&branch_id.to_string()) {
            return Err(anyhow::anyhow!(
                "Branch '{}' not found. Available branches:\n{}",
                branch_id,
                format_branch_suggestions(&available_branches)
            ));
        }
        return Ok(branch_id.to_string());
    }

    if cli_ids.len() > 1 {
        let branch_names: Vec<String> = cli_ids
            .iter()
            .filter_map(|id| match id {
                CliId::Branch { name, .. } => Some(name.clone()),
                _ => None,
            })
            .collect();

        if !branch_names.is_empty() {
            return Err(anyhow::anyhow!(
                "Ambiguous branch identifier '{}'. Did you mean one of:\n{}",
                branch_id,
                format_branch_suggestions(&branch_names)
            ));
        } else {
            return Err(anyhow::anyhow!(
                "Identifier '{}' matches multiple non-branch items. Please use a branch name or branch CLI ID.",
                branch_id
            ));
        }
    }

    match &cli_ids[0] {
        CliId::Branch { name, .. } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind_for_humans()
        )),
    }
}

fn get_available_branch_names(ctx: &Context) -> anyhow::Result<Vec<String>> {
    let stacks = crate::legacy::commits::stacks(ctx)?;
    let mut branch_names = Vec::new();

    for stack in stacks {
        for head in &stack.heads {
            branch_names.push(head.name.to_string());
        }
    }

    branch_names.sort();
    branch_names.dedup();
    Ok(branch_names)
}

fn format_branch_suggestions(branches: &[String]) -> String {
    if branches.is_empty() {
        return "  (no branches available)".to_string();
    }

    branches
        .iter()
        .map(|name| format!("  - {}", name))
        .collect::<Vec<_>>()
        .join("\n")
}

fn find_stack_id_by_branch_name(project: &Project, branch_name: &str) -> anyhow::Result<StackId> {
    let stacks = but_api::legacy::workspace::stacks(
        project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Find which stack this branch belongs to
    for stack_entry in &stacks {
        if stack_entry.heads.iter().any(|b| b.name == branch_name) && stack_entry.id.is_some() {
            return Ok(stack_entry.id.unwrap());
        }
    }

    // This should rarely happen now since we validate branch existence earlier,
    // but provide a helpful error just in case
    let available_branches: Vec<String> = stacks
        .iter()
        .flat_map(|s| s.heads.iter().map(|h| h.name.to_string()))
        .collect();

    Err(anyhow::anyhow!(
        "Branch '{}' not found in any stack. Available branches:\n{}",
        branch_name,
        format_branch_suggestions(&available_branches)
    ))
}