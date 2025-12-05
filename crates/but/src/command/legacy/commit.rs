use std::collections::BTreeMap;

use anyhow::{Result, bail};
use bstr::{BString, ByteSlice};
use but_api::{
    json::HexHash,
    legacy::{diff, virtual_branches, workspace},
};
use but_core::{DiffSpec, ui::TreeChange};
use but_ctx::Context;
use but_hunk_assignment::HunkAssignment;
use gitbutler_project::Project;

use crate::{
    command::legacy::status::assignment::FileAssignment,
    legacy::id::{CliId, IdMap},
    tui,
    utils::OutputChannel,
};

pub(crate) fn insert_blank_commit(
    project: &Project,
    out: &mut OutputChannel,
    target: &str,
) -> Result<()> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let id_map = IdMap::new(&mut ctx)?;

    // Resolve the target ID
    let cli_ids = id_map.parse_str(&mut ctx, target)?;

    if cli_ids.is_empty() {
        bail!("Target '{}' not found", target);
    }

    if cli_ids.len() > 1 {
        bail!(
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
        CliId::Branch { name, .. } => {
            // For branches, we need to find the branch and get its head commit
            let head_commit_id = find_branch_head_commit(project.id, name)?;
            (
                head_commit_id,
                -1,
                format!("Created blank commit at the top of stack '{name}'"),
            )
        }
        _ => {
            bail!(
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
    if let Some(out) = out.for_human() {
        writeln!(out, "{success_message}")?;
    }
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
                    bail!("Branch '{}' has no commits", branch_name);
                };
            }
        }
    }

    bail!("Branch '{}' not found in any stack", branch_name);
}

fn find_stack_containing_commit(
    project_id: gitbutler_project::ProjectId,
    commit_id: gix::ObjectId,
) -> Result<but_core::ref_metadata::StackId> {
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

    bail!("Commit {} not found in any stack", commit_id);
}

