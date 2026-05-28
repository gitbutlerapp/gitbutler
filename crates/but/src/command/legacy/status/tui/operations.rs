//! This module contains all the actual git related operations that the TUI performs.
//!
//! It shouldn't contain any UI concerns.
//!
//! All functions that use legacy APIs must be postfixed with `_legacy`.

use anyhow::Context as _;
use bstr::BString;
use but_api::{
    commit::types::{CommitDiscardResult, CommitInsertBlankResult, CommitRewordResult},
    diff::ComputeLineStats,
    legacy::oplog::RestoreKind,
};
use but_core::{DiffSpec, DryRun, diff::CommitDetails, ref_metadata::StackId};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::entry::Snapshot;

use crate::{
    CliId,
    command::legacy::{
        self, ShowDiffInEditor,
        rub::RubOperation,
        status::{
            StatusFlags, StatusOutput, StatusOutputLine, StatusRenderMode, TuiLaunchOptions,
            tui::{CommitSource, SelectAfterReload, mode::StackCommitSource},
        },
    },
    utils::OutputChannel,
};

pub(super) async fn reload_legacy(
    ctx: &mut Context,
    out: &mut OutputChannel,
    mode: &OperatingMode,
    flags: StatusFlags,
    options: TuiLaunchOptions,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    {
        let meta = ctx.meta()?;
        let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
        ws.refresh_from_head(&repo, &meta)?;
    }

    let mut new_lines = Vec::new();

    legacy::status::build_status_context(ctx, out, mode, flags, StatusRenderMode::Tui(options))
        .await
        .and_then(|status_ctx| {
            legacy::status::build_status_output(
                ctx,
                &status_ctx,
                &mut StatusOutput::Buffer {
                    lines: &mut new_lines,
                },
            )
        })?;

    Ok(new_lines)
}

pub(super) fn create_empty_commit_relative_to_branch(
    ctx: &mut Context,
    branch_name: &str,
) -> anyhow::Result<CommitInsertBlankResult> {
    let full_name = {
        let repo = ctx.repo.get()?;
        let reference = repo.find_reference(branch_name)?;
        reference.name().to_owned()
    };
    but_api::commit::insert_blank::commit_insert_blank(
        ctx,
        RelativeTo::Reference(full_name),
        InsertSide::Below,
        DryRun::No,
    )
}

pub(super) fn create_empty_commit_relative_to_commit(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<CommitInsertBlankResult> {
    but_api::commit::insert_blank::commit_insert_blank(
        ctx,
        RelativeTo::Commit(commit_id),
        InsertSide::Above,
        DryRun::No,
    )
}

pub(super) fn prepare_changes_to_commit(
    db: &mut but_db::DbHandle,
    repo: &gix::Repository,
    workspace: &but_graph::Workspace,
    context_lines: u32,
    source: &CommitSource,
    scope_to_stack: Option<StackId>,
) -> anyhow::Result<Option<Vec<DiffSpec>>> {
    // find what to commit
    let changes_to_commit = match source {
        CommitSource::Unassigned(..) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
                db.hunk_assignments_mut()?,
                repo,
                workspace,
                Some(changes.clone()),
                context_lines,
            )?;
            let assignments = assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_none());
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
        CommitSource::Uncommitted(uncommitted_cli_id) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let assignments = uncommitted_cli_id
                .hunk_assignments
                .iter()
                .filter(|assignment| assignment.stack_id == scope_to_stack)
                .cloned();
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
        CommitSource::Stack(StackCommitSource { stack_id, .. }) => {
            let changes = but_core::diff::ui::worktree_changes(repo)?.changes;
            let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
                db.hunk_assignments_mut()?,
                repo,
                workspace,
                Some(changes.clone()),
                context_lines,
            )?;
            let assignments = assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_some_and(|id| &id == stack_id));
            but_hunk_assignment::diff_specs_from_assignments_with_changes(assignments, &changes)
        }
    };

    Ok(Some(but_workspace::flatten_diff_specs(changes_to_commit)))
}

