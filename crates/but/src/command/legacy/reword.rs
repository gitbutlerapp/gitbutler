use anyhow::{Context as _, Result, bail};
use bstr::{BString, ByteSlice};
use but_api::diff::ComputeLineStats;
use but_core::{DryRun, sync::RepoExclusive};
use but_ctx::Context;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};

use super::{
    ShowDiffInEditor,
    commit_message_prep::{normalize_commit_message, should_update_commit_message},
    estimate_diff_blob_size,
};
use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose},
    bad_input,
    command::legacy::reword2::get_branch_name_from_editor,
    theme::Theme,
    tui,
    utils::{
        CliOutput, CliOutputHuman, IntermediateChannel, WriteWithUtils, get_change_id_for_commit,
        merged_upstream::MergedUpstream,
    },
};

pub(crate) fn reword_target(
    ctx: &mut Context,
    out: IntermediateChannel<'_>,
    target: CliIdArg,
    message: Option<&str>,
    format: bool,
    show_diff_in_editor: ShowDiffInEditor,
    allow_merged: crate::args::atoms::AllowMergedArg,
) -> CliResult<RewordOutcome> {
    // Formats without an interactive editor must provide the new message up front.
    if message.is_none() && !format && !out.format().allows_human_ui() {
        return Err(bad_input(
            "A message must be provided via --message (-m) or --file (-F) for this output format",
        )
        .into());
    }

    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), None)?;

    let target = {
        let repo = ctx.repo.get()?;
        target.resolve_in_workspace(&repo, &id_map, Purpose::Target, None)?
    };
    let target = match target {
        crate::args::atoms::ResolvedCliIdArg::AnonymousSegment(segment) => {
            if format {
                return Err(bad_input(
                    "--fix-formatting flag can only be used with commits, not anonymous branches",
                )
                .into());
            }
            if !matches!(show_diff_in_editor, ShowDiffInEditor::Unspecified) {
                return Err(bad_input(
                    "--diff and --no-diff flags can only be used with commits, not anonymous branches"
                )
                .into());
            }
            return create_branch_for_anonymous_segment(
                ctx,
                segment,
                message,
                guard.write_permission(),
            );
        }
        target => target,
    };

    let outcome = match target.into_branch_or_commit()? {
        BranchOrCommit::Branch(BranchArg(name)) => {
            if format {
                return Err(bad_input(
                    "--fix-formatting flag can only be used with commits, not branches",
                )
                .into());
            }
            if !matches!(show_diff_in_editor, ShowDiffInEditor::Unspecified) {
                return Err(bad_input(
                    "--diff and --no-diff flags can only be used with commits, not branches",
                )
                .into());
            }
            edit_branch_name(ctx, &name, message, guard.write_permission())?
        }
        BranchOrCommit::Commit(commit) => {
            // Rewording a landed commit is lost work: the rewritten commit is
            // dropped again on the next `but pull`.
            MergedUpstream::from_ctx(ctx, allow_merged)?
                .ensure_commit_not_merged(commit.commit_id)?;
            edit_commit_message_by_id_and_reword_commit(
                ctx,
                commit.commit_id,
                message,
                format,
                show_diff_in_editor,
                guard.write_permission(),
            )?
        }
    };

    Ok(outcome)
}

#[must_use]
pub(crate) enum RewordOutcome {
    NamedAnonymousBranch {
        cli_id: String,
        new_name: String,
    },
    BranchNameUnchanged {
        name: String,
    },
    RenamedBranch {
        old_name: String,
        new_name: String,
    },
    RewordedCommit {
        change_id: but_core::ChangeId,
        new_commit_id: gix::ObjectId,
    },
    CommitMessageUnchanged,
}

impl CliOutputHuman for RewordOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        _theme: &'static Theme,
    ) -> anyhow::Result<()> {
        match self {
            RewordOutcome::NamedAnonymousBranch { cli_id, new_name } => {
                writeln!(out, "Named anonymous branch '{cli_id}' as '{new_name}'")?;
            }
            RewordOutcome::BranchNameUnchanged { name } => {
                writeln!(out, "Branch already named '{name}' - nothing to do")?;
            }
            RewordOutcome::RenamedBranch { old_name, new_name } => {
                writeln!(out, "Renamed branch '{old_name}' to '{new_name}'")?;
            }
            RewordOutcome::RewordedCommit { change_id, .. } => {
                let new_commit = crate::theme::Commit(change_id);
                writeln!(out, "Updated commit message for {new_commit}")?;
            }
            RewordOutcome::CommitMessageUnchanged => {
                writeln!(out, "No changes to commit message - nothing to be done")?;
            }
        }
        Ok(())
    }
}

