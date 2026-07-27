use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt::Write,
};

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice;
use but_api::legacy::modes::{
    abort_edit_and_return_to_workspace, edit_initial_index_state, enter_edit_mode, operating_mode,
    save_edit_and_return_to_workspace_with_output,
};
use but_ctx::Context;
use gitbutler_commit::commit_ext::{CommitExt, CommitMessageBstr};
use gitbutler_edit_mode::commands::changes_from_initial;
use gitbutler_operating_modes::OperatingMode;

use crate::{
    IdMap,
    args::resolve::Subcommands,
    id::{CliId, CommitId, CommitIdRef},
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel, shorten_object_id},
};

pub(crate) fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    cmd: Option<Subcommands>,
    commit_id: Option<String>,
    ai: bool,
) -> Result<()> {
    if ai {
        if cmd.is_some() {
            bail!("--ai cannot be combined with a resolve subcommand");
        }
        return resolve_with_ai(ctx, out, commit_id.as_deref());
    }
    match cmd {
        Some(Subcommands::Status) => show_status(ctx, out),
        Some(Subcommands::Finish) => finish_resolution(ctx, out),
        Some(Subcommands::Cancel { force }) => cancel_resolution(ctx, out, force),
        None => {
            // Default action: enter resolution mode for the specified commit
            if let Some(commit_id_str) = commit_id {
                enter_resolution(ctx, out, &commit_id_str)
            } else {
                // Check if we're already in edit mode
                let mode = operating_mode(ctx)?.operating_mode;
                if matches!(mode, OperatingMode::Edit(_)) {
                    // If in edit mode, show status instead of help
                    show_status(ctx, out)
                } else {
                    // Not in edit mode and no commit specified - check for conflicted commits
                    check_and_prompt_for_conflicts(ctx, out)
                }
            }
        }
    }
}

/// Resolve a user-provided commit identifier (CLI ID or partial SHA) to an
/// object id, along with its display ref rendered from the same map.
fn parse_commit_id(ctx: &mut Context, commit_id_str: &str) -> Result<(gix::ObjectId, String)> {
    // Create an IdMap to resolve commit IDs (supports both CLI IDs and partial SHAs)
    let id_map = IdMap::legacy_new_from_context(ctx)?;

    // Resolve the commit ID using the IdMap
    let matches = id_map.parse_using_context(commit_id_str, ctx)?;

    if matches.is_empty() {
        bail!(
            "Commit '{commit_id_str}' not found. Try running 'but status' to see available commits."
        );
    }

    if matches.len() > 1 {
        bail!(
            "Commit ID '{commit_id_str}' is ambiguous. Please provide more characters to uniquely identify the commit."
        );
    }

    // Extract the commit OID from the matched CliId
    match &matches[0] {
        CliId::Commit {
            commit: CommitId { commit_id, .. },
            id: _,
        } => {
            let commit_ref = theme::Commit(CommitIdRef {
                commit_id: *commit_id,
                change_id: id_map
                    .change_id_ref(*commit_id)
                    .map(|change_id| &change_id.change_id),
            })
            .to_string();
            Ok((*commit_id, commit_ref))
        }
        _ => bail!("'{commit_id_str}' does not refer to a commit"),
    }
}

fn enter_resolution(ctx: &mut Context, out: &mut OutputChannel, commit_id_str: &str) -> Result<()> {
    let t = theme::get();
    use gix::{prelude::ObjectIdExt as _, revision::walk::Sorting};

    let (commit_gix_oid, commit_ref) = parse_commit_id(ctx, commit_id_str)?;

    // Get the commit and check if it's conflicted
    let repo = ctx.repo.get()?;
    let commit = repo
        .find_commit(commit_gix_oid)
        .context("Failed to find commit")?;

    if !commit.is_conflicted() {
        bail!(
            "Commit {commit_ref} is not in a conflicted state. Only conflicted commits can be resolved."
        );
    }

    // Find which stack this commit belongs to
    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;
    let mut found_stack_id = None;
    'outer: for stack in &stacks {
        // Check if this commit is in any of the stack's heads
        // TODO(ctx): use `ws` for that.
        // TODO(perf): This is likely to walk the whole graph.
        for head in &stack.branches {
            // Walk the commit history to see if our commit is in this stack
            let traversal = head
                .tip
                .attach(&repo)
                .ancestors()
                .sorting(Sorting::BreadthFirst)
                .all()?;

            for info in traversal {
                let info = info?;
                if info.id == commit_gix_oid {
                    found_stack_id = stack.id;
                    break 'outer;
                }
            }
        }
    }

    let stack_id = found_stack_id
        .ok_or_else(|| anyhow::anyhow!("Could not find stack containing commit {commit_ref}"))?;

    drop(commit);
    drop(repo);

    // Enter edit mode
    enter_edit_mode(ctx, commit_gix_oid, stack_id).context("Failed to enter edit mode")?;

    // Show checkout message
    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{} {}",
            t.important.paint("Checking out conflicted commit"),
            commit_ref
        )?;
    }

    // Now show the same status as `but resolve status` would show
    show_status(ctx, out)
}