pub(super) fn where_to_place_commit(
    ctx: &Context,
    target: &CliId,
    insert_side: InsertSide,
) -> anyhow::Result<Option<(RelativeTo, InsertSide)>> {
    match target {
        CliId::Branch { name, .. } => {
            let repo = ctx.repo.get()?;
            let reference = repo.find_reference(name)?;
            Ok(Some((
                RelativeTo::Reference(reference.name().to_owned()),
                InsertSide::Below,
            )))
        }
        CliId::Commit { commit_id, .. } => Ok(Some((RelativeTo::Commit(*commit_id), insert_side))),
        CliId::Uncommitted(_)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Unassigned { .. }
        | CliId::Stack { .. } => Ok(None),
    }
}

pub(super) fn rub(
    ctx: &mut Context,
    operation: &RubOperation<'_>,
) -> anyhow::Result<Option<SelectAfterReload>> {
    // `perform_operation` is in a legacy module but it's explicitly written to not use legacy code.
    // When it has reached feature parity with `but rub` it'll be promoted to a non-legacy module.
    // Hence why this function doesn't have the legacy postfix.
    legacy::status::tui::rub::perform_operation(ctx, operation)
}

pub(super) fn reword_commit_with_editor_legacy(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<CommitRewordResult>> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();
    reword_commit_with_editor_with_message_legacy(ctx, commit_details, current_message)
}

pub(super) fn reword_commit_with_editor_with_message_legacy(
    ctx: &mut Context,
    commit_details: CommitDetails,
    editor_initial_message: String,
) -> anyhow::Result<Option<CommitRewordResult>> {
    let commit_id = commit_details.commit.id;
    let current_message = commit_details.commit.inner.message.to_string();
    let new_message = legacy::reword::get_commit_message_from_editor(
        &*ctx.repo.get()?,
        ctx.settings.context_lines,
        commit_details,
        editor_initial_message,
        &current_message,
        ShowDiffInEditor::Unspecified,
    )?;

    let Some(new_message) = new_message else {
        return Ok(None);
    };

    if !legacy::commit_message_prep::should_update_commit_message(&current_message, &new_message) {
        return Ok(None);
    }

    but_api::commit::reword::commit_reword(ctx, commit_id, BString::from(new_message), DryRun::No)
        .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))
        .map(Some)
}

pub(super) fn current_commit_message(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<String> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    Ok(commit_details.commit.inner.message.to_string())
}

pub(super) fn commit_message_has_multiple_lines_legacy(message: &str) -> bool {
    legacy::commit_message_prep::commit_message_has_multiple_lines(message)
}

pub(super) fn reword_commit_legacy(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
    new_message: &str,
) -> anyhow::Result<Option<CommitRewordResult>> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();
    let new_message =
        legacy::commit_message_prep::normalize_commit_message(new_message).to_string();

    if !legacy::commit_message_prep::should_update_commit_message(&current_message, &new_message) {
        return Ok(None);
    }

    but_api::commit::reword::commit_reword(ctx, commit_id, BString::from(new_message), DryRun::No)
        .with_context(|| format!("failed to reword {}", commit_id.to_hex_with_len(7)))
        .map(Some)
}

pub(super) fn move_commit_to_branch(
    ctx: &mut Context,
    subject_commit_id: gix::ObjectId,
    branch_name: &str,
) -> anyhow::Result<but_api::commit::types::CommitMoveResult> {
    let repo = ctx.repo.get()?;
    let target_branch_name = repo
        .find_reference(branch_name)
        .context("failed to find reference")?
        .name()
        .to_owned();
    drop(repo);
    but_api::commit::move_commit::commit_move(
        ctx,
        vec![subject_commit_id],
        RelativeTo::Reference(target_branch_name),
        InsertSide::Below,
        DryRun::No,
    )
    .context("failed to move commit")
}

pub(super) fn move_commit_to_commit(
    ctx: &mut Context,
    subject_commit_id: gix::ObjectId,
    target_commit_id: gix::ObjectId,
    insert_side: InsertSide,
) -> anyhow::Result<but_api::commit::types::CommitMoveResult> {
    but_api::commit::move_commit::commit_move(
        ctx,
        vec![subject_commit_id],
        RelativeTo::Commit(target_commit_id),
        insert_side,
        DryRun::No,
    )
    .context("failed to move commit")
}

