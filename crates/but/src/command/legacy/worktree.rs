use std::path::PathBuf;

use anyhow::{Context as _, Result, bail};
use but_api::legacy::worktree::IntegrationStatus;
use but_ctx::Context;
use but_worktrees::WorktreeId;

use crate::{CliId, IdMap, args::worktree::Subcommands, utils::OutputChannel};
/// Parse a worktree identifier which can be either:
/// - A full path to the worktree
/// - Just the worktree name
///
/// Returns the WorktreeId.
fn parse_worktree_identifier(input: &str) -> Result<WorktreeId> {
    // If it's an absolute path or looks like a full path, extract the name from it
    let input_path = PathBuf::from(input);
    if input_path.is_absolute() || input_path.components().count() > 1 {
        return WorktreeId::from_path(&input_path);
    }

    // Otherwise treat it as just the worktree name
    Ok(WorktreeId::from_bstr(input))
}

pub fn handle(cmd: Subcommands, ctx: &mut Context, out: &mut OutputChannel) -> Result<()> {
    match cmd {
        Subcommands::New { reference } => {
            // Naivly append refs/heads/ if it's not present to always have a
            // full reference.
            let reference = if reference.starts_with("refs/heads/") {
                gix::refs::FullName::try_from(reference.clone())?
            } else {
                gix::refs::FullName::try_from(format!("refs/heads/{reference}"))?
            };
            let output = but_api::legacy::worktree::worktree_new(ctx, reference)?;
            if let Some(out) = out.for_json() {
                out.write_value(output)?;
            } else if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "Created worktree at: {}",
                    output.created.path.display()
                )?;
                if let Some(reference) = output.created.created_from_ref {
                    writeln!(out, "Reference: {reference}")?;
                }
            }
            Ok(())
        }
        Subcommands::List => {
            let output = but_api::legacy::worktree::worktree_list(ctx)?;
            if let Some(out) = out.for_json() {
                out.write_value(output)?;
            } else if let Some(out) = out.for_human() {
                if output.entries.is_empty() {
                    writeln!(out, "No worktrees found")?;
                } else {
                    for entry in &output.entries {
                        writeln!(out, "Path: {}", entry.path.display())?;
                        if let Some(reference) = &entry.created_from_ref {
                            writeln!(out, "Reference: {reference}")?;
                        }
                        if let Some(base) = entry.base {
                            writeln!(out, "Base: {base}")?;
                        }
                        writeln!(out)?;
                    }
                }
            }
            Ok(())
        }
        Subcommands::Integrate { path, target, dry } => {
            let id = parse_worktree_identifier(&path)?;

            // Determine the target reference
            let target_ref = if let Some(target_str) = target {
                // User specified a target - parse it
                if target_str.starts_with("refs/") {
                    gix::refs::FullName::try_from(target_str.clone())?
                } else {
                    // Assume it's a branch name and prepend refs/heads/
                    gix::refs::FullName::try_from(format!("refs/heads/{target_str}"))?
                }
            } else {
                // No target specified - get it from the worktree metadata
                // First, we need to get the worktree metadata to find what reference it was created from
                let worktree_list = but_api::legacy::worktree::worktree_list(ctx)?;
                let worktree_entry = worktree_list
                    .entries
                    .iter()
                    .find(|e| e.id == id)
                    .context("Worktree not found - ID does not match any known worktree")?;

                worktree_entry.created_from_ref.clone().context(
                    "Worktree does not have a created_from_ref - please specify --target",
                )?
            };

            if dry {
                // Dry run - check integration status
                let status = but_api::legacy::worktree::worktree_integration_status(
                    ctx,
                    id.clone(),
                    target_ref.clone(),
                )?;

                if let Some(out) = out.for_json() {
                    out.write_value(status)?;
                } else if let Some(out) = out.for_human() {
                    writeln!(out, "Integration status for worktree: {id}")?;
                    writeln!(out, "Target: {target_ref}")?;
                    match status {
                        IntegrationStatus::NoMergeBaseFound => {
                            writeln!(out, "Status: Cannot integrate - no merge base found")?;
                        }
                        IntegrationStatus::WorktreeIsBare => {
                            writeln!(out, "Status: Cannot integrate - worktree is bare")?;
                        }
                        IntegrationStatus::NothingToIntegrate => {
                            writeln!(
                                out,
                                "Status: Nothing to integrate - the worktree has no changes"
                            )?;
                        }
                        IntegrationStatus::CausesWorkspaceConflicts => {
                            writeln!(
                                out,
                                "Status: Cannot integrate - would cause workspace conflicts"
                            )?;
                        }
                        IntegrationStatus::Integratable {
                            cherry_pick_conflicts,
                            commits_above_conflict,
                            working_dir_conflicts,
                        } => {
                            writeln!(out, "Status: Integratable")?;
                            if cherry_pick_conflicts {
                                writeln!(out, "  Warning: Cherry-pick will have conflicts")?;
                            }
                            if commits_above_conflict {
                                writeln!(out, "  Warning: Commits above will have conflicts")?;
                            }
                            if working_dir_conflicts {
                                writeln!(out, "  Warning: Working directory will have conflicts")?;
                            }
                            if !cherry_pick_conflicts
                                && !commits_above_conflict
                                && !working_dir_conflicts
                            {
                                writeln!(out, "  No conflicts expected")?;
                            }
                        }
                    }
                }
            } else {
                // Actual integration
                but_api::legacy::worktree::worktree_integrate(ctx, id.clone(), target_ref.clone())?;

                if let Some(out) = out.for_json() {
                    out.write_value(serde_json::json!({"status": "success"}))?;
                } else if let Some(out) = out.for_human() {
                    writeln!(out, "Successfully integrated worktree: {id}")?;
                    writeln!(out, "Target: {target_ref}")?;
                }
            }

            Ok(())
        }
        Subcommands::Destroy { target, reference } => {
            if reference {
                // Treat target as a reference - parse it
                let reference = if target.starts_with("refs/") {
                    gix::refs::FullName::try_from(target.clone())?
                } else {
                    // Assume it's a branch name and prepend refs/heads/
                    gix::refs::FullName::try_from(format!("refs/heads/{target}"))?
                };

                let output = but_api::legacy::worktree::worktree_destroy_by_reference(
                    ctx,
                    reference.clone(),
                )?;

                if let Some(out) = out.for_json() {
                    out.write_value(output)?;
                } else if let Some(out) = out.for_human() {
                    if output.destroyed_ids.is_empty() {
                        writeln!(out, "No worktrees found for reference: {reference}")?;
                    } else {
                        writeln!(
                            out,
                            "Destroyed {} worktree(s) for reference: {}",
                            output.destroyed_ids.len(),
                            reference
                        )?;
                        for id in &output.destroyed_ids {
                            writeln!(out, "  - {id}")?;
                        }
                    }
                }
            } else {
                // Treat target as a path or worktree name
                let id = parse_worktree_identifier(&target)?;
                let output = but_api::legacy::worktree::worktree_destroy_by_id(ctx, id.clone())?;

                if let Some(out) = out.for_json() {
                    out.write_value(output)?;
                } else if let Some(out) = out.for_human() {
                    writeln!(out, "Destroyed worktree: {id}")?;
                }
            }

            Ok(())
        }
        Subcommands::Archive { id } => set_archived(ctx, out, &id, true),
        Subcommands::Unarchive { id } => set_archived(ctx, out, &id, false),
    }
}

