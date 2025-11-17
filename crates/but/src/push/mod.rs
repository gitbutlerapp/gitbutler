use crate::utils::OutputChannel;
use but_core::{RepositoryExt, ref_metadata::StackId};
use but_settings::AppSettings;
use gitbutler_branch_actions::internal::PushResult;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

#[derive(Debug, clap::Parser)]
pub struct Args {
    /// Branch name or CLI ID to push
    pub branch_id: String,
    /// Force push even if it's not fast-forward
    #[clap(long, short = 'f', default_value_t = true)]
    pub with_force: bool,
    /// Skip force push protection checks
    #[clap(long, short = 's')]
    pub skip_force_push_protection: bool,
    /// Run pre-push hooks
    #[clap(long, short = 'r', default_value_t = true)]
    pub run_hooks: bool,
    /// Mark change as work-in-progress (Gerrit). Mutually exclusive with --ready.
    #[clap(long, short = 'w', conflicts_with = "ready", hide = true)]
    pub wip: bool,
    /// Mark change as ready for review (Gerrit). This is the default state.
    #[clap(long, short = 'y', conflicts_with = "wip", hide = true)]
    pub ready: bool,
    /// Add hashtag(s) to change (Gerrit). Can be used multiple times.
    #[clap(long, short = 'a', alias = "tag", value_name = "TAG", hide = true)]
    pub hashtag: Vec<String>,
    /// Add custom topic to change (Gerrit). At most one topic can be set.
    #[clap(
        long,
        short = 't',
        value_name = "TOPIC",
        conflicts_with = "topic_from_branch",
        hide = true
    )]
    pub topic: Option<String>,
    /// Use branch name as topic (Gerrit)
    #[clap(
        long = "tb",
        alias = "topic-from-branch",
        conflicts_with = "topic",
        hide = true
    )]
    pub topic_from_branch: bool,
    /// Mark change as private (Gerrit)
    #[clap(long, short = 'p', hide = true)]
    pub private: bool,
}

fn get_gerrit_flags(
    args: &Args,
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

pub fn handle(args: Args, project: &Project, out: &mut OutputChannel) -> anyhow::Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Check gerrit mode early
    let gerrit_mode = ctx
        .gix_repo()?
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);

    // Resolve branch_id to actual branch name
    let branch_name = resolve_branch_name(&mut ctx, &args.branch_id)?;

    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, &branch_name)?;

    // Convert CLI args to gerrit flags with validation
    let gerrit_flags = get_gerrit_flags(&args, &branch_name, gerrit_mode)?;

    // Call push_stack
    let result: PushResult = but_api::commands::stack::push_stack(
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

fn resolve_branch_name(ctx: &mut CommandContext, branch_id: &str) -> anyhow::Result<String> {
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
                crate::id::CliId::Branch { name } => Some(name.clone()),
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
        crate::id::CliId::Branch { name } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}. Please use a branch name or branch CLI ID.",
            cli_ids[0].kind()
        )),
    }
}

fn get_available_branch_names(ctx: &CommandContext) -> anyhow::Result<Vec<String>> {
    let stacks = crate::log::stacks(ctx)?;
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
    let stacks = but_api::commands::workspace::stacks(
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

#[cfg(test)]
mod tests;
