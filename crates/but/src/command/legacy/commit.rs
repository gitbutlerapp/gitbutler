use std::{collections::BTreeMap, fmt::Write as _};

use anyhow::{Context, Result, bail};
use bstr::{BString, ByteSlice};
use but_api::{commit::create::commit_create, diff, legacy::repo};
use but_core::{DiffSpec, DryRun, ref_metadata::StackId, sync::RepoExclusive, ui::TreeChange};
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_repo::hooks;

use super::{ShowDiffInEditor, estimate_diff_blob_size};
use crate::{
    CliId, CliResult, IdMap,
    args::atoms::{BranchArg, BranchOrCommit, CliIdArg, Priority, Purpose, ResolvedCliIdArg},
    bad_input,
    command::legacy::status::assignment::{CLIHunkAssignment, FileAssignment},
    legacy::workspace::HeadInfoStack,
    theme::{self, Paint},
    tui,
    utils::{InputOutputChannel, OutputChannel},
};

type TargetStack = (StackId, HeadInfoStack);

pub(crate) fn insert_blank_commit(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    target: Option<CliIdArg>,
    before: Option<CliIdArg>,
    after: Option<CliIdArg>,
) -> CliResult<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let (target, insert_side) = if let Some(t) = before {
        (
            t.resolve_in_workspace(ctx, &id_map, Purpose::Target, None)?
                .into_branch_or_commit()?,
            InsertSide::Below,
        )
    } else if let Some(t) = after {
        (
            t.resolve_in_workspace(ctx, &id_map, Purpose::Target, None)?
                .into_branch_or_commit()?,
            InsertSide::Above,
        )
    } else if let Some(t) = target {
        // Default to --before behavior when using positional argument
        (
            t.resolve_in_workspace(ctx, &id_map, Purpose::Target, None)?
                .into_branch_or_commit()?,
            InsertSide::Below,
        )
    } else {
        // No arguments provided - default to inserting at top of first branch

        let stacks = crate::legacy::workspace::applied_stacks(ctx)?;

        // Find the first stack with branches and convert BString to String
        let branch_name = stacks
            .iter()
            .filter(|stack| stack.id.is_some())
            .find_map(|stack| stack.top_branch_name().map(ToOwned::to_owned))
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "No branches found. Create a branch first or specify a target explicitly."
                )
            })?;

        (
            BranchOrCommit::Branch(BranchArg(branch_name)),
            InsertSide::Below,
        )
    };

    let position_desc = match insert_side {
        InsertSide::Above => "after",
        InsertSide::Below => "before",
    };

    // Determine target commit ID and use provided insert_side
    let (outcome, success_message) = match target {
        BranchOrCommit::Commit(oid) => {
            let outcome = but_api::commit::insert_blank::commit_insert_blank_with_perm(
                ctx,
                RelativeTo::Commit(oid),
                insert_side,
                DryRun::No,
                guard.write_permission(),
            )?;
            (
                outcome,
                format!("Created blank commit {position_desc} commit {target}"),
            )
        }
        BranchOrCommit::Branch(branch) => {
            let reference = branch.resolve_local_branch_name()?;

            if matches!(insert_side, InsertSide::Above) {
                // Must prevent inserting above a stack head, as that would create an anonymous
                // segment as a direct child of the workspace commit. This is not well supported
                // overall at the moment, and among other things causes `but status` to crash as it
                // uses the stack's ref name to compute the CLI ID.
                let head_info = but_api::legacy::workspace::head_info(ctx)?;
                for stack in head_info.stacks {
                    if let Some(stack_head_ref) = stack.ref_name()
                        && stack_head_ref == &reference
                    {
                        return Err(bad_input("Cannot insert empty commit above stack head")
                            .arg_name("--after")
                            .hint("Use '--before' to insert at the tip of the stack")
                            .into());
                    }
                }
            }

            let outcome = but_api::commit::insert_blank::commit_insert_blank_with_perm(
                ctx,
                RelativeTo::Reference(reference),
                insert_side,
                DryRun::No,
                guard.write_permission(),
            )?;
            let success_message = match insert_side {
                InsertSide::Above => format!("Created blank commit above branch '{branch}'"),
                InsertSide::Below => {
                    format!("Created blank commit at the tip of branch '{branch}'")
                }
            };
            (outcome, success_message)
        }
    };

    if let Some(out) = out.for_human() {
        writeln!(out, "{success_message}")?;
    } else if let Some(json_out) = out.for_json() {
        let commit_data = serde_json::json!({
            "commit_id": outcome.new_commit.to_string(),
        });
        json_out.write_value(commit_data)?;
    }
    Ok(())
}

