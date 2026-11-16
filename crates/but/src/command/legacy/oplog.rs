use but_api::legacy::oplog::RestoreKind;
use but_core::RepositoryExt;
use gitbutler_oplog::entry::{OperationKind, Snapshot, Trailer};
use gix::{date::time::CustomFormat, prelude::ObjectIdExt};

use crate::{
    theme::{self, Paint, Theme},
    utils::{CliOutput, CliOutputHuman, OutputChannel, WriteWithUtils, shorten_object_id},
};

pub const ISO8601_NO_TZ: CustomFormat = CustomFormat::new("%Y-%m-%d %H:%M:%S");

#[derive(Debug, Clone, Copy)]
pub enum OplogFilter {
    /// Show only on-demand snapshot entries
    Snapshot,
}

impl OplogFilter {
    fn to_include_kinds(self) -> Vec<OperationKind> {
        match self {
            OplogFilter::Snapshot => vec![OperationKind::OnDemandSnapshot],
        }
    }
}

pub(crate) fn show_oplog(
    ctx: &mut but_ctx::Context,
    since: Option<&str>,
    filter: Option<OplogFilter>,
) -> anyhow::Result<OplogOutcome> {
    let include_kind = filter.map(|f| f.to_include_kinds());

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

    // Find the longest short ID length to keep all lines aligned.
    let longest_short_id_len = {
        let repo = ctx.repo.get()?.clone().for_commit_shortening();
        snapshots
            .iter()
            .filter_map(|s| {
                let prefix = s.commit_id.attach(&repo).shorten().ok()?;
                Some(prefix.hex_len())
            })
            .max()
            .unwrap_or(7)
    };
    let restore_targets = snapshots
        .iter()
        .filter(|snapshot| {
            snapshot.details.as_ref().is_some_and(|details| {
                matches!(
                    details.operation,
                    OperationKind::RestoreFromSnapshotViaUndo
                        | OperationKind::RestoreFromSnapshotViaRedo
                        | OperationKind::RestoreFromSnapshot
                )
            })
        })
        .filter_map(|snapshot| {
            let restore_target =
                but_api::legacy::oplog::peel_restore_snapshot(ctx, snapshot.commit_id)
                    .ok()
                    .flatten()?;
            (restore_target.commit_id != snapshot.commit_id).then_some(())?;
            let details = restore_target.details.as_ref()?;
            Some((
                snapshot.commit_id,
                (
                    details.operation.title().to_owned(),
                    restore_target.commit_id,
                ),
            ))
        })
        .collect();

    Ok(OplogOutcome {
        snapshots,
        longest_short_id_len,
        restore_targets,
    })
}

#[must_use]
pub(crate) struct OplogOutcome {
    snapshots: Vec<Snapshot>,
    longest_short_id_len: usize,
    /// The title and commit of the snapshot each restore snapshot restored to, by restore
    /// snapshot commit.
    restore_targets: std::collections::HashMap<gix::ObjectId, (String, gix::ObjectId)>,
}