/// Resolve `id` - a worktree CLI id or a worktree name - and persist its archived state.
fn set_archived(
    ctx: &mut Context,
    out: &mut OutputChannel,
    id: &str,
    archived: bool,
) -> Result<()> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        bail!("worktree manipulation is not enabled (featureFlags.worktreeManipulation)");
    }
    let name = {
        let id_map = IdMap::legacy_new_from_context(ctx, None)?;
        let mut matches: Vec<_> = id_map
            .parse_using_context(id, ctx)?
            .into_iter()
            .filter_map(|cli_id| match cli_id {
                CliId::Worktree { name, .. } => Some(name),
                _ => None,
            })
            .collect();
        match matches.len() {
            1 => matches.remove(0),
            0 => bail!("Could not find worktree: '{id}'. Run `but status` for applicable ids."),
            _ => bail!("Ambiguous worktree id '{id}', matches multiple worktrees"),
        }
    };
    but_api::worktrees::worktree_set_archived(ctx, name.to_string(), archived)?;

    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({ "name": name.to_string(), "archived": archived }))?;
    } else if let Some(out) = out.for_human() {
        if archived {
            writeln!(out, "Archived worktree: {name}")?;
        } else {
            writeln!(out, "Unarchived worktree: {name}")?;
        }
    }
    Ok(())
}
