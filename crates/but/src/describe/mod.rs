use crate::id::CliId;
use anyhow::Result;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::Project;

pub(crate) fn edit_commit_message(
    project: &Project,
    _json: bool,
    commit_target: &str,
) -> Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Resolve the commit ID
    let cli_ids = CliId::from_str(&mut ctx, commit_target)?;

    if cli_ids.is_empty() {
        anyhow::bail!("Commit '{}' not found", commit_target);
    }

    if cli_ids.len() > 1 {
        anyhow::bail!(
            "Commit '{}' is ambiguous. Found {} matches",
            commit_target,
            cli_ids.len()
        );
    }

    let cli_id = &cli_ids[0];

    match cli_id {
        CliId::Commit { oid } => {
            edit_commit_message_by_id(&ctx, project, *oid)?;
        }
        _ => {
            anyhow::bail!("Target must be a commit ID, not {}", cli_id.kind());
        }
    }

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
    // Get editor command
    let editor = get_editor_command()?;

    // Create temporary file with current message and file list
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("but_commit_msg_{}", std::process::id()));

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

    std::fs::write(&temp_file, template)?;

    // Launch editor
    let status = std::process::Command::new(&editor)
        .arg(&temp_file)
        .status()?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // Read the result and strip comments
    let content = std::fs::read_to_string(&temp_file)?;
    std::fs::remove_file(&temp_file).ok(); // Best effort cleanup

    let message = content
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if message.is_empty() {
        anyhow::bail!("Aborting due to empty commit message");
    }

    Ok(message)
}

fn get_editor_command() -> Result<String> {
    // Try $EDITOR first
    if let Ok(editor) = std::env::var("EDITOR") {
        return Ok(editor);
    }

    // Try git config core.editor
    if let Ok(output) = std::process::Command::new("git")
        .args(["config", "--get", "core.editor"])
        .output()
        && output.status.success()
    {
        let editor = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Fallback to platform defaults
    #[cfg(windows)]
    return Ok("notepad".to_string());

    #[cfg(not(windows))]
    return Ok("vi".to_string());
}