pub(super) fn move_branch_onto_branch(
    ctx: &mut Context,
    source_branch_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let source_ref = repo.find_reference(source_branch_name)?.name().to_owned();
    let target_ref = repo.find_reference(target_branch_name)?.name().to_owned();
    drop(repo);
    but_api::branch::move_branch(ctx, source_ref.as_ref(), target_ref.as_ref(), DryRun::No)
        .context("failed to move branch")?;
    Ok(())
}

pub(super) fn tear_off_branch(ctx: &mut Context, source_branch_name: &str) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let source_ref = repo.find_reference(source_branch_name)?.name().to_owned();
    drop(repo);
    but_api::branch::tear_off_branch(ctx, source_ref.as_ref(), DryRun::No)
        .context("failed to unstack branch")?;
    Ok(())
}

pub(super) fn create_branch_anchored_legacy(
    ctx: &mut Context,
    short_name: String,
) -> anyhow::Result<String> {
    let new_name = but_api::legacy::workspace::canned_branch_name(ctx)
        .context("failed to generate branch name")?;
    let anchor = but_api::legacy::stack::create_reference::Anchor::AtReference {
        short_name,
        position: but_workspace::branch::create_reference::Position::Above,
    };
    let req = but_api::legacy::stack::create_reference::Request {
        new_name: new_name.clone(),
        anchor: Some(anchor),
    };
    but_api::legacy::stack::create_reference(ctx, req).context("failed to create branch")?;
    Ok(new_name)
}

pub(super) fn create_branch_legacy(ctx: &mut Context) -> anyhow::Result<String> {
    let new_name = but_api::legacy::workspace::canned_branch_name(ctx)
        .context("failed to generate branch name")?;
    let req = but_api::legacy::stack::create_reference::Request {
        new_name: new_name.clone(),
        anchor: None,
    };
    but_api::legacy::stack::create_reference(ctx, req).context("failed to create branch")?;
    Ok(new_name)
}

#[expect(dead_code)]
pub(super) fn has_unassigned_changes(ctx: &Context) -> anyhow::Result<bool> {
    let context_lines = ctx.settings.context_lines;

    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;

    Ok(assignments
        .into_iter()
        .any(|assignment| assignment.stack_id.is_none()))
}

pub(super) fn stack_has_assigned_changes(ctx: &Context, stack: StackId) -> anyhow::Result<bool> {
    let context_lines = ctx.settings.context_lines;

    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;

    Ok(assignments
        .into_iter()
        .any(|assignment| assignment.stack_id.is_some_and(|id| id == stack)))
}

pub(super) fn assigned_file_count_for_stack(
    ctx: &Context,
    stack_id: StackId,
) -> anyhow::Result<usize> {
    let context_lines = ctx.settings.context_lines;

    let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
    let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
    let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
        db.hunk_assignments_mut()?,
        &repo,
        &ws,
        Some(changes),
        context_lines,
    )?;

    let files = assignments
        .into_iter()
        .filter(|assignment| assignment.stack_id == Some(stack_id))
        .map(|assignment| assignment.path_bytes.to_vec())
        .collect::<std::collections::HashSet<_>>();

    Ok(files.len())
}

pub(super) fn commit_is_empty(ctx: &mut Context, commit_id: gix::ObjectId) -> anyhow::Result<bool> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    Ok(commit_details.diff_with_first_parent.is_empty())
}

