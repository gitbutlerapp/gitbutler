//! `but_api::land::branch_land`: land a branch directly onto the target ref (the "avoid pull
//! requests" workflow), exposed for every client (CLI, desktop, SDK).
//!
//! Inside a managed GitButler workspace this fast-forwards (or merges) the branch onto the
//! configured target — pushing to the real remote, or moving the local refs for a self-remote
//! (`gb-local`) — and then reconciles the remaining applied branches onto the moved target.
//!
//! ## Boundary
//!
//! The input is the branch's name (a ref, resolved by the caller from whatever identifier it
//! uses) plus flags; no `StackId` crosses the boundary. The output is a `BranchLandResult`
//! carrying the standard [`WorkspaceState`](crate::WorkspaceState), so clients reason over graph
//! state.
//!
//! ## Layering
//!
//! `merge` decides the topology and builds the (signed) merge commit, `deliver` pushes or moves
//! the refs, and `reconcile` updates the remaining branches via the modern graph integration path
//! ([`crate::workspace::workspace_integrate_upstream_with_perm`]). This module orchestrates them.
//!
//! Unlike the modern single-mutation endpoints, `branch_land` cannot hold one exclusive permission
//! throughout: it interleaves `fetch_from_remotes` (which re-acquires shared access) with the
//! retry loop, so it acquires and releases worktree access per step, like the legacy public APIs.
//! The reconcile step owns its own permission and oplog snapshot. The target move itself is not
//! captured by the oplog (see the CLI's undo caveats), matching the prior CLI behavior.

mod deliver;
mod merge;
mod reconcile;

use std::path::Path;

use anyhow::bail;
use but_api_macros::but_api;
use but_ctx::Context;
use gix::prelude::ObjectIdExt;
use tracing::instrument;

use crate::WorkspaceState;
use merge::LandOutcome;

/// How many times we re-fetch and re-merge when the target moved underneath us before giving up.
const MAX_PUSH_ATTEMPTS: usize = 5;

/// What `branch_land` ended up doing, used to drive honest end-of-command reporting.
#[derive(Debug, Clone)]
pub enum BranchLandKind {
    /// The branch was already reachable from the target; nothing was pushed or moved.
    AlreadyIntegrated,
    /// The target advanced to `new_target_oid` (a fast-forward to the branch tip, or a merge commit).
    Updated {
        /// The commit the target now points at.
        new_target_oid: gix::ObjectId,
        /// The commit the target pointed at before landing, for undo recipes.
        prev_target_oid: gix::ObjectId,
    },
}

/// The result of landing a branch onto the target.
#[derive(Debug, Clone)]
pub struct BranchLandResult {
    /// What landing did to the target.
    pub landed: BranchLandKind,
    /// Whether delivery moved local refs (a `gb-local` self-remote) rather than pushing to a remote.
    pub local_delivery: bool,
    /// Set when the remaining branches were not reconciled onto the moved target — either the
    /// tracking ref hadn't caught up yet, or uncommitted worktree changes conflicted with the
    /// rebase. The land itself succeeded; the caller should suggest running `but pull`.
    pub reconcile_skipped: bool,
    /// The post-land workspace state.
    pub workspace: WorkspaceState,
}

/// JSON transport types for the land API.
pub mod json {
    use crate::json::HexHash;
    use serde::Serialize;

