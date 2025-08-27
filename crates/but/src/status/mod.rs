use assignment::FileAssignment;
use bstr::{BString, ByteSlice};
use but_core::ui::{TreeChange, TreeStatus};
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use colored::Colorize;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;
use gitbutler_stack;
use std::collections::BTreeMap;
use std::path::Path;
pub(crate) mod assignment;

use crate::id::CliId;

pub(crate) fn worktree(repo_path: &Path, json: bool, show_base: bool) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path).expect("Failed to create project from path");
    let ctx = &mut CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    // Get stacks with detailed information
    let stack_entries = crate::log::stacks(ctx)?;
    let stacks: Vec<(
        Option<but_workspace::StackId>,
        but_workspace::ui::StackDetails,
    )> = stack_entries
        .iter()
        .filter_map(|s| {
            s.id.map(|id| (s.id, crate::log::stack_details(ctx, id)))
                .and_then(|(stack_id, result)| result.ok().map(|details| (stack_id, details)))
        })
        .collect();

    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.path.clone())?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;

    // Group assignments by file
    let mut by_file: BTreeMap<BString, Vec<HunkAssignment>> = BTreeMap::new();
    for assignment in &assignments {
        by_file
            .entry(assignment.path_bytes.clone())
            .or_default()
            .push(assignment.clone());
    }
    let mut assignments_by_file: BTreeMap<BString, FileAssignment> = BTreeMap::new();
    for (path, assignments) in &by_file {
        assignments_by_file.insert(
            path.clone(),
            FileAssignment::from_assignments(path, assignments),
        );
    }

    // Handle JSON output
    if json {
        let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
        return output_json(&stacks, &assignments_by_file, &unassigned, &changes, show_base, ctx);
    }

    // Print base information only if requested
    if show_base {
        print_base_info(ctx)?;
        println!();
    }

    // Print branches with commits and assigned files
    if !stacks.is_empty() {
        print_branch_sections(&stacks, &assignments_by_file, &changes, &project)?;
    }

    // Print unassigned files
    let unassigned = assignment::filter_by_stack_id(assignments_by_file.values(), &None);
    if !unassigned.is_empty() {
        print_unassigned_section(unassigned, &changes)?;
    }

    Ok(())
}

pub(crate) fn all_files(ctx: &mut CommandContext) -> anyhow::Result<Vec<CliId>> {
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(ctx.project().path.clone())?.changes;
    let (assignments, _assignments_error) =
        but_hunk_assignment::assignments_with_fallback(ctx, false, Some(changes.clone()), None)?;
    let out = assignments
        .iter()
        .map(CliId::file_from_assignment)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    Ok(out)
}

pub(crate) fn all_branches(ctx: &CommandContext) -> anyhow::Result<Vec<CliId>> {
    let stacks = crate::log::stacks(ctx)?;
    let mut branches = Vec::new();
    for stack in stacks {
        for head in stack.heads {
            branches.push(CliId::branch(&head.name.to_string()));
        }
    }
    Ok(branches)
}

fn status_from_changes(changes: &[TreeChange], path: BString) -> Option<TreeStatus> {
    changes.iter().find_map(|change| {
        if change.path_bytes == path {
            Some(change.status.clone())
        } else {
            None
        }
    })
}

fn print_base_info(ctx: &CommandContext) -> anyhow::Result<()> {
    // Get base information
    let target =
        gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir()).get_default_target()?;

    let base_sha = &target.sha.to_string()[..7];
    println!("📍 Base: {} @ {}", "origin/main".cyan(), base_sha.yellow());

    // For now, we'll show a placeholder for behind count
    // In a real implementation, you'd calculate this by comparing HEAD with the target
    println!(
        "🔺 You are {} commits behind {}",
        "0".red(),
        "origin/main".cyan()
    );
    println!("    (Run `but base update` to rebase your stack)");

    Ok(())
}

