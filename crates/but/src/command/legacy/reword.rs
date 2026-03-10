use anyhow::{Result, bail};
use bstr::{BString, ByteSlice};
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
    show_diff_in_editor: ShowDiffInEditor,
) -> Result<()> {
    let id_map = IdMap::new_from_context(ctx, None)?;

    // Resolve the commit ID
    let cli_ids = id_map.parse_using_context(target, ctx)?;

    if cli_ids.is_empty() {
        bail!("ID '{target}' not found");
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
            if format {
                bail!("--format flag can only be used with commits, not branches");
            }
            edit_branch_name(ctx, name, out, message)?;
        }
        CliId::Commit { commit_id: oid, .. } => {
            edit_commit_message_by_id(ctx, *oid, out, message, format, show_diff_in_editor)?;
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(crate) enum ShowDiffInEditor {
    /// The user requested we always show the diff.
    Always,
    /// The user requested we never show the diff.
    Never,
    /// The user didn't specify a preference.
    Unspecified,
}

impl ShowDiffInEditor {
    pub(crate) fn from_args(diff: bool, no_diff: bool) -> Option<Self> {
        match (diff, no_diff) {
            (true, true) => None,
            (true, false) => Some(Self::Always),
            (false, true) => Some(Self::Never),
            (false, false) => Some(Self::Unspecified),
        }
    }
}

fn edit_branch_name(
    ctx: &mut Context,
    branch_name: &str,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> Result<()> {
    // Find which stack this branch belongs to
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
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
                ctx,
                sid,
                branch_name.to_owned(),
                new_name.clone(),
            )?;
            if let Some(out) = out.for_human() {
                writeln!(out, "Renamed branch '{branch_name}' to '{new_name}'")?;
            }
            return Ok(());
        }
    }

    bail!("Branch '{branch_name}' not found in any stack")
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

/// The maximum total blob size (in bytes) for which we'll show the diff in the editor
/// when the user hasn't specified a preference. This uses object header lookups
/// which are cheap compared to actually computing diffs.
///
/// 900KB is very roughly 15,000 lines at ~60 bytes per line. Just to protect the user from
/// stalling their system if they accidentally commit a big log file.
const MAX_DIFF_BLOB_SIZE_FOR_EDITOR_IF_UNSPECIFIED: u64 = 900_000;

fn edit_commit_message_by_id(
    ctx: &mut Context,
    commit_oid: gix::ObjectId,
    out: &mut OutputChannel,
    message: Option<&str>,
    format: bool,
    show_diff_in_editor: ShowDiffInEditor,
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

            let should_show_diff = match show_diff_in_editor {
                ShowDiffInEditor::Always => true,
                ShowDiffInEditor::Never => false,
                ShowDiffInEditor::Unspecified => {
                    let total_blob_size =
                        estimate_diff_blob_size(&commit_details.diff_with_first_parent, ctx)?;
                    total_blob_size <= MAX_DIFF_BLOB_SIZE_FOR_EDITOR_IF_UNSPECIFIED
                }
            };
            let diff = should_show_diff
                .then(|| {
                    commit_details
                        .diff_with_first_parent
                        .iter()
                        .map(|change| change.unified_diff(&*ctx.repo.get()?, 3))
                        .filter_map(|diff| diff.transpose())
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?;

            // Open editor with current message and file list
            get_commit_message_from_editor(&current_message, &changed_files, diff.as_deref())
        })?
    };

    if new_message.trim() == current_message.trim() {
        if let Some(out) = out.for_human() {
            writeln!(out, "No changes to commit message - nothing to be done")?;
        }
        return Ok(());
    }

    let new_commit_oid =
        but_api::commit::commit_reword_only(ctx, commit_oid, BString::from(new_message))?;

    if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        writeln!(
            out,
            "Updated commit message for {} (now {})",
            commit_oid.attach(&repo).shorten_or_id(),
            new_commit_oid.new_commit.attach(&repo).shorten_or_id()
        )?;
    }

    Ok(())
}

/// Sum the blob sizes involved in the given tree changes using cheap object header lookups.
/// For modifications/renames, uses the larger of the two sides as an upper bound.
fn estimate_diff_blob_size(changes: &[but_core::TreeChange], ctx: &mut Context) -> Result<u64> {
    fn blob_size(repo: &gix::Repository, id: &gix::ObjectId) -> u64 {
        repo.find_header(*id).map(|h| h.size()).unwrap_or(0)
    }

    let repo = ctx.repo.get()?;

    Ok(changes
        .iter()
        .map(|change| match &change.status {
            but_core::TreeStatus::Addition { state, .. } => blob_size(&repo, &state.id),
            but_core::TreeStatus::Deletion { previous_state } => {
                blob_size(&repo, &previous_state.id)
            }
            but_core::TreeStatus::Modification {
                previous_state,
                state,
                ..
            }
            | but_core::TreeStatus::Rename {
                previous_state,
                state,
                ..
            } => {
                let a = blob_size(&repo, &previous_state.id);
                let b = blob_size(&repo, &state.id);
                a.max(b)
            }
        })
        .fold(0, |a, b| a.saturating_add(b)))
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
    diff: Option<&[BString]>,
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

    let mut template_rest = String::new();
    if let Some(diff) = diff
        && !diff.is_empty()
    {
        for line in diff {
            template_rest.push_str(&line.to_str_lossy());
        }
    }

    // Read the result and strip comments
    let lossy_message = tui::get_text::from_editor_no_comments_as_patch(
        "commit_msg",
        &template,
        Some(template_rest.as_str()).filter(|s| !s.is_empty()),
    )?
    .to_string();

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

    let branch_name_lossy =
        tui::get_text::from_editor_no_comments("branch_name", &template)?.to_string();

    if branch_name_lossy.is_empty() {
        bail!("Aborting due to empty branch name");
    }

    Ok(branch_name_lossy)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod prepare_provided_message {
        use super::*;

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
    #[test]
    fn test_show_diff_in_editor() {
        assert_eq!(
            Some(ShowDiffInEditor::Always),
            ShowDiffInEditor::from_args(true, false)
        );

        assert_eq!(
            Some(ShowDiffInEditor::Never),
            ShowDiffInEditor::from_args(false, true)
        );

        assert_eq!(
            Some(ShowDiffInEditor::Unspecified),
            ShowDiffInEditor::from_args(false, false)
        );

        assert_eq!(None, ShowDiffInEditor::from_args(true, true));
    }
}
