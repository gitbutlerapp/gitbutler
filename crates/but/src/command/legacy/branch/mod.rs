use anyhow::bail;
use branch::Subcommands;
use but_core::ref_metadata::StackId;
use colored::Colorize;

use crate::{
    CliId, IdMap,
    args::branch,
    utils::{Confirm, ConfirmDefault, OutputChannel},
};

pub mod apply;
mod json;
mod list;
mod show;

pub fn handle(cmd: Option<Subcommands>, ctx: &mut but_ctx::Context, out: &mut OutputChannel) -> anyhow::Result<()> {
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
            list::list(ctx, local, remote, all, ahead, review, filter, out, check)?;
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
        Some(Subcommands::New { branch_name, anchor }) => {
            let id_map = IdMap::new_from_context(ctx, None)?;
            // Get branch name or use canned name
            let branch_name = branch_name
                .map(Ok)
                .unwrap_or_else(|| but_api::legacy::workspace::canned_branch_name(ctx))?;

            // Store anchor string for JSON output
            let anchor_for_json = anchor.clone();

            let anchor = if let Some(anchor_str) = anchor {
                // Use the new create_reference API when anchor is provided

                // Resolve the anchor string to a CliId
                let anchor_ids = id_map.parse_using_context(&anchor_str, ctx)?;
                if anchor_ids.is_empty() {
                    return Err(anyhow::anyhow!("Could not find anchor: {anchor_str}"));
                }
                if anchor_ids.len() > 1 {
                    return Err(anyhow::anyhow!(
                        "Ambiguous anchor '{anchor_str}', matches multiple items"
                    ));
                }
                let anchor_id = &anchor_ids[0];

                // Create the anchor for create_reference
                // as dependent branch
                match anchor_id {
                    CliId::Commit { commit_id: oid, .. } => {
                        Some(but_api::legacy::stack::create_reference::Anchor::AtCommit {
                            commit_id: (*oid),
                            position: but_workspace::branch::create_reference::Position::Above,
                        })
                    }
                    CliId::Branch { name, .. } => Some(but_api::legacy::stack::create_reference::Anchor::AtReference {
                        short_name: name.clone(),
                        position: but_workspace::branch::create_reference::Position::Above,
                    }),
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

            let anchor_display = anchor.as_ref().map(|anchor_ref| match anchor_ref {
                but_api::legacy::stack::create_reference::Anchor::AtReference { short_name, .. } => short_name.clone(),
                but_api::legacy::stack::create_reference::Anchor::AtCommit { commit_id, .. } => {
                    commit_id.to_string()[..7].to_string()
                }
            });

            but_api::legacy::stack::create_reference(
                ctx,
                but_api::legacy::stack::create_reference::Request {
                    new_name: branch_name.clone(),
                    anchor,
                },
            )?;

            if let Some(out) = out.for_human() {
                if let Some(anchor_name) = anchor_display {
                    writeln!(
                        out,
                        "{} {} stacked on {}",
                        "✓ Created branch".green(),
                        branch_name.yellow(),
                        anchor_name.dimmed()
                    )?;
                } else {
                    writeln!(out, "{} {}", "✓ Created branch".green(), branch_name.yellow())?;
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
        Some(Subcommands::Delete { branch_name, force }) => {
            let stacks =
                but_api::legacy::workspace::stacks(ctx, Some(but_workspace::legacy::StacksFilter::InWorkspace))?;

            // Find which stack this branch belongs to
            for stack_entry in &stacks {
                if stack_entry.heads.iter().all(|b| b.name != *branch_name) {
                    // Not found in this stack,
                    continue;
                }

                if let Some(sid) = stack_entry.id {
                    return confirm_branch_deletion(ctx, sid, &branch_name, force, out);
                }
            }

            if let Some(out) = out.for_human() {
                writeln!(out, "Branch '{branch_name}' not found in any stack")?;
            }
            Ok(())
        }
    }
}

fn confirm_branch_deletion(
    ctx: &mut but_ctx::Context,
    sid: StackId,
    branch_name: &str,
    force: bool,
    out: &mut OutputChannel,
) -> Result<(), anyhow::Error> {
    if !force
        && let Some(mut inout) = out.prepare_for_terminal_input()
        && inout.confirm(
            format!("Are you sure you want to delete branch '{branch_name}'?"),
            ConfirmDefault::No,
        )? == Confirm::No
    {
        bail!("Aborted branch deletion.");
    }

    but_api::legacy::stack::remove_branch(ctx, sid, branch_name.to_owned())?;

    if let Some(out) = out.for_human() {
        writeln!(out, "Deleted branch {branch_name}")?;
    }
    Ok(())
}
