use but_api::legacy::oplog::RestoreKind;
use but_core::RepositoryExt;
use gitbutler_oplog::entry::{OperationKind, Snapshot, Trailer};
use gix::{date::time::CustomFormat, prelude::ObjectIdExt};

use crate::{
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id},
};

pub const ISO8601_NO_TZ: CustomFormat = CustomFormat::new("%Y-%m-%d %H:%M:%S");

/// Filter for oplog entries by operation kind
#[derive(Debug, Clone, Copy)]
pub enum OplogFilter {
    /// Show only on-demand snapshot entries
    Snapshot,
}

impl OplogFilter {
    /// Convert the filter to a list of OperationKind to include
    fn to_include_kinds(self) -> Vec<OperationKind> {
        match self {
            OplogFilter::Snapshot => vec![OperationKind::OnDemandSnapshot],
        }
    }
}

pub(crate) fn show_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    since: Option<&str>,
    filter: Option<OplogFilter>,
) -> anyhow::Result<()> {
    // Convert filter to include_kind parameter for the API
    let include_kind = filter.map(|f| f.to_include_kinds());

    // Resolve partial SHA to full SHA using rev_parse if provided
    let since_sha = if let Some(sha_prefix) = since {
        let repo = ctx.repo.get()?;
        let resolved = repo
            .rev_parse_single(sha_prefix)
            .map_err(|_| anyhow::anyhow!("No oplog entry found matching SHA: {sha_prefix}"))?;
        Some(resolved.detach())
    } else {
        None
    };

    let snapshots = but_api::legacy::oplog::snapshots_iter(ctx, since_sha, None, include_kind)?
        .take(20)
        .collect::<anyhow::Result<Vec<_>>>()?;

    if snapshots.is_empty() {
        if let Some(out) = out.for_json() {
            out.write_value(&snapshots)?;
        } else if let Some(out) = out.for_human() {
            writeln!(out, "No operations found in history.")?;
        }
        return Ok(());
    }

    if let Some(out) = out.for_json() {
        out.write_value(&snapshots)?;
    } else if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        let t = theme::get();
        writeln!(out, "{}", t.important.paint("Operations History"))?;
        writeln!(out, "{}", t.hint.paint("─".repeat(50)))?;
        // Find the longest short ID length to keep all lines aligned.
        let longest_short_id_len = snapshots
            .iter()
            .filter_map(|s| {
                let prefix = s.commit_id.attach(&repo).shorten().ok()?;
                Some(prefix.hex_len())
            })
            .max()
            .unwrap_or(7);

        for snapshot in snapshots {
            let time_string = snapshot_time_string(&snapshot);
            let short = snapshot.commit_id;
            let short = short.to_hex_with_len(longest_short_id_len);
            let commit_id = t.cli_id.paint(short.to_string());

            let (operation_type, title) = if let Some(details) = &snapshot.details {
                let display_title = match details.operation {
                    OperationKind::OnDemandSnapshot => details
                        .body
                        .as_ref()
                        .filter(|b| !b.is_empty())
                        .cloned()
                        .unwrap_or_else(|| details.operation.title().to_owned()),
                    OperationKind::Discard => {
                        let file_names = details
                            .trailers
                            .iter()
                            .filter_map(|t| {
                                if let Trailer::File(value) = t {
                                    Some(&**value)
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        if !file_names.is_empty() {
                            format!("{} ({})", details.operation.title(), file_names.join(", "))
                        } else {
                            details.operation.title().to_owned()
                        }
                    }
                    OperationKind::RestoreFromSnapshotViaUndo
                    | OperationKind::RestoreFromSnapshotViaRedo
                    | OperationKind::RestoreFromSnapshot => {
                        if let Ok(Some(restore_target)) =
                            but_api::legacy::oplog::peel_restore_snapshot(ctx, snapshot.commit_id)
                            && restore_target.commit_id != snapshot.commit_id
                            && let Some(restore_target_details) = &restore_target.details
                        {
                            format!(
                                "Restored from snapshot: {} ({})",
                                restore_target_details.operation.title(),
                                t.cli_id.paint(
                                    restore_target
                                        .commit_id
                                        .to_hex_with_len(longest_short_id_len)
                                        .to_string()
                                ),
                            )
                        } else {
                            details.operation.title().to_owned()
                        }
                    }
                    OperationKind::CreateCommit
                    | OperationKind::CreateBranch
                    | OperationKind::StashIntoBranch
                    | OperationKind::SetBaseBranch
                    | OperationKind::MergeUpstream
                    | OperationKind::UpdateWorkspaceBase
                    | OperationKind::MoveHunk
                    | OperationKind::UpdateBranchName
                    | OperationKind::UpdateBranchNotes
                    | OperationKind::ReorderBranches
                    | OperationKind::UpdateBranchRemoteName
                    | OperationKind::GenericBranchUpdate
                    | OperationKind::DeleteBranch
                    | OperationKind::ApplyBranch
                    | OperationKind::DiscardLines
                    | OperationKind::DiscardHunk
                    | OperationKind::DiscardFile
                    | OperationKind::DiscardChanges
                    | OperationKind::AmendCommit
                    | OperationKind::Absorb
                    | OperationKind::AutoCommit
                    | OperationKind::UndoCommit
                    | OperationKind::DiscardCommit
                    | OperationKind::UnapplyBranch
                    | OperationKind::CherryPick
                    | OperationKind::SquashCommit
                    | OperationKind::UpdateCommitMessage
                    | OperationKind::MoveCommit
                    | OperationKind::MoveBranch
                    | OperationKind::TearOffBranch
                    | OperationKind::ReorderCommit
                    | OperationKind::InsertBlankCommit
                    | OperationKind::MoveCommitFile
                    | OperationKind::FileChanges
                    | OperationKind::EnterEditMode
                    | OperationKind::SyncWorkspace
                    | OperationKind::CreateDependentBranch
                    | OperationKind::RemoveDependentBranch
                    | OperationKind::UpdateDependentBranchName
                    | OperationKind::UpdateDependentBranchDescription
                    | OperationKind::UpdateDependentBranchPrNumber
                    | OperationKind::AutoHandleChangesBefore
                    | OperationKind::AutoHandleChangesAfter
                    | OperationKind::SplitBranch
                    | OperationKind::CleanWorkspace
                    | OperationKind::Unknown => details.operation.title().to_owned(),
                };

                let display_title = out.truncate_if_unpaged(&display_title, 80);
                (details.operation, display_title)
            } else {
                (OperationKind::Unknown, "Unknown operation".to_string())
            };

            let operation_colored = match operation_type {
                OperationKind::CreateCommit => t.success.paint(operation_type.kind_str()),
                OperationKind::UpdateCommitMessage | OperationKind::AmendCommit => {
                    t.attention.paint(operation_type.kind_str())
                }
                OperationKind::UndoCommit
                | OperationKind::RestoreFromSnapshot
                | OperationKind::RestoreFromSnapshotViaUndo
                | OperationKind::RestoreFromSnapshotViaRedo => {
                    t.error.paint(operation_type.kind_str())
                }
                OperationKind::DiscardChanges | OperationKind::Discard => {
                    t.error.paint(operation_type.kind_str())
                }
                OperationKind::CreateBranch => t.local_branch.paint(operation_type.kind_str()),
                OperationKind::MoveCommit
                | OperationKind::ReorderCommit
                | OperationKind::MoveHunk => t.info.paint(operation_type.kind_str()),
                OperationKind::OnDemandSnapshot => t.hint.paint(operation_type.kind_str()),
                OperationKind::StashIntoBranch
                | OperationKind::SetBaseBranch
                | OperationKind::MergeUpstream
                | OperationKind::UpdateWorkspaceBase
                | OperationKind::UpdateBranchName
                | OperationKind::UpdateBranchNotes
                | OperationKind::ReorderBranches
                | OperationKind::UpdateBranchRemoteName
                | OperationKind::GenericBranchUpdate
                | OperationKind::DeleteBranch
                | OperationKind::ApplyBranch
                | OperationKind::DiscardLines
                | OperationKind::DiscardHunk
                | OperationKind::DiscardFile
                | OperationKind::Absorb
                | OperationKind::AutoCommit
                | OperationKind::DiscardCommit
                | OperationKind::UnapplyBranch
                | OperationKind::CherryPick
                | OperationKind::SquashCommit
                | OperationKind::MoveBranch
                | OperationKind::TearOffBranch
                | OperationKind::InsertBlankCommit
                | OperationKind::MoveCommitFile
                | OperationKind::FileChanges
                | OperationKind::EnterEditMode
                | OperationKind::SyncWorkspace
                | OperationKind::CreateDependentBranch
                | OperationKind::RemoveDependentBranch
                | OperationKind::UpdateDependentBranchName
                | OperationKind::UpdateDependentBranchDescription
                | OperationKind::UpdateDependentBranchPrNumber
                | OperationKind::AutoHandleChangesBefore
                | OperationKind::AutoHandleChangesAfter
                | OperationKind::SplitBranch
                | OperationKind::CleanWorkspace
                | OperationKind::Unknown => t.default.paint(operation_type.kind_str()),
            };

            writeln!(
                out,
                "{} {} [{}] {}",
                commit_id,
                t.time.paint(&time_string),
                operation_colored,
                title
            )?;
        }
    }

    Ok(())
}

fn snapshot_time_string(snapshot: &Snapshot) -> String {
    let time = snapshot.created_at;
    // TODO: use `format_or_unix`.
    time.format(ISO8601_NO_TZ)
        .unwrap_or_else(|_| time.seconds.to_string())
}

pub(crate) fn restore_to_oplog(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    oplog_sha: &str,
) -> anyhow::Result<()> {
    let commit_id = ctx.repo.get()?.rev_parse_single(oplog_sha)?.detach();
    let target_snapshot = &but_api::legacy::oplog::get_snapshot(ctx, commit_id)?;
    let commit_short = {
        let repo = ctx.repo.get()?;
        shorten_object_id(&repo, commit_id)
    };

    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.title.as_str())
        .unwrap_or("Unknown operation");

    let target_time = snapshot_time_string(target_snapshot);

    if let Some(mut out) = out.prepare_for_terminal_input() {
        use std::fmt::Write;
        let t = theme::get();
        writeln!(
            out,
            "{}",
            t.progress.paint("Restoring to oplog snapshot...")
        )?;
        writeln!(
            out,
            "  Target: {} ({})",
            t.important.paint(target_operation),
            t.time.paint(&target_time)
        )?;
        writeln!(out, "  Snapshot: {}", t.commit_id.paint(&commit_short))?;
    }

    // Restore to the target snapshot using the but-api crate
    but_api::legacy::oplog::restore_snapshot_with_kind(
        ctx,
        RestoreKind::ExplicitRestoreFromSnapshot,
        commit_id,
    )?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(out, "\n{} Restore completed successfully!", t.sym().success,)?;

        writeln!(
            out,
            "{}",
            t.success
                .paint("\nWorkspace has been restored to the selected snapshot.")
        )?;
    }

    Ok(())
}

