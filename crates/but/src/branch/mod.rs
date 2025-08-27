use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_branch::BranchCreateRequest;
use gitbutler_branch_actions::{create_virtual_branch, unapply_stack};
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::{ObjectIdExt, RepoExt};
use gitbutler_project::Project;
use gitbutler_stack::VirtualBranchesHandle;
use std::path::Path;

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

            // Find the target stack and get its current head commit
            let stacks = crate::log::stacks(&ctx)?;
            let target_stack = stacks.iter().find(|s| {
                s.heads.iter().any(|head| head.name.to_string() == target_branch_name)
            });
            
            let target_stack = match target_stack {
                Some(s) => s,
                None => return Err(anyhow::anyhow!("No stack found for branch '{}'", target_branch_name)),
            };
            
            let target_stack_id = target_stack.id.ok_or_else(|| anyhow::anyhow!("Target stack has no ID"))?;
            
            // Get the stack details to find the head commit
            let target_stack_details = crate::log::stack_details(&ctx, target_stack_id)?;
            if target_stack_details.branch_details.is_empty() {
                return Err(anyhow::anyhow!("Target stack has no branch details"));
            }
            
            // Find the target branch in the stack details
            let target_branch_details = target_stack_details.branch_details.iter().find(|b| 
                b.name == target_branch_name
            ).ok_or_else(|| anyhow::anyhow!("Target branch '{}' not found in stack details", target_branch_name))?;
            
            // Get the head commit of the target branch
            let target_head_oid = if !target_branch_details.commits.is_empty() {
                // Use the last local commit
                target_branch_details.commits.last().unwrap().id
            } else if !target_branch_details.upstream_commits.is_empty() {
                // If no local commits, use the last upstream commit
                target_branch_details.upstream_commits.last().unwrap().id
            } else {
                return Err(anyhow::anyhow!("Target branch '{}' has no commits", target_branch_name));
            };
            
            // Create a new virtual branch 
            let mut guard = project.exclusive_worktree_access();
            let create_request = BranchCreateRequest {
                name: Some(branch_name.to_string()),
                ownership: None,
                order: None,
                selected_for_changes: None,
            };

            let new_stack_id = create_virtual_branch(&ctx, &create_request, guard.write_permission())?;
            
            // Now set up the new branch to start from the target branch's head
            let vb_state = VirtualBranchesHandle::new(ctx.project().gb_dir());
            let mut new_stack = vb_state.get_stack(new_stack_id.id)?;
            
            // Set the head of the new stack to be the target branch's head
            // This creates the stacking relationship
            let gix_repo = ctx.repo().to_gix()?;
            new_stack.set_stack_head(&vb_state, &gix_repo, target_head_oid.to_git2(), None)?;
            vb_state.set_stack(new_stack)?;

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

pub(crate) fn unapply_branch(
    repo_path: &Path,
    _json: bool,
    branch_id: &str,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    
    // Try to resolve the branch ID
    let cli_ids = CliId::from_str(&mut ctx, branch_id)?;
    
    if cli_ids.is_empty() {
        return Err(anyhow::anyhow!(
            "Branch '{}' not found. Try using a branch CLI ID or full branch name.",
            branch_id
        ));
    }
    
    if cli_ids.len() > 1 {
        let matches: Vec<String> = cli_ids.iter().map(|id| {
            match id {
                CliId::Branch { name } => format!("{} (branch '{}')", id.to_string(), name),
                _ => format!("{} ({})", id.to_string(), id.kind())
            }
        }).collect();
        return Err(anyhow::anyhow!(
            "Branch '{}' is ambiguous. Matches: {}. Try using more characters or the full branch name.",
            branch_id,
            matches.join(", ")
        ));
    }
    
    let cli_id = &cli_ids[0];
    let stack_id = match cli_id {
        CliId::Branch { .. } => {
            // Find the stack ID for this branch
            let stacks = crate::log::stacks(&ctx)?;
            let stack = stacks.iter().find(|s| {
                s.heads.iter().any(|head| {
                    if let CliId::Branch { name } = cli_id {
                        head.name.to_string() == *name
                    } else {
                        false
                    }
                })
            });
            
            match stack {
                Some(s) => s.id.ok_or_else(|| anyhow::anyhow!("Stack has no ID"))?,
                None => return Err(anyhow::anyhow!("No stack found for branch '{}'", branch_id)),
            }
        }
        _ => {
            return Err(anyhow::anyhow!(
                "ID '{}' does not refer to a branch (it's {})",
                branch_id,
                cli_id.kind()
            ));
        }
    };
    
    let branch_name = match cli_id {
        CliId::Branch { name } => name,
        _ => unreachable!(),
    };
    
    println!(
        "Unapplying branch '{}' ({})",
        branch_name.yellow().bold(),
        branch_id.blue().underline()
    );
    
    unapply_stack(&ctx, stack_id, Vec::new())?;
    
    println!(
        "{} Branch '{}' unapplied successfully!",
        "✓".green().bold(),
        branch_name.yellow().bold()
    );
    
    Ok(())
}
