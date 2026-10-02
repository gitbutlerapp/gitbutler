use anyhow::Context as _;
use but_api::WorkspaceState;
use but_ctx::Context;
use itertools::Itertools;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::atoms::CliIdArg,
    command::legacy::discard::{self, DiscardOutcome},
    utils::{IntermediateChannel, OutputChannel},
};

pub mod new;

mod json;
mod list;
mod show;

pub fn delete(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    branch_args: Vec<CliIdArg>,
) -> CliResult<(DiscardOutcome, WorkspaceState)> {
    let mut guard = ctx.exclusive_worktree_access();
    let mut meta = ctx.meta()?;
    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let branches = {
        let repo = ctx.repo.get()?;
        let branches = branch_args
            .iter()
            .map(|branch| branch.resolve_branch_in_workspace(&repo, &id_map))
            .map(|branch| Ok(branch?.resolve_local_branch_name()?))
            .collect::<CliResult<Vec<_>>>()?
            .into_iter()
            .unique()
            .collect::<Vec<_>>();
        NonEmpty::from_vec(branches)
            .context("BUG: branches is required to be non-empty in clap args")?
    };

    Ok(discard::run(
        ctx,
        &mut meta,
        guard.write_permission(),
        discard::DiscardOperation::Branches(branches),
        gitbutler_oplog::entry::OperationKind::Discard,
    )?)
}

pub fn show_branches(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_arg: CliIdArg,
    review: bool,
    files: bool,
    ai: bool,
    check: bool,
) -> CliResult<()> {
    show::show(ctx, branch_arg, out, review, files, ai, check)
}

#[expect(clippy::too_many_arguments)]
pub fn list_branches(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    filter: Option<String>,
    local: bool,
    remote: bool,
    all: bool,
    no_ahead: bool,
    review: bool,
    no_check: bool,
    empty: bool,
) -> Result<(), anyhow::Error> {
    let ahead = !no_ahead;
    // Invert the flag
    let check = !no_check;
    // Invert the flag
    list::list(
        ctx, local, remote, all, ahead, review, filter, out, check, empty,
    )?;
    Ok(())
}

pub fn handle_no_subcommand(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    list_branches(
        ctx, out, None, false, false, false, false, false, false, false,
    )
}