pub(super) fn reword_branch_legacy(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> anyhow::Result<String> {
    gitbutler_branch_actions::stack::update_branch_name(ctx, stack_id, branch_name, new_name)
}

/// Collect paths whose worktree status is either addition or deletion.
fn addition_or_deletion_paths(
    changes: &[but_core::ui::TreeChange],
) -> std::collections::HashSet<Vec<u8>> {
    changes
        .iter()
        .filter_map(|change| {
            if matches!(
                change.status,
                but_core::ui::TreeStatus::Addition { .. }
                    | but_core::ui::TreeStatus::Deletion { .. }
            ) {
                let path: &bstr::BStr = change.path.as_ref();
                Some(path.to_vec())
            } else {
                None
            }
        })
        .collect()
}

/// Convert hunk assignments to diff specs, using whole-file mode for additions/deletions.
fn diff_specs_from_assignments(
    assignments: impl IntoIterator<Item = but_hunk_assignment::HunkAssignment>,
    addition_or_deletion_paths: &std::collections::HashSet<Vec<u8>>,
) -> Vec<DiffSpec> {
    assignments
        .into_iter()
        .map(|assignment| {
            let is_addition_or_deletion =
                addition_or_deletion_paths.contains(&assignment.path_bytes.to_vec());

            DiffSpec {
                previous_path: None,
                path: assignment.path_bytes,
                hunk_headers: if is_addition_or_deletion {
                    Vec::new()
                } else {
                    assignment.hunk_header.into_iter().collect()
                },
            }
        })
        .collect()
}

/// Discard uncommitted assignments with precomputed addition/deletion paths.
fn discard_uncommitted_legacy_with_paths(
    ctx: &mut Context,
    hunk_assignments: Vec<but_hunk_assignment::HunkAssignment>,
    addition_or_deletion_paths: &std::collections::HashSet<Vec<u8>>,
) -> anyhow::Result<()> {
    let changes_to_discard =
        diff_specs_from_assignments(hunk_assignments, addition_or_deletion_paths);

    if changes_to_discard.is_empty() {
        return Ok(());
    }

    but_api::legacy::workspace::discard_worktree_changes(ctx, changes_to_discard)?;

    Ok(())
}

pub(super) fn discard_uncommitted_legacy(
    ctx: &mut Context,
    hunk_assignments: Vec<but_hunk_assignment::HunkAssignment>,
) -> anyhow::Result<()> {
    let addition_or_deletion_paths = {
        let repo = ctx.repo.get()?;
        let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
        addition_or_deletion_paths(&changes)
    };

    discard_uncommitted_legacy_with_paths(ctx, hunk_assignments, &addition_or_deletion_paths)
}

pub(super) fn discard_unassigned_legacy(ctx: &mut Context) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let unassigned_changes = {
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;

        let addition_or_deletion_paths = addition_or_deletion_paths(&changes);

        let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            Some(changes),
            context_lines,
        )?;

        let unassigned_assignments = assignments
            .into_iter()
            .filter(|assignment| assignment.stack_id.is_none())
            .collect::<Vec<_>>();

        diff_specs_from_assignments(unassigned_assignments, &addition_or_deletion_paths)
    };

    if unassigned_changes.is_empty() {
        return Ok(());
    }

    but_api::legacy::workspace::discard_worktree_changes(ctx, unassigned_changes)?;

    Ok(())
}

pub(super) fn discard_stack(ctx: &mut Context, stack_id: StackId) -> anyhow::Result<()> {
    let context_lines = ctx.settings.context_lines;
    let (stack_changes, addition_or_deletion_paths) = {
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let changes = but_core::diff::ui::worktree_changes(&repo)?.changes;
        let addition_or_deletion_paths = addition_or_deletion_paths(&changes);

        let (assignments, _assignments_error) = but_hunk_assignment::assignments_with_fallback(
            db.hunk_assignments_mut()?,
            &repo,
            &ws,
            Some(changes),
            context_lines,
        )?;

        let stack_changes = assignments
            .into_iter()
            .filter(|assignment| assignment.stack_id == Some(stack_id))
            .collect::<Vec<_>>();

        (stack_changes, addition_or_deletion_paths)
    };

    discard_uncommitted_legacy_with_paths(ctx, stack_changes, &addition_or_deletion_paths)
}

pub(super) fn commit_discard(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<CommitDiscardResult> {
    but_api::commit::discard_commit::commit_discard(ctx, commit_id, DryRun::No)
}

pub(super) fn get_undo_target_snapshot_legacy(ctx: &Context) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::get_undo_target_snapshot(ctx)
}

pub(super) fn get_redo_target_snapshot_legacy(ctx: &Context) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::get_redo_target_snapshot(ctx)
}

pub(super) fn peel_restore_snapshot_legacy(
    ctx: &Context,
    sha: gix::ObjectId,
) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::peel_restore_snapshot(ctx, sha)
}

pub(super) fn restore_snapshot_with_kind_legacy(
    ctx: &mut Context,
    restore_kind: RestoreKind,
    sha: gix::ObjectId,
) -> anyhow::Result<()> {
    but_api::legacy::oplog::restore_snapshot_with_kind(ctx, restore_kind, sha)
}
