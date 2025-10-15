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
    if !result.branch_to_remote.is_empty() {
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
        // If no CliId matches, treat as literal branch name
        return Ok(branch_id.to_string());
    }

    if cli_ids.len() > 1 {
        return Err(anyhow::anyhow!(
            "Ambiguous branch identifier '{}', matches multiple items",
            branch_id
        ));
    }

    match &cli_ids[0] {
        crate::id::CliId::Branch { name } => Ok(name.clone()),
        _ => Err(anyhow::anyhow!(
            "Expected branch identifier, got {}",
            cli_ids[0].kind()
        )),
    }
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

    Err(anyhow::anyhow!(
        "Branch '{}' not found in any stack",
        branch_name
    ))
}
