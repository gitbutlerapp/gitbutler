use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result, bail};
use bstr::ByteSlice as _;
use but_api::legacy::worktree::IntegrationStatus;
use but_ctx::Context;
use but_worktrees::WorktreeId;

use crate::{
    CliId, IdMap,
    args::worktree::Subcommands,
    theme::Paint as _,
    utils::{OutputChannel, rejection},
};

/// Return the active linked worktree rooted at `workdir`.
pub(crate) fn active_worktree_name_at(ctx: &Context, workdir: &Path) -> Result<gix::bstr::BString> {
    let workdir = workdir.canonicalize()?;
    let worktree = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| {
            worktree
                .path
                .canonicalize()
                .is_ok_and(|path| path == workdir)
        })
        .context("The current linked worktree is not managed by this project")?;
    if worktree.archived {
        bail!(
            "Worktree {} is archived; unarchive it before committing",
            worktree.tip.name
        );
    }
    Ok(worktree.tip.name)
}

/// Commit selected changes, or every current change, in the active linked worktree `name`.
pub(crate) fn commit(
    ctx: &mut Context,
    out: &mut OutputChannel,
    name: gix::bstr::BString,
    message: &str,
    no_hooks: bool,
    selected_changes: &[String],
) -> Result<()> {
    if message.trim().is_empty() {
        bail!("Aborting commit due to empty commit message.");
    }
    let branch_name = ctx
        .worktrees_with_state()?
        .into_iter()
        .find(|worktree| worktree.tip.name == name)
        .context("The active linked worktree no longer exists")?
        .tip
        .ref_name
        .context("Cannot commit from a linked worktree with a detached HEAD")?
        .shorten()
        .to_string();
    let changes: Vec<but_core::DiffSpec> = if selected_changes.is_empty() {
        but_api::worktrees::linked_worktree_changes(ctx, name.to_string())?
            .changes
            .into_iter()
            .map(|change| but_core::DiffSpec::from(but_core::TreeChange::from(change)))
            .collect()
    } else {
        let id_map = IdMap::legacy_new_from_context(ctx, None)?;
        let mut changes = Vec::with_capacity(selected_changes.len());
        for selected in selected_changes {
            let mut matches = id_map.parse_using_context(selected, ctx)?;
            if matches.len() != 1 {
                bail!("Expected one linked-worktree change for '{selected}'");
            }
            let CliId::WorktreeChange(change) = matches.remove(0) else {
                bail!("'{selected}' is not a linked-worktree file or hunk");
            };
            if change.name != name {
                bail!(
                    "'{selected}' belongs to worktree {}, not {name}",
                    change.name
                );
            }
            let repo = ctx.repo.get()?;
            changes.push(diff_spec_for_change(&repo, &change)?);
        }
        changes
    };
    let changes = but_workspace::flatten_diff_specs(changes);
    if changes.is_empty() {
        bail!("No changes to commit.");
    }
    let hook_ctx = if no_hooks {
        None
    } else {
        let repo = ctx.repo.get()?;
        let worktree_repo = but_workspace::worktrees::open_worktree_repo(&repo, name.as_ref())?;
        drop(repo);
        let mut hook_ctx = Context::from_repo_with_settings(worktree_repo, ctx.settings.clone())?;
        hook_ctx.legacy_project = ctx.legacy_project.clone();
        Some(hook_ctx)
    };
    let message = if let Some(hook_ctx) = hook_ctx.as_ref() {
        super::commit::run_pre_commit_hook(hook_ctx, &changes)?;
        super::commit::run_message_hook(hook_ctx, message.to_owned())?
    } else {
        message.to_owned()
    };
    let result = but_api::worktrees::worktree_commit_create(
        ctx,
        name.to_string(),
        changes,
        message,
        but_core::DryRun::No,
    )?;
    let rejected: Vec<_> = result
        .rejected_specs
        .iter()
        .map(|(reason, spec)| rejection::RejectedChange {
            path: spec.path.clone(),
            reason: *reason,
            dependencies: Vec::new(),
        })
        .collect();
    if !rejected.is_empty() {
        tracing::warn!(
            rejected_specs = ?result.rejected_specs,
            "Failed to commit at least one linked-worktree change"
        );
    }
    let commit = result.new_commit.context("No changes could be committed")?;
    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "commit_id": commit.to_string(),
            "branch": &branch_name,
            "worktree": name.to_string(),
            "rejected": serde_json::to_value(&rejected).unwrap_or_default(),
        }))?;
    } else if let Some(out) = out.for_human() {
        let repo = ctx.repo.get()?;
        let t = crate::theme::get();
        writeln!(
            out,
            "{} Created commit {} on branch {}",
            t.sym().success,
            t.commit_id
                .paint(crate::utils::shorten_object_id(&repo, commit)),
            t.local_branch.paint(&branch_name),
        )?;
        rejection::write_rejection_report(out, &rejected, Some(&branch_name))?;
    }
    if let Some(hook_ctx) = hook_ctx.as_ref() {
        super::commit::run_post_commit_hook(hook_ctx, out)?;
    }
    Ok(())
}

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
        Subcommands::Amend {
            name,
            commit,
            changes,
        } => amend(ctx, out, &name, &commit, changes),
    }
}

/// Resolve `id` - a worktree CLI id or a worktree name - to the worktree name,
/// using the given `id_map`.
fn resolve_worktree_name(ctx: &Context, id_map: &IdMap, id: &str) -> Result<gix::bstr::BString> {
    let mut matches: Vec<_> = id_map
        .parse_using_context(id, ctx)?
        .into_iter()
        .filter_map(|cli_id| match cli_id {
            CliId::Worktree { name, .. } => Some(name),
            _ => None,
        })
        .collect();
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("Could not find worktree: '{id}'. Run `but status` for applicable ids."),
        _ => bail!("Ambiguous worktree id '{id}', matches multiple worktrees"),
    }
}