impl CliOutput for RewordOutcome {
    fn on_json(self) -> impl serde::Serialize {
        #[derive(serde::Serialize)]
        struct Output {
            new_commit_id: String,
        }
        match self {
            RewordOutcome::RewordedCommit { new_commit_id, .. } => Some(Output {
                new_commit_id: new_commit_id.to_string(),
            }),
            RewordOutcome::NamedAnonymousBranch { .. }
            | RewordOutcome::BranchNameUnchanged { .. }
            | RewordOutcome::RenamedBranch { .. }
            | RewordOutcome::CommitMessageUnchanged => None,
        }
    }
}

fn create_branch_for_anonymous_segment(
    ctx: &mut Context,
    segment: crate::id::AnonymousSegmentId,
    message: Option<&str>,
    perm: &mut RepoExclusive,
) -> CliResult<RewordOutcome> {
    let non_validated_new_name = prepare_provided_message(message, "branch name")
        .unwrap_or_else(|| get_branch_name_from_editor(""))?;
    let new_ref = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
        BranchArg(non_validated_new_name).resolve_for_creation(&repo, &ws)?
    };

    let cli_id = segment.id;
    let anchor_commit_id = segment.anchor_commit_id.ok_or_else(|| {
        bad_input(format!(
            "Cannot name anonymous branch '{cli_id}' because it has no anchor commit"
        ))
    })?;
    let checkout = match segment.lane {
        crate::id::LaneId::Stack(_) => but_graph::WorktreeKind::Main,
        crate::id::LaneId::Worktree(name) => but_graph::WorktreeKind::LinkedId(name),
    };
    let mut meta = ctx.meta()?;
    let (new_ref, _ws) = but_transaction::with_transaction_with_perm(
        ctx,
        &mut meta,
        perm,
        SnapshotDetails::new(OperationKind::CreateBranch),
        &[],
        DryRun::No,
        |mut tx| {
            tx.name_segment(&checkout, anchor_commit_id, new_ref.as_ref())?;
            Ok(but_transaction::Commit(new_ref))
        },
    )?;

    Ok(RewordOutcome::NamedAnonymousBranch {
        cli_id,
        new_name: new_ref.shorten().to_string(),
    })
}