fn show_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out, true)
}

/// Public function to show resolve status without prompting (for use by `but status`)
pub(crate) fn show_resolve_status(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    show_status_impl(ctx, out, false)
}

fn show_status_impl(
    ctx: &mut Context,
    out: &mut OutputChannel,
    prompt_to_finalize: bool,
) -> Result<()> {
    let t = theme::get();
    // Check if we're in edit mode
    let mode = operating_mode(ctx)?.operating_mode;
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    // The resolution state is the command's result, so it goes to stdout for human
    // and agent output alike; only the finalize prompt below is terminal-gated.
    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{}\n - resolve all conflicts \n - finalize with {} \n - OR cancel with {}\n",
            t.important
                .paint("You are currently in conflict resolution mode."),
            t.success.paint("but resolve finish"),
            t.error.paint("but resolve cancel")
        )?;
    }

    let all_resolved = show_conflicted_files(ctx, out)?;

    // If all conflicts are resolved and we're in human mode, offer to finalize
    if all_resolved && prompt_to_finalize {
        if let Some(human_out) = out.for_human() {
            writeln!(human_out)?;
            writeln!(
                human_out,
                "{}",
                t.success.paint("All conflicts have been resolved!")
            )?;
        }

        let mut progress = out.progress_channel();
        let should_finalize = if let Some(mut inout) = out.prepare_for_terminal_input() {
            if inout.confirm("Finalize the resolution now?", ConfirmDefault::Yes)? == Confirm::Yes {
                writeln!(progress)?;
                true
            } else {
                writeln!(
                    progress,
                    "Resolution not finalized. Run {} when ready.",
                    t.success.paint("but resolve finish")
                )?;
                false
            }
        } else {
            false
        };

        if should_finalize {
            return finish_resolution(ctx, out);
        }
    }

    Ok(())
}

fn show_conflicted_files(ctx: &mut Context, out: &mut OutputChannel) -> Result<bool> {
    let t = theme::get();
    let conflicted_files =
        edit_initial_index_state(ctx).context("Failed to get conflicted files")?;

    let initially_conflicted: Vec<_> = conflicted_files
        .iter()
        .filter(|(_, conflict)| conflict.is_some())
        .collect();

    // Check which files still have conflict markers
    let repo = ctx.repo.get()?;
    let repo_path = repo.workdir().context("No workdir")?;
    let mut still_conflicted = Vec::new();
    let mut resolved = Vec::new();

    // Contents are only needed to print conflict regions in human output;
    // JSON output keeps just the paths.
    let keep_content = !out.is_json();
    for (change, _) in &initially_conflicted {
        let file_path = repo_path.join(change.path.to_str_lossy().as_ref());
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    if has_conflict_markers(&content) {
                        still_conflicted.push((change, keep_content.then_some(content)));
                    } else {
                        resolved.push(change);
                    }
                }
                Err(_) => {
                    // If we can't read the file, consider it still conflicted
                    still_conflicted.push((change, None));
                }
            }
        } else {
            // If file doesn't exist, it was deleted - consider it resolved
            resolved.push(change);
        }
    }

    let all_resolved = still_conflicted.is_empty();

    // The conflicted/resolved listing is the command's result, shown to humans and agents alike.
    if let Some(human_out) = out.for_human() {
        if all_resolved {
            writeln!(
                human_out,
                "{}",
                t.success.paint("No conflicted files remaining!")
            )?;
        } else {
            writeln!(
                human_out,
                "{}:",
                t.attention.paint("Conflicted files remaining")
            )?;
            for (change, content) in &still_conflicted {
                writeln!(
                    human_out,
                    "  {} {}",
                    t.sym().error,
                    t.attention.paint(change.path.to_str_lossy())
                )?;
                if let Some(content) = content {
                    write_conflict_regions(human_out, content)?;
                }
            }
        }
        if !resolved.is_empty() {
            if !all_resolved {
                writeln!(human_out)?;
            }
            writeln!(human_out, "{} resolved:", t.success.paint("Files"))?;
            for change in &resolved {
                writeln!(
                    human_out,
                    "  {} {}",
                    t.sym().success,
                    t.success.paint(change.path.to_str_lossy())
                )?;
            }
        }
    }

    if let Some(out) = out.for_json() {
        let conflicted_list: Vec<String> = still_conflicted
            .iter()
            .map(|(change, _)| change.path.to_str_lossy().to_string())
            .collect();
        let resolved_list: Vec<String> = resolved
            .iter()
            .map(|change| change.path.to_str_lossy().to_string())
            .collect();
        out.write_value(serde_json::json!({
            "conflicted_files": conflicted_list,
            "resolved_files": resolved_list,
            "conflicted_count": conflicted_list.len(),
            "resolved_count": resolved_list.len(),
            "all_resolved": all_resolved
        }))?;
    }

    Ok(all_resolved)
}

