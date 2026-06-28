//! `but land <branch>`: land a branch directly onto the target ref (the "avoid pull requests"
//! workflow). Inside a managed workspace this fast-forwards (or merges) the branch onto the
//! configured target — pushing to the real remote, or moving the local refs for a self-remote
//! (`gb-local`) — and then reconciles the remaining applied branches via the upstream-integration
//! flow that `but pull` uses.
//!
//! The work is split across submodules: [`merge`] decides the topology and builds the (signed)
//! merge commit, [`deliver`] pushes or moves the refs, [`reconcile`] updates the remaining
//! branches, and [`messaging`] holds the confirmation and end-of-command reporting. This module
//! orchestrates them.

mod deliver;
mod merge;
mod messaging;
mod reconcile;

use std::{fmt::Write, path::Path};

use anyhow::bail;
use but_ctx::Context;

use crate::{
    CliId, IdMap,
    theme::{self, Paint},
    utils::{OutputChannel, shorten_object_id},
};

use merge::LandOutcome;

/// How many times we re-fetch and re-merge when the target moved underneath us before giving up.
const MAX_PUSH_ATTEMPTS: usize = 5;

/// What `but land` ended up doing, used to drive honest end-of-command messaging.
enum Landed {
    /// The branch was already reachable from the target; nothing was pushed or moved.
    AlreadyIntegrated,
    /// The target advanced to `new_target_oid` (a fast-forward to the branch tip, or a merge commit).
    Updated {
        new_target_oid: gix::ObjectId,
        prev_target_oid: gix::ObjectId,
    },
}

