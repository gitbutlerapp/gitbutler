//! This module contains all the actual git related operations that the TUI performs.
//!
//! It shouldn't contain any UI concerns.
//!
//! All functions that use legacy APIs must be postfixed with `_legacy`.

use anyhow::Context as _;
use bstr::BString;
use but_api::{
    commit::types::{CommitCreateResult, CommitInsertBlankResult, CommitRewordResult},
    diff::ComputeLineStats,
};
use but_core::DiffSpec;
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_operating_modes::OperatingMode;
use gitbutler_stack::StackId;

use crate::{
    CliId,
    command::legacy::{
        self, ShowDiffInEditor,
        rub::RubOperation,
        status::{
            StatusFlags, StatusOutput, StatusOutputLine, StatusRenderMode,
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
    debug_enabled: bool,
) -> anyhow::Result<Vec<StatusOutputLine>> {
    {
        let meta = ctx.meta()?;
        let (_guard, repo, mut ws, _) = ctx.workspace_mut_and_db()?;
        ws.refresh_from_head(&repo, &meta)?;
    }

    let mut new_lines = Vec::new();

    legacy::status::build_status_context(
        ctx,
        out,
        mode,
        flags,
        StatusRenderMode::Tui {
            debug: debug_enabled,
        },
    )
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
    )
}

pub(super) fn create_commit_legacy(
    ctx: &mut Context,
    target: &CliId,
    source: &CommitSource,
    scope_to_stack: Option<gitbutler_stack::StackId>,
    insert_side: InsertSide,
) -> anyhow::Result<Option<CommitCreateResult>> {
    let (insert_commit_relative_to, insert_side) = match target {
        CliId::Branch { name, .. } => {
            let repo = ctx.repo.get()?;
            let reference = repo.find_reference(name)?;
            (
                RelativeTo::Reference(reference.name().to_owned()),
                InsertSide::Below,
            )
        }
        CliId::Commit { commit_id, .. } => (RelativeTo::Commit(*commit_id), insert_side),
        CliId::Uncommitted(_)
        | CliId::PathPrefix { .. }
        | CliId::CommittedFile { .. }
        | CliId::Unassigned { .. }
        | CliId::Stack { .. } => {
            return Ok(None);
        }
    };

    // find what to commit
    let changes_to_commit = match source {
        CommitSource::Unassigned(..) => {
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
            assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_none())
                .map(DiffSpec::from)
                .collect::<Vec<_>>()
        }
        CommitSource::Uncommitted(uncommitted_cli_id) => uncommitted_cli_id
            .hunk_assignments
            .iter()
            .filter(|assignment| assignment.stack_id == scope_to_stack)
            .cloned()
            .map(DiffSpec::from)
            .collect::<Vec<_>>(),
        CommitSource::Stack(StackCommitSource { stack_id, .. }) => {
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
            assignments
                .into_iter()
                .filter(|assignment| assignment.stack_id.is_some_and(|id| &id == stack_id))
                .map(DiffSpec::from)
                .collect::<Vec<_>>()
        }
    };

    let changes_to_commit = but_workspace::flatten_diff_specs(changes_to_commit);

    // create commit
    but_api::commit::create::commit_create(
        ctx,
        insert_commit_relative_to,
        insert_side,
        changes_to_commit,
        // we reword the commit with the editor before the next render
        String::new(),
    )
    .context("failed to create commit")
    .map(Some)
}

pub(super) fn rub_legacy(
    ctx: &mut Context,
    out: &mut OutputChannel,
    operation: RubOperation<'_>,
) -> anyhow::Result<()> {
    operation.execute(ctx, out)
}

pub(super) fn rub_using_but_api(
    ctx: &mut Context,
    operation: &RubOperation<'_>,
) -> anyhow::Result<Option<SelectAfterReload>> {
    // `perform_operation` is in a legacy module but it's explicitly written to not use legacy code.
    // When it has reached feature parity with `but rub` it'll be promoted to a non-legacy module.
    // Hence why this function doesn't have the legacy postfix.
    legacy::status::tui::rub_api::perform_operation(ctx, operation)
}

pub(super) fn reword_commit_with_editor_legacy(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<CommitRewordResult>> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();

    let new_message = legacy::reword::get_commit_message_from_editor(
        ctx,
        commit_details,
        current_message.clone(),
        ShowDiffInEditor::Unspecified,
    )?;

    let Some(new_message) = new_message else {
        return Ok(None);
    };

    if !legacy::commit_message_prep::should_update_commit_message(&current_message, &new_message) {
        return Ok(None);
    }

    but_api::commit::reword::commit_reword_only(ctx, commit_id, BString::from(new_message))
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

pub(super) fn reword_commit_inline_legacy(
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

    but_api::commit::reword::commit_reword_only(ctx, commit_id, BString::from(new_message))
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
        subject_commit_id,
        RelativeTo::Reference(target_branch_name),
        InsertSide::Below,
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
        subject_commit_id,
        RelativeTo::Commit(target_commit_id),
        insert_side,
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
    but_api::branch::move_branch(ctx, source_ref.as_ref(), target_ref.as_ref())
        .context("failed to move branch")?;
    Ok(())
}

pub(super) fn tear_off_branch(ctx: &mut Context, source_branch_name: &str) -> anyhow::Result<()> {
    let repo = ctx.repo.get()?;
    let source_ref = repo.find_reference(source_branch_name)?.name().to_owned();
    drop(repo);
    but_api::branch::tear_off_branch(ctx, source_ref.as_ref())
        .context("failed to tear off branch")?;
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

pub(super) fn has_unassigned_changes(ctx: &mut Context) -> anyhow::Result<bool> {
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

pub(super) fn commit_is_empty(ctx: &mut Context, commit_id: gix::ObjectId) -> anyhow::Result<bool> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    Ok(commit_details.diff_with_first_parent.is_empty())
}

pub(super) fn reword_branch_inline_legacy(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> anyhow::Result<String> {
    gitbutler_branch_actions::stack::update_branch_name(ctx, stack_id, branch_name, new_name)
}