/// Check if a file contains git conflict markers
/// Matches the logic from the GUI's looksConflicted() function
fn has_conflict_markers(content: &str) -> bool {
    content.lines().any(|line| line.starts_with("<<<<<<<"))
}

/// Print the conflict-marker regions of a conflicted file with line numbers,
/// so resolving does not require a separate read of the file. Large conflicts
/// are summarized as line ranges instead of quoted in full.
fn write_conflict_regions<W: std::fmt::Write + ?Sized>(
    out: &mut W,
    content: &str,
) -> std::fmt::Result {
    const MAX_QUOTED_LINES: usize = 60;
    let t = theme::get();
    let lines: Vec<&str> = content.lines().collect();

    let mut regions: Vec<(usize, usize)> = Vec::new();
    let mut start = None;
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("<<<<<<<") && start.is_none() {
            start = Some(index);
        } else if line.starts_with(">>>>>>>")
            && let Some(from) = start.take()
        {
            regions.push((from, index));
        }
    }
    // A start marker whose closing marker was lost (e.g. to a partial manual
    // edit) still deserves line numbers; treat it as running to end of file.
    if let Some(from) = start {
        regions.push((from, lines.len().saturating_sub(1)));
    }

    let quoted_lines: usize = regions.iter().map(|(from, to)| to - from + 1).sum();
    if quoted_lines > MAX_QUOTED_LINES {
        let ranges = regions
            .iter()
            .map(|(from, to)| format!("{}-{}", from + 1, to + 1))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            out,
            "      {}",
            t.hint.paint(format!("conflicts at lines {ranges}"))
        )?;
        return Ok(());
    }

    for (from, to) in regions {
        for (index, line) in lines.iter().enumerate().take(to + 1).skip(from) {
            // The file content comes from the repository and is untrusted;
            // strip control characters so it cannot inject terminal escapes.
            writeln!(out, "    {:>4}│{}", index + 1, sanitize_terminal_text(line))?;
        }
    }
    Ok(())
}

fn finish_resolution(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    let t = theme::get();
    // Check if we're in edit mode
    let mode = operating_mode(ctx)?.operating_mode;
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    // Capture conflicted commits BEFORE the rebase
    let conflicts_before = find_conflicted_commits(ctx)?;

    // Note files that still contain conflict markers, so the finish output can
    // answer the "did I leave markers behind?" question without a re-scan.
    // The scan may false-positive on legitimate content, so it warns rather
    // than refusing to finalize.
    let files_with_markers = files_with_conflict_markers(ctx)?;

    // Save and return to workspace, capturing the rebase output
    save_edit_and_return_to_workspace_with_output(ctx)
        .context("Failed to save resolution and return to workspace")?;

    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{}",
            t.success
                .paint("✓ Conflict resolution finalized successfully!")
        )?;
        writeln!(
            human_out,
            "The commit has been updated with your resolved changes."
        )?;
        if files_with_markers.is_empty() {
            writeln!(
                human_out,
                "{}",
                t.success
                    .paint("No conflict markers remain in the resolved files.")
            )?;
        } else {
            for path in &files_with_markers {
                writeln!(
                    human_out,
                    "{} {} still contains conflict markers — resolve it again if that was not intentional",
                    t.sym().error,
                    t.attention.paint(sanitize_terminal_text(path))
                )?;
            }
        }

        let uncommitted = uncommitted_worktree_paths(ctx);
        match uncommitted {
            Ok(paths) if paths.is_empty() => {
                writeln!(human_out, "Workspace restored; no uncommitted changes.")?;
            }
            Ok(paths) => {
                writeln!(
                    human_out,
                    "Workspace restored; uncommitted changes intact: {}",
                    paths.join(", ")
                )?;
            }
            Err(_) => {}
        }
    }

    // Report the complete remaining resolution state. Descendant commits that
    // were already conflicted before this resolution are just as actionable as
    // conflicts introduced by the rebase.
    report_conflicts_after_rebase(ctx, out, conflicts_before)?;

    Ok(())
}