    /// JSON transport type for what landing did to the target.
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(
        rename_all = "camelCase",
        rename_all_fields = "camelCase",
        tag = "type"
    )]
    pub enum BranchLandKind {
        /// The branch was already reachable from the target.
        AlreadyIntegrated,
        /// The target advanced to a new commit.
        Updated {
            /// The commit the target now points at.
            #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
            new_target_oid: HexHash,
            /// The commit the target pointed at before landing.
            #[cfg_attr(feature = "export-schema", schemars(with = "String"))]
            prev_target_oid: HexHash,
        },
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchLandKind);

    impl From<super::BranchLandKind> for BranchLandKind {
        fn from(value: super::BranchLandKind) -> Self {
            match value {
                super::BranchLandKind::AlreadyIntegrated => Self::AlreadyIntegrated,
                super::BranchLandKind::Updated {
                    new_target_oid,
                    prev_target_oid,
                } => Self::Updated {
                    new_target_oid: new_target_oid.into(),
                    prev_target_oid: prev_target_oid.into(),
                },
            }
        }
    }

    /// JSON transport type returned by [`branch_land`](super::branch_land).
    #[derive(Debug, Serialize)]
    #[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
    #[serde(rename_all = "camelCase")]
    pub struct BranchLandResult {
        /// What landing did to the target.
        pub landed: BranchLandKind,
        /// Whether delivery moved local refs rather than pushing to a remote.
        pub local_delivery: bool,
        /// Whether the remaining branches were left un-reconciled (run `but pull` to finish).
        pub reconcile_skipped: bool,
        /// The post-land workspace state.
        pub workspace: crate::json::WorkspaceState,
    }

    #[cfg(feature = "export-schema")]
    but_schemars::register_sdk_type!(BranchLandResult);

    impl TryFrom<super::BranchLandResult> for BranchLandResult {
        type Error = anyhow::Error;

        fn try_from(value: super::BranchLandResult) -> Result<Self, Self::Error> {
            Ok(Self {
                landed: value.landed.into(),
                local_delivery: value.local_delivery,
                reconcile_skipped: value.reconcile_skipped,
                workspace: value.workspace.try_into()?,
            })
        }
    }
}

/// Land `branch` directly onto the configured target ref.
///
/// `branch` is the short name of the branch to land (its `refs/heads/<branch>` ref). The branch
/// must be the bottom segment of its stack and free of conflicted commits, and the workspace must
/// be a managed GitButler workspace with a configured, non-triangular target remote.
///
/// This fetches the target, lands the branch (fast-forward or signed merge commit, retrying when
/// the target moves underneath us), then reconciles the remaining applied branches onto the moved
/// target. The remote push is not undoable; see [`BranchLandResult::reconcile_skipped`] and the
/// workspace state for what to report.
#[but_api(napi, try_from = json::BranchLandResult)]
#[instrument(skip(ctx), err(Debug))]
pub fn branch_land(
    ctx: &mut Context,
    branch: String,
    no_ff: bool,
) -> anyhow::Result<BranchLandResult> {
    let base_branch = {
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
        crate::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
            .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?
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

    // Safety guards a non-CLI caller must not be able to bypass: never publish lower stack segments
    // the user did not name, and never publish conflicted commits onto the target.
    validate_branch_landing(ctx, &branch, &target_display)?;

    crate::legacy::virtual_branches::fetch_from_remotes(ctx, Some("land".to_string()))?;

    // Land the branch, retrying when the target moves underneath us (optimistic concurrency).
    let mut landed: Option<BranchLandKind> = None;
    for attempt in 1..=MAX_PUSH_ATTEMPTS {
        // Recompute the merge decision against the freshly-fetched target on every attempt — a
        // branch can go from fast-forwardable to divergent between retries.
        let outcome = {
            let _guard = ctx.exclusive_worktree_access();
            let repo = ctx.repo.get()?;
            merge::decide_land_outcome(
                &repo,
                &branch,
                &fetch_remote_name,
                &target_branch_name,
                no_ff,
            )
        }?;

        let (new_target_oid, prev_target_oid) = match &outcome {
            LandOutcome::AlreadyIntegrated => {
                landed = Some(BranchLandKind::AlreadyIntegrated);
                break;
            }
            LandOutcome::FastForward {
                feature_oid,
                target_oid,
            } => (*feature_oid, *target_oid),
            LandOutcome::Merge { oid, target_oid } => (*oid, *target_oid),
        };

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
                landed = Some(BranchLandKind::Updated {
                    new_target_oid,
                    prev_target_oid,
                });
                break;
            }
            Err(err)
                if deliver::is_retryable_concurrency_error(&err) && attempt < MAX_PUSH_ATTEMPTS =>
            {
                crate::legacy::virtual_branches::fetch_from_remotes(ctx, Some("land".to_string()))?;
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
        bail!("Failed to land {branch} onto {target_display}");
    };

    // On the real-remote path, re-fetch so the tracking ref reflects the landed commit, then verify
    // it actually advanced before reconciling — otherwise the reconcile would be a silent no-op.
    if let BranchLandKind::Updated { new_target_oid, .. } = &landed
        && !update_target_locally
    {
        crate::legacy::virtual_branches::fetch_from_remotes(ctx, Some("land".to_string()))?;

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
            return Ok(BranchLandResult {
                landed,
                local_delivery: update_target_locally,
                reconcile_skipped: true,
                workspace: current_workspace_state(ctx)?,
            });
        }
    }

    // Drop the cached workspace view so the reconcile's status read peels the freshly-fetched target.
    ctx.invalidate_workspace_cache()?;

    let reconciled = reconcile::reconcile_after_land(ctx)?;
    Ok(BranchLandResult {
        landed,
        local_delivery: update_target_locally,
        reconcile_skipped: reconciled.blocked_by_worktree,
        workspace: reconciled.workspace,
    })
}