/// Resolve `id` - a commit CLI id or a (partial) commit hash - to an object id,
/// using the given `id_map`.
fn resolve_commit_id(ctx: &Context, id_map: &IdMap, id: &str) -> Result<gix::ObjectId> {
    let mut matches: Vec<_> = id_map
        .parse_using_context(id, ctx)?
        .into_iter()
        .filter_map(|cli_id| match cli_id {
            CliId::Commit { commit_id, .. } => Some(commit_id),
            _ => None,
        })
        .collect();
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => {
            // Commits that live only on a worktree's branch aren't part of the
            // workspace projection the id map is built from, yet they are valid
            // amend targets - fall back to git revision parsing (full/short
            // SHA, refs), like `but pick` does.
            let repo = ctx.repo.get()?;
            if let Ok(oid) = repo.rev_parse_single(id) {
                let object_id = oid.detach();
                if repo.find_commit(object_id).is_ok() {
                    return Ok(object_id);
                }
            }
            bail!("Could not find commit: '{id}'. Run `but status` for applicable ids.")
        }
        _ => bail!("Ambiguous commit id '{id}', matches multiple commits"),
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
        resolve_worktree_name(ctx, &id_map, id)?
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

/// Amend uncommitted changes of the worktree `id` into the commit resolved from
/// `commit`, moving them out of the worktree.
fn amend(
    ctx: &mut Context,
    out: &mut OutputChannel,
    id: &str,
    commit: &str,
    changes: Vec<String>,
) -> Result<()> {
    if !ctx.settings.feature_flags.worktree_manipulation {
        bail!("worktree manipulation is not enabled (featureFlags.worktreeManipulation)");
    }
    let (name, commit_id) = {
        let id_map = IdMap::legacy_new_from_context(ctx, None)?;
        (
            resolve_worktree_name(ctx, &id_map, id)?,
            resolve_commit_id(ctx, &id_map, commit)?,
        )
    };

    let result = amend_changes(ctx, name.as_ref(), commit_id, &changes)?;

    let rejected_paths: Vec<String> = result
        .rejected_specs
        .iter()
        .map(|(_, spec)| spec.path.to_string())
        .collect();
    if let Some(out) = out.for_json() {
        out.write_value(serde_json::json!({
            "name": name.to_string(),
            "newCommitId": result.new_commit.map(|id| id.to_string()),
            "rejectedPaths": rejected_paths,
        }))?;
    } else if let Some(out) = out.for_human() {
        match result.new_commit {
            Some(new_commit) => {
                let repo = ctx.repo.get()?;
                writeln!(
                    out,
                    "Amended changes from worktree {name} into {}",
                    crate::utils::shorten_object_id(&repo, new_commit)
                )?;
                writeln!(
                    out,
                    "The amended changes were moved out of the worktree (files whose content changed mid-operation are left in place)."
                )?;
            }
            None => writeln!(out, "No changes could be amended.")?,
        }
        for path in &rejected_paths {
            writeln!(out, "  rejected: {path}")?;
        }
    }
    Ok(())
}

/// Amend whole-file changes from `name` into `commit_id`.
pub(crate) fn amend_changes(
    ctx: &mut Context,
    name: &gix::bstr::BStr,
    commit_id: gix::ObjectId,
    changes: &[String],
) -> Result<but_api::commit::types::CommitCreateResult> {
    // Build whole-file specs strictly from the worktree's actual changes - a
    // spec for an unchanged path would commit the worktree's `HEAD` rendition
    // of that file instead of failing.
    let all_specs: Vec<but_core::DiffSpec> =
        but_api::worktrees::linked_worktree_changes(ctx, name.to_string())?
            .changes
            .into_iter()
            .map(|change| but_core::DiffSpec::from(but_core::TreeChange::from(change)))
            .collect();
    let specs = if changes.is_empty() {
        all_specs
    } else {
        let mut seen = std::collections::BTreeSet::new();
        changes
            .iter()
            .filter(|path| seen.insert(path.as_str()))
            .map(|path| {
                all_specs
                    .iter()
                    .find(|spec| spec.path == path.as_str())
                    .cloned()
                    .with_context(|| {
                        format!("Worktree {name} has no uncommitted change at path '{path}'")
                    })
            })
            .collect::<Result<Vec<_>>>()?
    };
    if specs.is_empty() {
        bail!("Worktree {name} has no uncommitted changes");
    }

    but_api::worktrees::worktree_commit_amend(
        ctx,
        name.to_string(),
        commit_id,
        specs,
        but_core::DryRun::No,
    )
}

/// Resolve a linked-worktree file or hunk ID against its current checkout.
pub(crate) fn diff_spec_for_change(
    repo: &gix::Repository,
    change: &crate::id::WorktreeChange,
) -> Result<but_core::DiffSpec> {
    let wt_repo = but_workspace::worktrees::open_worktree_repo(&repo, change.name.as_bstr())?;
    let tree_change = but_core::diff::worktree_changes(&wt_repo)?
        .changes
        .into_iter()
        .find(|candidate| candidate.path == change.path)
        .with_context(|| {
            format!(
                "Worktree {} has no uncommitted change at path '{}'",
                change.name, change.path
            )
        })?;
    let mut spec = but_core::DiffSpec::from(tree_change);
    if let Some(header) = change.hunk_header {
        spec.hunk_headers = vec![header];
    }
    Ok(spec)
}