pub(crate) fn handle_undo(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let Some(target_snapshot) = but_api::legacy::oplog::get_undo_target_snapshot(ctx)? else {
        print_no_snapshot_to_restore_to(out)?;
        return Ok(());
    };

    restore_to_target_snapshot(ctx, target_snapshot, UndoOrRedo::Undo, out)?;

    Ok(())
}

pub(crate) fn handle_redo(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let Some(target_snapshot) = but_api::legacy::oplog::get_redo_target_snapshot(ctx)? else {
        if let Some(out) = out.for_human() {
            let t = theme::get();
            writeln!(out, "{}", t.attention.paint("No previous undo to redo."))?;
        }
        return Ok(());
    };

    restore_to_target_snapshot(ctx, target_snapshot, UndoOrRedo::Redo, out)?;

    Ok(())
}

#[derive(Copy, Clone)]
enum UndoOrRedo {
    Undo,
    Redo,
}

fn restore_to_target_snapshot(
    ctx: &mut but_ctx::Context,
    target_snapshot: Snapshot,
    kind: UndoOrRedo,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    let target_operation = target_snapshot
        .details
        .as_ref()
        .map(|d| d.operation.title())
        .unwrap_or_else(|| OperationKind::Unknown.title());

    let target_time = snapshot_time_string(&target_snapshot);

    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(
            out,
            "{}",
            t.progress.paint(match kind {
                UndoOrRedo::Undo => "Undoing operation...",
                UndoOrRedo::Redo => "Redoing operation...",
            })
        )?;
        writeln!(
            out,
            "  Reverting to: {} ({})",
            t.important.paint(target_operation),
            t.time.paint(&target_time)
        )?;
    }

    // Restore to the previous snapshot using the but_api
    // TODO: Why does this not require force? It will also overwrite user changes (I think).
    but_api::legacy::oplog::restore_snapshot_with_kind(
        ctx,
        match kind {
            UndoOrRedo::Undo => RestoreKind::RestoreFromSnapshotViaUndo,
            UndoOrRedo::Redo => RestoreKind::RestoreFromSnapshotViaRedo,
        },
        target_snapshot.commit_id,
    )?;

    if let Some(out) = out.for_human() {
        let t = theme::get();
        let repo = ctx.repo.get()?;
        let short = shorten_object_id(&repo, target_snapshot.commit_id);

        match kind {
            UndoOrRedo::Undo => {
                writeln!(
                    out,
                    "{} Undo completed successfully! Restored to snapshot: {}",
                    t.sym().success,
                    t.cli_id.paint(&short)
                )?;
            }
            UndoOrRedo::Redo => {
                writeln!(
                    out,
                    "{} Redo completed successfully! Restored to snapshot: {}",
                    t.sym().success,
                    t.cli_id.paint(&short)
                )?;
            }
        }
    }

    Ok(())
}

fn print_no_snapshot_to_restore_to(out: &mut OutputChannel) -> anyhow::Result<()> {
    if let Some(out) = out.for_human() {
        let t = theme::get();
        writeln!(
            out,
            "{}",
            t.attention.paint("No previous operations to undo.")
        )?;
    }
    Ok(())
}

pub(crate) fn create_snapshot(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    message: Option<&str>,
) -> anyhow::Result<()> {
    let snapshot_id = but_api::legacy::oplog::create_snapshot(ctx, message.map(String::from))?;

    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "snapshot_id": snapshot_id.to_string(),
            "message": message.unwrap_or(""),
            "operation": "create_snapshot"
        }))?;
    } else if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let short = shorten_object_id(&repo, snapshot_id);
        let t = theme::get();
        writeln!(out, "{}", t.success.paint("Snapshot created successfully!"))?;

        if let Some(msg) = message {
            writeln!(out, "  Message: {}", t.info.paint(msg))?;
        }

        writeln!(out, "  Snapshot ID: {}", t.cli_id.paint(&short))?;
        writeln!(
            out,
            "\n{} Use 'but oplog restore {}' to restore to this snapshot later.",
            t.info.paint("💡"),
            short
        )?;
    }

    Ok(())
}