/// Refuse landing a non-bottom stack segment (it would publish the lower segments the user did not
/// name) or a branch with conflicted commits (the same guard `but push` applies before sending
/// commits to a remote). Both are computed from the graph workspace, not stack projections.
fn validate_branch_landing(
    ctx: &mut Context,
    branch: &str,
    target_display: &str,
) -> anyhow::Result<()> {
    let guard = ctx.exclusive_worktree_access();
    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let head_info = but_workspace::graph_to_ref_info(
        &ws,
        &repo,
        but_workspace::ref_info::Options {
            project_meta: ws.graph.project_meta.clone(),
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: true,
            ..Default::default()
        },
    )?;

    for stack in &head_info.stacks {
        let Some(pos) = stack
            .segments
            .iter()
            .position(|s| segment_short_name(s).as_deref() == Some(branch))
        else {
            continue;
        };

        // Segments below the named one (oldest last) carry commits the branch's tip would also
        // publish. Naming a non-bottom segment is refused so lower segments aren't landed silently.
        let lower_segments: Vec<String> = stack.segments[pos + 1..]
            .iter()
            .filter_map(segment_short_name)
            .collect();
        if !lower_segments.is_empty() {
            bail!(
                "Refusing to land `{branch}`: it is stacked on top of {} other segment(s) ({}) \
                 whose commits would also be published to {target_display}. Land the bottom segment \
                 `{}` (or the whole stack) instead.",
                lower_segments.len(),
                lower_segments.join(", "),
                lower_segments.last().expect("non-empty checked above"),
            );
        }

        let conflicted: Vec<String> = stack.segments[pos]
            .commits
            .iter()
            .filter(|c| c.has_conflicts)
            .map(|c| c.id.attach(&repo).shorten_or_id().to_string())
            .collect();
        if !conflicted.is_empty() {
            bail!(
                "Cannot land `{branch}`: it contains {} conflicted commit{} ({}). \
                 Resolve them first with `but resolve <commit>`.",
                conflicted.len(),
                if conflicted.len() == 1 { "" } else { "s" },
                conflicted.join(", "),
            );
        }
        return Ok(());
    }
    Ok(())
}

/// The short local-branch name of a segment, or `None` if the segment isn't a named local branch.
fn segment_short_name(segment: &but_workspace::ref_info::Segment) -> Option<String> {
    let ref_name = &segment.ref_info.as_ref()?.ref_name;
    (ref_name.category() == Some(gix::refs::Category::LocalBranch))
        .then(|| ref_name.shorten().to_string())
}

/// The current workspace state with no commit rewrites, for paths that return without reconciling.
/// Invalidates the cached workspace first so the state reflects the freshly-fetched target rather
/// than the pre-fetch graph.
fn current_workspace_state(ctx: &mut Context) -> anyhow::Result<WorkspaceState> {
    ctx.invalidate_workspace_cache()?;
    let guard = ctx.exclusive_worktree_access();
    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    WorkspaceState::from_workspace(&ws, &repo, std::collections::BTreeMap::new())
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