/// Generate a unified diff string from files to be committed
fn generate_unified_diff(
    ctx: &mut but_ctx::Context,
    files_to_commit: &[FileAssignment],
    changes: &[TreeChange],
) -> anyhow::Result<String> {
    let mut diff_output = String::new();
    let repo = ctx.repo.get()?;

    for fa in files_to_commit {
        // Find the corresponding TreeChange for this file
        if let Some(change) = changes.iter().find(|c| c.path_bytes == fa.path) {
            // Convert to but_core::TreeChange and get unified patch
            let core_change: but_core::TreeChange = change.clone().into();

            // Propagate errors from unified_patch, only skip when it returns Ok(None)
            match core_change.unified_patch(&repo, ctx.settings.context_lines)? {
                Some(patch) => {
                    // Add file header
                    writeln!(
                        &mut diff_output,
                        "diff --git a/{} b/{}",
                        fa.path.to_str_lossy(),
                        fa.path.to_str_lossy()
                    )?;

                    // Add patch content based on type
                    match patch {
                        but_core::UnifiedPatch::Binary => {
                            writeln!(&mut diff_output, "Binary files differ")?;
                        }
                        but_core::UnifiedPatch::TooLarge { size_in_bytes } => {
                            writeln!(&mut diff_output, "File too large ({size_in_bytes} bytes)")?;
                        }
                        but_core::UnifiedPatch::Patch { hunks, .. } => {
                            for hunk in hunks {
                                // Add hunk header
                                writeln!(
                                    &mut diff_output,
                                    "@@ -{},{} +{},{} @@",
                                    hunk.old_start, hunk.old_lines, hunk.new_start, hunk.new_lines
                                )?;
                                // Add hunk content (already includes +/- prefixes)
                                diff_output.push_str(hunk.diff.to_str_lossy().as_ref());
                            }
                        }
                    }
                }
                None => {
                    // Ok(None) means the file can't produce a diff (e.g., submodules, type changes)
                    // This is expected and we skip the file silently
                }
            }
        }
    }

    Ok(diff_output)
}

fn build_diff_specs(files_to_commit: &[FileAssignment], changes: &[TreeChange]) -> Vec<DiffSpec> {
    files_to_commit
        .iter()
        .map(|fa| {
            // Collect hunk headers from all assignments for this file
            let hunk_headers: Vec<but_core::HunkHeader> = fa
                .assignments
                .iter()
                .filter_map(|assignment| assignment.inner.hunk_header)
                .collect();

            let previous_path = changes
                .iter()
                .find(|change| change.path_bytes == fa.path)
                .and_then(|change| match &change.status {
                    but_core::ui::TreeStatus::Rename {
                        previous_path_bytes,
                        ..
                    } => Some(previous_path_bytes.clone()),
                    _ => None,
                });

            DiffSpec {
                previous_path,
                path: fa.path.clone(),
                hunk_headers,
            }
        })
        .collect()
}

