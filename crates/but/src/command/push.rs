use but_core::{RepositoryExt, ref_metadata::StackId};
use but_ctx::Context;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_project::Project;

use crate::{
    args::{push, push::get_gerrit_flags},
    utils::OutputChannel,
};

pub fn handle(
    args: push::Command,
    project: &Project,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;

    // Check gerrit mode early
    let gerrit_mode = {
        let repo = ctx.repo.get()?;
        repo.git_settings()?.gitbutler_gerrit_mode.unwrap_or(false)
    };

    // Resolve branch_id to actual branch name
    let branch_name = resolve_branch_name(&mut ctx, &args.branch_id)?;

    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, &branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(&args, &branch_name, gerrit_mode)?;

    // Call push_stack
    let result: PushResult = but_api::legacy::stack::push_stack(
        project.id,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.clone(),
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
    }

    Ok(())
}

pub fn print_help(out: &mut OutputChannel) -> std::fmt::Result {
    use std::fmt::Write;
    writeln!(out, "Push a branch/stack to remote")?;
    writeln!(out,)?;
    writeln!(out, "Usage: but push [OPTIONS] <BRANCH_ID>")?;
    writeln!(out,)?;
    writeln!(out, "Arguments:")?;
    writeln!(out, "  <BRANCH_ID>  Branch name or CLI ID to push")?;
    writeln!(out,)?;
    writeln!(out, "Options:")?;
    writeln!(
        out,
        "  -f, --with-force                  Force push even if it's not fast-forward"
    )?;
    writeln!(
        out,
        "  -s, --skip-force-push-protection  Skip force push protection checks"
    )?;
    writeln!(
        out,
        "  -r, --run-hooks                   Run pre-push hooks"
    )?;

    // Check if gerrit mode is enabled and show gerrit options
    if is_gerrit_enabled_for_help() {
        writeln!(out,)?;
        writeln!(out, "Gerrit Options:")?;
        writeln!(
            out,
            "  -w, --wip                         Mark change as work-in-progress"
        )?;
        writeln!(
            out,
            "  -y, --ready                       Mark change as ready for review (default)"
        )?;
        writeln!(
            out,
            "  -a, --hashtag, --tag <TAG>        Add hashtag to change (can be used multiple times)"
        )?;
        writeln!(
            out,
            "  -t, --topic <TOPIC>               Add custom topic to change"
        )?;
        writeln!(
            out,
            "      --tb, --topic-from-branch     Use branch name as topic"
        )?;
        writeln!(
            out,
            "  -p, --private                     Mark change as private"
        )?;
        writeln!(out,)?;
        writeln!(out, "Notes:")?;
        writeln!(
            out,
            "  - --wip and --ready are mutually exclusive. Ready is the default state."
        )?;
        writeln!(
            out,
            "  - --topic and --topic-from-branch are mutually exclusive. At most one topic can be set."
        )?;
        writeln!(
            out,
            "  - Multiple hashtags can be specified by using --hashtag (or --tag) multiple times."
        )?;
        writeln!(
            out,
            "  - Multiple flags can be combined (e.g., --ready --private --tag tag1 --hashtag tag2)."
        )?;
    }

    writeln!(out, "  -h, --help                        Print help")?;

    Ok(())
}

fn is_gerrit_enabled_for_help() -> bool {
    // Parse the -C flag from command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut current_dir = std::path::Path::new(".");

    // Look for -C flag
    for (i, arg) in args.iter().enumerate() {
        if arg == "-C" && i + 1 < args.len() {
            current_dir = std::path::Path::new(&args[i + 1]);
            break;
        }
    }

    // Try to check if we're in a gerrit-enabled repository for help display
    if let Ok(repo) = gix::discover(current_dir)
        && let Ok(settings) = repo.git_settings()
    {
        return settings.gitbutler_gerrit_mode.unwrap_or(false);
    }
    false
}

fn resolve_branch_name(ctx: &mut Context, branch_id: &str) -> anyhow::Result<String> {
    // Try to resolve as CliId first
    let cli_ids = crate::id::CliId::from_str(ctx, branch_id)?;

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
                crate::id::CliId::Branch { name, .. } => Some(name.clone()),
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
        crate::id::CliId::Branch { name, .. } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind()
        )),
    }
}

fn get_available_branch_names(ctx: &Context) -> anyhow::Result<Vec<String>> {
    let stacks = crate::utils::commits::stacks(ctx)?;
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
