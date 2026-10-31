use anyhow::Context as _;
use but_api::WorkspaceState;
use but_ctx::Context;
use itertools::Itertools;
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

    let (branches, in_workspace, outside) = {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        let mut branches = Vec::new();
        for arg in &branch_args {
            let branch = arg.resolve_existing_local_branch(&repo, &id_map)?;
            if ws.is_target_or_upstream(branch.as_ref(), &repo)? {
                return Err(bad_input(format!(
                    "Cannot delete '{}': it is the workspace target branch.",
                    branch.shorten()
                ))
                .into());
            }
            // Like `git branch -D`, refuse to delete the branch a linked worktree has checked out.
            if let Some(worktree) = ws
                .worktrees
                .iter()
                .find(|worktree| worktree.ref_name.as_ref() == Some(&branch))
            {
                let (branch, worktree) = (branch.shorten(), &worktree.name);
                return Err(bad_input(format!(
                    "Cannot delete '{branch}': it is checked out in worktree {worktree}"
                ))
                .hint(format!(
                    "Run `but worktree remove {worktree}` first, then `but branch delete {branch}`"
                ))
                .into());
            }
            branches.push(branch);
        }
        let branches = NonEmpty::from_vec(branches.into_iter().unique().collect())
            .context("BUG: branches is required to be non-empty in clap args")?;
        // Branches outside the workspace, like unapplied ones, have no commits there to discard.
        let (in_workspace, outside): (Vec<_>, Vec<_>) = branches
            .iter()
            .cloned()
            .partition(|branch| ws.refname_is_segment(branch.as_ref()));
        (branches, in_workspace, outside)
    };

    let mut state = None;
    for branch in outside {
        let removed =
            but_api::branch::branch_remove_with_perm(ctx, branch, guard.write_permission())?;
        state = Some(removed.workspace);
    }
    // Discard last: its state carries the checkout and conflict outcome the CLI reports.
    if let Some(in_workspace) = NonEmpty::from_vec(in_workspace) {
        let mut meta = ctx.meta()?;
        let (_, ws) = discard::run(
            ctx,
            &mut meta,
            guard.write_permission(),
            discard::DiscardOperation::Branches(in_workspace),
            gitbutler_oplog::entry::OperationKind::Discard,
        )?;
        state = Some(ws);
    }
    let state = state.context("BUG: at least one branch was deleted")?;
    Ok((DiscardOutcome::Branches(branches), state))
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