pub async fn handle(
    ctx: &mut Context,
    out: &mut OutputChannel,
    branch_id: &str,
    yes: bool,
    no_ff: bool,
) -> anyhow::Result<()> {
    let t = theme::get();

    // Resolve the branch and read the target configuration. The managed-workspace guard runs here,
    // before anything is mutated, because the reconcile step only exists for a managed workspace.
    let (branch_name, base_branch) = {
        let mut guard = ctx.exclusive_worktree_access();

        {
            let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
            if !ws.kind.has_managed_ref() {
                bail!(
                    "`but land` requires an active GitButler workspace (`gitbutler/workspace`). \
                     Switch into the workspace and try again."
                );
            }
        }

        let id_map = IdMap::new_from_context(ctx, None, guard.read_permission())?;
        let resolved_ids = id_map.parse_using_context(branch_id, ctx)?;
        if resolved_ids.is_empty() {
            bail!("Could not find branch: {branch_id}");
        }
        if resolved_ids.len() > 1 {
            bail!("Ambiguous branch '{branch_id}', matches multiple items");
        }

        let cli_id = &resolved_ids[0];
        let branch_name = match cli_id {
            CliId::Branch { name, .. } => name.clone(),
            _ => bail!("Expected a branch ID, got {}", cli_id.kind_for_humans()),
        };

        let base_branch =
            but_api::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
                .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?;
        (branch_name, base_branch)
    };

    let target_branch_name = base_branch.short_name.clone();
    if target_branch_name.is_empty() {
        bail!("Configured target branch has no branch name");
    }
    let fetch_remote_name = base_branch.remote_name.clone();
    let push_remote_name = if base_branch.push_remote_name.is_empty() {
        fetch_remote_name.clone()
    } else {
        base_branch.push_remote_name.clone()
    };
    if push_remote_name.is_empty() {
        bail!("Configured target branch has no push remote");
    }

    // Triangular remotes (fetch remote != push remote) are out of scope for now: the post-land
    // reconcile reads the fetch remote's tracking ref, so a push to a different remote would not
    // advance it and the reconcile would silently no-op. Refuse before mutating anything.
    if push_remote_name != fetch_remote_name {
        bail!(
            "`but land` does not yet support triangular remotes (fetch `{fetch_remote_name}`, \
             push `{push_remote_name}`). Land via a pull request instead, or configure a single \
             remote for the target branch."
        );
    }

    let target_display = format!("{push_remote_name}/{target_branch_name}");
    let push_remote_url = if base_branch.push_remote_url.is_empty() {
        &base_branch.remote_url
    } else {
        &base_branch.push_remote_url
    };
    let update_target_locally = {
        let repo = ctx.repo.get()?;
        remote_points_at_current_repo(&repo, push_remote_url)?
    };

    let landing_info = messaging::branch_landing_info(ctx, &branch_name)?;
    // Refuse landing a non-bottom stack segment: the branch's tip carries every commit below it, so
    // landing it would silently publish the lower segments the user did not name. Land the bottom.
    if !landing_info.lower_segments.is_empty() {
        let bottom = landing_info
            .lower_segments
            .last()
            .expect("non-empty checked above");
        bail!(
            "Refusing to land `{branch_name}`: it is stacked on top of {} other segment(s) ({}) \
             whose commits would also be published to {target_display}. Land the bottom segment \
             `{bottom}` (or the whole stack) instead.",
            landing_info.lower_segments.len(),
            landing_info.lower_segments.join(", ")
        );
    }
    // Never publish a branch containing conflicted commits (GitButler conflict metadata) onto the
    // target — the same guard `but push` applies before sending commits to a remote.
    reject_conflicted_branch(ctx, &branch_name)?;

    messaging::confirm_direct_target_update(
        out,
        &branch_name,
        landing_info.pr_number,
        &target_display,
        update_target_locally,
        yes,
    )?;

    let mut progress = out.progress_channel();
    writeln!(
        progress,
        "Fetching newest data for target {}...",
        t.remote_branch.paint(&target_display)
    )?;
    but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("land".to_string()))?;

    // Land the branch, retrying when the target moves underneath us (optimistic concurrency).
    let mut landed: Option<Landed> = None;
    for attempt in 1..=MAX_PUSH_ATTEMPTS {
        // Recompute the merge decision against the freshly-fetched target on every attempt — a
        // branch can go from fast-forwardable to divergent between retries.
        let outcome = {
            let _guard = ctx.exclusive_worktree_access();
            let repo = ctx.repo.get()?;
            merge::decide_land_outcome(
                &repo,
                &branch_name,
                &fetch_remote_name,
                &target_branch_name,
                no_ff,
            )
        }?;

        let (new_target_oid, prev_target_oid, is_fast_forward) = match &outcome {
            LandOutcome::AlreadyIntegrated { target_oid } => {
                writeln!(
                    progress,
                    "{} is already reachable from {} ({})",
                    t.local_branch.paint(&branch_name),
                    t.remote_branch.paint(&target_display),
                    t.hint.paint(short_id(ctx, *target_oid)?)
                )?;
                landed = Some(Landed::AlreadyIntegrated);
                break;
            }
            LandOutcome::FastForward {
                feature_oid,
                target_oid,
            } => (*feature_oid, *target_oid, true),
            LandOutcome::Merge { oid, target_oid } => (*oid, *target_oid, false),
        };

        if update_target_locally {
            writeln!(
                progress,
                "Updating local target {} to {} {}...",
                t.remote_branch.paint(&target_display),
                if is_fast_forward {
                    "commit"
                } else {
                    "merge commit"
                },
                t.hint.paint(short_id(ctx, new_target_oid)?)
            )?;
        } else {
            writeln!(
                progress,
                "Pushing {} {} to {}...",
                if is_fast_forward {
                    "commit"
                } else {
                    "merge commit"
                },
                t.hint.paint(short_id(ctx, new_target_oid)?),
                t.remote_branch.paint(&target_display)
            )?;
        }

        let push_result = if update_target_locally {
            let repo = ctx.repo.get()?;
            deliver::update_local_target_refs(
                &repo,
                new_target_oid,
                prev_target_oid,
                &push_remote_name,
                &target_branch_name,
            )
        } else {
            deliver::push_to_target(ctx, new_target_oid, &push_remote_name, &target_branch_name)
        };

        match push_result {
            Ok(()) => {
                landed = Some(Landed::Updated {
                    new_target_oid,
                    prev_target_oid,
                });
                break;
            }
            Err(err)
                if deliver::is_retryable_concurrency_error(&err) && attempt < MAX_PUSH_ATTEMPTS =>
            {
                writeln!(
                    progress,
                    "Target moved while landing; fetching and retrying ({}/{})...",
                    attempt + 1,
                    MAX_PUSH_ATTEMPTS
                )?;
                but_api::legacy::virtual_branches::fetch_from_remotes(
                    ctx,
                    Some("land".to_string()),
                )?;
            }
            Err(err) if deliver::is_retryable_concurrency_error(&err) => {
                return Err(err.context(format!(
                    "Target branch kept moving; fetched and retried {MAX_PUSH_ATTEMPTS} times"
                )));
            }
            Err(err) if update_target_locally => {
                return Err(err.context("Failed to update local target branch"));
            }
            Err(err) => return Err(err.context("Failed to push to target branch")),
        }
    }

    let Some(landed) = landed else {
        // The loop only exits without a result when every attempt hit a retryable race; that path
        // already returned an error above, so this is unreachable in practice.
        bail!("Failed to land {branch_name} onto {target_display}");
    };

    // On the real-remote path, re-fetch so the tracking ref reflects the landed commit, then verify
    // it actually advanced before reconciling — otherwise the reconcile would be a silent no-op (for
    // example a fetch that didn't update the ref). This reads the ref directly, so it doesn't depend
    // on the workspace cache (invalidated below, before the reconcile's status read).
    if let Landed::Updated { new_target_oid, .. } = &landed
        && !update_target_locally
    {
        writeln!(
            progress,
            "Landed {} on {}. Fetching updated target...",
            t.hint.paint(short_id(ctx, *new_target_oid)?),
            t.remote_branch.paint(&target_display)
        )?;
        but_api::legacy::virtual_branches::fetch_from_remotes(ctx, Some("land".to_string()))?;

        // A concurrent push may have moved the tip *past* our commit, which still counts as the
        // target having advanced to include what we landed — so test reachability, not equality.
        let advanced = {
            let repo = ctx.repo.get()?;
            match peel_target_tip(&repo, &fetch_remote_name, &target_branch_name)? {
                Some(tip) => target_ref_contains(&repo, *new_target_oid, tip)?,
                None => false,
            }
        };
        if !advanced {
            if let Some(out) = out.for_human() {
                writeln!(
                    out,
                    "\n{}",
                    t.attention.paint(format!(
                        "Landed {} on {target_display}, but its tracking ref did not advance — \
                         skipping local reconcile. Run `but pull` once the fetch catches up.",
                        short_id(ctx, *new_target_oid)?
                    ))
                )?;
            }
            messaging::print_push_undo_caveat(
                out,
                &landed,
                &push_remote_name,
                &target_branch_name,
            )?;
            return Ok(());
        }
    }

    // Drop the cached workspace view so the reconcile's status read peels the freshly-fetched target.
    ctx.invalidate_workspace_cache()?;

    reconcile::reconcile_after_land(
        ctx,
        out,
        &branch_name,
        &target_display,
        update_target_locally,
        &landed,
        &push_remote_name,
        &target_branch_name,
    )
    .await
}