pub(crate) fn commit(
    project: &Project,
    out: &mut OutputChannel,
    message: Option<&str>,
    branch_hint: Option<&str>,
    only: bool,
    create_branch: bool,
) -> anyhow::Result<()> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let id_map = IdMap::new(&mut ctx)?;

    // Get all stacks using but-api
    let project_id = project.id;
    let stack_entries = workspace::stacks(project_id, None)?;
    let stacks: Vec<(
        but_core::ref_metadata::StackId,
        but_workspace::ui::StackDetails,
    )> = stack_entries
        .iter()
        .filter_map(|s| {
            s.id.and_then(|id| {
                workspace::stack_details(project_id, Some(id))
                    .ok()
                    .map(|details| (id, details))
            })
        })
        .collect();

    let (target_stack_id, target_stack) = select_stack(
        &mut ctx,
        &id_map,
        project,
        &stacks,
        branch_hint,
        create_branch,
        out,
    )?;

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
        let unassigned = crate::command::legacy::status::assignment::filter_by_stack_id(
            assignments_by_file.values(),
            &None,
        );
        files_to_commit.extend(unassigned);
    }

    // Add files assigned to target stack
    let stack_assigned = crate::command::legacy::status::assignment::filter_by_stack_id(
        assignments_by_file.values(),
        &Some(target_stack_id),
    );
    files_to_commit.extend(stack_assigned);

    if files_to_commit.is_empty() {
        bail!("No changes to commit.")
    }

    // Get commit message
    let commit_message = if let Some(msg) = message {
        msg.to_string()
    } else {
        get_commit_message_from_editor(&files_to_commit, &changes)?
    };

    if commit_message.trim().is_empty() {
        bail!("Aborting commit due to empty commit message.");
    }

    // If a branch hint was provided, find that specific branch; otherwise use first branch
    let target_branch = if let Some(hint) = branch_hint {
        // First try exact name match
        target_stack
            .branch_details
            .iter()
            .find(|branch| branch.name == hint)
            .or_else(|| {
                // If no exact match, try to parse as CLI ID and match
                if let Ok(cli_ids) = id_map.parse_str(&mut ctx, hint) {
                    for cli_id in cli_ids {
                        if let crate::legacy::id::CliId::Branch { name, .. } = cli_id
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
            let hunk_headers: Vec<but_core::HunkHeader> = fa
                .assignments
                .iter()
                .filter_map(|assignment| assignment.inner.hunk_header)
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

    if let Some(out) = out.for_human() {
        let commit_short = match outcome.new_commit {
            Some(id) => id.to_hex_with_len(7).to_string(),
            None => "unknown".to_string(),
        };
        writeln!(
            out,
            "Created commit {} on branch {}",
            commit_short, target_branch.name
        )?;
    }

    Ok(())
}

fn create_independent_branch(
    branch_name: &str,
    project: &Project,
    out: &mut OutputChannel,
) -> anyhow::Result<(
    but_core::ref_metadata::StackId,
    but_workspace::ui::StackDetails,
)> {
    // Create a new independent stack with the given branch name
    let (new_stack_id_opt, _new_ref) = but_api::legacy::stack::create_reference(
        project.id,
        but_api::legacy::stack::create_reference::Request {
            new_name: branch_name.to_string(),
            anchor: None,
        },
    )?;

    if let Some(new_stack_id) = new_stack_id_opt {
        if let Some(out) = out.for_human() {
            writeln!(out, "Created new independent branch '{}'", branch_name)?;
        }
        Ok((
            new_stack_id,
            workspace::stack_details(project.id, Some(new_stack_id))?,
        ))
    } else {
        bail!("Failed to create new branch '{}'", branch_name);
    }
}

fn select_stack(
    ctx: &mut Context,
    id_map: &IdMap,
    project: &Project,
    stacks: &[(
        but_core::ref_metadata::StackId,
        but_workspace::ui::StackDetails,
    )],
    branch_hint: Option<&str>,
    create_branch: bool,
    out: &mut OutputChannel,
) -> anyhow::Result<(
    but_core::ref_metadata::StackId,
    but_workspace::ui::StackDetails,
)> {
    // Handle empty stacks case
    if stacks.is_empty() {
        anyhow::ensure!(
            create_branch,
            "No stacks found. Create a stack for this commit using 'but commit -c <branch-name>' or 'but branch new <name>' and then commit"
        );
        let branch_name = match branch_hint {
            Some(hint) => String::from(hint),
            None => but_api::legacy::workspace::canned_branch_name(project.id)?,
        };
        return create_independent_branch(&branch_name, project, out);
    }

    match branch_hint {
        Some(hint) => {
            // Try to find stack by branch hint
            if let Some(stack) = find_stack_by_hint(ctx, id_map, stacks, hint) {
                return Ok(stack);
            }

            // Branch not found - create if flag is set, otherwise error
            if create_branch {
                create_independent_branch(hint, project, out)
            } else {
                bail!("Branch '{}' not found", hint)
            }
        }
        None if create_branch => {
            // Create with canned name
            let branch_name = but_api::legacy::workspace::canned_branch_name(project.id)?;
            create_independent_branch(&branch_name, project, out)
        }
        None if stacks.len() == 1 => {
            // Only one stack - use it
            Ok(stacks[0].clone())
        }
        None => {
            // Prompt user to select
            if out.for_human().is_some() {
                prompt_for_stack_selection(stacks)
            } else {
                bail!("Multiple candidate stacks found")
            }
        }
    }
}

fn find_stack_by_hint(
    ctx: &mut Context,
    id_map: &IdMap,
    stacks: &[(
        but_core::ref_metadata::StackId,
        but_workspace::ui::StackDetails,
    )],
    hint: &str,
) -> Option<(
    but_core::ref_metadata::StackId,
    but_workspace::ui::StackDetails,
)> {
    // Try exact branch name match
    for (stack_id, stack_details) in stacks {
        if stack_details.branch_details.iter().any(|b| b.name == hint) {
            return Some((*stack_id, stack_details.clone()));
        }
    }

    // Try CLI ID parsing
    let cli_ids = id_map.parse_str(ctx, hint).ok()?;
    for cli_id in cli_ids {
        if let CliId::Branch { name, .. } = cli_id {
            for (stack_id, stack_details) in stacks {
                if stack_details.branch_details.iter().any(|b| b.name == name) {
                    return Some((*stack_id, stack_details.clone()));
                }
            }
        }
    }

    None
}

fn prompt_for_stack_selection(
    stacks: &[(
        but_core::ref_metadata::StackId,
        but_workspace::ui::StackDetails,
    )],
) -> Result<(
    but_core::ref_metadata::StackId,
    but_workspace::ui::StackDetails,
)> {
    use std::io::Write;
    let mut stdout = std::io::stdout();
    writeln!(stdout, "Multiple stacks found. Choose one to commit to:")?;

    for (i, (stack_id, stack_details)) in stacks.iter().enumerate() {
        let branch_names: Vec<String> = stack_details
            .branch_details
            .iter()
            .map(|b| b.name.to_string())
            .collect();
        writeln!(
            stdout,
            "  {}. {} [{}]",
            i + 1,
            stack_id,
            branch_names.join(", ")
        )?;
    }

    write!(stdout, "Enter selection (1-{}): ", stacks.len())?;
    std::io::stdout().flush()?;

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    let selection: usize = input
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid selection"))?;

    anyhow::ensure!(
        (1..=stacks.len()).contains(&selection),
        "Selection out of range"
    );

    Ok(stacks[selection - 1].clone())
}

fn get_commit_message_from_editor(
    files_to_commit: &[FileAssignment],
    changes: &[TreeChange],
) -> anyhow::Result<String> {
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

    // Read the result from the editor and strip comments
    let message = tui::get_text::from_editor_no_comments("but_commit_msg", &template)?;
    Ok(message)
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
