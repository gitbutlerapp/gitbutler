use crate::id::CliId;
use anyhow::Result;
use but_settings::AppSettings;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::Project;
use std::path::Path;

pub(crate) fn edit_commit_message(
    repo_path: &Path,
    _json: bool,
    commit_target: &str,
) -> Result<()> {
    let project = Project::from_path(repo_path)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

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
            edit_commit_message_by_id(&ctx, &project, *oid)?;
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
    let stacks = crate::log::stacks(ctx)?;
    let mut found_commit_message = None;
    let mut stack_id = None;

    for stack_entry in &stacks {
        if let Some(sid) = stack_entry.id {
            let stack_details = crate::log::stack_details(ctx, sid)?;

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

    // Get the files changed in this commit
    let changed_files = get_commit_changed_files(&ctx.repo(), commit_oid)?;

    // Get current commit message
    let current_message = commit_message.to_string();

    // Open editor with current message and file list
    let new_message = get_commit_message_from_editor(&current_message, &changed_files)?;

    if new_message.trim() == current_message.trim() {
        println!("No changes to commit message.");
        return Ok(());
    }

    // Create a snapshot before making changes
    let mut guard = project.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::AmendCommit),
            guard.write_permission(),
        )
        .ok(); // Ignore errors for snapshot creation

    // Amend the commit with the new message
    let gix_repo = crate::mcp_internal::project::project_repo(&project.path)?;
    let outcome = but_workspace::commit_engine::create_commit_and_update_refs_with_project(
        &gix_repo,
        project,
        Some(stack_id),
        but_workspace::commit_engine::Destination::AmendCommit {
            commit_id: commit_oid,
            new_message: Some(new_message.clone()),
        },
        None,   // move_source
        vec![], // No file changes, just message
        0,      // context_lines
        guard.write_permission(),
    )?;

    if let Some(new_commit_id) = outcome.new_commit {
        println!(
            "Updated commit message for {} (now {})",
            &commit_oid.to_string()[..7],
            &new_commit_id.to_string()[..7]
        );
    } else {
        println!(
            "Updated commit message for {}",
            &commit_oid.to_string()[..7]
        );
    }

    Ok(())
}

fn get_commit_changed_files(
    repo: &git2::Repository,
    commit_oid: gix::ObjectId,
) -> Result<Vec<String>> {
    let git2_oid = commit_oid.to_git2();
    let commit = repo.find_commit(git2_oid)?;

    if commit.parent_count() == 0 {
        // Initial commit - show all files as new
        let tree = commit.tree()?;
        let mut files = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                let full_path = if root.is_empty() {
                    entry.name().unwrap_or("").to_string()
                } else {
                    format!("{}{}", root, entry.name().unwrap_or(""))
                };
                files.push(format!("new file:   {}", full_path));
            }
            git2::TreeWalkResult::Ok
        })?;
        return Ok(files);
    }

    // Get parent commit and compare trees
    let parent = commit.parent(0)?;
    let parent_tree = parent.tree()?;
    let commit_tree = commit.tree()?;

    // Use git2 diff to get the changes with status information
    let mut diff_opts = git2::DiffOptions::new();
    diff_opts.show_binary(true).ignore_submodules(true);
    let diff =
        repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), Some(&mut diff_opts))?;

    let mut files = Vec::new();
    diff.print(git2::DiffFormat::NameStatus, |delta, _hunk, _line| {
        let status = match delta.status() {
            git2::Delta::Added => "new file:",
            git2::Delta::Modified | git2::Delta::Renamed | git2::Delta::Copied => "modified:",
            git2::Delta::Deleted => "deleted:",
            _ => "modified:",
        };
        let file_path = delta.new_file().path().unwrap_or_else(|| {
            delta
                .old_file()
                .path()
                .expect("failed to get file name from diff")
        });
        files.push(format!("{}   {}", status, file_path.display()));
        true // Continue iteration
    })?;

    files.sort();
    Ok(files)
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
    template.push_str(&current_message);
    if !current_message.is_empty() && !current_message.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the commit message for your changes. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty message aborts the commit.\n");
    template.push_str("#\n");
    template.push_str("# Changes in this commit:\n");

    for file in changed_files {
        template.push_str(&format!("#\t{}\n", file));
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
        .args(&["config", "--get", "core.editor"])
        .output()
    {
        if output.status.success() {
            let editor = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !editor.is_empty() {
                return Ok(editor);
            }
        }
    }

    // Fallback to platform defaults
    #[cfg(windows)]
    return Ok("notepad".to_string());

    #[cfg(not(windows))]
    return Ok("vi".to_string());
}