/// Paths of initially-conflicted files that still contain conflict markers in
/// the edit-mode worktree.
fn files_with_conflict_markers(ctx: &mut Context) -> Result<Vec<String>> {
    let conflicted_files = edit_initial_index_state(ctx)?;
    let repo = ctx.repo.get()?;
    let repo_path = repo.workdir().context("No workdir")?;

    Ok(conflicted_files
        .iter()
        .filter(|(_, conflict)| conflict.is_some())
        .filter_map(|(change, _)| {
            let path = change.path.to_str_lossy().to_string();
            let content = std::fs::read_to_string(repo_path.join(&path)).ok()?;
            has_conflict_markers(&content).then_some(path)
        })
        .collect())
}

/// Paths with uncommitted worktree changes, for the finish summary. Sorted
/// for stable output, sanitized because paths come from the repository.
fn uncommitted_worktree_paths(ctx: &mut Context) -> Result<Vec<String>> {
    let repo = ctx.repo.get()?;
    let changes = but_core::diff::worktree_changes(&repo)?;
    let mut paths: Vec<String> = changes
        .changes
        .iter()
        .map(|change| sanitize_terminal_text(&change.path.to_str_lossy()))
        .collect();
    paths.sort();
    Ok(paths)
}

fn cancel_resolution(ctx: &mut Context, out: &mut OutputChannel, force: bool) -> Result<()> {
    let t = theme::get();
    // Check if we're in edit mode
    let mode = operating_mode(ctx)?.operating_mode;
    if !matches!(mode, OperatingMode::Edit(_)) {
        // Not in edit mode, show the workflow help instead
        return show_workflow_help(out);
    }

    if !force && {
        let guard = ctx.shared_worktree_access();
        !changes_from_initial(ctx, guard.read_permission())?.is_empty()
    } {
        bail!(
            "There are changes that differ from the original commit you were editing. Canceling will drop those changes.\n\nIf you want to go through with this, please re-run with `--force`.\n\nIf you want to keep the changes you have made, consider finishing the resolution and then moving the changes with `but squash`."
        )
    }

    // Abort and return to workspace
    abort_edit_and_return_to_workspace(ctx, force)
        .context("Failed to cancel resolution and return to workspace")?;

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "{}",
            t.attention.paint("Conflict resolution cancelled.")
        )?;
        writeln!(
            out,
            "All changes made during resolution have been discarded."
        )?;
    }

    Ok(())
}

/// Resolve conflicts with the configured AI model: one commit when
/// `commit_id_str` is given, otherwise every conflicted commit in the
/// workspace, oldest first.
fn resolve_with_ai(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_id_str: Option<&str>,
) -> Result<()> {
    let t = theme::get();
    let mode = operating_mode(ctx)?.operating_mode;
    if matches!(mode, OperatingMode::Edit(_)) {
        bail!(
            "You are in conflict resolution mode. Finish with `but resolve finish` or cancel with `but resolve cancel` before using --ai."
        );
    }

    let mut results = Vec::new();
    if let Some(commit_id_str) = commit_id_str {
        let (commit_oid, _commit_ref) = parse_commit_id(ctx, commit_id_str)?;
        results.push(resolve_one_with_ai(ctx, out, commit_oid)?);
    } else {
        // Resolving a commit rebases its descendants, which changes their ids
        // and can change (or clear) their conflicts — so re-discover the
        // oldest conflicted commit after every resolution.
        loop {
            let Some(commit_oid) = oldest_conflicted_commit(ctx)? else {
                break;
            };
            results.push(resolve_one_with_ai(ctx, out, commit_oid)?);
        }
    }

    // A single JSON document for the whole invocation, regardless of how many
    // commits were resolved.
    if let Some(json_out) = out.for_json() {
        let results = results
            .iter()
            .map(|result| {
                serde_json::json!({
                    "commit_id": result.commit_id.to_string(),
                    "new_commit_id": result.new_commit.to_string(),
                    "summary": result.summary,
                    "files": result
                        .files
                        .iter()
                        .map(|file| {
                            serde_json::json!({
                                "path": file.path,
                                "reasoning": file.reasoning,
                            })
                        })
                        .collect::<Vec<_>>(),
                })
            })
            .collect::<Vec<_>>();
        json_out.write_value(serde_json::json!({ "resolved": results }))?;
    }

    if results.is_empty() {
        if let Some(human_out) = out.for_human() {
            writeln!(
                human_out,
                "{}",
                t.success.paint("No conflicted commits found.")
            )?;
        }
        return Ok(());
    }

    if let Some(human_out) = out.for_human() {
        // The rebase of descendants can leave (or newly introduce) conflicted
        // commits; in single-commit mode nothing else surfaces that. Commits
        // are listed once per branch that contains them, so count unique ids.
        let remaining = find_conflicted_commits(ctx)?
            .values()
            .flatten()
            .map(|commit| commit.commit_oid)
            .collect::<HashSet<_>>()
            .len();
        if remaining > 0 {
            writeln!(
                human_out,
                "{}",
                t.attention.paint(format!(
                    "{remaining} commit{} still conflicted — run `but resolve --ai` or `but status` to see them.",
                    if remaining == 1 { " is" } else { "s are" }
                ))
            )?;
        }
        writeln!(
            human_out,
            "{}",
            t.hint
                .paint("If you disagree with a resolution, run `but undo` to revert it.")
        )?;
    }
    Ok(())
}

