//! This module contains all the actual git related operations that the TUI performs.
//!
//! It shouldn't contain any UI concerns.
//!
//! All functions that use legacy APIs must be postfixed with `_legacy`.

use anyhow::Context as _;
use but_ctx::Context;
use gitbutler_operating_modes::OperatingMode;
use gix::prelude::ObjectIdExt;

use crate::{
    args::OutputFormat,
    command::legacy::{
        self,
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

pub fn current_commit_message(
    ctx: &mut Context,
    commit_id: gix::ObjectId,
) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    Ok(repo.find_commit(commit_id)?.message_raw()?.to_string())
}

pub fn commit_message_has_multiple_lines_legacy(message: &str) -> bool {
    legacy::commit_message_prep::commit_message_has_multiple_lines(message)
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