/// Resolves file CliIDs to their corresponding FileAssignments.
/// Returns an error if any ID is invalid, ambiguous, or assigned to a different stack.
/// Deduplicates by file path to handle cases where the same file is passed multiple times.
fn resolve_file_ids(
    id_map: &IdMap,
    ctx: &mut but_ctx::Context,
    file_ids: &[String],
    target_stack_id: Option<but_core::ref_metadata::StackId>,
) -> anyhow::Result<Vec<FileAssignment>> {
    let mut resolved_files: BTreeMap<BString, FileAssignment> = BTreeMap::new();
    let mut errors = Vec::new();

    for file_id in file_ids {
        let cli_ids = match id_map.parse_using_context(file_id, ctx) {
            Ok(ids) => ids,
            Err(e) => {
                errors.push(format!("'{file_id}': {e}"));
                continue;
            }
        };

        if cli_ids.is_empty() {
            errors.push(format!(
                "'{file_id}' not found. Run 'but status' to see available file IDs."
            ));
            continue;
        }

        if cli_ids.len() > 1 {
            errors.push(format!(
                "'{}' is ambiguous - matches {} entities. Use more characters to disambiguate.",
                file_id,
                cli_ids.len()
            ));
            continue;
        }

        match &cli_ids[0] {
            CliId::Uncommitted(uncommitted) => {
                // Validate stack assignment for ALL hunks - each must be unassigned OR assigned to target stack
                for hunk in &uncommitted.hunk_assignments {
                    if hunk.stack_id.is_some() && hunk.stack_id != target_stack_id {
                        errors.push(format!(
                            "'{file_id}' is assigned to a different stack. Use 'but rub {file_id} zz' to unassign it first."
                        ));
                        break;
                    }
                }
                if errors
                    .iter()
                    .any(|e| e.starts_with(&format!("'{file_id}'")))
                {
                    continue;
                }

                // Convert UncommittedCliId to FileAssignment and merge with existing entry if present
                let path = uncommitted.hunk_assignments.first().path_bytes.clone();
                let new_assignments: Vec<CLIHunkAssignment> = uncommitted
                    .hunk_assignments
                    .iter()
                    .map(|ha| CLIHunkAssignment {
                        inner: ha.clone(),
                        cli_id: file_id.to_owned(),
                    })
                    .collect();

                // Merge with existing entry for same path, or insert new
                if let Some(existing) = resolved_files.get_mut(&path) {
                    existing.assignments.extend(new_assignments);
                } else {
                    resolved_files.insert(
                        path.clone(),
                        FileAssignment {
                            path,
                            assignments: new_assignments,
                        },
                    );
                }
            }
            other => {
                errors.push(format!(
                    "'{}' is {} but must be an uncommitted file or hunk",
                    file_id,
                    other.kind_for_humans()
                ));
            }
        }
    }

    if !errors.is_empty() {
        bail!("Invalid file ID(s):\n  {}", errors.join("\n  "));
    }

    Ok(resolved_files.into_values().collect())
}

