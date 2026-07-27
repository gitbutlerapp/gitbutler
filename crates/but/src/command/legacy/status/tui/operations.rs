//! This module contains all the actual git related operations that the TUI performs.
//!
//! It shouldn't contain any UI concerns.
//!
//! All functions that use legacy APIs must be postfixed with `_legacy`.

use anyhow::Context as _;
use bstr::BString;
use but_api::{
    commit::types::{CommitInsertBlankResult, CommitRewordResult},
    diff::ComputeLineStats,
    legacy::oplog::RestoreKind,
};
use but_core::{DryRun, diff::CommitDetails, ref_metadata::StackId};
use but_ctx::Context;
use but_rebase::graph_rebase::mutate::{InsertSide, RelativeTo};
use gitbutler_operating_modes::OperatingMode;
use gitbutler_oplog::entry::Snapshot;
use gix::prelude::ObjectIdExt;

use crate::{
    args::OutputFormat,
    command::legacy::{
        self, ShowDiffInEditor,
        status::{StatusFlags, StatusOutput, StatusOutputLine, StatusRenderMode, TuiLaunchOptions},
    },
    utils::WriteWithUtils,
};

pub fn head_sha(ctx: &mut Context) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    Ok(repo
        .head()
        .context("failed to read HEAD")?
        .peel_to_commit()
        .context("failed to peel HEAD to a commit")?
        .id
        .to_string())
}

pub fn reload_legacy(
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
        OutputFormat::Human { agent: false },
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

pub fn create_empty_commit_relative_to_branch(
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

pub fn create_empty_commit_relative_to_commit(
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

pub fn reword_commit_with_editor_legacy(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<Option<CommitRewordResult>> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    let current_message = commit_details.commit.inner.message.to_string();
    reword_commit_with_editor_with_message_legacy(ctx, commit_details, current_message)
}

pub fn reword_commit_with_editor_with_message_legacy(
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

pub fn current_commit_message(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<String> {
    let commit_details = but_api::diff::commit_details(ctx, commit_id, ComputeLineStats::No)?;
    Ok(commit_details.commit.inner.message.to_string())
}

pub fn commit_message_has_multiple_lines_legacy(message: &str) -> bool {
    legacy::commit_message_prep::commit_message_has_multiple_lines(message)
}

pub fn reword_commit_legacy(
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

pub fn create_branch_anchored_legacy(
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

pub fn create_branch_legacy(ctx: &mut Context) -> anyhow::Result<String> {
    let new_name = but_api::legacy::workspace::canned_branch_name(ctx)
        .context("failed to generate branch name")?;
    let req = but_api::legacy::stack::create_reference::Request {
        new_name: new_name.clone(),
        anchor: None,
    };
    but_api::legacy::stack::create_reference(ctx, req).context("failed to create branch")?;
    Ok(new_name)
}

pub fn commit_is_empty(ctx: &mut Context, commit_id: gix::ObjectId) -> anyhow::Result<bool> {
    let repo = ctx.repo.get()?;
    let commit = but_core::Commit::from_id(commit_id.attach(&repo))?;
    let commit_tree_id = commit.tree_id_or_auto_resolution()?.detach();

    let Some(first_parent_id) = commit.inner.parents.first().copied() else {
        let commit_tree = repo.find_tree(commit_tree_id)?;
        return Ok(commit_tree.iter().next().transpose()?.is_none());
    };

    let first_parent = but_core::Commit::from_id(first_parent_id.attach(&repo))?;
    let first_parent_tree_id = first_parent.tree_id_or_auto_resolution()?.detach();
    Ok(commit_tree_id == first_parent_tree_id)
}

pub fn reword_branch_legacy(
    ctx: &mut Context,
    stack_id: StackId,
    branch_name: String,
    new_name: String,
) -> anyhow::Result<String> {
    gitbutler_branch_actions::stack::update_branch_name(ctx, stack_id, branch_name, new_name)
}

pub fn get_undo_target_snapshot_legacy(ctx: &Context) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::get_undo_target_snapshot(ctx)
}

pub fn get_redo_target_snapshot_legacy(ctx: &Context) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::get_redo_target_snapshot(ctx)
}

pub fn peel_restore_snapshot_legacy(
    ctx: &Context,
    sha: gix::ObjectId,
) -> anyhow::Result<Option<Snapshot>> {
    but_api::legacy::oplog::peel_restore_snapshot(ctx, sha)
}

pub fn restore_snapshot_with_kind_legacy(
    ctx: &mut Context,
    restore_kind: RestoreKind,
    sha: gix::ObjectId,
) -> anyhow::Result<()> {
    but_api::legacy::oplog::restore_snapshot_with_kind(ctx, restore_kind, sha)
}
