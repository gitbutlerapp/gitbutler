use anyhow::{Result, bail};
use but_api::diff::ComputeLineStats;
use but_ctx::Context;
use but_oxidize::{ObjectIdExt as _, OidExt};
use gitbutler_project::Project;
use gix::prelude::ObjectIdExt;

use crate::{
    legacy::id::{CliId, IdMap},
    tui,
    utils::OutputChannel,
};

pub(crate) fn describe_target(
    project: &Project,
    out: &mut OutputChannel,
    target: &str,
    message: Option<&str>,
) -> Result<()> {
    let mut ctx = Context::new_from_legacy_project(project.clone())?;
    let mut id_map = IdMap::new_from_context(&ctx)?;
    id_map.add_file_info_from_context(&mut ctx)?;

    // Resolve the commit ID
    let cli_ids = id_map.parse_str(target)?;

    if cli_ids.is_empty() {
        bail!("ID '{}' not found", target);
    }

    if cli_ids.len() > 1 {
        bail!(
            "Target ID '{}' is ambiguous. Found {} matches",
            target,
            cli_ids.len()
        );
    }

    let cli_id = &cli_ids[0];

    match cli_id {
        CliId::Branch { name, .. } => {
            edit_branch_name(&ctx, project, name, out, message)?;
        }
        CliId::Commit { oid } => {
            edit_commit_message_by_id(&ctx, project, *oid, out, message)?;
        }
        _ => {
            bail!(
                "Target must be a commit ID, not {}",
                cli_id.kind_for_humans()
            );
        }
    }

    Ok(())
}

fn edit_branch_name(
    _ctx: &Context,
    project: &Project,
    branch_name: &str,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> Result<()> {
    // Find which stack this branch belongs to
    let stacks = but_api::legacy::workspace::stacks(
        project.id,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;
    for stack_entry in &stacks {
        if stack_entry.heads.iter().all(|b| b.name != branch_name) {
            // Not found in this stack,
            continue;
        }

        if let Some(sid) = stack_entry.id {
            let new_name = prepare_provided_message(message, "branch name")
                .unwrap_or_else(|| get_branch_name_from_editor(branch_name))?;
            but_api::legacy::stack::update_branch_name(
                project.id,
                sid,
                branch_name.to_owned(),
                new_name.clone(),
            )?;
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
    ctx: &Context,
    project: &Project,
    commit_oid: gix::ObjectId,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> Result<()> {
    // Find which stack this commit belongs to
    let stacks = but_api::legacy::workspace::stacks(project.id, None)?;
    let mut found_commit_message = None;
    let mut stack_id = None;

    for stack_entry in &stacks {
        if let Some(sid) = stack_entry.id {
            let stack_details = but_api::legacy::workspace::stack_details(project.id, Some(sid))?;

            // Check if this commit exists in any branch of this stack
            for branch_details in &stack_details.branch_details {
                // Check local commits
                for commit in &branch_details.commits {
                    if commit.id == commit_oid {
                        found_commit_message = Some(commit.message.clone());
                        stack_id = Some(sid);
                        break;
                    }
                }

                // Also check upstream commits
                if found_commit_message.is_none() {
                    for commit in &branch_details.upstream_commits {
                        if commit.id == commit_oid {
                            found_commit_message = Some(commit.message.clone());
                            stack_id = Some(sid);
                            break;
                        }
                    }
                }

                if found_commit_message.is_some() {
                    break;
                }
            }
            if found_commit_message.is_some() {
                break;
            }
        }
    }

    let commit_message = found_commit_message
        .ok_or_else(|| anyhow::anyhow!("Commit {} not found in any stack", commit_oid))?;

    let stack_id = stack_id
        .ok_or_else(|| anyhow::anyhow!("Could not find stack for commit {}", commit_oid))?;

    // Get current commit message
    let current_message = commit_message.to_string();

    // Get new message from provided argument or editor
    let new_message = prepare_provided_message(message, "commit message").unwrap_or_else(|| {
        let commit_details = but_api::diff::commit_details(ctx, commit_oid, ComputeLineStats::No)?;
        let changed_files = get_changed_files_from_commit_details(&commit_details);

        // Open editor with current message and file list
        get_commit_message_from_editor(&current_message, &changed_files)
    })?;

    if new_message.trim() == current_message.trim() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No changes to commit message - nothing to be done")?;
        }
        return Ok(());
    }

    // Use gitbutler_branch_actions::update_commit_message instead of low-level primitives
    let git2_commit_oid = commit_oid.to_git2();
    let new_commit_oid = gitbutler_branch_actions::update_commit_message(
        ctx,
        stack_id,
        git2_commit_oid,
        &new_message,
    )?;

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            out,
            "Updated commit message for {} (now {})",
            commit_oid.attach(&repo).shorten_or_id(),
            new_commit_oid.to_gix().attach(&repo).shorten_or_id()
        )?;
    }

    Ok(())
}

fn get_changed_files_from_commit_details(
    commit_details: &but_core::diff::CommitDetails,
) -> Vec<String> {
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

fn get_commit_message_from_editor(
    current_message: &str,
    changed_files: &[String],
) -> Result<String> {
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
    let message = tui::get_text::from_editor_no_comments("but_commit_msg", &template)?;

    if message.is_empty() {
        bail!("Aborting due to empty commit message");
    }

    Ok(message)
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

    let branch_name = tui::get_text::from_editor_no_comments("but_branch_name", &template)?;

    if branch_name.is_empty() {
        bail!("Aborting due to empty branch name");
    }

    Ok(branch_name)
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
