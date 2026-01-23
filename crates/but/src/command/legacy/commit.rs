use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use bstr::{BString, ByteSlice};
use but_api::{
    commit::commit_insert_blank,
    json::HexHash,
    legacy::{diff, repo, workspace},
};
use but_core::{DiffSpec, ui::TreeChange};
use but_rebase::graph_rebase::mutate::InsertSide;
use colored::Colorize;
use gitbutler_project::Project;
use gitbutler_repo::hooks;

use crate::utils::InputOutputChannel;
use crate::{
    CliId, IdMap, command::legacy::status::assignment::FileAssignment, tui, utils::OutputChannel,
};

pub(crate) fn insert_blank_commit(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    target: &str,
    insert_side: InsertSide,
) -> Result<()> {
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;

    // Resolve the target ID
    let cli_ids = id_map.resolve_entity_to_ids(target)?;

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

    // Determine the position description for the success message
    let position_desc = match insert_side {
        InsertSide::Above => "before",
        InsertSide::Below => "after",
    };

    // Determine target commit ID and use provided insert_side
    let success_message = match cli_id {
        CliId::Commit { commit_id: oid, .. } => {
            commit_insert_blank(
                ctx,
                but_api::commit::ui::RelativeTo::Commit(*oid),
                insert_side,
            )?;
            format!(
                "Created blank commit {} commit {}",
                position_desc,
                &oid.to_string()[..7]
            )
        }
        CliId::Branch { name, .. } => {
            let repo = ctx.repo.get()?;
            let reference = repo.find_reference(name)?;
            commit_insert_blank(
                ctx,
                but_api::commit::ui::RelativeTo::Reference(reference.name().into()),
                insert_side,
            )?;
            match insert_side {
                InsertSide::Below => format!("Created blank commit at the top of stack '{name}'"),
                InsertSide::Above => {
                    format!("Created blank commit at the bottom of stack '{name}'")
                }
            }
        }
        _ => {
            bail!(
                "Target must be a commit ID or branch name, not {}",
                cli_id.kind_for_humans()
            );
        }
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{success_message}")?;
    }
    Ok(())
}

pub(crate) fn commit(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    message: Option<&str>,
    branch_hint: Option<&str>,
    only: bool,
    create_branch: bool,
    no_hooks: bool,
) -> anyhow::Result<()> {
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;

    // Get all stacks using but-api
    let project_id = ctx.legacy_project.id;
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
        &id_map,
        &ctx.legacy_project,
        &stacks,
        branch_hint,
        create_branch,
        out,
    )?;

    // Get changes and assignments using but-api
    let worktree_changes = diff::changes_in_worktree(ctx)?;
    let changes = worktree_changes.worktree_changes.changes;

    let assignments_by_file: BTreeMap<BString, FileAssignment> =
        FileAssignment::get_assignments_by_file(&id_map);

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

    // Convert files to DiffSpec early so we can run pre-commit hooks before prompting for message
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

    // Run pre-commit hook unless --no-hooks was specified
    // This runs BEFORE getting the commit message so the user doesn't waste time writing a message
    // for a commit that will fail the hook
    if !no_hooks {
        let hook_result = repo::pre_commit_hook_diffspecs(project_id, diff_specs.clone())?;
        match hook_result {
            hooks::HookResult::Success | hooks::HookResult::NotConfigured => {
                // Hook passed or not configured, proceed with commit
            }
            hooks::HookResult::Failure(error_data) => {
                bail!(
                    "pre-commit hook failed:\n{}\n\nTo bypass the hook, run: but commit --no-hooks",
                    error_data.error
                );
            }
        }
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

    // Run commit-msg hook unless --no-hooks was specified
    // This hook can validate and optionally modify the commit message
    let final_commit_message = if !no_hooks {
        let hook_result = repo::message_hook(project_id, commit_message.clone())?;
        match hook_result {
            gitbutler_repo::hooks::MessageHookResult::Success => {
                // Hook passed without modification
                commit_message
            }
            gitbutler_repo::hooks::MessageHookResult::Message(message_data) => {
                // Hook passed and modified the message, use the new message
                message_data.message
            }
            gitbutler_repo::hooks::MessageHookResult::NotConfigured => {
                // No hook configured
                commit_message
            }
            gitbutler_repo::hooks::MessageHookResult::Failure(error_data) => {
                bail!(
                    "commit-msg hook failed:\n{}\n\nTo bypass the hook, run: but commit --no-hooks",
                    error_data.error
                );
            }
        }
    } else {
        commit_message
    };

    // If a branch hint was provided, find that specific branch; otherwise use first branch
    let target_branch = if let Some(hint) = branch_hint {
        // First try exact name match
        target_stack
            .branch_details
            .iter()
            .find(|branch| branch.name == hint)
            .or_else(|| {
                // If no exact match, try to parse as CLI ID and match
                if let Ok(cli_ids) = id_map.resolve_entity_to_ids(hint) {
                    for cli_id in cli_ids {
                        if let CliId::Branch { name, .. } = cli_id
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

    // Get the HEAD commit of the target branch to use as parent (preserves stacking)
    let parent_commit_id = target_branch.tip;

    // Use but-api to create the commit
    let outcome = workspace::create_commit_from_worktree_changes(
        project_id,
        target_stack_id,
        Some(HexHash::from(parent_commit_id)),
        diff_specs,
        final_commit_message,
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

    // Run post-commit hook unless --no-hooks was specified
    // Note: post-commit hooks run after the commit is created, so failures don't prevent the commit
    if !no_hooks {
        let hook_result = repo::post_commit_hook(project_id)?;
        match hook_result {
            hooks::HookResult::Success | hooks::HookResult::NotConfigured => {
                // Hook passed or not configured, nothing to do
            }
            hooks::HookResult::Failure(error_data) => {
                // Warn the user but don't fail since the commit is already created
                if let Some(out) = out.for_human() {
                    writeln!(out, "\n{}", "Warning: post-commit hook failed:".yellow())?;
                    writeln!(out, "{}", error_data.error)?;
                }
            }
        }
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
    // Handle empty stacks case - automatically create a branch
    if stacks.is_empty() {
        let branch_name = match branch_hint {
            Some(hint) => String::from(hint),
            None => but_api::legacy::workspace::canned_branch_name(project.id)?,
        };
        return create_independent_branch(&branch_name, project, out);
    }

    match branch_hint {
        Some(hint) => {
            // Try to find stack by branch hint
            if let Some(stack) = find_stack_by_hint(id_map, stacks, hint) {
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
            if let Some(inout) = out.prepare_for_terminal_input() {
                prompt_for_stack_selection(stacks, inout)
            } else {
                bail!("Multiple candidate stacks found")
            }
        }
    }
}

fn find_stack_by_hint(
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
    let cli_ids = id_map.resolve_entity_to_ids(hint).ok()?;
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
    mut inout: InputOutputChannel,
) -> Result<(
    but_core::ref_metadata::StackId,
    but_workspace::ui::StackDetails,
)> {
    use std::fmt::Write;
    writeln!(inout, "Multiple stacks found. Choose one to commit to:")?;

    for (i, (_stack_id, stack_details)) in stacks.iter().enumerate() {
        writeln!(inout, "  {}. {}", i + 1, stack_details.derived_name.green())?;
    }

    let selection: usize = inout
        .prompt(format!("Enter selection (1-{}): ", stacks.len()))?
        .context("Missing selection")?
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
    let lossy_message =
        tui::get_text::from_editor_no_comments("commit_msg", &template)?.to_string();
    Ok(lossy_message)
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
