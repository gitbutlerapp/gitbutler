use anyhow::Context as _;
use but_api::json::HexHash;
use but_core::DryRun;
use gitbutler_oplog::entry::{OperationKind, SnapshotDetails};

use crate::{
    CliResult, IdMap,
    args::atoms::{BranchArg, BranchOrCommit, CliIdArg, Purpose},
    theme::{self, Paint},
    utils::OutputChannel,
};

mod json;
mod list;
mod show;

pub fn delete(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_arg: CliIdArg,
) -> CliResult<()> {
    let t = theme::get();

    let branch_arg = {
        let guard = ctx.exclusive_worktree_access();
        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        branch_arg.resolve_branch_in_workspace(ctx, &id_map)?
    };

    let segment = branch_arg.resolve_segment(ctx)?;

    let ref_name = &segment
        .ref_info
        .context("segment missing ref_info")?
        .ref_name;

    let mut meta = ctx.meta()?;
    let snapshot_details = SnapshotDetails::new(OperationKind::DeleteBranch);
    but_transaction::with_transaction(ctx, &mut meta, snapshot_details, DryRun::No, |mut tx| {
        tx.remove_reference(ref_name.as_ref())?;
        if !segment.commits.is_empty() {
            tx.discard_commits(segment.commits.iter().map(|commit| commit.id))?;
        }
        Ok(())
    })?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Deleted branch {}", t.local_branch.paint(&branch_arg))?;
    }

    Ok(())
}

pub fn new(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_name_arg: Option<BranchArg>,
    anchor_arg: Option<CliIdArg>,
) -> CliResult<()> {
    let t = theme::get();

    let branch_name = if let Some(branch_name_arg) = branch_name_arg {
        branch_name_arg.resolve_for_creation(&*ctx.repo.get()?)?
    } else {
        but_api::legacy::workspace::canned_branch_name(ctx)?
    };

    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;

    let resolved_anchor = anchor_arg
        .clone()
        .and_then(|anchor| {
            anchor
                .try_resolve(ctx, &id_map, Purpose::Anchor, None)
                .transpose()
        })
        .transpose()?;

    let anchor = resolved_anchor
        .clone()
        .map(|anchor| -> CliResult<_> {
            match anchor.into_branch_or_commit()? {
                BranchOrCommit::Commit(commit) => {
                    Ok(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                        commit_id: HexHash(commit),
                        position: but_workspace::branch::create_reference::Position::Above,
                    })
                }
                BranchOrCommit::Branch(BranchArg(name)) => Ok(
                    but_api::legacy::stack::create_reference::Anchor::AtReference {
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
    } else if let Some(out) = out.for_shell() {
        writeln!(out, "{branch_name}")?;
    } else if let Some(out) = out.for_json() {
        let value = json::BranchNewOutput {
            branch: branch_name.clone(),
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
