//! Direct workspace mutation debug commands.

use std::io;

use anyhow::{Context as _, Result};

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
    let (repo, mut ws, mut db) =
        ctx.workspace_mut_and_db_mut_with_perm(guard.write_permission())?;
    let branch = ref_name(&repo, &mutation_args.ref_name)?;

    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &repo, &mut db)?;
    let but_workspace::branch::apply::ApplyStackOutcome {
        rebase,
        status,
        applied_branches,
        tracking,
    } = but_workspace::branch::apply_stack(editor, branch.as_ref(), None)?;
    rebase.materialize(Default::default())?;
    if let Some(tracking) = tracking {
        tracking.write(&repo)?;
    }

    writeln!(out, "{status:?} {applied_branches:#?}")?;
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
    let (repo, mut ws, mut db) =
        ctx.workspace_mut_and_db_mut_with_perm(guard.write_permission())?;
    let branch = ref_name(&repo, &mutation_args.ref_name)?;

    let editor = but_rebase::graph_rebase::Editor::create(&mut ws, &repo, &mut db)?;
    let rebase = but_workspace::branch::unapply_stack(
        editor,
        branch.as_ref(),
        but_workspace::branch::unapply::Options {
            workspace_disposition: mutation_args.disposition.into(),
        },
    )?;
    let workspace_changed = rebase.references_updated()?;
    rebase.materialize(Default::default())?;

    let head = repo
        .head_name()?
        .map_or_else(|| "detached".into(), |name| name.to_string());
    writeln!(out, "workspace_changed: {workspace_changed}, HEAD: {head}")?;
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
