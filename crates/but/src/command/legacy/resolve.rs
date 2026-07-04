use std::{
    collections::{BTreeMap, HashSet},
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
    id::CliId,
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
            bail!(
                "--ai cannot be combined with a resolve subcommand. For one conflict, use `but resolve apply <path>[:<N>] --ai` instead."
            );
        }
        return resolve_with_ai(ctx, out, commit_id.as_deref());
    }
    match cmd {
        Some(Subcommands::Conflicts { commit }) => list_conflicts(ctx, out, commit.as_deref()),
        Some(Subcommands::Apply {
            target,
            commit,
            ours,
            theirs,
            ai,
            file,
        }) => {
            let flags = ResolutionFlags {
                ours,
                theirs,
                ai,
                file,
            };
            // Lock stdin at the CLI boundary; the handler reads it only for a
            // piped mixed-content resolution.
            apply_resolutions(
                ctx,
                out,
                commit.as_deref(),
                &target,
                flags,
                std::io::stdin().lock(),
            )
        }
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

/// Resolve a user-provided commit identifier (CLI ID or partial SHA) to an object id.
fn parse_commit_id(ctx: &mut Context, commit_id_str: &str) -> Result<gix::ObjectId> {
    // Create an IdMap to resolve commit IDs (supports both CLI IDs and partial SHAs)
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;

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
        CliId::Commit { commit_id, .. } => Ok(*commit_id),
        _ => bail!("'{commit_id_str}' does not refer to a commit"),
    }
}