fn edit_branch_name(
    ctx: &mut Context,
    branch_name: &str,
    message: Option<&str>,
    perm: &mut RepoExclusive,
) -> CliResult<RewordOutcome> {
    let lanes = crate::legacy::workspace::applied_lanes(ctx)?;
    for lane in &lanes {
        if let Some(branch) = lane.branch(branch_name) {
            let non_validated_new_name = prepare_provided_message(message, "branch name")
                .unwrap_or_else(|| get_branch_name_from_editor(branch_name))?;

            if non_validated_new_name == branch_name {
                return Ok(RewordOutcome::BranchNameUnchanged {
                    name: branch_name.to_owned(),
                });
            };

            let new_branch_name = {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(perm.read_permission())?;
                BranchArg(non_validated_new_name)
                    .resolve_unused_local_name(&repo, &ws, Some(branch.reference.as_ref()))?
                    .shorten()
                    .to_string()
            };
            let new_branch_name = but_api::branch::branch_rename_with_perm(
                ctx,
                branch.reference.clone(),
                new_branch_name,
                perm,
            )?
            .new_ref
            .shorten()
            .to_string();
            return Ok(RewordOutcome::RenamedBranch {
                old_name: branch_name.to_owned(),
                new_name: new_branch_name,
            });
        }
    }

    Err(bad_input("Branch not found in workspace")
        .arg_value(branch_name)
        .hint("You can only reword applied branches")
        .into())
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

pub(crate) fn get_commit_message_from_editor(
    repo: &gix::Repository,
    context_lines: u32,
    commit_details: but_core::diff::CommitDetails,
    editor_initial_message: String,
    current_message_for_comparison: &str,
    show_diff_in_editor: ShowDiffInEditor,
) -> Result<Option<String>> {
    let changed_files = get_changed_files_from_commit_details(&commit_details);

    let should_show_diff = show_diff_in_editor.should_show_diff(|| {
        estimate_diff_blob_size(&commit_details.diff_with_first_parent, repo)
    })?;
    let diff = should_show_diff
        .then(|| {
            commit_details
                .diff_with_first_parent
                .iter()
                .map(|change| change.unified_diff(repo, context_lines))
                .filter_map(|diff| diff.transpose())
                .collect::<Result<Vec<_>>>()
        })
        .transpose()?;

    let comment_prefix = comment_prefix(&repo.config_snapshot(), &editor_initial_message)?;
    let new_message = actually_get_commit_message_from_editor(
        &editor_initial_message,
        &changed_files,
        diff.as_deref(),
        &comment_prefix,
    )?;

    if should_update_commit_message(current_message_for_comparison, &new_message) {
        Ok(Some(normalize_commit_message(&new_message).to_owned()))
    } else {
        Ok(None)
    }
}

fn edit_commit_message_by_id_and_reword_commit(
    ctx: &mut Context,
    commit_oid: gix::ObjectId,
    message: Option<&str>,
    format: bool,
    show_diff_in_editor: ShowDiffInEditor,
    perm: &mut RepoExclusive,
) -> Result<RewordOutcome> {
    // Get commit details directly - no need to iterate through stacks
    let commit_details = but_api::diff::commit_details(ctx, commit_oid, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();

    let new_message = if format {
        if message.is_some() {
            bail!("Cannot use both --fix-formatting and --message flags together");
        }
        Some(but_core::commit::format_commit_message(&current_message))
    } else if let Some(message) = message {
        Some(normalize_commit_message(message).to_owned())
    } else {
        get_commit_message_from_editor(
            &*ctx.repo.get()?,
            ctx.settings.context_lines,
            commit_details,
            current_message.clone(),
            &current_message,
            show_diff_in_editor,
        )?
    }
    .filter(|new_message| should_update_commit_message(&current_message, new_message));

    if let Some(new_message) = new_message {
        let change_id = {
            let repo = ctx.repo.get()?;
            get_change_id_for_commit(&repo, commit_oid)?
        };
        let new_commit_oid = but_api::commit::reword::commit_reword_with_perm(
            ctx,
            commit_oid,
            BString::from(new_message),
            DryRun::No,
            perm,
        )?;

        Ok(RewordOutcome::RewordedCommit {
            change_id,
            new_commit_id: new_commit_oid.new_commit,
        })
    } else {
        Ok(RewordOutcome::CommitMessageUnchanged)
    }
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

/// The prefix of comment lines in the commit message template, read from Git's
/// `core.commentString` or `core.commentChar` and defaulting to `#`.
///
/// Like `git commit`, `auto` picks the first candidate that doesn't start a line of `message`.
fn comment_prefix(config: &gix::config::Snapshot<'_>, message: &str) -> Result<String> {
    let value = config
        .string("core.commentString")
        .or_else(|| config.string("core.commentChar"))
        .map(|value| value.to_str_lossy().into_owned())
        .filter(|value| !value.is_empty() && !value.contains('\n'));
    match value.as_deref() {
        Some(value) if value.eq_ignore_ascii_case("auto") => "#;@!$%^&|:"
            .chars()
            .find(|candidate| {
                !message
                    .lines()
                    .any(|line| line.starts_with(*candidate))
            })
            .map(String::from)
            .context(
                "Unable to select a comment character that is not used in the current commit message",
            ),
        Some(value) => Ok(value.to_owned()),
        None => Ok("#".to_owned()),
    }
}

fn actually_get_commit_message_from_editor(
    current_message: &str,
    changed_files: &[String],
    diff: Option<&[BString]>,
    comment_prefix: &str,
) -> Result<String> {
    let mut template = String::new();
    template.push_str(current_message);
    if !current_message.is_empty() && !current_message.ends_with('\n') {
        template.push('\n');
    }
    let c = comment_prefix;
    template.push_str(&format!(
        "\n{c} Please enter the commit message for your changes. Lines starting\n"
    ));
    template.push_str(&format!(
        "{c} with '{c}' will be ignored, and an empty message aborts the commit.\n"
    ));
    template.push_str(&format!("{c}\n{c} Changes in this commit:\n"));
    for file in changed_files {
        template.push_str(&format!("{c}\t{file}\n"));
    }
    template.push_str(&format!("{c}\n"));

    let mut template_rest = String::new();
    if let Some(diff) = diff
        && !diff.is_empty()
    {
        for line in diff {
            template_rest.push_str(&line.to_str_lossy());
        }
    }

    let lossy_message = tui::get_text::from_editor_no_comments_as_patch(
        "commit_msg",
        &template,
        Some(template_rest.as_str()).filter(|s| !s.is_empty()),
        comment_prefix,
    )?
    .to_string();

    Ok(lossy_message)
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
}
