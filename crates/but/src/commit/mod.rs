use crate::status::assignment::FileAssignment;
use bstr::{BString, ByteSlice};
use but_core::ui::TreeChange;
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use but_workspace::DiffSpec;
use gitbutler_command_context::CommandContext;
use gitbutler_oplog::{
    OplogExt,
    entry::{OperationKind, SnapshotDetails},
};
use gitbutler_project::Project;
use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::Path;

pub(crate) fn commit(
    repo_path: &Path,
    _json: bool,
    message: Option<&str>,
    branch_hint: Option<&str>,
    only: bool,
) -> anyhow::Result<()> {
    let project = Project::from_path(repo_path)?;
    let mut ctx = CommandContext::open(&project, AppSettings::load_from_default_path_creating()?)?;

    // Get all stacks
    let stack_entries = crate::log::stacks(&ctx)?;
    let stacks: Vec<(but_workspace::StackId, but_workspace::ui::StackDetails)> = stack_entries
        .iter()
        .filter_map(|s| {
            s.id.map(|id| {
                crate::log::stack_details(&ctx, id)
                    .ok()
                    .map(|details| (id, details))
            })
            .flatten()
        })
        .collect();

    // Determine which stack to commit to
    let target_stack_id = if stacks.is_empty() {
        anyhow::bail!("No stacks found. Create a stack first with 'but branch new <name>'.");
    } else if stacks.len() == 1 {
        // Only one stack, use it
        stacks[0].0
    } else {
        // Multiple stacks - need to select one
        select_stack(&mut ctx, &stacks, branch_hint)?
    };

    // Get changes and assignments
    let changes =
        but_core::diff::ui::worktree_changes_by_worktree_dir(project.path.clone())?.changes;
    let (assignments, _) = but_hunk_assignment::assignments_with_fallback(
        &mut ctx,
        false,
        Some(changes.clone()),
        None,
    )?;

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

    // Get files to commit: unassigned files + files assigned to target stack
    let mut files_to_commit = Vec::new();

    if !only {
        // Add unassigned files (unless --only flag is used)
        let unassigned =
            crate::status::assignment::filter_by_stack_id(assignments_by_file.values(), &None);
        files_to_commit.extend(unassigned);
    }

    // Add files assigned to target stack
    let stack_assigned = crate::status::assignment::filter_by_stack_id(
        assignments_by_file.values(),
        &Some(target_stack_id),
    );
    files_to_commit.extend(stack_assigned);

    if files_to_commit.is_empty() {
        println!("No changes to commit.");
        return Ok(());
    }

    // Get commit message
    let commit_message = if let Some(msg) = message {
        msg.to_string()
    } else {
        get_commit_message_from_editor(&files_to_commit, &changes)?
    };

    if commit_message.trim().is_empty() {
        anyhow::bail!("Aborting commit due to empty commit message.");
    }

    // Find the target branch (first head of the target stack)
    let target_stack = &stacks
        .iter()
        .find(|(id, _)| *id == target_stack_id)
        .unwrap()
        .1;
    let target_branch = target_stack
        .branch_details
        .first()
        .ok_or_else(|| anyhow::anyhow!("No branches found in target stack"))?;

    // Convert files to DiffSpec
    let diff_specs: Vec<DiffSpec> = files_to_commit
        .iter()
        .map(|fa| {
            // Collect hunk headers from all assignments for this file
            let hunk_headers: Vec<but_workspace::HunkHeader> = fa
                .assignments
                .iter()
                .filter_map(|assignment| assignment.hunk_header.clone())
                .collect();

            DiffSpec {
                previous_path: None,
                path: fa.path.clone(),
                hunk_headers,
            }
        })
        .collect();

    // Create a snapshot before committing
    let mut guard = project.exclusive_worktree_access();
    let _snapshot = ctx
        .create_snapshot(
            SnapshotDetails::new(OperationKind::CreateCommit),
            guard.write_permission(),
        )
        .ok(); // Ignore errors for snapshot creation

    // Commit using the simpler commit engine
    let outcome = but_workspace::commit_engine::create_commit_simple(
        &ctx,
        target_stack_id,
        None, // parent_id - let it auto-detect from branch head
        diff_specs,
        commit_message,
        target_branch.name.to_string(),
        guard.write_permission(),
    )?;

    let commit_short = match outcome.new_commit {
        Some(id) => id.to_string()[..7].to_string(),
        None => "unknown".to_string(),
    };
    println!(
        "Created commit {} on branch {}",
        commit_short, target_branch.name
    );

    Ok(())
}

