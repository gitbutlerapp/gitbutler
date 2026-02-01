use anyhow::{Result, bail};
use bstr::BString;
use but_api::diff::ComputeLineStats;
use but_ctx::Context;
use gix::prelude::ObjectIdExt;

use crate::{CliId, IdMap, tui, utils::OutputChannel};

pub(crate) fn reword_target(
    ctx: &mut Context,
    out: &mut OutputChannel,
    target: &str,
    message: Option<&str>,
    format: bool,
) -> Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve the commit ID
    let cli_ids = id_map.parse_using_context(target, ctx)?;

    if cli_ids.is_empty() {
        bail!("ID '{}' not found", target);
    }

    if cli_ids.len() > 1 {
        bail!("Target ID '{}' is ambiguous. Found {} matches", target, cli_ids.len());
    }

    let cli_id = &cli_ids[0];

    match cli_id {
        CliId::Branch { name, .. } => {
            if format {
                bail!("--format flag can only be used with commits, not branches");
            }
            edit_branch_name(ctx, name, out, message)?;
        }
        CliId::Commit { commit_id: oid, .. } => {
            edit_commit_message_by_id(ctx, *oid, out, message, format)?;
        }
        _ => {
            bail!("Target must be a commit ID, not {}", cli_id.kind_for_humans());
        }
    }

    Ok(())
}

fn edit_branch_name(
    ctx: &mut Context,
    branch_name: &str,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> Result<()> {
    // Find which stack this branch belongs to
    let stacks = but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;
    for stack_entry in &stacks {
        if stack_entry.heads.iter().all(|b| b.name != branch_name) {
            // Not found in this stack,
            continue;
        }

        if let Some(sid) = stack_entry.id {
            let new_name = prepare_provided_message(message, "branch name")
                .unwrap_or_else(|| get_branch_name_from_editor(branch_name))?;
            but_api::legacy::stack::update_branch_name(ctx, sid, branch_name.to_owned(), new_name.clone())?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Renamed branch '{}' to '{}'", branch_name, new_name)?;
            }
            return Ok(());
        }
    }

    bail!("Branch '{}' not found in any stack", branch_name)
}

fn prepare_provided_message(msg: Option<&str>, entity: &str) -> Option<Result<String>> {
    msg.map(|msg| {
        let trimmed = msg.trim();
        if trimmed.is_empty() {
            bail!("Aborting due to empty {entity}");
        }
        Ok(trimmed.to_string())
    })
}

fn edit_commit_message_by_id(
    ctx: &mut Context,
    commit_oid: gix::ObjectId,
    out: &mut OutputChannel,
    message: Option<&str>,
    format: bool,
) -> Result<()> {
    // Get commit details directly - no need to iterate through stacks
    let commit_details = but_api::diff::commit_details(ctx, commit_oid, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();

    // Get new message from provided argument, format flag, or editor
    let new_message = if format {
        if message.is_some() {
            bail!("Cannot use both --format and --message flags together");
        }
        // Format the current message without opening an editor
        but_action::commit_format::format_commit_message(&current_message)
    } else {
        prepare_provided_message(message, "commit message").unwrap_or_else(|| {
            let changed_files = get_changed_files_from_commit_details(&commit_details);

            // Open editor with current message and file list
            get_commit_message_from_editor(&current_message, &changed_files)
        })?
    };

    if new_message.trim() == current_message.trim() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No changes to commit message - nothing to be done")?;
        }
        return Ok(());
    }

    let new_commit_oid = but_api::commit::commit_reword_only(ctx, commit_oid, BString::from(new_message))?;

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            out,
            "Updated commit message for {} (now {})",
            commit_oid.attach(&repo).shorten_or_id(),
            new_commit_oid.attach(&repo).shorten_or_id()
        )?;
    }

    Ok(())
}

fn get_changed_files_from_commit_details(commit_details: &but_core::diff::CommitDetails) -> Vec<String> {
    let mut files = Vec::new();

    for change in &commit_details.diff_with_first_parent {
        let status = match &change.status {
            but_core::TreeStatus::Addition { .. } => "new file:",
            but_core::TreeStatus::Deletion { .. } => "deleted:",
            but_core::TreeStatus::Modification { .. } => "modified:",
            but_core::TreeStatus::Rename { .. } => "modified:",
        };

        let file_path = change.path.to_string();
        files.push(format!("{status}   {file_path}"));
    }

    files.sort();
    files
}

fn get_commit_message_from_editor(current_message: &str, changed_files: &[String]) -> Result<String> {
    // Generate commit message template with current message
    let mut template = String::new();
    template.push_str(current_message);
    if !current_message.is_empty() && !current_message.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the commit message for your changes. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty message aborts the commit.\n");
    template.push_str("#\n");
    template.push_str("# Changes in this commit:\n");

    for file in changed_files {
        template.push_str(&format!("#\t{file}\n"));
    }
    template.push_str("#\n");

    // Read the result and strip comments
    let lossy_message = tui::get_text::from_editor_no_comments("commit_msg", &template)?.to_string();

    if lossy_message.is_empty() {
        bail!("Aborting due to empty commit message");
    }

    Ok(lossy_message)
}

fn get_branch_name_from_editor(current_name: &str) -> Result<String> {
    let mut template = String::new();
    template.push_str(current_name);
    if !current_name.is_empty() && !current_name.ends_with('\n') {
        template.push('\n');
    }
    template.push_str("\n# Please enter the new branch name. Lines starting\n");
    template.push_str("# with '#' will be ignored, and an empty name aborts the operation.\n");
    template.push_str("#\n");

    let branch_name_lossy = tui::get_text::from_editor_no_comments("branch_name", &template)?.to_string();

    if branch_name_lossy.is_empty() {
        bail!("Aborting due to empty branch name");
    }

    Ok(branch_name_lossy)
}

#[cfg(test)]
mod tests {

    mod prepare_provided_message {
        use super::super::*;

        #[test]
        fn empty_is_fails() {
            assert_eq!(
                prepare_provided_message(Some(""), "message")
                    .unwrap()
                    .unwrap_err()
                    .to_string(),
                "Aborting due to empty message"
            );
        }

        #[test]
        fn empty_is_after_trimming_fails() {
            assert_eq!(
                prepare_provided_message(Some("    "), "message")
                    .unwrap()
                    .unwrap_err()
                    .to_string(),
                "Aborting due to empty message"
            );
        }
    }
}