fn print_branch_sections(
    stacks: &[(
        Option<but_workspace::StackId>,
        but_workspace::ui::StackDetails,
    )],
    assignments_by_file: &BTreeMap<BString, FileAssignment>,
    changes: &[TreeChange],
    project: &Project,
) -> anyhow::Result<()> {
    let nesting = 0;

    for (i, (stack_id, stack)) in stacks.iter().enumerate() {
        let mut first_branch = true;

        for (_j, branch) in stack.branch_details.iter().enumerate() {
            let branch_name = branch.name.to_string();
            let branch_id = CliId::branch(&branch_name).to_string().underline().blue();

            // Determine the connecting character for this branch
            let prefix = "│ ".repeat(nesting);
            let connector = if first_branch { "╭" } else { "├" };

            println!(
                "{}{}  {} [{}]",
                prefix,
                connector,
                branch_id,
                branch_name.green().bold()
            );

            // Show assigned files first - only for the first (topmost) branch in a stack
            // In GitButler's model, files are assigned to the stack, not individual branches
            let has_files = if first_branch {
                if let Some(stack_id) = stack_id {
                    let branch_assignments = assignment::filter_by_stack_id(
                        assignments_by_file.values(),
                        &Some(*stack_id),
                    );
                    if !branch_assignments.is_empty() {
                        for fa in &branch_assignments {
                            let status_char = get_status_char(&fa.path, changes);
                            let file_id = CliId::file_from_assignment(&fa.assignments[0])
                                .to_string()
                                .underline()
                                .blue();

                            println!(
                                "{}│  {} {} {}",
                                prefix,
                                file_id,
                                status_char,
                                fa.path.to_string().white()
                            );
                        }
                        println!("{}│", prefix);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };

            // Show commits after files
            let has_commits = !branch.commits.is_empty() || !branch.upstream_commits.is_empty();
            if has_commits {
                // Show upstream commits first
                for commit in &branch.upstream_commits {
                    let commit_short = &commit.id.to_string()[..7];
                    let commit_id = CliId::commit(commit.id).to_string().underline().blue();
                    let message_line = format_commit_message(&commit.message);
                    println!(
                        "{}●  {} {} {}",
                        prefix,
                        commit_id,
                        commit_short.blue(),
                        message_line
                    );
                }

                // Show local commits
                for commit in &branch.commits {
                    let commit_short = &commit.id.to_string()[..7];
                    let commit_id = CliId::commit(commit.id).to_string().underline().blue();
                    let message_line = format_commit_message(&commit.message);
                    println!(
                        "{}●  {} {} {}",
                        prefix,
                        commit_id,
                        commit_short.blue(),
                        message_line
                    );
                }
                println!("{}│", prefix);
            }

            if !has_commits && !has_files {
                println!("{}│     (no commits)", prefix);
            }

            first_branch = false;
        }

        // Close the current stack
        if !stack.branch_details.is_empty() {
            if i == stacks.len() - 1 {
                // Last stack - close with simple ├─╯
                println!("│");
            } else {
                // Not the last stack - close with ├─╯ and add blank line
                println!("│");
                println!();
            }
        }
    }

    // Get and display the base commit
    let ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;
    let common_merge_base = gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir())
        .get_default_target()?
        .sha
        .to_string()[..7]
        .to_string();
    println!("● {} (base)", common_merge_base);

    Ok(())
}

fn print_unassigned_section(
    unassigned: Vec<FileAssignment>,
    changes: &[TreeChange],
) -> anyhow::Result<()> {
    let unassigned_id = CliId::unassigned().to_string().underline().blue();
    println!("\n{} Unassigned Changes", unassigned_id);

    for fa in unassigned {
        let status_char = get_status_char(&fa.path, changes);
        let file_id = CliId::file_from_assignment(&fa.assignments[0])
            .to_string()
            .underline()
            .blue();

        println!(
            "{} {} {}",
            file_id,
            status_char,
            fa.path.to_string().white()
        );
    }

    Ok(())
}

fn get_status_char(path: &BString, changes: &[TreeChange]) -> colored::ColoredString {
    match status_from_changes(changes, path.clone()) {
        Some(TreeStatus::Addition { .. }) => "A".green(),
        Some(TreeStatus::Modification { .. }) => "M".yellow(),
        Some(TreeStatus::Deletion { .. }) => "D".red(),
        Some(TreeStatus::Rename { .. }) => "R".purple(),
        None => " ".normal(),
    }
}

fn output_json(
    stacks: &[(Option<but_workspace::StackId>, but_workspace::ui::StackDetails)],
    assignments_by_file: &std::collections::BTreeMap<BString, FileAssignment>,
    unassigned: &[FileAssignment],
    changes: &[TreeChange],
    show_base: bool,
    ctx: &CommandContext,
) -> anyhow::Result<()> {
    use serde_json::json;
    
    // Get base information if requested
    let base_info = if show_base {
        let target = gitbutler_stack::VirtualBranchesHandle::new(ctx.project().gb_dir()).get_default_target().ok();
        target.map(|t| json!({
            "branch": "origin/main", // TODO: Get actual base branch name
            "sha": t.sha.to_string()[..7].to_string(),
            "behind_count": 0 // TODO: Calculate actual behind count
        }))
    } else {
        None
    };

    // Process stacks
    let mut stacks_json = Vec::new();
    for (stack_id, stack_details) in stacks {
        let mut branches_json = Vec::new();
        
        for branch_details in &stack_details.branch_details {
            let branch_name = branch_details.name.to_string();
            let branch_cli_id = CliId::branch(&branch_name).to_string();

            // Get assigned files for this stack (only for the first branch in the stack)
            let assigned_files = if branches_json.is_empty() {
                if let Some(stack_id) = stack_id {
                    assignment::filter_by_stack_id(assignments_by_file.values(), &Some(*stack_id))
                        .into_iter()
                        .map(|fa| {
                            let status = get_status_string(&fa.path, changes);
                            let file_cli_id = CliId::file_from_assignment(&fa.assignments[0]).to_string();
                            json!({
                                "id": file_cli_id,
                                "path": fa.path.to_string(),
                                "status": status
                            })
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                }
            } else {
                Vec::new()
            };

            // Process commits
            let mut commits_json = Vec::new();
            
            // Add upstream commits
            for commit in &branch_details.upstream_commits {
                let commit_cli_id = CliId::commit(commit.id).to_string();
                commits_json.push(json!({
                    "id": commit_cli_id,
                    "sha": commit.id.to_string()[..7].to_string(),
                    "message": format_commit_message(&commit.message),
                    "type": "upstream"
                }));
            }
            
            // Add local commits
            for commit in &branch_details.commits {
                let commit_cli_id = CliId::commit(commit.id).to_string();
                commits_json.push(json!({
                    "id": commit_cli_id,
                    "sha": commit.id.to_string()[..7].to_string(),
                    "message": format_commit_message(&commit.message),
                    "type": "local"
                }));
            }

            branches_json.push(json!({
                "id": branch_cli_id,
                "name": branch_name,
                "assigned_files": assigned_files,
                "commits": commits_json
            }));
        }

        if let Some(stack_id) = stack_id {
            stacks_json.push(json!({
                "id": stack_id.to_string(),
                "branches": branches_json
            }));
        }
    }

    // Process unassigned files
    let unassigned_files: Vec<_> = unassigned
        .iter()
        .map(|fa| {
            let status = get_status_string(&fa.path, changes);
            let file_cli_id = CliId::file_from_assignment(&fa.assignments[0]).to_string();
            json!({
                "id": file_cli_id,
                "path": fa.path.to_string(),
                "status": status
            })
        })
        .collect();

    let output = json!({
        "base": base_info,
        "stacks": stacks_json,
        "unassigned_files": unassigned_files
    });

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

fn get_status_string(path: &BString, changes: &[TreeChange]) -> &'static str {
    for change in changes {
        if change.path_bytes == *path {
            return match change.status {
                but_core::ui::TreeStatus::Addition { .. } => "A",
                but_core::ui::TreeStatus::Modification { .. } => "M",
                but_core::ui::TreeStatus::Deletion { .. } => "D",
                but_core::ui::TreeStatus::Rename { .. } => "R",
            };
        }
    }
    "M" // fallback
}

fn format_commit_message(message: &BString) -> String {
    let message_str = message.to_str_lossy();
    let message_line = message_str.lines().next().unwrap_or("");
    if message_line.trim().is_empty() {
        "(blank message)".to_string()
    } else {
        message_line.to_string()
    }
}
