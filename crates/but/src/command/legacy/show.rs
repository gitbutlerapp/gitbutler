use anyhow::{Result, bail};
use bstr::ByteSlice;
use but_ctx::Context;
use colored::Colorize;

use crate::{
    CLI_DATE, CliId, IdMap,
    utils::{OutputChannel, time::format_relative_time},
};

pub(crate) fn show_commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    commit_id_str: &str,
) -> Result<()> {
    // Try to resolve the commit ID through the IdMap
    let mut id_map = IdMap::new_from_context(ctx, None)?;
    id_map.add_committed_file_info_from_context(ctx)?;

    let cli_ids = id_map.resolve_entity_to_ids(commit_id_str)?;

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
            _ => {
                bail!(
                    "Target must be a commit ID, not {}",
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
        let relative = format_relative_time(commit_time.seconds);
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