/// The first conflicted commit in the oldest-first ordering of
/// [`find_conflicted_commits()`], if any.
fn oldest_conflicted_commit(ctx: &mut Context) -> Result<Option<gix::ObjectId>> {
    Ok(find_conflicted_commits(ctx)?
        .into_values()
        .flatten()
        .next()
        .map(|commit| commit.commit_oid))
}

fn resolve_one_with_ai(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_oid: gix::ObjectId,
) -> Result<but_api::resolve::AiResolutionResult> {
    let t = theme::get();
    let commit_ref = {
        let repo = ctx.repo.get()?;
        theme::Commit(crate::utils::get_change_id_for_commit(&repo, commit_oid)?).to_string()
    };
    {
        let mut progress = out.progress_channel();
        writeln!(
            progress,
            "{} {}{}",
            t.important.paint("Resolving conflicts in commit"),
            commit_ref,
            t.important.paint(" with AI…")
        )?;
    }

    let result =
        but_api::resolve::resolve_commit_conflicts_ai(ctx, commit_oid, but_core::DryRun::No)?;

    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{} {} {} {}",
            t.success.paint("✓ Resolved"),
            commit_ref,
            t.success.paint("→"),
            {
                let repo = ctx.repo.get()?;
                theme::Commit(crate::utils::get_change_id_for_commit(
                    &repo,
                    result.new_commit,
                )?)
            },
        )?;
        if let Some(summary) = &result.summary {
            writeln!(human_out)?;
            writeln!(human_out, "{}", sanitize_terminal_text(summary))?;
        }
        writeln!(human_out)?;
        for file in &result.files {
            writeln!(
                human_out,
                "  {} {}",
                t.sym().success,
                t.success.paint(&file.path)
            )?;
            writeln!(
                human_out,
                "    {}",
                t.hint.paint(sanitize_terminal_text(&file.reasoning))
            )?;
        }
        writeln!(human_out)?;
    }

    Ok(result)
}

/// Strip control characters (including ANSI escape sequences) from
/// model-authored text before printing it to the terminal.
/// Strip control characters (keeping newlines and tabs) from text that will
/// be written to the terminal but originates outside this program - model
/// output or repository file content.
fn sanitize_terminal_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// Structure to hold information about a conflicted commit
#[derive(Clone, Debug)]
pub(crate) struct ConflictedCommit {
    pub(crate) commit_oid: gix::ObjectId,
    /// Stable across rebases when the commit carries GitButler's change-id header.
    pub(crate) change_id: but_core::ChangeId,
    pub(crate) commit_short_id: String,
    pub(crate) commit_message: String,
}

