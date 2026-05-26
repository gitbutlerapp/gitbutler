use anyhow::bail;
use bstr::BStr;
use but_api::json::HexHash;
use but_core::{ref_metadata::StackId, sync::RepoExclusive};

use crate::{
    CliError, CliId, CliResult, IdMap, bad_input,
    theme::{self, Paint},
    utils::{Confirm, ConfirmDefault, OutputChannel, shorten_object_id},
};

mod json;
mod list;
mod show;

pub fn delete(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_name: String,
    force: bool,
) -> CliResult<()> {
    let mut guard = ctx.exclusive_worktree_access();
    let stacks = but_api::legacy::workspace::stacks(
        ctx,
        Some(but_workspace::legacy::StacksFilter::InWorkspace),
    )?;

    // Find which stack this branch belongs to
    for stack_entry in &stacks {
        if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
            // Not found in this stack,
            continue;
        }

        if let Some(sid) = stack_entry.id {
            return confirm_branch_deletion(
                ctx,
                sid,
                &branch_name,
                force,
                out,
                guard.write_permission(),
            )
            .map_err(CliError::from);
        }
    }

    Err(bad_input(format!("Branch '{branch_name}' not found in any stack")).into())
}

pub fn new(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_name: Option<String>,
    anchor: Option<String>,
) -> crate::CliResult<()> {
    let t = theme::get();

    let mut guard = ctx.exclusive_worktree_access();
    let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
    // Get branch name or use canned name
    let branch_name = if let Some(branch_name) = branch_name {
        let repo = ctx.repo.get()?;
        check_can_create_branch_with_user_provided_name(&repo, &branch_name)?;
        branch_name
    } else {
        but_api::legacy::workspace::canned_branch_name(ctx)?
    };

    // Store anchor string for JSON output
    let anchor_for_json = anchor.clone();

    let anchor = if let Some(anchor_str) = anchor {
        // Use the new create_reference API when anchor is provided

        // Resolve the anchor string to a CliId
        let anchor_ids = id_map.parse_using_context(&anchor_str, ctx)?;
        if anchor_ids.is_empty() {
            return Err(bad_input(format!("Could not find anchor: {anchor_str}")).into());
        }
        if anchor_ids.len() > 1 {
            return Err(bad_input(format!(
                "Ambiguous anchor '{anchor_str}', matches multiple items"
            ))
            .into());
        }
        let anchor_id = &anchor_ids[0];

        // Create the anchor for create_reference
        // as dependent branch
        match anchor_id {
            CliId::Commit { commit_id: oid, .. } => {
                Some(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                    commit_id: HexHash(*oid),
                    position: but_workspace::branch::create_reference::Position::Above,
                })
            }
            CliId::Branch { name, .. } => Some(
                but_api::legacy::stack::create_reference::Anchor::AtReference {
                    short_name: name.clone(),
                    position: but_workspace::branch::create_reference::Position::Above,
                },
            ),
            _ => {
                return Err(bad_input(format!(
                    "Invalid anchor type: {}, expected commit or branch",
                    anchor_id.kind_for_humans()
                ))
                .into());
            }
        }
    } else {
        // Create an independent branch
        None
    };

    let anchor_display = {
        let repo = ctx.repo.get()?;
        anchor.as_ref().map(|anchor_ref| match anchor_ref {
            but_api::legacy::stack::create_reference::Anchor::AtReference {
                short_name, ..
            } => short_name.clone(),
            but_api::legacy::stack::create_reference::Anchor::AtCommit { commit_id, .. } => {
                shorten_object_id(&repo, commit_id.0)
            }
        })
    };

    but_api::legacy::stack::create_reference_with_perm(
        ctx,
        but_api::legacy::stack::create_reference::Request {
            new_name: branch_name.clone(),
            anchor,
        },
        guard.write_permission(),
    )?;

    if let Some(out) = out.for_human() {
        if let Some(anchor_name) = anchor_display {
            writeln!(
                out,
                "{} Created branch {} stacked on {}",
                t.sym().success,
                t.local_branch.paint(branch_name),
                t.hint.paint(anchor_name),
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
            anchor: anchor_for_json,
        };
        out.write_value(value)?;
    }
    Ok(())
}

/// Validate that `user_provided_branch_name` is a valid branch name that does not already exist.
///
/// Unlike the GUI, we don't normalize branch names for users in the CLI, as this could lead to
/// unexpected behavior in scripts. This function rejects names that are possible to normalize.
fn check_can_create_branch_with_user_provided_name(
    repo: &gix::Repository,
    user_provided_branch_name: &str,
) -> Result<(), CliError> {
    let normalized =
        but_core::branch::normalize_short_name(user_provided_branch_name).map_err(|err| {
            CliError::from(
                bad_input(format!("Invalid branch name: {err}"))
                    .arg_name("<BRANCH_NAME>")
                    .arg_value(user_provided_branch_name),
            )
        })?;

    let user_name_bstr: &BStr = user_provided_branch_name.into();
    if normalized != user_name_bstr {
        return Err(bad_input("Invalid branch name")
            .arg_name("<BRANCH_NAME>")
            .arg_value(user_provided_branch_name)
            .hint(format!("Try '{normalized}' instead"))
            .into());
    }

    let branch_ref_name = if user_provided_branch_name.starts_with("refs/heads") {
        user_provided_branch_name.to_string()
    } else {
        format!("refs/heads/{user_provided_branch_name}")
    };

    if repo
        .try_find_reference(&branch_ref_name.to_owned())?
        .is_some()
    {
        return Err(bad_input(format!(
            "A branch named '{user_provided_branch_name}' already exists"
        ))
        .into());
    }

    Ok(())
}

pub fn show_branches(
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
    branch_id: String,
    review: bool,
    files: bool,
    ai: bool,
    check: bool,
) -> Result<(), anyhow::Error> {
    show::show(ctx, &branch_id, out, review, files, ai, check)?;
    Ok(())
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

fn confirm_branch_deletion(
    ctx: &mut but_ctx::Context,
    sid: StackId,
    branch_name: &str,
    force: bool,
    out: &mut OutputChannel,
    perm: &mut RepoExclusive,
) -> Result<(), anyhow::Error> {
    let t = theme::get();
    if !force
        && let Some(mut inout) = out.prepare_for_terminal_input()
        && inout.confirm(
            format!(
                "Are you sure you want to delete branch {}?",
                t.local_branch.paint(branch_name)
            ),
            ConfirmDefault::No,
        )? == Confirm::No
    {
        bail!("Aborted branch deletion.");
    }

    but_api::legacy::stack::remove_branch_with_perm(ctx, sid, branch_name.to_owned(), perm)?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Deleted branch {}", t.local_branch.paint(branch_name))?;
    }
    Ok(())
}