fn enter_resolution(ctx: &mut Context, out: &mut OutputChannel, commit_id_str: &str) -> Result<()> {
    let t = theme::get();
    use gix::{prelude::ObjectIdExt as _, revision::walk::Sorting};

    let commit_gix_oid = parse_commit_id(ctx, commit_id_str)?;

    // Get the commit and check if it's conflicted
    let repo = ctx.repo.get()?;
    let commit = repo
        .find_commit(commit_gix_oid)
        .context("Failed to find commit")?;

    if !commit.is_conflicted() {
        let commit_short = shorten_object_id(&repo, commit_gix_oid);
        bail!(
            "Commit {commit_short} is not in a conflicted state. Only conflicted commits can be resolved."
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

    let stack_id = found_stack_id.ok_or_else(|| {
        let commit_short = shorten_object_id(&repo, commit_gix_oid);
        anyhow::anyhow!("Could not find stack containing commit {commit_short}")
    })?;

    drop(commit);
    drop(repo);

    // Enter edit mode
    enter_edit_mode(ctx, commit_gix_oid, stack_id).context("Failed to enter edit mode")?;

    // Show checkout message
    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            out,
            "{} {}",
            t.important.paint("Checking out conflicted commit"),
            t.commit_id.paint(shorten_object_id(&repo, commit_gix_oid))
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

    for (change, _) in &initially_conflicted {
        let file_path = repo_path.join(change.path.to_str_lossy().as_ref());
        if file_path.exists() {
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    if has_conflict_markers(&content) {
                        still_conflicted.push(change);
                    } else {
                        resolved.push(change);
                    }
                }
                Err(_) => {
                    // If we can't read the file, consider it still conflicted
                    still_conflicted.push(change);
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
            for change in &still_conflicted {
                writeln!(
                    human_out,
                    "  {} {}",
                    t.sym().error,
                    t.attention.paint(change.path.to_str_lossy())
                )?;
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
            .map(|change| change.path.to_str_lossy().to_string())
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
    }

    // Check for new conflicts introduced during the rebase
    check_for_new_conflicts_after_rebase(ctx, out, conflicts_before)?;

    Ok(())
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
            "There are changes that differ from the original commit you were editing. Canceling will drop those changes.\n\nIf you want to go through with this, please re-run with `--force`.\n\nIf you want to keep the changes you have made, consider finishing the resolution and then moving the changes with the rub command."
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

/// Where a conflicted-commit target resolved to, with enough context to tell
/// the user what scope they are working in.
struct ConflictTarget {
    commit_oid: gix::ObjectId,
    /// The branch the commit sits on, if it is a known conflicted commit.
    branch: Option<String>,
    /// Conflicted commits elsewhere in the workspace (other ids than the target).
    other_conflicted: usize,
}

/// Resolve `target` — a commit id, a CLI id, or a branch name (meaning that
/// branch's oldest conflicted commit) — or default to the oldest conflicted
/// commit of the first conflicted branch. `Ok(None)` means nothing is
/// conflicted.
fn target_conflicted_commit(
    ctx: &mut Context,
    target: Option<&str>,
) -> Result<Option<ConflictTarget>> {
    let conflicts_by_branch = find_conflicted_commits(ctx)?;
    let commit_oid = match target {
        // An exact conflicted-branch name scopes to that branch.
        Some(target) if conflicts_by_branch.contains_key(target) => {
            conflicts_by_branch[target][0].commit_oid
        }
        Some(target) => match parse_commit_or_branch(ctx, target)? {
            CommitOrBranch::Commit(commit_oid) => commit_oid,
            CommitOrBranch::Branch(name) => match conflicts_by_branch.get(&name) {
                Some(commits) => commits[0].commit_oid,
                None => bail!("Branch \"{name}\" has no conflicted commits."),
            },
        },
        None => match conflicts_by_branch.values().flatten().next() {
            Some(commit) => commit.commit_oid,
            None => return Ok(None),
        },
    };

    let branch = conflicts_by_branch
        .iter()
        .find(|(_, commits)| commits.iter().any(|commit| commit.commit_oid == commit_oid))
        .map(|(branch, _)| branch.clone());
    let other_conflicted = conflicts_by_branch
        .values()
        .flatten()
        .filter(|commit| commit.commit_oid != commit_oid)
        .map(|commit| commit.commit_oid)
        .collect::<HashSet<_>>()
        .len();
    Ok(Some(ConflictTarget {
        commit_oid,
        branch,
        other_conflicted,
    }))
}

enum CommitOrBranch {
    Commit(gix::ObjectId),
    Branch(String),
}

/// Resolve a user-provided identifier to a commit or a branch.
fn parse_commit_or_branch(ctx: &mut Context, target: &str) -> Result<CommitOrBranch> {
    let id_map = IdMap::legacy_new_from_context(ctx, None)?;
    let matches = id_map.parse_using_context(target, ctx)?;
    match matches.as_slice() {
        [] => bail!(
            "\"{target}\" is neither a commit nor a branch. Try running 'but status' to see what is available."
        ),
        [CliId::Commit { commit_id, .. }] => Ok(CommitOrBranch::Commit(*commit_id)),
        [CliId::Branch { name, .. }] => Ok(CommitOrBranch::Branch(name.clone())),
        [_] => bail!("\"{target}\" does not refer to a commit or a branch"),
        _ => bail!(
            "\"{target}\" is ambiguous. Please provide more characters to uniquely identify it."
        ),
    }
}

/// List the conflicts of a conflicted commit without entering resolution mode.
fn list_conflicts(ctx: &mut Context, out: &mut OutputChannel, commit: Option<&str>) -> Result<()> {
    let t = theme::get();
    let Some(target) = target_conflicted_commit(ctx, commit)? else {
        if let Some(human_out) = out.for_human() {
            writeln!(
                human_out,
                "{}",
                t.success.paint("No conflicted commits found.")
            )?;
        }
        if let Some(json_out) = out.for_json() {
            json_out.write_value(serde_json::json!({ "commit_id": null, "files": [] }))?;
        }
        return Ok(());
    };

    let conflicts = but_api::resolve::commit_conflicts(ctx, target.commit_oid)?;
    if conflicts.files.is_empty() {
        if let Some(human_out) = out.for_human() {
            let repo = ctx.repo.get()?;
            writeln!(
                human_out,
                "{} {} {}",
                t.important.paint("Commit"),
                t.commit_id
                    .paint(shorten_object_id(&repo, target.commit_oid)),
                t.success.paint("is not conflicted.")
            )?;
        }
        if let Some(json_out) = out.for_json() {
            json_out.write_value(serde_json::json!({
                "commit_id": target.commit_oid.to_string(),
                "files": [],
            }))?;
        }
        return Ok(());
    }

    if let Some(human_out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            human_out,
            "{} {}{}",
            t.important.paint("Conflicts in commit"),
            t.commit_id
                .paint(shorten_object_id(&repo, target.commit_oid)),
            target
                .branch
                .as_deref()
                .map(|branch| format!(
                    "{} {}",
                    t.important.paint(" on branch"),
                    t.local_branch.paint(branch)
                ))
                .unwrap_or_default()
        )?;
        for file in &conflicts.files {
            writeln!(human_out)?;
            writeln!(human_out, "{}", t.attention.paint(&file.path))?;
            let total = file.hunks.len();
            for (index, hunk) in file.hunks.iter().enumerate() {
                writeln!(
                    human_out,
                    "{}",
                    t.important
                        .paint(format!("── conflict {} of {total}", index + 1))
                )?;
                writeln!(human_out, "{}", t.hint.paint("<<< ours (new base)"))?;
                writeln!(human_out, "{}", hunk.ours)?;
                if let Some(base) = &hunk.base {
                    writeln!(human_out, "{}", t.hint.paint("||| base"))?;
                    writeln!(human_out, "{base}")?;
                }
                writeln!(human_out, "{}", t.hint.paint(">>> theirs (this commit)"))?;
                writeln!(human_out, "{}", hunk.theirs)?;
            }
        }
        writeln!(human_out)?;
        if target.other_conflicted > 0 {
            writeln!(
                human_out,
                "{}",
                t.attention.paint(format!(
                    "{} other conflicted commit{} exist{} — scope with `but resolve conflicts <branch>`.",
                    target.other_conflicted,
                    if target.other_conflicted == 1 { "" } else { "s" },
                    if target.other_conflicted == 1 { "s" } else { "" },
                ))
            )?;
        }
        // With several conflicted commits, a bare `apply` targets the default
        // commit, not necessarily the one listed here — pin it in the hint.
        let commit_flag = (target.other_conflicted > 0)
            .then_some(target.branch.as_deref())
            .flatten()
            .map(|branch| format!(" --commit {branch}"))
            .unwrap_or_default();
        writeln!(
            human_out,
            "{}",
            t.hint.paint(format!(
                "Resolve with `but resolve apply <path>[:<N>]{commit_flag} --ours|--theirs` or pipe mixed content into `but resolve apply <path>:<N>{commit_flag}`. `but resolve --ai` resolves everything at once."
            ))
        )?;
    }

    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({
            "commit_id": conflicts.commit_id.to_string(),
            "branch": target.branch,
            "files": conflicts.files,
        }))?;
    }

    Ok(())
}

/// Apply resolutions to one conflict (`<path>:<N>`) or to every conflict of a
/// file, without entering resolution mode.
/// How `but resolve apply` was asked to resolve, straight from the CLI flags.
struct ResolutionFlags {
    ours: bool,
    theirs: bool,
    ai: bool,
    file: Option<std::path::PathBuf>,
}

fn apply_resolutions(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit: Option<&str>,
    target: &str,
    flags: ResolutionFlags,
    mut read: impl std::io::Read,
) -> Result<()> {
    use but_api::resolve::{HunkResolution, ResolutionSpec};

    let ResolutionFlags {
        ours,
        theirs,
        ai,
        file,
    } = flags;

    let t = theme::get();
    let mode = operating_mode(ctx)?.operating_mode;
    if matches!(mode, OperatingMode::Edit(_)) {
        bail!(
            "You are in conflict resolution mode. Finish with `but resolve finish` or cancel with `but resolve cancel` first."
        );
    }
    let Some(conflict_target) = target_conflicted_commit(ctx, commit)? else {
        bail!("No conflicted commits found.");
    };
    let commit_oid = conflict_target.commit_oid;

    // Split a trailing `:<number>` off the target; everything else is the path.
    let (path, hunk) = match target.rsplit_once(':') {
        Some((path, number))
            if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (path, Some(number.parse::<usize>()?))
        }
        _ => (target, None),
    };

    let resolution = if ours {
        HunkResolution::Ours
    } else if theirs {
        HunkResolution::Theirs
    } else if ai {
        HunkResolution::Ai
    } else {
        let content = match &file {
            Some(file) => std::fs::read_to_string(file)
                .with_context(|| format!("Failed to read {}", file.display()))?,
            None => {
                if out.can_prompt() {
                    bail!(
                        "Provide the replacement content via --file or stdin, or take a side with --ours/--theirs/--ai."
                    );
                }
                let mut content = String::new();
                std::io::Read::read_to_string(&mut read, &mut content)?;
                if content.is_empty() {
                    bail!(
                        "stdin was empty. To intentionally delete the conflicted region, pass --file with an empty file."
                    );
                }
                content
            }
        };
        HunkResolution::Content(content)
    };

    // Expand a bare path into explicit per-hunk specs; an explicit `:<N>`
    // target needs no lookup, the API validates it.
    let hunks: Vec<usize> = match (hunk, &resolution) {
        (Some(hunk), _) => vec![hunk],
        (None, _) => {
            let conflicts = but_api::resolve::commit_conflicts(ctx, commit_oid)?;
            if conflicts.files.is_empty() {
                bail!("Commit {commit_oid} is not conflicted.");
            }
            // Match with the same normalization the API applies, so spellings
            // it would accept (`./`, backslashes, duplicate slashes) resolve.
            let lookup = but_api::resolve::normalize_path(path);
            let total_hunks = conflicts
                .files
                .iter()
                .find(|conflicted| but_api::resolve::normalize_path(&conflicted.path) == lookup)
                .map(|conflicted| conflicted.hunks.len())
                .with_context(|| format!("\"{path}\" is not a conflicted file of this commit"))?;
            match &resolution {
                HunkResolution::Ours | HunkResolution::Theirs | HunkResolution::Ai => {
                    (1..=total_hunks).collect()
                }
                HunkResolution::Content(_) if total_hunks == 1 => vec![1],
                HunkResolution::Content(_) => bail!(
                    "\"{path}\" has {total_hunks} conflicts; content applies to one at a time, use \"{path}:<N>\"."
                ),
            }
        }
    };
    let specs = hunks
        .into_iter()
        .map(|hunk| ResolutionSpec {
            path: path.to_owned(),
            hunk,
            resolution: resolution.clone(),
        })
        .collect();

    let result = but_api::resolve::resolve_commit_conflict_hunks(ctx, commit_oid, specs)?;

    if let Some(human_out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            human_out,
            "{} {} {} {} {} {}",
            t.success.paint(format!(
                "✓ Resolved {} conflict{} in",
                result.resolved,
                if result.resolved == 1 { "" } else { "s" }
            )),
            t.attention.paint(path),
            t.hint.paint("—"),
            t.commit_id
                .paint(shorten_object_id(&repo, result.commit_id)),
            t.success.paint("→"),
            t.commit_id
                .paint(shorten_object_id(&repo, result.new_commit)),
        )?;
        if result.remaining.is_empty() {
            writeln!(
                human_out,
                "{}",
                t.success
                    .paint("All conflicts in this commit are resolved.")
            )?;
            if result.commit_emptied {
                writeln!(
                    human_out,
                    "{}",
                    t.attention
                        .paint("The commit is now empty — the kept content matches its parent.")
                )?;
            }
            drop(repo);
            let more = oldest_conflicted_commit(ctx)?.is_some();
            if more {
                writeln!(
                    human_out,
                    "{}",
                    t.attention
                        .paint("Other commits are still conflicted — run `but resolve conflicts`.")
                )?;
            }
        } else {
            for remaining in &result.remaining {
                writeln!(
                    human_out,
                    "{}",
                    t.attention.paint(format!(
                        "{} conflict{} remaining in {} — run `but resolve conflicts` to see {}.",
                        remaining.hunks,
                        if remaining.hunks == 1 { "" } else { "s" },
                        remaining.path,
                        if remaining.hunks == 1 { "it" } else { "them" },
                    ))
                )?;
            }
        }
        writeln!(
            human_out,
            "{}",
            t.hint
                .paint("If this isn't right, run `but undo` to revert it.")
        )?;
    }

    if let Some(json_out) = out.for_json() {
        json_out.write_value(serde_json::json!({
            "commit_id": result.commit_id.to_string(),
            "new_commit_id": result.new_commit.to_string(),
            "resolved": result.resolved,
            "commit_emptied": result.commit_emptied,
            "remaining": result.remaining,
        }))?;
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
        let commit_oid = parse_commit_id(ctx, commit_id_str)?;
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
    {
        let repo = ctx.repo.get()?;
        let short_id = shorten_object_id(&repo, commit_oid);
        drop(repo);
        let mut progress = out.progress_channel();
        writeln!(
            progress,
            "{} {}{}",
            t.important.paint("Resolving conflicts in commit"),
            t.commit_id.paint(short_id),
            t.important.paint(" with AI…")
        )?;
    }

    let result =
        but_api::resolve::resolve_commit_conflicts_ai(ctx, commit_oid, but_core::DryRun::No)?;

    if let Some(human_out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            human_out,
            "{} {} {} {}",
            t.success.paint("✓ Resolved"),
            t.commit_id
                .paint(shorten_object_id(&repo, result.commit_id)),
            t.success.paint("→"),
            t.commit_id
                .paint(shorten_object_id(&repo, result.new_commit)),
        )?;
        if let Some(summary) = &result.summary {
            writeln!(human_out)?;
            writeln!(human_out, "{}", sanitize_model_text(summary))?;
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
                t.hint.paint(sanitize_model_text(&file.reasoning))
            )?;
        }
        writeln!(human_out)?;
    }

    Ok(result)
}

