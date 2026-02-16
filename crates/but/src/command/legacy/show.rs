use anyhow::{Result, bail};
use bstr::ByteSlice;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt, OidExt};
use colored::Colorize;

use crate::{
    CLI_DATE, CliId, IdMap,
    utils::{OutputChannel, time::format_relative_time},
};

pub(crate) fn show_commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_id_str: &str,
    verbose: bool,
) -> Result<()> {
    // First check if this is a branch by trying to find it in the branch list
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    let branch_match = branches.iter().find(|b| {
        b.name.to_string() == commit_id_str || b.name.to_string().to_lowercase() == commit_id_str.to_lowercase()
    });

    if let Some(branch) = branch_match {
        // This is a branch, display branch name and list of commits
        return show_branch(ctx, out, &branch.name.to_string(), verbose);
    }

    // Also check stacks to find branches within stacks
    let stacks = but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;

    for stack in &stacks {
        for head in &stack.heads {
            let head_name = head.name.to_str_lossy().to_string();
            if head_name == commit_id_str || head_name.to_lowercase() == commit_id_str.to_lowercase() {
                // Found the branch in a stack
                return show_branch(ctx, out, &head_name, verbose);
            }
        }
    }

    // Not a branch, proceed with commit logic
    // Try to resolve the commit ID through the IdMap
    let id_map = IdMap::new_from_context(ctx, None)?;

    let cli_ids = id_map.parse_using_context(commit_id_str, ctx)?;

    let commit_id = if cli_ids.is_empty() {
        // If not found in IdMap, try to parse as a git commit ID directly
        let repo = ctx.repo.get()?;
        let obj = repo
            .rev_parse_single(commit_id_str)
            .map_err(|_| anyhow::anyhow!("Commit '{}' not found", commit_id_str))?;
        let commit = obj
            .object()?
            .try_into_commit()
            .map_err(|_| anyhow::anyhow!("'{}' is not a commit", commit_id_str))?;
        commit.id
    } else if cli_ids.len() > 1 {
        bail!(
            "Commit ID '{}' is ambiguous. Found {} matches",
            commit_id_str,
            cli_ids.len()
        );
    } else {
        match &cli_ids[0] {
            CliId::Commit { commit_id, .. } => *commit_id,
            CliId::Branch { name, .. } => {
                // This is a branch identified by CLI ID, show the branch
                return show_branch(ctx, out, &name.to_string(), verbose);
            }
            _ => {
                bail!(
                    "Target must be a commit ID or branch, not {}",
                    cli_ids[0].kind_for_humans()
                );
            }
        }
    };

    // Get commit and file details
    let repo = ctx.repo.get()?;
    let raw_commit = repo.find_commit(commit_id)?;
    let decoded = raw_commit.decode()?;

    // Get diff with first parent
    let parent_id = raw_commit.parent_ids().next().map(|id| id.detach());
    let tree_changes = but_core::diff::TreeChanges::from_trees(&repo, parent_id, commit_id)?;

    // Extract change-id if present (try both header names)
    let change_id = decoded
        .extra_headers()
        .find("change-id")
        .or_else(|| decoded.extra_headers().find("gitbutler-change-id"))
        .map(|v| v.to_str_lossy().to_string());

    // Display commit information
    if let Some(out) = out.for_human() {
        // Commit SHA - full hash
        writeln!(
            out,
            "{} {}",
            "Commit:   ".yellow().bold(),
            commit_id.to_string().yellow()
        )?;

        // Change ID (if present)
        if let Some(ref change_id) = change_id {
            writeln!(out, "{} {}", "Change-ID:".bold(), change_id)?;
        }

        // Author
        let author_sig = decoded.author()?;
        writeln!(
            out,
            "{} {} <{}>",
            "Author:   ".bold(),
            author_sig.name.to_str_lossy().cyan(),
            author_sig.email.to_str_lossy().cyan()
        )?;

        // Date with relative time
        let commit_time = raw_commit.time()?;
        let date_str = commit_time.format(CLI_DATE)?;
        let relative = format_relative_time(std::time::SystemTime::now(), commit_time.seconds);
        writeln!(
            out,
            "{}  {} {}",
            "Date:    ".bold(),
            date_str.green(),
            format!("({})", relative).dimmed()
        )?;

        // Committer (only if different from author)
        let committer_sig = decoded.committer()?;
        if committer_sig.name != author_sig.name || committer_sig.email != author_sig.email {
            writeln!(
                out,
                "{} {} <{}>",
                "Committer:".bold(),
                committer_sig.name.to_str_lossy().cyan(),
                committer_sig.email.to_str_lossy().cyan()
            )?;
        }

        writeln!(out)?;

        // Commit message - first line bold, rest normal, no indentation
        let message = decoded.message.to_str_lossy();
        let mut lines = message.lines();
        if let Some(first_line) = lines.next() {
            writeln!(out, "{}", first_line.bold())?;
            // Print remaining lines without indentation
            for line in lines {
                writeln!(out, "{}", line)?;
            }
        }

        writeln!(out)?;

        // File list
        let changes = tree_changes.into_tree_changes();
        if !changes.is_empty() {
            writeln!(out, "{}", "Files changed:".bold())?;
            for change in &changes {
                let (status_char, status_color) = match &change.status {
                    but_core::TreeStatus::Addition { .. } => ("A", "green"),
                    but_core::TreeStatus::Deletion { .. } => ("D", "red"),
                    but_core::TreeStatus::Modification { .. } => ("M", "yellow"),
                    but_core::TreeStatus::Rename { .. } => ("R", "cyan"),
                };

                writeln!(
                    out,
                    "  {} {}",
                    status_char.color(status_color),
                    change.path.to_str_lossy()
                )?;
            }
        }
    } else if let Some(out) = out.for_json() {
        // JSON output
        let changes = tree_changes.into_tree_changes();
        let mut files = Vec::new();
        for change in &changes {
            let status = match &change.status {
                but_core::TreeStatus::Addition { .. } => "added",
                but_core::TreeStatus::Deletion { .. } => "deleted",
                but_core::TreeStatus::Modification { .. } => "modified",
                but_core::TreeStatus::Rename { .. } => "renamed",
            };

            files.push(serde_json::json!({
                "path": change.path.to_str_lossy(),
                "status": status
            }));
        }

        let author_sig = decoded.author()?;
        let committer_sig = decoded.committer()?;
        let date_str = raw_commit.time()?.format(CLI_DATE)?;

        let mut json_output = serde_json::json!({
            "commit": commit_id.to_string(),
            "author": {
                "name": author_sig.name.to_str_lossy(),
                "email": author_sig.email.to_str_lossy()
            },
            "committer": {
                "name": committer_sig.name.to_str_lossy(),
                "email": committer_sig.email.to_str_lossy()
            },
            "date": date_str,
            "message": decoded.message.to_str_lossy(),
            "files": files
        });

        // Add change-id if present
        if let Some(ref change_id) = change_id {
            json_output["changeId"] = serde_json::json!(change_id);
        }

        out.write_value(json_output)?;
    }

    Ok(())
}