/// Report every conflicted commit left after the rebase, highlighting any that
/// were newly introduced.
fn report_conflicts_after_rebase(
    ctx: &mut Context,
    out: &mut OutputChannel,
    conflicts_before: BTreeMap<String, Vec<ConflictedCommit>>,
) -> Result<()> {
    let t = theme::get();
    // Get the current list of conflicted commits after the rebase
    let conflicts_after = find_conflicted_commits(ctx)?;

    let mut change_ids_before = HashMap::new();
    for commit in unique_conflict_queue(&conflicts_before) {
        *change_ids_before
            .entry(commit.change_id.clone())
            .or_insert(0usize) += 1;
    }
    let resolution_queue = unique_conflict_queue(&conflicts_after);
    let newly_conflicted = resolution_queue
        .iter()
        .filter(|commit| {
            let remaining = change_ids_before
                .entry(commit.change_id.clone())
                .or_insert(0);
            if *remaining == 0 {
                true
            } else {
                *remaining -= 1;
                false
            }
        })
        .copied()
        .collect::<Vec<_>>();

    if let Some(human_out) = out.for_human() {
        if conflicts_after.is_empty() {
            writeln!(
                human_out,
                "{}",
                t.success.paint("No conflicted commits remain.")
            )?;
        } else {
            writeln!(human_out)?;
            if newly_conflicted.is_empty() {
                writeln!(
                    human_out,
                    "{}",
                    t.attention
                        .paint("Remaining conflicted commits (oldest first):")
                )?;
            } else {
                writeln!(
                    human_out,
                    "{}",
                    t.attention
                        .paint("⚠ Warning: New conflicts were introduced during the rebase.")
                )?;
                writeln!(
                    human_out,
                    "{}",
                    t.attention
                        .paint("Remaining conflicted commits (oldest first):")
                )?;
            }

            for (branch_name, commits) in &conflicts_after {
                writeln!(
                    human_out,
                    "  {} {}",
                    t.important.paint("Branch:"),
                    t.local_branch.paint(branch_name)
                )?;
                for commit in commits {
                    writeln!(
                        human_out,
                        "    {} {} {}",
                        t.sym().dot.error(),
                        t.hint.paint(&commit.commit_short_id),
                        commit.commit_message
                    )?;
                }
            }

            let next_commit = resolution_queue
                .first()
                .copied()
                .expect("non-empty conflicts map contains a commit");
            writeln!(
                human_out,
                "Resolve the next commit with {}.",
                t.command_suggestion
                    .paint(format!("but resolve {}", next_commit.commit_short_id))
            )?;
        }
    }

    if let Some(json_out) = out.for_json() {
        let conflicts_by_branch = conflicts_after
            .iter()
            .map(|(branch_name, commits)| {
                (
                    branch_name,
                    commits
                        .iter()
                        .map(|commit| {
                            serde_json::json!({
                                "commit_id": commit.commit_oid.to_string(),
                                "commit_short_id": commit.commit_short_id,
                                "commit_message": commit.commit_message,
                            })
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let newly_conflicted_json = newly_conflicted
            .iter()
            .map(|commit| {
                serde_json::json!({
                    "commit_id": commit.commit_oid.to_string(),
                    "commit_short_id": commit.commit_short_id,
                    "commit_message": commit.commit_message,
                })
            })
            .collect::<Vec<_>>();
        let resolution_queue_json = resolution_queue
            .iter()
            .map(|commit| {
                serde_json::json!({
                    "commit_id": commit.commit_oid.to_string(),
                    "commit_short_id": commit.commit_short_id,
                    "commit_message": commit.commit_message,
                })
            })
            .collect::<Vec<_>>();

        json_out.write_value(serde_json::json!({
            "remaining_conflicted_commits_by_branch": conflicts_by_branch,
            "resolution_queue": resolution_queue_json,
            "total_remaining_conflicted_commits": resolution_queue.len(),
            "newly_conflicted_commits": newly_conflicted_json,
            "count": newly_conflicted.len(),
        }))?;
    }

    Ok(())
}

/// Merge each branch's oldest-first conflict list into one stable queue.
///
/// A descendant branch's walk contains its conflicted ancestors, so preserving
/// each list's order keeps dependencies bottom-up. The branch-map order only
/// breaks ties between independent histories, where either order is valid.
fn unique_conflict_queue(
    conflicts_by_branch: &BTreeMap<String, Vec<ConflictedCommit>>,
) -> Vec<&ConflictedCommit> {
    let mut seen = HashSet::new();
    conflicts_by_branch
        .values()
        .flatten()
        .filter(|commit| seen.insert(commit.commit_oid))
        .collect()
}

/// Find all conflicted commits across all stacks, grouped by branch
pub(crate) fn find_conflicted_commits(
    ctx: &mut Context,
) -> Result<BTreeMap<String, Vec<ConflictedCommit>>> {
    use gix::{prelude::ObjectIdExt as _, revision::walk::Sorting};

    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;
    // Built lazily on the first conflicted commit — building it is not free.
    // Best-effort: in edit mode the map may not build; fall back to shas then.
    let mut id_map: Option<Option<IdMap>> = None;
    let repo = ctx.repo.get()?;
    let mut conflicts_by_branch: BTreeMap<String, Vec<ConflictedCommit>> = BTreeMap::new();

    for stack in &stacks {
        // Check commits in each head of the stack
        for head in &stack.branches {
            let branch_name = head.name.clone();

            // Walk the commit history to find conflicted commits
            // We use BreadthFirst (topological) and then reverse the results
            let traversal = head
                .tip
                .attach(&repo)
                .ancestors()
                .sorting(Sorting::BreadthFirst)
                .all()?;

            // Collect commits first, then reverse for REVERSE sorting behavior
            let commit_ids: Vec<gix::ObjectId> = traversal
                .filter_map(Result::ok)
                .map(|info| info.id)
                .collect();

            for oid in commit_ids.into_iter().rev() {
                let commit = repo.find_commit(oid)?;

                if commit.is_conflicted() {
                    // Commit messages can come from untrusted upstreams and
                    // end up on the terminal (pull summary, resolve listing).
                    let message = sanitize_terminal_text(
                        &commit
                            .message_bstr()
                            .to_string()
                            .lines()
                            .next()
                            .context("Commit has no message")?
                            .chars()
                            .take(50)
                            .collect::<String>(),
                    );

                    let change_id = crate::utils::get_change_id_for_commit(&repo, oid)?;
                    let conflicted = ConflictedCommit {
                        commit_oid: oid,
                        change_id,
                        commit_short_id: id_map
                            .get_or_insert_with(|| IdMap::legacy_new_from_context(ctx).ok())
                            .as_ref()
                            .and_then(|id_map| id_map.change_id_ref(oid))
                            .map(|change_id| change_id.padded_short_id())
                            .unwrap_or_else(|| shorten_object_id(&repo, oid)),
                        commit_message: message,
                    };

                    conflicts_by_branch
                        .entry(branch_name.clone())
                        .or_default()
                        .push(conflicted);
                }
            }
        }
    }

    Ok(conflicts_by_branch)
}

/// Check for conflicted commits and prompt user to resolve them
fn check_and_prompt_for_conflicts(ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    let t = theme::get();
    // Find all conflicted commits
    let conflicts_by_branch = find_conflicted_commits(ctx)?;

    if conflicts_by_branch.is_empty() {
        // No conflicts found, show the normal help text
        return show_workflow_help(out);
    }

    if let Some(json_out) = out.for_json() {
        let mut json_conflicts = serde_json::Map::new();

        for (branch_name, commits) in &conflicts_by_branch {
            let commits_array: Vec<serde_json::Value> = commits
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "commit_id": c.commit_oid.to_string(),
                        "commit_short_id": c.commit_short_id,
                        "commit_message": c.commit_message,
                    })
                })
                .collect();

            json_conflicts.insert(branch_name.clone(), serde_json::Value::Array(commits_array));
        }

        json_out.write_value(serde_json::json!({
            "conflicted_commits_by_branch": json_conflicts,
            "total_conflicted_commits": conflicts_by_branch.values().map(|v| v.len()).sum::<usize>(),
        }))?;
        return Ok(());
    }

    // We have conflicts - show them grouped by branch. The listing is the command's
    // result, so it goes to stdout for human and agent output alike; only the prompt
    // below is gated on an interactive terminal.
    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{}",
            t.attention.paint("Found conflicted commits:")
        )?;
        writeln!(human_out)?;

        for (branch_name, commits) in &conflicts_by_branch {
            writeln!(
                human_out,
                "{} {}",
                t.important.paint("Branch:"),
                t.local_branch.paint(branch_name)
            )?;
            for commit in commits {
                writeln!(
                    human_out,
                    "  {} {} {}",
                    t.sym().dot.error(),
                    t.hint.paint(&commit.commit_short_id),
                    commit.commit_message
                )?;
            }
            writeln!(human_out)?;
        }
    }

    let commit_options = conflicts_by_branch
        .values()
        .flatten()
        .map(|commit| {
            (
                format!("{} {}", commit.commit_short_id, commit.commit_message),
                commit.commit_short_id.clone(),
            )
        })
        .collect::<Vec<_>>();

    // Interactive prompting only for human output mode with terminal
    let commit_id_to_resolve = if let Some(mut inout) = out.prepare_for_terminal_input() {
        let Some(commit_options) = nonempty::NonEmpty::from_vec(commit_options) else {
            return Ok(());
        };
        writeln!(
            inout,
            "{}",
            t.important
                .paint("Would you like to start resolving these conflicts?")
        )?;
        inout
            .prompt_select("Select commit to resolve", &commit_options)?
            .cloned()
    } else {
        None
    };

    if let Some(commit_id_to_resolve) = commit_id_to_resolve {
        // Enter resolution mode for the selected commit
        let mut progress = out.progress_channel();
        writeln!(progress)?;
        return enter_resolution(ctx, out, &commit_id_to_resolve);
    }

    if let Some(human_out) = out.for_human() {
        writeln!(
            human_out,
            "{}",
            t.hint
                .paint("Run `but resolve <commit-id>` to start resolving a commit.")
        )?;
    }

    Ok(())
}

