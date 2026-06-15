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
use but_core::{DryRun, diff::CommitDetails, ref_metadata::StackId};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::entry::Snapshot;

use crate::{
    CliId,
    args::OutputFormat,
    command::legacy::{
        self, ShowDiffInEditor,
        rub::RubOperation,
        status::{
            StatusFlags, StatusOutput, StatusOutputLine, StatusRenderMode, TuiLaunchOptions,
            tui::SelectAfterReload,
        },
    },
    utils::{WriteWithUtils, diff_specs},
};

pub(super) fn reload_legacy(
    ctx: &mut Context,
    out: &mut dyn WriteWithUtils,
    mode: &OperatingMode,
    flags: StatusFlags,
    options: TuiLaunchOptions,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    let mut guard = ctx.exclusive_worktree_access();

    {
        let meta = ctx.meta()?;
        let project_meta = ctx.project_meta()?;
        let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
        ws.refresh_from_head(&repo, &meta, project_meta)?;
    }

    let mut new_lines = Vec::new();

    let status_ctx = legacy::status::build_status_context(
        ctx,
        guard.write_permission(),
        out,
        OutputFormat::Human,
        mode,
        flags,
        StatusRenderMode::Tui(options),
    )?;
    legacy::status::build_status_output(
        ctx,
        &status_ctx,
        &mut StatusOutput::Buffer {
            lines: &mut new_lines,
        },
    )?;

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
    let anchor = but_api::legacy::stack::create_reference::Anchor::AtSegment {
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

fn discard_uncommitted_legacy_with_assignments(
    ctx: &mut Context,
    hunk_assignments: Vec<but_hunk_assignment::HunkAssignment>,
) -> anyhow::Result<()> {
    let changes_to_discard = {
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let mut builder = diff_specs::DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_hunk_assignments(hunk_assignments)?;
        builder.into_diff_specs()
    };

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
    discard_uncommitted_legacy_with_assignments(ctx, hunk_assignments)
}

pub(super) fn discard_unassigned_legacy(ctx: &mut Context) -> anyhow::Result<()> {
    let unassigned_changes = {
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let mut builder = diff_specs::DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_changes_from_unassigned(&crate::id::UNASSIGNED.to_string())?;
        builder.into_diff_specs()
    };

    if unassigned_changes.is_empty() {
        return Ok(());
    }

    but_api::legacy::workspace::discard_worktree_changes(ctx, unassigned_changes)?;

    Ok(())
}

pub(super) fn discard_stack(ctx: &mut Context, stack_id: StackId) -> anyhow::Result<()> {
    let stack_changes = {
        let context_lines = ctx.settings.context_lines;
        let (_guard, repo, ws, mut db) = ctx.workspace_and_db_mut()?;
        let mut builder = diff_specs::DiffSpecBuilder::new(&mut db, &repo, &ws, context_lines);
        builder.push_changes_from_stack(stack_id)?;
        builder.into_diff_specs()
    };

    if stack_changes.is_empty() {
        return Ok(());
    }

    but_api::legacy::workspace::discard_worktree_changes(ctx, stack_changes)?;

    Ok(())
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
