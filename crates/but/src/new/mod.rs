use std::path::Path;
use anyhow::Result;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_oxidize::ObjectIdExt;
use but_settings::AppSettings;
use crate::id::CliId;

pub(crate) fn insert_blank_commit(
    repo_path: &Path,
    _json: bool,
    target: &str,
) -> Result<()> {
    let project = Project::from_path(repo_path)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    // Resolve the target ID
    let cli_ids = CliId::from_str(&mut ctx, target)?;
    
    if cli_ids.is_empty() {
        anyhow::bail!("Target '{}' not found", target);
    }
    
    if cli_ids.len() > 1 {
        anyhow::bail!("Target '{}' is ambiguous. Found {} matches", target, cli_ids.len());
    }

    let cli_id = &cli_ids[0];
    
    match cli_id {
        CliId::Commit { oid } => {
            // Insert blank commit before this specific commit
            insert_before_commit(&ctx, *oid)?;
        }
        CliId::Branch { name } => {
            // Insert blank commit at the top of this stack
            insert_at_top_of_stack(&ctx, name)?;
        }
        _ => {
            anyhow::bail!("Target must be a commit ID or branch name, not {}", cli_id.kind());
        }
    }

    Ok(())
}

fn insert_before_commit(ctx: &CommandContext, commit_oid: gix::ObjectId) -> Result<()> {
    // Find which stack this commit belongs to
    let stacks = crate::log::stacks(ctx)?;
    
    for stack_entry in &stacks {
        if let Some(stack_id) = stack_entry.id {
            let stack_details = crate::log::stack_details(ctx, stack_id)?;
            
            // Check if this commit exists in any branch of this stack
            for branch_details in &stack_details.branch_details {
                for commit in &branch_details.commits {
                    if commit.id == commit_oid {
                        // Found the commit - insert blank commit before it
                        let (new_commit_id, _changes) = gitbutler_branch_actions::insert_blank_commit(
                            ctx,
                            stack_id,
                            commit_oid.to_git2(),
                            0, // offset: 0 means "before this commit"
                            Some(""), // Empty commit message
                        )?;
                        
                        println!(
                            "Created blank commit {} before commit {}",
                            &new_commit_id.to_string()[..7],
                            &commit_oid.to_string()[..7]
                        );
                        return Ok(());
                    }
                }
                
                // Also check upstream commits
                for commit in &branch_details.upstream_commits {
                    if commit.id == commit_oid {
                        let (new_commit_id, _changes) = gitbutler_branch_actions::insert_blank_commit(
                            ctx,
                            stack_id,
                            commit_oid.to_git2(),
                            0, // offset: 0 means "before this commit"
                            Some(""), // Empty commit message
                        )?;
                        
                        println!(
                            "Created blank commit {} before commit {}",
                            &new_commit_id.to_string()[..7],
                            &commit_oid.to_string()[..7]
                        );
                        return Ok(());
                    }
                }
            }
        }
    }
    
    anyhow::bail!("Commit {} not found in any stack", commit_oid);
}

fn insert_at_top_of_stack(ctx: &CommandContext, branch_name: &str) -> Result<()> {
    // Find the stack that contains this branch
    let stacks = crate::log::stacks(ctx)?;
    
    for stack_entry in &stacks {
        if let Some(stack_id) = stack_entry.id {
            let stack_details = crate::log::stack_details(ctx, stack_id)?;
            
            // Check if this branch exists in this stack
            for branch_details in &stack_details.branch_details {
                if branch_details.name.to_string() == branch_name {
                    // Get the head commit of this branch
                    let head_commit = if let Some(commit) = branch_details.commits.first() {
                        commit.id
                    } else if let Some(commit) = branch_details.upstream_commits.first() {
                        commit.id
                    } else {
                        anyhow::bail!("Branch '{}' has no commits", branch_name);
                    };
                    
                    // Insert blank commit at the top (after the head commit)
                    let (new_commit_id, _changes) = gitbutler_branch_actions::insert_blank_commit(
                        ctx,
                        stack_id,
                        head_commit.to_git2(),
                        -1, // offset: -1 means "after this commit" (at the top)
                        Some(""), // Empty commit message
                    )?;
                    
                    println!(
                        "Created blank commit {} at the top of stack '{}'",
                        &new_commit_id.to_string()[..7],
                        branch_name
                    );
                    return Ok(());
                }
            }
        }
    }
    
    anyhow::bail!("Branch '{}' not found in any stack", branch_name);
}