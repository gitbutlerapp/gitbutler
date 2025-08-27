use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::{create_virtual_branch, create_virtual_branch_from_branch};
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_reference::Refname;
use gitbutler_stack::VirtualBranchesHandle;
use std::path::Path;
use std::str::FromStr;

use crate::id::CliId;

pub(crate) fn create_branch(
    repo_path: &Path,
    _json: bool,
    branch_name: &str,
    base_id: Option<&str>,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    match base_id {
        Some(id_str) => {
            // First try to resolve as CLI ID
            let cli_ids = CliId::from_str(&mut ctx, id_str)?;

            let target_branch_name = if !cli_ids.is_empty() {
                if cli_ids.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Ambiguous ID '{}', matches multiple items",
                        id_str
                    ));
                }

                // Get the branch CLI ID
                let cli_id = &cli_ids[0];
                if !matches!(cli_id, CliId::Branch { .. }) {
                    return Err(anyhow::anyhow!(
                        "ID '{}' does not refer to a branch",
                        id_str
                    ));
                }

                // Get the branch name from the CLI ID
                match cli_id {
                    CliId::Branch { name } => name.clone(),
                    _ => unreachable!(),
                }
            } else {
                // If no CLI ID matches, try treating it as a direct branch name
                let repo = ctx.repo();

                // Check if the branch exists as a local branch
                if repo.find_branch(id_str, git2::BranchType::Local).is_ok() {
                    id_str.to_string()
                } else {
                    return Err(anyhow::anyhow!(
                        "No branch found matching ID or name: {}",
                        id_str
                    ));
                }
            };

            println!(
                "Creating stacked branch '{}' based on branch {} ({})",
                branch_name.green().bold(),
                target_branch_name.cyan(),
                id_str.blue().underline()
            );

            // Create a Refname from the branch name
            let branch_ref = Refname::from_str(&format!("refs/heads/{}", target_branch_name))?;

            let new_stack_id = create_virtual_branch_from_branch(&ctx, &branch_ref, None, None)?;

            // Update the branch name if it's different
            if branch_name != target_branch_name {
                let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
                let mut stack = vb_state.get_stack(new_stack_id)?;
                stack.name = branch_name.to_string();
                vb_state.set_stack(stack)?;
            }

            println!(
                "{} Stacked branch '{}' created successfully!",
                "✓".green().bold(),
                branch_name.green().bold()
            );
        }
        None => {
            // Create new empty virtual branch
            println!(
                "Creating new virtual branch '{}'",
                branch_name.green().bold()
            );

            let mut guard = project.exclusive_worktree_access();
            let create_request = BranchCreateRequest {
                name: Some(branch_name.to_string()),
                ownership: None,
                order: None,
                selected_for_changes: None,
            };

            create_virtual_branch(&ctx, &create_request, guard.write_permission())?;

            println!(
                "{} Virtual branch '{}' created successfully!",
                "✓".green().bold(),
                branch_name.green().bold()
            );
        }
    }

    Ok(())
}
