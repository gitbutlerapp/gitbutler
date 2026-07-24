//! Direct `but-api` debug commands.

use std::io;

use anyhow::{Context as _, Result, bail};
use but_core::ref_metadata::StackId;
use gix::bstr::ByteSlice;

use crate::args::{ApiArgs, ApiSubcommands, ApiUnapplyStackArgs, Args, DebugWorkspaceArgs};

/// Execute the `api` subcommand.
pub(crate) fn run(
    args: &Args,
    api_args: &ApiArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    match &api_args.cmd {
        ApiSubcommands::BranchList => branch_list(args, out),
        ApiSubcommands::UnapplyStack(unapply_args) => unapply_stack(args, unapply_args, out, err),
    }
}

fn branch_list(args: &Args, out: &mut dyn io::Write) -> Result<()> {
    let ctx = discover_context(args)?;
    let branches = but_api::branch::branch_list(&ctx)?;
    writeln!(out, "{branches:#?}")?;
    Ok(())
}

fn unapply_stack(
    args: &Args,
    unapply_args: &ApiUnapplyStackArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let mut ctx = discover_context(args)?;
    let stack_id = resolve_stack_id(&ctx, &unapply_args.stack)?;
    but_api::legacy::virtual_branches::unapply_stack(&mut ctx, stack_id)?;
    writeln!(out, "unapplied stack {stack_id}")?;
    emit_after(&ctx, &unapply_args.debug, err)?;
    Ok(())
}

fn discover_context(args: &Args) -> Result<but_ctx::Context> {
    but_ctx::Context::discover(&args.current_dir).with_context(|| {
        format!(
            "Could not open GitButler context at '{}'",
            args.current_dir.display()
        )
    })
}

fn emit_after(
    ctx: &but_ctx::Context,
    debug: &DebugWorkspaceArgs,
    err: &mut dyn io::Write,
) -> Result<()> {
    if debug.invalidate_workspace {
        ctx.invalidate_workspace_cache()?;
    }
    let (_guard, _repo, workspace, _db) = ctx.workspace_and_db()?;
    debug.emit_workspace(&workspace, err)
}

fn resolve_stack_id(ctx: &but_ctx::Context, stack: &str) -> Result<StackId> {
    fn stack_matches(stack: &but_graph::workspace::Stack, name: &str) -> bool {
        stack.segments.iter().any(|segment| {
            segment.ref_name().is_some_and(|ref_name| {
                ref_name.as_bstr() == name.as_bytes()
                    || ref_name.shorten().as_bstr() == name.as_bytes()
            })
        })
    }
    if let Ok(stack_id) = stack.parse::<StackId>() {
        return Ok(stack_id);
    }

    let (_guard, _repo, workspace, _db) = ctx.workspace_and_db()?;
    let mut matches = workspace.stacks.iter().filter_map(|workspace_stack| {
        stack_matches(workspace_stack, stack)
            .then_some(workspace_stack.id)
            .flatten()
    });
    let Some(stack_id) = matches.next() else {
        bail!("Could not resolve stack '{stack}' by UUID or workspace branch name");
    };
    if matches.next().is_some() {
        bail!("Stack name '{stack}' is ambiguous in the current workspace");
    }
    Ok(stack_id)
}