impl CliOutputHuman for OplogOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        t: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self {
            snapshots,
            longest_short_id_len,
            restore_targets,
        } = self;
        if snapshots.is_empty() {
            writeln!(out, "No operations found in history.")?;
            return Ok(());
        }

        writeln!(out, "{}", t.important.paint("Operations History"))?;
        writeln!(out, "{}", t.hint.paint("─".repeat(50)))?;

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
                    OperationKind::DiscardCommit => {
                        let commit_count = details
                            .trailers
                            .iter()
                            .filter(|trailer| matches!(trailer, Trailer::Sha(_)))
                            .count();
                        if commit_count > 1 {
                            format!("Discarded {commit_count} commits")
                        } else {
                            details.operation.title().to_owned()
                        }
                    }
                    OperationKind::RestoreFromSnapshotViaUndo
                    | OperationKind::RestoreFromSnapshotViaRedo
                    | OperationKind::RestoreFromSnapshot => {
                        if let Some((restore_target_title, restore_target_id)) =
                            restore_targets.get(&snapshot.commit_id)
                        {
                            format!(
                                "Restored from snapshot: {} ({})",
                                restore_target_title,
                                t.cli_id.paint(
                                    restore_target_id
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
                    | OperationKind::MergeBranch
                    | OperationKind::UpdateWorkspaceBase
                    | OperationKind::MoveHunk
                    | OperationKind::UpdateBranchName
                    | OperationKind::UpdateBranchNotes
                    | OperationKind::ReorderBranches
                    | OperationKind::UpdateBranchRemoteName
                    | OperationKind::GenericBranchUpdate
                    | OperationKind::DeleteBranch
                    | OperationKind::ApplyBranch
                    | OperationKind::SwitchBranch
                    | OperationKind::SwitchToWorkspace
                    | OperationKind::DiscardLines
                    | OperationKind::DiscardHunk
                    | OperationKind::DiscardFile
                    | OperationKind::DiscardChanges
                    | OperationKind::AmendCommit
                    | OperationKind::Absorb
                    | OperationKind::AutoCommit
                    | OperationKind::UndoCommit
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
                    | OperationKind::ResolveConflicts
                    | OperationKind::ResolveConflictsAi
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
                OperationKind::UpdateCommitMessage
                | OperationKind::AmendCommit
                | OperationKind::ResolveConflicts
                | OperationKind::ResolveConflictsAi => t.attention.paint(operation_type.kind_str()),
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
                | OperationKind::MergeBranch
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
                | OperationKind::SwitchBranch
                | OperationKind::SwitchToWorkspace
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

        Ok(())
    }
}

impl CliOutput for OplogOutcome {
    fn on_json(self) -> impl serde::Serialize {
        self.snapshots
    }
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
) -> anyhow::Result<RestoreOutcome> {
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

    but_api::legacy::oplog::restore_snapshot_with_kind(
        ctx,
        RestoreKind::ExplicitRestoreFromSnapshot,
        commit_id,
    )?;

    Ok(RestoreOutcome)
}

#[must_use]
pub(crate) struct RestoreOutcome;

impl CliOutputHuman for RestoreOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        t: &'static Theme,
    ) -> anyhow::Result<()> {
        writeln!(out, "\n{} Restore completed successfully!", t.sym().success,)?;

        writeln!(
            out,
            "{}",
            t.success
                .paint("\nWorkspace has been restored to the selected snapshot.")
        )?;
        Ok(())
    }
}

impl CliOutput for RestoreOutcome {
    fn on_json(self) -> impl serde::Serialize {}
}

pub(crate) fn create_snapshot(
    ctx: &mut but_ctx::Context,
    message: Option<&str>,
) -> anyhow::Result<SnapshotOutcome> {
    let snapshot_id = but_api::legacy::oplog::create_snapshot(ctx, message.map(String::from))?;
    let short_id = shorten_object_id(&*ctx.repo.get()?, snapshot_id);
    Ok(SnapshotOutcome {
        snapshot_id,
        short_id,
        message: message.map(ToOwned::to_owned),
    })
}

#[must_use]
pub(crate) struct SnapshotOutcome {
    snapshot_id: gix::ObjectId,
    short_id: String,
    message: Option<String>,
}

impl CliOutputHuman for SnapshotOutcome {
    fn on_human(
        self,
        out: &mut dyn WriteWithUtils,
        _agent: bool,
        t: &'static Theme,
    ) -> anyhow::Result<()> {
        let Self {
            snapshot_id: _,
            short_id: short,
            message,
        } = self;
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
        Ok(())
    }
}

impl CliOutput for SnapshotOutcome {
    fn on_json(self) -> impl serde::Serialize {
        serde_json::json!({
            "snapshot_id": self.snapshot_id.to_string(),
            "message": self.message.unwrap_or_default(),
            "operation": "create_snapshot"
        })
    }
}
