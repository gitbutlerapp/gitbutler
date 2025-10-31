use anyhow::Result;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::Project;

use crate::{editor::get_text_from_editor_no_comments, id::CliId};

pub(crate) fn describe_target(project: &Project, _json: bool, target: &str) -> Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Resolve the commit ID
    let cli_ids = CliId::from_str(&mut ctx, target)?;

    if cli_ids.is_empty() {
        anyhow::bail!("ID '{}' not found", target);
    }

    if cli_ids.len() > 1 {
        anyhow::bail!(
            "Target ID '{}' is ambiguous. Found {} matches",
            target,
            cli_ids.len()
        );
    }

    let cli_id = &cli_ids[0];

    match cli_id {
        CliId::Branch { name } => {
            edit_branch_name(&ctx, project, name)?;
        }
        CliId::Commit { oid } => {
            edit_commit_message_by_id(&ctx, project, *oid)?;
        }
        _ => {
            anyhow::bail!("Target must be a commit ID, not {}", cli_id.kind());
        }
    }

    Ok(())
}

fn edit_branch_name(_ctx: &CommandContext, project: &Project, branch_name: &str) -> Result<()> {
    // Find which stack this branch belongs to
    let stacks =
        but_api::workspace::stacks(project.id, Some(but_workspace::StacksFilter::InWorkspace))?;
    for stack_entry in &stacks {
        if stack_entry.heads.iter().all(|b| b.name != branch_name) {
            // Not found in this stack,
            continue;
        }

        if let Some(sid) = stack_entry.id {
            let new_name = get_branch_name_from_editor(branch_name)?;
            but_api::stack::update_branch_name(
                project.id,
                sid,
                branch_name.to_owned(),
                new_name.clone(),
            )?;
            println!("Renamed branch '{}' to '{}", branch_name, new_name);
            return Ok(());
        }
    }

    println!("Branch '{}' not found in any stack", branch_name);
    Ok(())
}

fn edit_commit_message_by_id(
    ctx: &CommandContext,
    project: &Project,
    commit_oid: gix::ObjectId,
) -> Result<()> {
    // Find which stack this commit belongs to
    let stacks = but_api::workspace::stacks(project.id, None)?;
    let mut found_commit_message = None;
    let mut stack_id = None;

    for stack_entry in &stacks {
        if let Some(sid) = stack_entry.id {
            let stack_details = but_api::workspace::stack_details(project.id, Some(sid))?;

            // Check if this commit exists in any branch of this stack
            for branch_details in &stack_details.branch_details {
                // Check local commits
                for commit in &branch_details.commits {
                    if commit.id == commit_oid {
                        found_commit_message = Some(commit.message.clone());
                        stack_id = Some(sid);
                        break;
                    }
                }

                // Also check upstream commits
                if found_commit_message.is_none() {
                    for commit in &branch_details.upstream_commits {
                        if commit.id == commit_oid {
                            found_commit_message = Some(commit.message.clone());
                            stack_id = Some(sid);
                            break;
                        }
                    }
                }

                if found_commit_message.is_some() {
                    break;
                }
            }
            if found_commit_message.is_some() {
                break;
            }
        }
    }

    let commit_message = found_commit_message
        .ok_or_else(|| anyhow::anyhow!("Commit {} not found in any stack", commit_oid))?;

    let stack_id = stack_id
        .ok_or_else(|| anyhow::anyhow!("Could not find stack for commit {}", commit_oid))?;

    // Get the files changed in this commit using but_api
    let commit_details = but_api::diff::commit_details(project.id, commit_oid.into())?;
    let changed_files = get_changed_files_from_commit_details(&commit_details);

    // Get current commit message
    let current_message = commit_message.to_string();

    // Open editor with current message and file list
    let new_message = get_commit_message_from_editor(&current_message, &changed_files)?;

    if new_message.trim() == current_message.trim() {
        println!("No changes to commit message.");
        return Ok(());
    }

    // Use gitbutler_branch_actions::update_commit_message instead of low-level primitives
    let git2_commit_oid = commit_oid.to_git2();
    let new_commit_oid = gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_id,
        git2_commit_oid,
        &new_message,
    )?;

    println!(
        "Updated commit message for {} (now {})",
        &commit_oid.to_string()[..7],
        &new_commit_oid.to_string()[..7]
    );

    Ok(())
}

fn get_changed_files_from_commit_details(
    commit_details: &but_api::diff::CommitDetails,
) -> Vec<String> {
    let mut files = Vec::new();

    for change in &commit_details.changes.changes {
        let status = match &change.status {
            but_core::ui::TreeStatus::Addition { .. } => "new file:",
            but_core::ui::TreeStatus::Deletion { .. } => "deleted:",
            but_core::ui::TreeStatus::Modification { .. } => "modified:",
            but_core::ui::TreeStatus::Rename { .. } => "modified:",
        };

        let file_path = change.path.to_string();
        files.push(format!("{status}   {file_path}"));
    }

    files.sort();
    files
}

fn get_commit_message_from_editor(
    current_message: &str,
    changed_files: &[String],
) -> Result<String> {
    // Generate commit message template with current message
    let mut template = String::new();
    template.push_str(current_message);
    if !current_message.is_empty() && !current_message.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the commit message for your changes. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty message aborts the commit.\n");
    template.push_str("#\n");
    template.push_str("# Changes in this commit:\n");

    for file in changed_files {
        template.push_str(&format!("#\t{file}\n"));
    }
    template.push_str("#\n");

    // Read the result and strip comments
    let message = get_text_from_editor_no_comments("but_commit_msg", &template)?;

    if message.is_empty() {
        anyhow::bail!("Aborting due to empty commit message");
    }

    Ok(message)
}

fn get_branch_name_from_editor(current_name: &str) -> Result<String> {
    let mut template = String::new();
    template.push_str(current_name);
    if !current_name.is_empty() && !current_name.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the new branch name. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty name aborts the operation.\n");
    template.push_str("#\n");

    let branch_name = get_text_from_editor_no_comments("but_branch_name", &template)?;

    if branch_name.is_empty() {
        anyhow::bail!("Aborting due to empty branch name");
    }

    Ok(branch_name)
}