/// Strip control characters (including ANSI escape sequences) from
/// model-authored text before printing it to the terminal.
fn sanitize_model_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// Structure to hold information about a conflicted commit
#[derive(Debug)]
struct ConflictedCommit {
    commit_oid: gix::ObjectId,
    commit_short_id: String,
    commit_message: String,
}

/// Check for new conflicts introduced during rebase and report them
fn check_for_new_conflicts_after_rebase(
    ctx: &mut Context,
    out: &mut OutputChannel,
    conflicts_before: BTreeMap<String, Vec<ConflictedCommit>>,
) -> Result<()> {
    let t = theme::get();
    // Get the current list of conflicted commits after the rebase
    let conflicts_after = find_conflicted_commits(ctx)?;

    // Build a set of commit OIDs that were conflicted before
    let mut oids_before = HashSet::new();
    for commits in conflicts_before.values() {
        for commit in commits {
            oids_before.insert(commit.commit_oid);
        }
    }

    // Find newly conflicted commits (present after but not before)
    let mut newly_conflicted: Vec<&ConflictedCommit> = Vec::new();
    for commits in conflicts_after.values() {
        for commit in commits {
            if !oids_before.contains(&commit.commit_oid) {
                newly_conflicted.push(commit);
            }
        }
    }

    // Report newly conflicted commits if any
    if !newly_conflicted.is_empty() {
        if let Some(human_out) = out.for_human() {
            writeln!(human_out)?;
            writeln!(
                human_out,
                "{}",
                t.attention
                    .paint("⚠ Warning: New conflicts were introduced during the rebase:")
            )?;
            writeln!(human_out)?;

            for commit in &newly_conflicted {
                writeln!(
                    human_out,
                    "  {} {} {}",
                    t.sym().dot.error(),
                    t.hint.paint(&commit.commit_short_id),
                    commit.commit_message
                )?;
            }

            writeln!(human_out)?;
            writeln!(
                human_out,
                "Run {} to see all conflicted commits, or {} to resolve them.",
                t.command_suggestion.paint("but status"),
                t.command_suggestion.paint("but resolve <commit>")
            )?;
        } else if let Some(json_out) = out.for_json() {
            let newly_conflicted_json: Vec<serde_json::Value> = newly_conflicted
                .iter()
                .map(|c| {
                    serde_json::json!({
                        "commit_id": c.commit_oid.to_string(),
                        "commit_short_id": c.commit_short_id,
                        "commit_message": c.commit_message,
                    })
                })
                .collect();

            json_out.write_value(serde_json::json!({
                "newly_conflicted_commits": newly_conflicted_json,
                "count": newly_conflicted.len(),
            }))?;
        }
    }

    Ok(())
}

/// Find all conflicted commits across all stacks, grouped by branch
fn find_conflicted_commits(ctx: &mut Context) -> Result<BTreeMap<String, Vec<ConflictedCommit>>> {
    use gix::{prelude::ObjectIdExt as _, revision::walk::Sorting};

    let stacks = crate::legacy::workspace::applied_stacks(ctx)?;
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
                    let message = commit
                        .message_bstr()
                        .to_string()
                        .lines()
                        .next()
                        .context("Commit has no message")?
                        .chars()
                        .take(50)
                        .collect::<String>();

                    let conflicted = ConflictedCommit {
                        commit_oid: oid,
                        commit_short_id: shorten_object_id(&repo, oid),
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
