use anyhow::Context as _;
use but_api::WorkspaceState;
use but_ctx::Context;
use gix::refs::Category;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::atoms::CliIdArg,
    bad_input,
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
    let id_map = IdMap::new_from_context(ctx, guard.read_permission(), None)?;

    let mut deleted = Vec::new();
    let mut applied = Vec::new();
    let mut unapplied = Vec::new();
    {
        let repo = ctx.repo.get()?;
        for branch_arg in &branch_args {
            let (branch, is_applied) = match branch_arg.try_resolve_branch(&repo, &id_map)? {
                Some(branch) => (branch.resolve_local_branch_name()?, true),
                // A local branch outside the workspace has no CLI ID, so only its name finds it.
                None => match Category::LocalBranch
                    .to_full_name(branch_arg.0.as_str())
                    .ok()
                {
                    Some(branch) if repo.try_find_reference(branch.as_ref())?.is_some() => {
                        (branch, false)
                    }
                    _ => {
                        return Err(bad_input(format!("Could not find branch: '{branch_arg}'"))
                            .hint(CliIdArg::TARGET_MISSING_HINT)
                            .into());
                    }
                },
            };
            if !deleted.contains(&branch) {
                deleted.push(branch.clone());
                if is_applied {
                    applied.push(branch);
                } else {
                    unapplied.push(branch);
                }
            }
        }
    }

    let mut ws = None;
    for branch in unapplied {
        let result =
            but_api::branch::branch_remove_with_perm(ctx, branch, guard.write_permission())?;
        ws = Some(result.workspace);
    }
    if let Some(applied) = NonEmpty::from_vec(applied) {
        // Opened after removing unapplied branches so writing it back can't restore their entries.
        let mut meta = ctx.meta()?;
        let (_, applied_ws) = discard::run(
            ctx,
            &mut meta,
            guard.write_permission(),
            discard::DiscardOperation::Branches(applied),
            gitbutler_oplog::entry::OperationKind::Discard,
        )?;
        ws = Some(applied_ws);
    }

    let deleted = NonEmpty::from_vec(deleted)
        .context("BUG: branches is required to be non-empty in clap args")?;
    let ws = ws.context("BUG: every branch is either applied or unapplied")?;
    Ok((DiscardOutcome::Branches(deleted), ws))
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