fn show_branch(ctx: &mut Context, out: &mut OutputChannel, branch_name: &str, verbose: bool) -> Result<()> {
    // Get the commits for the branch
    let (commits, base_commit_info) = get_branch_commits(ctx, branch_name, verbose)?;

    // Get the stack chain (branches this branch is stacked on)
    let stack_chain = get_stack_chain(ctx, branch_name)?;

    // Display branch information
    if let Some(out) = out.for_human() {
        writeln!(out, "{} {}", "Branch:".yellow().bold(), branch_name.green())?;
        writeln!(out)?;

        if commits.is_empty() {
            writeln!(out, "No commits on this branch.")?;
        } else {
            writeln!(out, "{}", "Commits:".bold())?;
            for (idx, commit) in commits.iter().enumerate() {
                if verbose {
                    let now_t = std::time::SystemTime::now();
                    // Verbose mode: show full commit message and files with visual separators
                    writeln!(
                        out,
                        "{} {} {}",
                        "●".cyan(),
                        commit.short_sha.blue(),
                        commit.message.blue().bold()
                    )?;
                    writeln!(
                        out,
                        "{} {}, {} by {}",
                        "│".dimmed(),
                        format_timestamp(commit.timestamp).dimmed(),
                        format_relative_time(now_t, commit.timestamp).dimmed(),
                        commit.author_name.dimmed()
                    )?;

                    // Show full message if available
                    if let Some(full_message) = &commit.full_message {
                        let lines: Vec<&str> = full_message.lines().collect();
                        if lines.len() > 1 {
                            for line in lines.iter().skip(1) {
                                if !line.is_empty() {
                                    writeln!(out, "{} {}", "│".dimmed(), line)?;
                                } else {
                                    writeln!(out, "{}", "│".dimmed())?;
                                }
                            }
                        }
                    }

                    // Show file changes
                    if let Some(files) = &commit.files
                        && !files.is_empty()
                    {
                        writeln!(out, "{}", "│".dimmed())?;
                        writeln!(out, "{} {}:", "│".dimmed(), "Files changed".dimmed())?;
                        for file in files {
                            let status_str = match file.status.as_str() {
                                "added" => format!("A {}", file.path.green()),
                                "deleted" => format!("D {}", file.path.red()),
                                "modified" => format!("M {}", file.path.yellow()),
                                _ => format!("  {}", file.path),
                            };
                            writeln!(
                                out,
                                "{} {}  ({}, {})",
                                "│".dimmed(),
                                status_str,
                                format!("+{}", file.insertions).green(),
                                format!("-{}", file.deletions).red()
                            )?;
                        }
                    }

                    if let (Some(files_changed), Some(insertions), Some(deletions)) =
                        (commit.files_changed, commit.insertions, commit.deletions)
                    {
                        writeln!(out, "{}", "│".dimmed())?;
                        writeln!(
                            out,
                            "{} {} file{} changed, {} insertion{}, {} deletion{}",
                            "│".dimmed(),
                            files_changed,
                            if files_changed == 1 { "" } else { "s" },
                            insertions,
                            if insertions == 1 { "" } else { "s" },
                            deletions,
                            if deletions == 1 { "" } else { "s" }
                        )?;
                    }

                    // Add vertical line separator between commits (but not after the last one)
                    if idx < commits.len() - 1 {
                        writeln!(out, "{}", "│".dimmed())?;
                    }
                } else {
                    // Normal mode: compact display
                    writeln!(out, "  {} {}", commit.short_sha.yellow(), commit.message)?;
                    writeln!(
                        out,
                        "    {} by {}",
                        format_timestamp(commit.timestamp).dimmed(),
                        commit.author_name.dimmed()
                    )?;
                }
            }

            // Show base commit in verbose mode
            if verbose {
                if let Some(base_commit) = &base_commit_info {
                    let now_t = std::time::SystemTime::now();
                    writeln!(out, "{}", "│".dimmed())?;
                    writeln!(
                        out,
                        "{} {} {} {}",
                        "┴".dimmed(),
                        base_commit.short_sha.yellow(),
                        base_commit.message.dimmed(),
                        "(base)".dimmed()
                    )?;
                    writeln!(
                        out,
                        "  {}, {} by {}",
                        format_timestamp(base_commit.timestamp).dimmed(),
                        format_relative_time(now_t, base_commit.timestamp).dimmed(),
                        base_commit.author_name.dimmed()
                    )?;
                }

                // Show summary
                show_branch_summary(out, &commits)?;
            }
        }

        // Display stack chain if this branch is stacked on others
        if !stack_chain.is_empty() {
            writeln!(out)?;
            writeln!(out, "{}", "Stacked on:".bold())?;
            for (i, chain_branch) in stack_chain.iter().enumerate() {
                let connector = if i == stack_chain.len() - 1 { "└─" } else { "├─" };
                writeln!(
                    out,
                    "  {} {} ({})",
                    connector,
                    chain_branch.name.cyan(),
                    format!(
                        "{} commit{}",
                        chain_branch.commit_count,
                        if chain_branch.commit_count == 1 { "" } else { "s" }
                    )
                    .dimmed()
                )?;
            }
        }
    } else if let Some(out) = out.for_json() {
        let json_output = serde_json::json!({
            "branch": branch_name,
            "commits": commits,
            "stackedOn": stack_chain,
            "baseCommit": base_commit_info,
        });
        out.write_value(json_output)?;
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
struct BranchCommitInfo {
    sha: String,
    short_sha: String,
    message: String,
    full_message: Option<String>,
    author_name: String,
    author_email: String,
    timestamp: i64,
    files_changed: Option<usize>,
    insertions: Option<usize>,
    deletions: Option<usize>,
    files: Option<Vec<FileChange>>,
}

#[derive(Debug, serde::Serialize)]
struct FileChange {
    path: String,
    status: String,
    insertions: usize,
    deletions: usize,
}

#[derive(Debug, serde::Serialize)]
struct StackChainBranch {
    name: String,
    commit_count: usize,
}

/// Helper function to find a branch OID by name, checking both list_branches and stacks
fn find_branch_oid(ctx: &Context, branch_name: &str) -> Result<git2::Oid> {
    // First check list_branches
    let branches = but_api::legacy::virtual_branches::list_branches(ctx, None)?;
    if let Some(branch) = branches.iter().find(|b| b.name.to_string() == branch_name) {
        return Ok(branch.head);
    }

    // Not found in list_branches, check stacks
    let stacks = but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;

    for stack in &stacks {
        for head in &stack.heads {
            if head.name.to_str_lossy() == branch_name {
                return Ok(git2::Oid::from_bytes(head.tip.as_slice())?);
            }
        }
    }

    anyhow::bail!("Branch '{}' not found", branch_name)
}

fn get_branch_commits(
    ctx: &Context,
    branch_name: &str,
    verbose: bool,
) -> Result<(Vec<BranchCommitInfo>, Option<BranchCommitInfo>)> {
    use gix::prelude::ObjectIdExt as _;

    let repo = &*ctx.git2_repo.get()?;
    let gix_repo = ctx.repo.get()?;

    // Get the target from workspace
    let guard = ctx.shared_worktree_access();
    let (_, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    // Find the branch by name
    let branch_oid = find_branch_oid(ctx, branch_name)?;
    let branch_commit = repo.find_commit(branch_oid)?;

    // Find merge base
    let Some(merge_base) = ws
        .merge_base_with_target_branch(branch_commit.id().to_gix())
        .map(|t| t.0)
    else {
        tracing::warn!(
            branch_name,
            "Could not find merge base with target branch, which is unexpected"
        );
        return Ok((Vec::new(), None));
    };

    // Walk from branch head to merge base, collecting commits
    let branch_gix_oid = branch_commit.id().to_gix();
    let traversal = branch_gix_oid
        .attach(&gix_repo)
        .ancestors()
        .with_hidden(Some(merge_base))
        .all()?;

    let mut commits = Vec::new();
    for info in traversal {
        let info = info?;
        let oid = info.id.to_git2();
        let commit = repo.find_commit(oid)?;
        let author = commit.author();

        let full_message = commit.message().map(|m| m.to_string());
        let message = full_message
            .as_deref()
            .unwrap_or("(no message)")
            .lines()
            .next()
            .unwrap_or("(no message)")
            .to_string();

        let (files_changed, insertions, deletions, files) = if verbose {
            // Calculate diff stats and collect file information
            let tree = commit.tree()?;
            let parent_tree = if commit.parent_count() > 0 {
                Some(commit.parent(0)?.tree()?)
            } else {
                None
            };
            let diff = repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), None)?;
            let stats = diff.stats()?;

            let mut file_changes = Vec::new();
            for delta_idx in 0..diff.deltas().len() {
                let delta = diff.get_delta(delta_idx).unwrap();

                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .unwrap_or("(unknown)")
                    .to_string();

                let status = match delta.status() {
                    git2::Delta::Added => "added",
                    git2::Delta::Deleted => "deleted",
                    git2::Delta::Modified => "modified",
                    git2::Delta::Renamed => "renamed",
                    git2::Delta::Copied => "copied",
                    git2::Delta::Typechange => "typechange",
                    _ => "unknown",
                };

                // Get patch for this specific file to count lines
                let patch = git2::Patch::from_diff(&diff, delta_idx)?;
                let mut insertions = 0;
                let mut deletions = 0;

                if let Some(patch) = patch {
                    for hunk_idx in 0..patch.num_hunks() {
                        let hunk_lines = patch.num_lines_in_hunk(hunk_idx)?;
                        for line_idx in 0..hunk_lines {
                            let line = patch.line_in_hunk(hunk_idx, line_idx)?;
                            match line.origin() {
                                '+' => insertions += 1,
                                '-' => deletions += 1,
                                _ => {}
                            }
                        }
                    }
                }

                file_changes.push(FileChange {
                    path,
                    status: status.to_string(),
                    insertions,
                    deletions,
                });
            }

            (
                Some(stats.files_changed()),
                Some(stats.insertions()),
                Some(stats.deletions()),
                Some(file_changes),
            )
        } else {
            (None, None, None, None)
        };

        commits.push(BranchCommitInfo {
            sha: oid.to_string(),
            short_sha: oid.to_string()[..7].to_string(),
            message,
            full_message,
            author_name: author.name().unwrap_or("Unknown").to_string(),
            author_email: author.email().unwrap_or("").to_string(),
            timestamp: commit.time().seconds(),
            files_changed,
            insertions,
            deletions,
            files,
        });
    }

    // Get base commit info
    let base_commit_info = if verbose {
        let merge_base_git2 = merge_base.to_git2();
        let base_commit = repo.find_commit(merge_base_git2)?;
        let base_author = base_commit.author();
        let base_message = base_commit
            .message()
            .unwrap_or("(no message)")
            .lines()
            .next()
            .unwrap_or("(no message)")
            .to_string();

        Some(BranchCommitInfo {
            sha: merge_base_git2.to_string(),
            short_sha: merge_base_git2.to_string()[..7].to_string(),
            message: base_message,
            full_message: None,
            author_name: base_author.name().unwrap_or("Unknown").to_string(),
            author_email: base_author.email().unwrap_or("").to_string(),
            timestamp: base_commit.time().seconds(),
            files_changed: None,
            insertions: None,
            deletions: None,
            files: None,
        })
    } else {
        None
    };

    Ok((commits, base_commit_info))
}

fn format_timestamp(timestamp: i64) -> String {
    use chrono::{DateTime, Local, Utc};

    let datetime = DateTime::<Utc>::from_timestamp(timestamp, 0)
        .unwrap_or_else(Utc::now)
        .with_timezone(&Local);

    datetime.format("%Y-%m-%d %H:%M:%S").to_string()
}

fn show_branch_summary(out: &mut dyn std::fmt::Write, commits: &[BranchCommitInfo]) -> Result<()> {
    use std::collections::HashMap;

    writeln!(out)?;
    writeln!(out, "{}", "─".repeat(50).dimmed())?;
    writeln!(out)?;
    writeln!(out, "{}", "Branch Summary:".green().bold())?;

    // Count total commits
    writeln!(
        out,
        "  {} commit{}",
        commits.len(),
        if commits.len() == 1 { "" } else { "s" }
    )?;

    // Aggregate file changes
    let mut file_changes: HashMap<String, (usize, usize)> = HashMap::new();
    let mut total_insertions = 0;
    let mut total_deletions = 0;

    for commit in commits {
        if let Some(files) = &commit.files {
            for file in files {
                let entry = file_changes.entry(file.path.clone()).or_insert((0, 0));
                entry.0 += file.insertions;
                entry.1 += file.deletions;
                total_insertions += file.insertions;
                total_deletions += file.deletions;
            }
        }
    }

    if !file_changes.is_empty() {
        writeln!(
            out,
            "  {} file{} changed",
            file_changes.len(),
            if file_changes.len() == 1 { "" } else { "s" }
        )?;
        writeln!(
            out,
            "  {} insertion{} (+)",
            total_insertions,
            if total_insertions == 1 { "" } else { "s" }
        )?;
        writeln!(
            out,
            "  {} deletion{} (-)",
            total_deletions,
            if total_deletions == 1 { "" } else { "s" }
        )?;

        // Show top changed files
        let mut files_vec: Vec<_> = file_changes.iter().collect();
        files_vec.sort_by(|a, b| (b.1.0 + b.1.1).cmp(&(a.1.0 + a.1.1)));

        writeln!(out)?;
        writeln!(out, "  {}:", "Files changed".dimmed())?;
        for (path, (insertions, deletions)) in files_vec.iter().take(10) {
            writeln!(
                out,
                "    {} ({}, {})",
                path,
                format!("+{}", insertions).green(),
                format!("-{}", deletions).red()
            )?;
        }

        if files_vec.len() > 10 {
            writeln!(
                out,
                "    ... and {} more file{}",
                files_vec.len() - 10,
                if files_vec.len() - 10 == 1 { "" } else { "s" }
            )?;
        }
    }

    Ok(())
}

fn get_stack_chain(ctx: &Context, branch_name: &str) -> Result<Vec<StackChainBranch>> {
    // Get all stacks
    let stacks = but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;

    // Find the stack containing this branch
    let stack = stacks
        .iter()
        .find(|s| s.heads.iter().any(|h| h.name.to_str_lossy() == branch_name));

    let Some(stack) = stack else {
        // Branch not in a stack or is the only branch
        return Ok(vec![]);
    };

    // Find the position of this branch in the stack
    let branch_index = stack.heads.iter().position(|h| h.name.to_str_lossy() == branch_name);

    let Some(branch_index) = branch_index else {
        return Ok(vec![]);
    };

    // If this is the first branch (index 0), it's not stacked on anything
    if branch_index == 0 {
        return Ok(vec![]);
    }

    // Get all branches this branch is stacked on (branches with index < branch_index)
    let mut chain = Vec::new();

    // Iterate from the branch above this one (branch_index - 1) down to 0
    for i in (0..branch_index).rev() {
        let head = &stack.heads[i];
        let head_name = head.name.to_str_lossy().to_string();

        // Count commits for this branch
        let commit_count = get_branch_commit_count(ctx, &head_name)?;

        chain.push(StackChainBranch {
            name: head_name,
            commit_count,
        });
    }

    Ok(chain)
}

fn get_branch_commit_count(ctx: &Context, branch_name: &str) -> Result<usize> {
    use gix::prelude::ObjectIdExt as _;

    let repo = &*ctx.git2_repo.get()?;
    let gix_repo = ctx.repo.get()?;

    // Get the target from workspace
    let guard = ctx.shared_worktree_access();
    let (_, ws, _) = ctx.workspace_and_db_with_perm(guard.read_permission())?;

    // Find the branch OID
    let branch_oid = find_branch_oid(ctx, branch_name)?;
    let branch_commit = repo.find_commit(branch_oid)?;
    let Some(merge_base) = ws
        .merge_base_with_target_branch(branch_commit.id().to_gix())
        .map(|t| t.0)
    else {
        tracing::warn!(
            branch_name,
            "Could not find merge base with target branch, which is unexpected"
        );
        return Ok(0);
    };

    // Count commits
    let branch_gix_oid = branch_commit.id().to_gix();
    let traversal = branch_gix_oid
        .attach(&gix_repo)
        .ancestors()
        .with_hidden(Some(merge_base))
        .all()?;

    Ok(traversal.filter_map(Result::ok).count())
}
