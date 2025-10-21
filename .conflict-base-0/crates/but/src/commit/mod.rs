use std::{
    collections::BTreeMap,
    io::{self, Write},
};

use anyhow::Result;
use bstr::{BString, ByteSlice};
use but_api::{
    commands::{diff, virtual_branches, workspace},
    hex_hash::HexHash,
};
use but_core::ui::TreeChange;
use but_hunk_assignment::HunkAssignment;
use but_settings::AppSettings;
use but_workspace::DiffSpec;
use gitbutler_command_context::CommandContext;
use gitbutler_project::Project;

use crate::{id::CliId, status::assignment::FileAssignment};

pub(crate) fn insert_blank_commit(project: &Project, _json: bool, target: &str) -> Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Resolve the target ID
    let cli_ids = CliId::from_str(&mut ctx, target)?;

    if cli_ids.is_empty() {
        anyhow::bail!("Target '{}' not found", target);
    }

    if cli_ids.len() > 1 {
        anyhow::bail!(
            "Target '{}' is ambiguous. Found {} matches",
            target,
            cli_ids.len()
        );
    }

    let cli_id = &cli_ids[0];

    // Determine target commit ID and offset based on CLI ID type
    let (target_commit_id, offset, success_message) = match cli_id {
        CliId::Commit { oid } => {
            // For commits, insert before (offset 0) and use the commit ID directly
            (
                *oid,
                0,
                format!(
                    "Created blank commit before commit {}",
                    &oid.to_string()[..7]
                ),
            )
        }
        CliId::Branch { name } => {
            // For branches, we need to find the branch and get its head commit
            let head_commit_id = find_branch_head_commit(project.id, name)?;
            (
                head_commit_id,
                -1,
                format!("Created blank commit at the top of stack '{name}'"),
            )
        }
        _ => {
            anyhow::bail!(
                "Target must be a commit ID or branch name, not {}",
                cli_id.kind()
            );
        }
    };

    // Find the stack containing the target commit and insert blank commit
    let stack_id = find_stack_containing_commit(project.id, target_commit_id)?;
    virtual_branches::insert_blank_commit(
        project.id,
        stack_id,
        Some(target_commit_id.to_string()),
        offset,
    )?;
    println!("{success_message}");
    Ok(())
}

fn find_branch_head_commit(
    project_id: gitbutler_project::ProjectId,
    branch_name: &str,
) -> Result<gix::ObjectId> {
    let stack_entries = workspace::stacks(project_id, None)?;

    for stack_entry in &stack_entries {
        if let Some(stack_id) = stack_entry.id {
            let stack_details = workspace::stack_details(project_id, Some(stack_id))?;

            if let Some(branch_details) = stack_details
                .branch_details
                .iter()
                .find(|b| b.name == branch_name)
            {
                // Get the head commit of this branch (prefer regular commits over upstream)
                return if let Some(commit) = branch_details.commits.first() {
                    Ok(commit.id)
                } else if let Some(commit) = branch_details.upstream_commits.first() {
                    Ok(commit.id)
                } else {
                    anyhow::bail!("Branch '{}' has no commits", branch_name);
                };
            }
        }
    }

    anyhow::bail!("Branch '{}' not found in any stack", branch_name);
}

fn find_stack_containing_commit(
    project_id: gitbutler_project::ProjectId,
    commit_id: gix::ObjectId,
) -> Result<but_workspace::StackId> {
    let stack_entries = workspace::stacks(project_id, None)?;

    for stack_entry in &stack_entries {
        if let Some(stack_id) = stack_entry.id {
            let stack_details = workspace::stack_details(project_id, Some(stack_id))?;

            // Check if this commit exists in any branch of this stack
            for branch_details in &stack_details.branch_details {
                // Check both regular commits and upstream commits
                if branch_details.commits.iter().any(|c| c.id == commit_id)
                    || branch_details
                        .upstream_commits
                        .iter()
                        .any(|c| c.id == commit_id)
                {
                    return Ok(stack_id);
                }
            }
        }
    }

    anyhow::bail!("Commit {} not found in any stack", commit_id);
}

pub(crate) fn commit(
    project: &Project,
    _json: bool,
    message: Option<&str>,
    branch_hint: Option<&str>,
    only: bool,
) -> anyhow::Result<()> {
    let mut ctx = CommandContext::open(project, AppSettings::load_from_default_path_creating()?)?;

    // Get all stacks using but-api
    let project_id = project.id;
    let stack_entries = workspace::stacks(project_id, None)?;
    let stacks: Vec<(but_workspace::StackId, but_workspace::ui::StackDetails)> = stack_entries
        .iter()
        .filter_map(|s| {
            s.id.and_then(|id| {
                workspace::stack_details(project_id, Some(id))
                    .ok()
                    .map(|details| (id, details))
            })
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

    // Get changes and assignments using but-api
    let worktree_changes = diff::changes_in_worktree(project_id)?;
    let changes = worktree_changes.worktree_changes.changes;
    let assignments = worktree_changes.assignments;

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

    // Find the target stack and determine the target branch
    let target_stack = &stacks
        .iter()
        .find(|(id, _)| *id == target_stack_id)
        .unwrap()
        .1;

    // If a branch hint was provided, find that specific branch; otherwise use first branch
    let target_branch = if let Some(hint) = branch_hint {
        // First try exact name match
        target_stack
            .branch_details
            .iter()
            .find(|branch| branch.name == hint)
            .or_else(|| {
                // If no exact match, try to parse as CLI ID and match
                if let Ok(cli_ids) = crate::id::CliId::from_str(&mut ctx, hint) {
                    for cli_id in cli_ids {
                        if let crate::id::CliId::Branch { name } = cli_id
                            && let Some(branch) =
                                target_stack.branch_details.iter().find(|b| b.name == name)
                        {
                            return Some(branch);
                        }
                    }
                }
                None
            })
            .ok_or_else(|| anyhow::anyhow!("Branch '{}' not found in target stack", hint))?
    } else {
        // No branch hint, use first branch (HEAD of stack)
        target_stack
            .branch_details
            .first()
            .ok_or_else(|| anyhow::anyhow!("No branches found in target stack"))?
    };

    // Convert files to DiffSpec
    let diff_specs: Vec<DiffSpec> = files_to_commit
        .iter()
        .map(|fa| {
            // Collect hunk headers from all assignments for this file
            let hunk_headers: Vec<but_workspace::HunkHeader> = fa
                .assignments
                .iter()
                .filter_map(|assignment| assignment.hunk_header)
                .collect();

            DiffSpec {
                previous_path: None,
                path: fa.path.clone(),
                hunk_headers,
            }
        })
        .collect();

    // Get the HEAD commit of the target branch to use as parent (preserves stacking)
    let parent_commit_id = target_branch.tip;

    // Use but-api to create the commit
    let outcome = workspace::create_commit_from_worktree_changes(
        project_id,
        target_stack_id,
        Some(HexHash::from(parent_commit_id)),
        diff_specs,
        commit_message,
        target_branch.name.to_string(),
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
                if branch.name == hint {
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
                                if branch.name == name {
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