fn select_stack(
    ctx: &mut CommandContext,
    stacks: &[(but_workspace::StackId, but_workspace::ui::StackDetails)],
    branch_hint: Option<&str>,
) -> anyhow::Result<but_workspace::StackId> {
    // If a branch hint is provided, try to find it
    if let Some(hint) = branch_hint {
        // First, try to find by exact branch name match
        for (stack_id, stack_details) in stacks {
            for branch in &stack_details.branch_details {
                if branch.name.to_string() == hint {
                    return Ok(*stack_id);
                }
            }
        }
        
        // If no exact match, try to parse as CLI ID
        match crate::id::CliId::from_str(ctx, hint) {
            Ok(cli_ids) => {
                // Filter for branch CLI IDs and find corresponding stack
                for cli_id in cli_ids {
                    if let crate::id::CliId::Branch { name } = cli_id {
                        for (stack_id, stack_details) in stacks {
                            for branch in &stack_details.branch_details {
                                if branch.name.to_string() == name {
                                    return Ok(*stack_id);
                                }
                            }
                        }
                    }
                }
            }
            Err(_) => {
                // Ignore CLI ID parsing errors and continue with other methods
            }
        }
        
        anyhow::bail!("Branch '{}' not found", hint);
    }

    // No hint provided, show options and prompt
    println!("Multiple stacks found. Choose one to commit to:");
    for (i, (stack_id, stack_details)) in stacks.iter().enumerate() {
        let branch_names: Vec<String> = stack_details
            .branch_details
            .iter()
            .map(|b| b.name.to_string())
            .collect();
        println!("  {}. {} [{}]", i + 1, stack_id, branch_names.join(", "));
    }

    print!("Enter selection (1-{}): ", stacks.len());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let selection: usize = input
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid selection"))?;

    if selection < 1 || selection > stacks.len() {
        anyhow::bail!("Selection out of range");
    }

    Ok(stacks[selection - 1].0)
}

fn get_commit_message_from_editor(
    files_to_commit: &[FileAssignment],
    changes: &[TreeChange],
) -> anyhow::Result<String> {
    // Get editor command
    let editor = get_editor_command()?;

    // Create temporary file with template
    let temp_dir = std::env::temp_dir();
    let temp_file = temp_dir.join(format!("but_commit_msg_{}", std::process::id()));

    // Generate commit message template
    let mut template = String::new();
    template.push_str("\n# Please enter the commit message for your changes. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty message aborts the commit.\n");
    template.push_str("#\n");
    template.push_str("# Changes to be committed:\n");

    for fa in files_to_commit {
        let status_char = get_status_char(&fa.path, changes);
        template.push_str(&format!("#\t{}  {}\n", status_char, fa.path.to_str_lossy()));
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

    Ok(message)
}

fn get_editor_command() -> anyhow::Result<String> {
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

fn get_status_char(path: &BString, changes: &[TreeChange]) -> &'static str {
    for change in changes {
        if change.path_bytes == *path {
            return match change.status {
                but_core::ui::TreeStatus::Addition { .. } => "new file:",
                but_core::ui::TreeStatus::Modification { .. } => "modified:",
                but_core::ui::TreeStatus::Deletion { .. } => "deleted:",
                but_core::ui::TreeStatus::Rename { .. } => "renamed:",
            };
        }
    }
    "modified:" // fallback
}