#[expect(clippy::too_many_arguments)]
pub(crate) fn commit(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    message: Option<&str>,
    branch_arg: Option<CliIdArg>,
    file_ids: &[String],
    only: bool,
    all: bool,
    create_branch: bool,
    no_hooks: bool,
    generate_message: Option<Option<String>>,
    show_diff_in_editor: ShowDiffInEditor,
) -> CliResult<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let branch_hint = if let Some(branch_arg) = branch_arg {
        if let Some(branch) = branch_arg
            .try_resolve(ctx, &id_map, Purpose::Branch, Some(Priority::Branch))?
            .and_then(|id| {
                if let ResolvedCliIdArg::Branch(BranchArg(branch)) = id {
                    Some(branch)
                } else {
                    None
                }
            })
        {
            Some(branch)
        } else {
            let repo = ctx.repo.get()?;
            let head_info = but_api::legacy::workspace::head_info(ctx)?;
            Some(BranchArg(branch_arg.0).resolve_for_creation(&repo, &head_info)?)
        }
    } else {
        None
    };

    let t = theme::get();

    if all && let Some(out) = out.for_human() {
        writeln!(out, "no need for -a here my friend...")?;
    }

    let stacks: Vec<TargetStack> = crate::legacy::workspace::applied_stacks(ctx)?
        .into_iter()
        .filter_map(|stack| stack.id.map(|id| (id, stack)))
        .collect();

    // In JSON mode with multiple branches, require branch specification
    if out.for_json().is_some() && stacks.len() > 1 && branch_hint.is_none() {
        return Err(anyhow::anyhow!(
            "Multiple branches found. Specify a branch to commit to using the branch argument"
        )
        .into());
    }

    let (target_stack_id, target_stack) = select_stack(
        &id_map,
        ctx,
        &stacks,
        branch_hint.as_deref(),
        create_branch,
        out,
        guard.write_permission(),
    )?;

    // Get changes and assignments using but-api
    let worktree_changes = diff::changes_in_worktree_with_perm(ctx, guard.read_permission())?;
    let changes = worktree_changes.worktree_changes.changes;

    // Get files to commit - either specific files by ID or all eligible files
    let files_to_commit = if !file_ids.is_empty() {
        // User specified specific file IDs - resolve them
        resolve_file_ids(&id_map, ctx, file_ids, Some(target_stack_id))?
    } else {
        // Default behavior: unassigned files + files assigned to target stack
        let assignments_by_file: BTreeMap<BString, FileAssignment> =
            FileAssignment::get_assignments_by_file(&id_map);

        let mut files = Vec::new();

        if !only {
            // Add unassigned files (unless --only flag is used)
            let unassigned = crate::command::legacy::status::assignment::filter_by_stack_id(
                assignments_by_file.values(),
                &None,
            );
            files.extend(unassigned);
        }

        // Add files assigned to target stack
        let stack_assigned = crate::command::legacy::status::assignment::filter_by_stack_id(
            assignments_by_file.values(),
            &Some(target_stack_id),
        );
        files.extend(stack_assigned);

        files
    };

    if files_to_commit.is_empty() {
        return Err(anyhow::anyhow!("No changes to commit.").into());
    }

    // Convert files to DiffSpec early so we can run pre-commit hooks before prompting for message
    let diff_specs = build_diff_specs(&files_to_commit, &changes);

    // Run pre-commit hook unless --no-hooks was specified
    // This runs BEFORE getting the commit message so the user doesn't waste time writing a message
    // for a commit that will fail the hook
    if !no_hooks {
        let hook_result = repo::pre_commit_hook_diffspecs(ctx, diff_specs.clone())?;
        match hook_result {
            hooks::HookResult::Success | hooks::HookResult::NotConfigured => {
                // Hook passed or not configured, proceed with commit
            }
            hooks::HookResult::Failure(error_data) => {
                return Err(anyhow::anyhow!(
                    "pre-commit hook failed:\n{}\n\nTo bypass the hook, run: but commit --no-hooks",
                    error_data.error
                )
                .into());
            }
        }
    }

    // Get commit message
    let commit_message = if let Some(user_summary) = generate_message {
        let diff = generate_unified_diff(ctx, &files_to_commit, &changes)?;
        super::ai::generate_commit_message(out, &diff, user_summary)?
    } else if let Some(msg) = message {
        msg.to_string()
    } else {
        // In JSON mode, we should have already validated that a message was provided
        // This is a safeguard in case the validation was missed
        if out.for_json().is_some() {
            return Err(anyhow::anyhow!(
                "In JSON mode, a commit message must be provided via --message (-m), --message-file, or --ai (-i)"
            ).into());
        }
        get_commit_message_from_editor(ctx, &files_to_commit, &changes, show_diff_in_editor)?
    };

    if commit_message.trim().is_empty() {
        return Err(anyhow::anyhow!("Aborting commit due to empty commit message.").into());
    }

    // Run commit-msg hook unless --no-hooks was specified
    // This hook can validate and optionally modify the commit message
    let final_commit_message = if !no_hooks {
        let hook_result = repo::message_hook(ctx, commit_message.clone())?;
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
                return Err(anyhow::anyhow!(
                    "commit-msg hook failed:\n{}\n\nTo bypass the hook, run: but commit --no-hooks",
                    error_data.error
                )
                .into());
            }
        }
    } else {
        commit_message
    };

    // If a branch hint was provided, find that specific branch; otherwise use first branch
    let target_branch = if let Some(branch_hint) = branch_hint.as_deref() {
        // First try exact name match
        target_stack
            .branch(branch_hint)
            .or_else(|| {
                // If no exact match, try to parse as CLI ID and match
                if let Ok(cli_ids) = id_map.parse_using_context(branch_hint, ctx) {
                    for cli_id in cli_ids {
                        if let CliId::Branch { name, .. } = cli_id
                            && let Some(branch) = target_stack.branch(&name)
                        {
                            return Some(branch);
                        }
                    }
                }
                None
            })
            .ok_or_else(|| anyhow::anyhow!("Branch '{branch_hint}' not found in target stack"))?
    } else {
        // No branch hint, use first branch (HEAD of stack)
        target_stack
            .branches
            .first()
            .ok_or_else(|| anyhow::anyhow!("No branches found in target stack"))?
    };

    // Insert relative to the branch reference itself so only that branch tip is advanced.
    let outcome = commit_create(
        ctx,
        RelativeTo::Reference(target_branch.reference.clone()),
        InsertSide::Below,
        diff_specs,
        final_commit_message,
        DryRun::No,
        guard.write_permission(),
    )?;

    if !outcome.rejected_specs.is_empty() {
        tracing::warn!(
            ?outcome.rejected_specs,
            "Failed to commit at least one selected change"
        );
        if let Some(out) = out.for_human() {
            writeln!(
                out,
                "{} Some selected changes could not be committed.",
                t.attention.paint("Warning:"),
            )?;
        }
    }

    if let Some(out) = out.for_human() {
        let commit_short = match outcome.new_commit {
            Some(id) => id.to_hex_with_len(7).to_string(),
            None => "unknown".to_string(),
        };
        writeln!(
            out,
            "{} Created commit {} on branch {}",
            t.sym().success,
            t.commit_id.paint(commit_short),
            t.local_branch.paint(&target_branch.name),
        )?;
    } else if let Some(json_out) = out.for_json() {
        let commit_data = serde_json::json!({
            "commit_id": outcome.new_commit.map(|id| id.to_string()),
            "branch": &target_branch.name,
            "branch_tip": outcome.new_commit.map(|id| id.to_string()),
        });
        json_out.write_value(commit_data)?;
    }

    // Run post-commit hook unless --no-hooks was specified
    // Note: post-commit hooks run after the commit is created, so failures don't prevent the commit
    if !no_hooks {
        let hook_result = repo::post_commit_hook(ctx)?;
        match hook_result {
            hooks::HookResult::Success | hooks::HookResult::NotConfigured => {
                // Hook passed or not configured, nothing to do
            }
            hooks::HookResult::Failure(error_data) => {
                // Warn the user but don't fail since the commit is already created
                if let Some(out) = out.for_human() {
                    writeln!(
                        out,
                        "\n{} post-commit hook failed:",
                        t.attention.paint("Warning:")
                    )?;
                    writeln!(out, "{}", error_data.error)?;
                }
            }
        }
    }

    Ok(())
}

