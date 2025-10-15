use but_core::RepositoryExt;
use but_settings::AppSettings;
use but_workspace::StackId;
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
}

pub fn handle(args: &Args, project: &Project, _json: bool) -> anyhow::Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Resolve branch_id to actual branch name
    let branch_name = resolve_branch_name(&mut ctx, &args.branch_id)?;

    // Find stack_id from branch name
    let stack_id = find_stack_id_by_branch_name(project, &branch_name)?;

    // Call push_stack
    let result: PushResult = but_api::stack::push_stack(
        project.id,
        stack_id,
        args.with_force,
        args.skip_force_push_protection,
        branch_name.clone(),
        args.run_hooks,
    )?;

    println!("Push completed successfully");
    println!("Pushed to remote: {}", result.remote);
    let gerrit_mode = ctx
        .gix_repo()?
        .git_settings()?
        .gitbutler_gerrit_mode
        .unwrap_or(false);
    if !gerrit_mode && !result.branch_to_remote.is_empty() {
        for (branch, remote_ref) in &result.branch_to_remote {
            println!("  {} -> {}", branch, remote_ref);
        }
    }

    Ok(())
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
    let stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;

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
