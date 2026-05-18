//! Direct workspace mutation debug commands.

use std::io;

use anyhow::{Context as _, Result};
use but_core::worktree::checkout::UncommitedWorktreeChanges;
use but_workspace::branch::{
    OnWorkspaceMergeConflict,
    apply::{WorkspaceMerge, WorkspaceReferenceNaming},
};

use crate::args::{ApplyArgs, Args, DebugWorkspaceArgs, UnapplyArgs};

/// Apply a branch through `but-workspace`, bypassing app/API wiring.
pub(crate) fn apply(
    args: &Args,
    mutation_args: &ApplyArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let branch = ref_name(&repo, &mutation_args.ref_name)?;

    let outcome = but_workspace::branch::apply(
        branch.as_ref(),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::apply::Options {
            workspace_merge: WorkspaceMerge::MergeIfNeeded,
            on_workspace_conflict: OnWorkspaceMergeConflict::AbortAndReportConflictingStacks,
            workspace_reference_naming: WorkspaceReferenceNaming::Default,
            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
            order: None,
            new_stack_id: None,
        },
    )?;

    writeln!(out, "{outcome:#?}")?;
    *ws = outcome.workspace.into_owned();
    emit_after(&ws, &mutation_args.debug, err)
}

/// Unapply a branch through `but-workspace`, bypassing app/API wiring.
pub(crate) fn unapply(
    args: &Args,
    mutation_args: &UnapplyArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let mut ctx = but_ctx::Context::discover(&args.current_dir)?;
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let (repo, mut ws, _) = ctx.workspace_mut_and_db_with_perm(guard.write_permission())?;
    let branch = ref_name(&repo, &mutation_args.ref_name)?;

    let outcome = but_workspace::branch::unapply(
        branch.as_ref(),
        &ws,
        &repo,
        &mut meta,
        but_workspace::branch::unapply::Options {
            workspace_disposition: mutation_args.disposition.into(),
            uncommitted_changes: UncommitedWorktreeChanges::KeepAndAbortOnConflict,
        },
    )?;

    writeln!(out, "{outcome:#?}")?;
    *ws = outcome.workspace.into_owned();
    emit_after(&ws, &mutation_args.debug, err)
}

pub(crate) fn emit_after(
    ws: &but_graph::Workspace,
    debug: &DebugWorkspaceArgs,
    err: &mut dyn io::Write,
) -> Result<()> {
    emit_stack_summary(ws, err)?;
    debug.emit_workspace(ws, err)
}

fn emit_stack_summary(ws: &but_graph::Workspace, err: &mut dyn io::Write) -> Result<()> {
    writeln!(
        err,
        "workspace stacks after: {} ({})",
        ws.stacks.len(),
        ws.stacks
            .iter()
            .filter_map(|stack| stack.ref_name())
            .map(|ref_name| ref_name.shorten().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )?;
    Ok(())
}

fn ref_name(repo: &gix::Repository, name: &str) -> Result<gix::refs::FullName> {
    repo.find_reference(name)
        .with_context(|| format!("Could not resolve ref '{name}'"))
        .map(|reference| reference.name().to_owned())
}