fn create_independent_branch(
    branch_name: &str,
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<TargetStack> {
    // Create a new independent stack with the given branch name
    let (new_stack_id_opt, _new_ref) = but_api::legacy::stack::create_reference_with_perm(
        ctx,
        but_api::legacy::stack::create_reference::Request {
            new_name: branch_name.to_string(),
            anchor: None,
        },
        perm,
    )?;

    if let Some(new_stack_id) = new_stack_id_opt {
        if let Some(out) = out.for_human() {
            writeln!(out, "Created new independent branch '{branch_name}'")?;
        }
        Ok((
            new_stack_id,
            crate::legacy::workspace::applied_stack(ctx, Some(new_stack_id))?,
        ))
    } else {
        bail!("Failed to create new branch '{branch_name}'");
    }
}

fn select_stack(
    id_map: &IdMap,
    ctx: &mut but_ctx::Context,
    stacks: &[TargetStack],
    branch_hint: Option<&str>,
    create_branch: bool,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> anyhow::Result<TargetStack> {
    // Handle empty stacks case - automatically create a branch
    if stacks.is_empty() {
        let branch_name = match branch_hint {
            Some(hint) => String::from(hint),
            None => but_api::legacy::workspace::canned_branch_name(ctx)?,
        };
        return create_independent_branch(&branch_name, ctx, out, perm);
    }

    match branch_hint {
        Some(hint) => {
            // Try to find stack by branch hint
            if let Some(stack) = find_stack_by_hint(id_map, stacks, hint) {
                return Ok(stack);
            }

            // Branch not found - create if flag is set, otherwise error
            if create_branch {
                create_independent_branch(hint, ctx, out, perm)
            } else {
                bail!("Branch '{hint}' not found")
            }
        }
        None if create_branch => {
            // Create with canned name
            let branch_name = but_api::legacy::workspace::canned_branch_name(ctx)?;
            create_independent_branch(&branch_name, ctx, out, perm)
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

fn find_stack_by_hint(id_map: &IdMap, stacks: &[TargetStack], hint: &str) -> Option<TargetStack> {
    // Try exact branch name match
    for (stack_id, stack) in stacks {
        if stack.contains_branch(hint) {
            return Some((*stack_id, stack.clone()));
        }
    }

    // Try CLI ID parsing
    let cli_ids = id_map
        .parse(hint, Box::new(move |_, _| Ok(Vec::new())))
        .ok()?;
    for cli_id in cli_ids {
        if let CliId::Branch { name, .. } = cli_id {
            for (stack_id, stack) in stacks {
                if stack.contains_branch(&name) {
                    return Some((*stack_id, stack.clone()));
                }
            }
        }
    }

    None
}

fn prompt_for_stack_selection(
    stacks: &[TargetStack],
    mut inout: InputOutputChannel,
) -> Result<TargetStack> {
    use std::fmt::Write;

    let t = theme::get();
    writeln!(inout, "Multiple stacks found. Choose one to commit to:")?;

    for (i, (_stack_id, stack)) in stacks.iter().enumerate() {
        writeln!(
            inout,
            "  {}. {}",
            i + 1,
            t.local_branch
                .paint(stack.top_branch_name().unwrap_or("unnamed"))
        )?;
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
    ctx: &mut but_ctx::Context,
    files_to_commit: &[FileAssignment],
    changes: &[TreeChange],
    show_diff_in_editor: ShowDiffInEditor,
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

    // Compute diff for the editor if requested
    let should_show_diff = show_diff_in_editor.should_show_diff(|| {
        // Convert ui::TreeChange to core::TreeChange for blob size estimation
        let core_changes: Vec<but_core::TreeChange> = changes
            .iter()
            .filter(|c| files_to_commit.iter().any(|f| f.path == c.path_bytes))
            .cloned()
            .map(but_core::TreeChange::from)
            .collect();
        estimate_diff_blob_size(&core_changes, &*ctx.repo.get()?)
    })?;

    let diff_text = if should_show_diff {
        let diff = generate_unified_diff(ctx, files_to_commit, changes)?;
        if diff.is_empty() { None } else { Some(diff) }
    } else {
        None
    };

    // Read the result from the editor and strip comments
    let lossy_message = tui::get_text::from_editor_no_comments_as_patch(
        "commit_msg",
        &template,
        diff_text.as_deref(),
    )?
    .to_string();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn dummy_file_assignment(path: &str) -> FileAssignment {
        FileAssignment {
            path: path.into(),
            assignments: vec![CLIHunkAssignment {
                inner: but_hunk_assignment::HunkAssignment {
                    id: None,
                    hunk_header: None,
                    path: path.to_owned(),
                    path_bytes: path.as_bytes().into(),
                    stack_id: None,
                    branch_ref_bytes: None,
                    line_nums_added: None,
                    line_nums_removed: None,
                    diff: None,
                },
                cli_id: path.to_owned(),
            }],
        }
    }

    fn dummy_state() -> but_core::ui::ChangeState {
        but_core::ui::ChangeState {
            id: gix::ObjectId::from_str("0000000000000000000000000000000000000000").unwrap(),
            kind: gix::object::tree::EntryKind::Blob,
        }
    }

    fn dummy_rename_change(path: &str, previous_path: &str) -> TreeChange {
        TreeChange {
            path: path.into(),
            path_bytes: path.as_bytes().into(),
            status: but_core::ui::TreeStatus::Rename {
                previous_path: previous_path.into(),
                previous_path_bytes: previous_path.as_bytes().into(),
                previous_state: dummy_state(),
                state: dummy_state(),
                flags: None,
            },
        }
    }

    #[test]
    fn build_diff_specs_copies_previous_path_for_rename() {
        let files_to_commit = vec![dummy_file_assignment("new.txt")];
        let changes = vec![dummy_rename_change("new.txt", "old.txt")];

        let diff_specs = build_diff_specs(&files_to_commit, &changes);

        assert_eq!(diff_specs.len(), 1);
        assert_eq!(diff_specs[0].previous_path, Some("old.txt".into()));
    }
}
