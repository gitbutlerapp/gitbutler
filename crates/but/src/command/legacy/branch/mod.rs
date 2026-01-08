use anyhow::{Context, bail};
use branch::Subcommands;
use but_core::ref_metadata::StackId;
use but_ctx::LegacyProject;
use but_workspace::legacy::ui::StackEntry;

use crate::{CliId, IdMap, args::branch, utils::OutputChannel};

mod apply;
mod json;
mod list;
mod show;

pub fn handle(
    cmd: Option<Subcommands>,
    ctx: &mut but_ctx::Context,
    out: &mut OutputChannel,
) -> anyhow::Result<()> {
    match cmd {
        None => handle(
            Some(Subcommands::List {
                filter: None,
                local: false,
                remote: false,
                all: false,
                no_ahead: false,
                review: false,
                no_check: false,
            }),
            ctx,
            out,
        ),
        Some(Subcommands::List {
            filter,
            local,
            remote,
            all,
            no_ahead,
            review,
            no_check,
        }) => {
            let ahead = !no_ahead; // Invert the flag
            let check = !no_check; // Invert the flag
            list::list(
                &ctx.legacy_project,
                local,
                remote,
                all,
                ahead,
                review,
                filter,
                out,
                check,
            )?;
            Ok(())
        }
        Some(Subcommands::Show {
            branch_id,
            review,
            files,
            ai,
            check,
        }) => {
            show::show(ctx, &branch_id, out, review, files, ai, check)?;
            Ok(())
        }
        Some(Subcommands::New {
            branch_name,
            anchor,
        }) => {
            let mut id_map = IdMap::new_from_context(ctx, None)?;
            id_map.add_committed_file_info_from_context(ctx)?;
            // Get branch name or use canned name
            let branch_name = branch_name.map(Ok).unwrap_or_else(|| {
                but_api::legacy::workspace::canned_branch_name(ctx.legacy_project.id)
            })?;

            // Store anchor string for JSON output
            let anchor_for_json = anchor.clone();

            let anchor = if let Some(anchor_str) = anchor {
                // Use the new create_reference API when anchor is provided

                // Resolve the anchor string to a CliId
                let anchor_ids = id_map.resolve_entity_to_ids(&anchor_str)?;
                if anchor_ids.is_empty() {
                    return Err(anyhow::anyhow!("Could not find anchor: {}", anchor_str));
                }
                if anchor_ids.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Ambiguous anchor '{}', matches multiple items",
                        anchor_str
                    ));
                }
                let anchor_id = &anchor_ids[0];

                // Create the anchor for create_reference
                // as dependent branch
                match anchor_id {
                    CliId::Commit { commit_id: oid, .. } => {
                        Some(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                            commit_id: (*oid).into(),
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
                        return Err(anyhow::anyhow!(
                            "Invalid anchor type: {}, expected commit or branch",
                            anchor_id.kind_for_humans()
                        ));
                    }
                }
            } else {
                // Create an independent branch
                None
            };
            but_api::legacy::stack::create_reference(
                ctx.legacy_project.id,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                },
            )?;

            if let Some(out) = out.for_human() {
                writeln!(out, "Created branch {branch_name}")?;
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
        Some(Subcommands::Delete { branch_name, force }) => {
            let stacks = but_api::legacy::workspace::stacks(
                ctx.legacy_project.id,
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
                        &ctx.legacy_project,
                        sid,
                        &branch_name,
                        force,
                        out,
                    );
                }
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "Branch '{}' not found in any stack", branch_name)?;
            }
            Ok(())
        }
        Some(Subcommands::Apply { branch_name }) => {
            apply::apply(&ctx.legacy_project, &branch_name, out)
        }
        Some(Subcommands::Unapply { branch_name, force }) => {
            let stacks = but_api::legacy::workspace::stacks(
                ctx.legacy_project.id,
                Some(but_workspace::legacy::StacksFilter::InWorkspace),
            )?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_unapply_stack(
                        &ctx.legacy_project,
                        sid,
                        stack_entry,
                        force,
                        out,
                    );
                }
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "Branch '{}' not found in any stack", branch_name)?;
            }
            Ok(())
        }
    }
}

fn confirm_unapply_stack(
    project: &LegacyProject,
    sid: StackId,
    stack_entry: &StackEntry,
    force: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    let branches = stack_entry
        .heads
        .iter()
        .map(|head| head.name.to_string())
        .collect::<Vec<_>>()
        .join(", ");

    if !force && let Some(mut inout) = out.prepare_for_terminal_input() {
        let abort_msg = "Aborted unapply operation.";
        let input = inout
            .prompt(format!(
                "Are you sure you want to unapply stack with branches '{branches}'? [y/N]:"
            ))?
            .context(abort_msg)?
            .to_lowercase();

        if input != "y" && input != "yes" {
            bail!(abort_msg);
        }
    }

    if force {
        but_api::legacy::virtual_branches::unapply_stack(project.id, sid)?;
    } else {
        bail!("Refusing to unapply stack without --force");
    }

    if let Some(out) = out.for_human() {
        writeln!(
            out,
            "Unapplied stack with branches '{}' from workspace",
            branches
        )?;
    }
    Ok(())
}

fn confirm_branch_deletion(
    project: &LegacyProject,
    sid: StackId,
    branch_name: &str,
    force: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    if !force && let Some(mut inout) = out.prepare_for_terminal_input() {
        let abort_msg = "Aborted branch deletion.";
        let input = inout
            .prompt(format!(
                "Are you sure you want to delete branch '{}'? [y/N]:",
                branch_name
            ))?
            .context(abort_msg)?
            .to_lowercase();

        if input != "y" && input != "yes" {
            bail!(abort_msg);
        }
    }

    if force {
        but_api::legacy::stack::remove_branch(project.id, sid, branch_name.to_owned())?;
    } else {
        bail!(
            "Refusing to remove branch '{}' from workspace without --force",
            branch_name
        );
    }

    if let Some(out) = out.for_human() {
        writeln!(out, "Deleted branch {branch_name}")?;
    }
    Ok(())
}
