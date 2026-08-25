use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, CliIdArg},
    command::legacy::branch::{
        self,
        new::{NewOperation, NewUnstackedBranchOperation},
    },
    print_deprecation_warning,
    utils::OutputChannel,
};

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
        print_deprecation_warning(
            "`--new/-n` is deprecated and will be removed in a future release. Use `but branch new --switch` instead",
        );

        let mut meta = ctx.meta()?;
        let name = target
            .map(|target| {
                let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
                BranchArg(target.0).resolve_for_creation(&repo, &ws)
            })
            .transpose()?;
        let operation =
            NewOperation::NewUnstackedBranch(NewUnstackedBranchOperation { name, switch: true });
        let outcome = branch::new::run(ctx, &mut meta, guard.write_permission(), operation)?;
        out.print_cli_output(outcome)?;

        return Ok(());
    }

    let target = target
        .ok_or_else(|| anyhow::anyhow!("BUG: clap requires target, --workspace, or --new"))?;
    let branch = {
        let repo = ctx.repo.get()?;
        let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;
        target.resolve_existing_local_branch(&repo, &id_map)?
    };
    but_api::branch::branch_checkout_with_perm(ctx, branch.clone(), guard.write_permission())?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Switched to branch '{}'", branch.shorten())?;
    }
    Ok(())
}