/// Peel a ref to its commit id, or `None` if it doesn't exist.
fn peel_ref(repo: &gix::Repository, name: &str) -> anyhow::Result<Option<gix::ObjectId>> {
    Ok(match repo.try_find_reference(name)? {
        Some(reference) => Some(reference.into_fully_peeled_id()?.detach()),
        None => None,
    })
}

/// Peel the fetch remote's target tracking ref to its commit, if it exists.
fn peel_target_tip(
    repo: &gix::Repository,
    fetch_remote_name: &str,
    target_branch_name: &str,
) -> anyhow::Result<Option<gix::ObjectId>> {
    peel_ref(
        repo,
        &format!("refs/remotes/{fetch_remote_name}/{target_branch_name}"),
    )
}

/// Refuse landing a branch that contains conflicted commits (GitButler conflict-metadata commits,
/// e.g. left by a `but pull`/rebase that stopped at a conflict). Publishing them would move the
/// target to history carrying conflict markers — the same reason `but push` rejects them.
fn reject_conflicted_branch(ctx: &Context, branch_name: &str) -> anyhow::Result<()> {
    let stacks = crate::legacy::workspace::applied_stacks_with_expensive_commit_info(ctx)?;
    let repo = ctx.repo.get()?;
    for stack in &stacks {
        if stack.id.is_some()
            && let Some(branch) = stack.branch(branch_name)
        {
            let conflicted: Vec<String> = branch
                .commits
                .iter()
                .filter(|c| c.has_conflicts)
                .map(|c| shorten_object_id(&repo, c.id))
                .collect();
            if !conflicted.is_empty() {
                bail!(
                    "Cannot land `{branch_name}`: it contains {} conflicted commit{} ({}). \
                     Resolve them first with `but resolve <commit>`.",
                    conflicted.len(),
                    if conflicted.len() == 1 { "" } else { "s" },
                    conflicted.join(", ")
                );
            }
            return Ok(());
        }
    }
    Ok(())
}

/// The merge base of `a` and `b`, or `None` when they share no common ancestor.
fn merge_base_opt(
    repo: &gix::Repository,
    a: gix::ObjectId,
    b: gix::ObjectId,
) -> anyhow::Result<Option<gix::ObjectId>> {
    match repo.merge_base(a, b) {
        Ok(id) => Ok(Some(id.detach())),
        Err(gix::repository::merge_base::Error::FindMergeBase(_))
        | Err(gix::repository::merge_base::Error::NotFound { .. }) => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Whether `commit` is contained in the target tip — equal to it, or an ancestor of it (which
/// happens when a concurrent push landed further commits on top of ours).
fn target_ref_contains(
    repo: &gix::Repository,
    commit: gix::ObjectId,
    tip: gix::ObjectId,
) -> anyhow::Result<bool> {
    if commit == tip {
        return Ok(true);
    }
    Ok(merge_base_opt(repo, commit, tip)?.is_some_and(|base| base == commit))
}

fn short_id(ctx: &Context, oid: gix::ObjectId) -> anyhow::Result<String> {
    let repo = ctx.repo.get()?;
    Ok(shorten_object_id(&repo, oid))
}

/// Decide whether `remote_url` points at the repository we're sitting in (a `gb-local` self-remote)
/// so we can move local refs instead of pushing over the network.
fn remote_points_at_current_repo(repo: &gix::Repository, remote_url: &str) -> anyhow::Result<bool> {
    if remote_url.contains("://") || remote_url.starts_with("git@") {
        return Ok(false);
    }

    let workdir = repo.workdir().unwrap_or(repo.git_dir());
    let remote_path = Path::new(remote_url);
    let remote_path = if remote_path.is_absolute() {
        remote_path.to_path_buf()
    } else {
        workdir.join(remote_path)
    };

    let Ok(remote_path) = remote_path.canonicalize() else {
        return Ok(false);
    };
    let workdir = workdir.canonicalize()?;
    let git_dir = repo.git_dir().canonicalize()?;

    Ok(remote_path == workdir || remote_path == git_dir)
}
