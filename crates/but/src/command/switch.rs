use crate::{CliResult, IdMap, args::atoms::CliIdArg, utils::OutputChannel};

pub fn handle(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    target: Option<CliIdArg>,
    workspace: bool,
    new: bool,
) -> CliResult<()> {
    let mut guard = ctx.exclusive_worktree_access();

    if workspace {
        but_api::branch::workspace_checkout_with_perm(ctx, guard.write_permission())?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Switched to workspace")?;
        }
        return Ok(());
    }

    if new {
        let requested_name = target.map(|target| target.0);
        but_api::branch::branch_checkout_new_with_perm(
            ctx,
            requested_name,
            guard.write_permission(),
        )?;
        let branch_name = current_head_short_name(ctx)?;
        if let Some(out) = out.for_human() {
            writeln!(out, "Created and switched to branch '{branch_name}'")?;
        }
        return Ok(());
    }

    let target = target
        .ok_or_else(|| anyhow::anyhow!("BUG: clap requires target, --workspace, or --new"))?;
    let branch = {
        let repo = ctx.repo.get()?;
        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        target.resolve_existing_local_branch(&repo, &id_map)?
    };
    but_api::branch::branch_checkout_with_perm(ctx, branch.clone(), guard.write_permission())?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Switched to branch '{}'", branch.shorten())?;
    }
    Ok(())
}

fn current_head_short_name(ctx: &but_ctx::Context) -> CliResult<String> {
    let repo = ctx.repo.get()?;
    let head_name = repo
        .head_name()?
        .ok_or_else(|| anyhow::anyhow!("HEAD is detached after switching branches"))?;
    Ok(head_name.shorten().to_string())
}