fn show_workflow_help(out: &mut OutputChannel) -> Result<()> {
    let t = theme::get();
    if let Some(out) = out.for_human() {
        writeln!(out, "{}", t.important.paint("Conflict Resolution Workflow"))?;
        writeln!(out)?;
        writeln!(
            out,
            "This command is used when you have a commit in a conflicted state"
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "{}",
            t.important.paint("To resolve conflicts in a commit:")
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Enter resolution mode for a conflicted commit:",
            t.important.paint("1.")
        )?;
        writeln!(
            out,
            "     {}",
            t.command_suggestion.paint("but resolve <commit>")
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Resolve conflicts in the conflicted files",
            t.important.paint("2.")
        )?;
        writeln!(
            out,
            "     Edit the files to remove conflict markers ({}, {}, {})",
            t.error.paint("<<<<<<<"),
            t.attention.paint("======="),
            t.error.paint(">>>>>>>")
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Check which files are still conflicted:",
            t.important.paint("3.")
        )?;
        writeln!(
            out,
            "     {}",
            t.command_suggestion.paint("but resolve status")
        )?;
        writeln!(out)?;
        writeln!(
            out,
            "  {} Finalize or cancel the resolution:",
            t.important.paint("4.")
        )?;
        writeln!(
            out,
            "     {}",
            t.command_suggestion.paint("but resolve finish")
        )?;
        writeln!(out, "     {}", t.hint.paint("OR"))?;
        writeln!(
            out,
            "     {}",
            t.command_suggestion.paint("but resolve cancel")
        )?;
        writeln!(out)?;
        writeln!(out, "{}", t.important.paint("Example:"))?;
        writeln!(
            out,
            "  {} (find conflicted commits)",
            t.command_suggestion.paint("but status")
        )?;
        writeln!(
            out,
            "  {} (enter resolution mode)",
            t.command_suggestion.paint("but resolve 55")
        )?;
        writeln!(
            out,
            "  {} (edit files to resolve conflicts)",
            t.hint.paint("vim src/file.rs")
        )?;
        writeln!(
            out,
            "  {} (check remaining conflicts)",
            t.command_suggestion.paint("but resolve status")
        )?;
        writeln!(
            out,
            "  {} (finalize)",
            t.command_suggestion.paint("but resolve finish")
        )?;
    } else if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "workflow": [
                {
                    "step": 1,
                    "description": "Enter resolution mode for a conflicted commit",
                    "command": "but resolve <commit>"
                },
                {
                    "step": 2,
                    "description": "Resolve conflicts in the conflicted files",
                    "details": "Edit the files to remove conflict markers (<<<<<<<, =======, >>>>>>>)"
                },
                {
                    "step": 3,
                    "description": "Check which files are still conflicted",
                    "command": "but resolve status"
                },
                {
                    "step": 4,
                    "description": "Finalize the resolution",
                    "command": "but resolve finish"
                }
            ],
            "other_commands": {
                "cancel": "but resolve cancel",
                "view_status": "but status"
            }
        }))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{ConflictedCommit, unique_conflict_queue, write_conflict_regions};

    fn render(content: &str) -> String {
        let mut out = String::new();
        write_conflict_regions(&mut out, content).unwrap();
        out
    }

    #[test]
    fn prints_terminated_regions_with_line_numbers() {
        let out = render("context\n<<<<<<< ours\nx\n=======\ny\n>>>>>>> theirs\ntrailer\n");
        assert!(out.contains("2│<<<<<<< ours"));
        assert!(out.contains("6│>>>>>>> theirs"));
        assert!(
            !out.contains("context"),
            "lines outside the region stay out"
        );
    }

    #[test]
    fn unterminated_region_runs_to_end_of_file() {
        let out = render("<<<<<<< ours\nx\nlast line\n");
        assert!(out.contains("1│<<<<<<< ours"));
        assert!(out.contains("3│last line"));
    }

    #[test]
    fn strips_control_characters_from_conflict_lines() {
        let out = render("<<<<<<< ours\n\u{1b}[31mred\u{7}\n>>>>>>> theirs\n");
        assert!(!out.contains('\u{1b}'));
        assert!(!out.contains('\u{7}'));
        assert!(out.contains("[31mred"));
    }

    #[test]
    fn oversized_conflicts_summarize_as_line_ranges() {
        let mut content = String::from("<<<<<<< ours\n");
        for index in 0..70 {
            content.push_str(&format!("line {index}\n"));
        }
        content.push_str(">>>>>>> theirs\n");
        let out = render(&content);
        assert!(out.contains("conflicts at lines 1-72"));
        assert!(!out.contains("line 3"));
    }

    #[test]
    fn conflict_queue_deduplicates_oids_not_colliding_change_ids() {
        let change_id = but_core::ChangeId::from_bytes(b"same");
        let first = ConflictedCommit {
            commit_oid: gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")
                .unwrap(),
            change_id: change_id.clone(),
            commit_short_id: "same".into(),
            commit_message: "first".into(),
        };
        let second = ConflictedCommit {
            commit_oid: gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")
                .unwrap(),
            change_id,
            commit_short_id: "same#2".into(),
            commit_message: "second".into(),
        };
        let conflicts = BTreeMap::from([
            ("A".into(), vec![first.clone(), second.clone()]),
            ("B".into(), vec![first.clone()]),
        ]);

        let queue = unique_conflict_queue(&conflicts);
        assert_eq!(queue.len(), 2, "distinct commits must remain in the queue");
        assert_eq!(queue[0].commit_oid, first.commit_oid);
        assert_eq!(queue[1].commit_oid, second.commit_oid);
    }
}
