use anyhow::Context as _;
use but_api::json::HexHash;
use but_ctx::Context;
use itertools::Itertools;
use nonempty::NonEmpty;

use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose, ResolvedCliIdArg},
    command::legacy::discard::{self, DiscardOutcome},
    theme::{self, Paint},
    utils::{IntermediateChannel, OutputChannel},
};

mod json;
mod list;
mod show;

pub fn delete(
    ctx: &mut Context,
    _out: IntermediateChannel<'_>,
    branch_args: Vec<CliIdArg>,
) -> CliResult<DiscardOutcome> {
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
    )?)
}

pub fn new(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_name_arg: Option<BranchArg>,
    anchor_arg: Option<CliIdArg>,
) -> CliResult<()> {
    let mut guard = ctx.exclusive_worktree_access();

    let branch_name = if let Some(branch_name_arg) = branch_name_arg {
        let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        branch_name_arg
            .resolve_for_creation(&repo, &ws)?
            .shorten()
            .to_string()
    } else {
        but_api::legacy::workspace::canned_branch_name(ctx)?
    };

    let id_map = IdMap::new_from_context(ctx, guard.read_permission())?;

    let resolved_anchor = {
        let repo = ctx.repo.get()?;
        anchor_arg
            .clone()
            .map(|anchor| anchor.resolve_in_workspace(&repo, &id_map, Purpose::Anchor, None))
            .transpose()?
    };

    if resolved_anchor.is_none()
        && ctx.settings.feature_flags.single_branch
        && gitbutler_operating_modes::in_outside_workspace_mode(ctx, guard.read_permission())?
    {
        let head_name = ctx
            .repo
            .get()?
            .head()?
            .referent_name()
            .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch))
            .context("single-branch branch creation requires HEAD to be a local branch")?
            .to_owned();
        let new_ref: gix::refs::FullName = format!("refs/heads/{branch_name}").try_into()?;
        but_api::branch::branch_create_with_perm(
            ctx,
            Some(new_ref),
            but_api::branch::json::BranchCreatePlacement::Dependent {
                relative_to: but_api::commit::json::RelativeTo::Reference(head_name),
                side: but_rebase::graph_rebase::mutate::InsertSide::Above,
            },
            guard.write_permission(),
        )?;
        write_new_branch_output(out, &branch_name, None, anchor_arg)?;
        return Ok(());
    }

    let anchor = resolved_anchor
        .clone()
        .map(|anchor| -> CliResult<_> {
            match anchor.into_branch_or_commit()? {
                BranchOrCommit::Commit(commit) => {
                    Ok(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                        commit_id: HexHash(commit.commit_id),
                        position: but_workspace::branch::create_reference::Position::Above,
                    })
                }
                BranchOrCommit::Branch(BranchArg(name)) => Ok(
                    but_api::legacy::stack::create_reference::Anchor::AtSegment {
                        short_name: name.clone(),
                        position: but_workspace::branch::create_reference::Position::Above,
                    },
                ),
            }
        })
        .transpose()?;

    but_api::legacy::stack::create_reference_with_perm(
        ctx,
        but_api::legacy::stack::create_reference::Request {
            new_name: branch_name.clone(),
            anchor,
        },
        guard.write_permission(),
    )?;

    write_new_branch_output(out, &branch_name, resolved_anchor.as_ref(), anchor_arg)?;

    Ok(())
}

fn write_new_branch_output(
    out: &mut OutputChannel,
    branch_name: &str,
    resolved_anchor: Option<&ResolvedCliIdArg>,
    anchor_arg: Option<CliIdArg>,
) -> CliResult<()> {
    let t = theme::get();
    if let Some(out) = out.for_human() {
        if let Some(resolved_anchor) = resolved_anchor {
            writeln!(
                out,
                "{} Created branch {} stacked on {}",
                t.sym().success,
                t.local_branch.paint(branch_name),
                t.hint.paint(format!("{resolved_anchor}")),
            )?;
        } else {
            writeln!(
                out,
                "{} Created branch {}",
                t.sym().success,
                t.local_branch.paint(branch_name),
            )?;
        }
    } else if let Some(out) = out.for_json() {
        let value = json::BranchNewOutput {
            branch: branch_name.to_owned(),
            anchor: anchor_arg,
        };
        out.write_value(value)?;
    }

    Ok(())
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
