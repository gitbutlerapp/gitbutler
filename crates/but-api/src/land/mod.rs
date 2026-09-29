//! `but_api::land::branch_land`: land one or more branches directly onto the target ref (the
//! "avoid pull requests" workflow), exposed for every client (CLI, desktop, SDK).
//!
//! This fast-forwards (or merges) the branches onto the configured target with a single update —
//! pushing to the real remote, or moving the local refs for a self-remote (`gb-local`) or a target
//! without an upstream — and then reconciles the remaining applied branches onto the moved target.
//! The reconcile is the one `but pull` runs, so in single-branch mode landing the checked-out
//! branch checks out the target branch afterwards, or a generated branch when the target can't be
//! reused.
//!
//! ## Boundary
//!
//! The input is the branches' names (refs, resolved by the caller from whatever identifier it
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
//! throughout: it interleaves fetching the target remote with the retry loop, so it acquires and
//! releases worktree access per step, like the legacy public APIs. Only the target's fetch remote
//! is fetched — landing needs a fresh target tracking ref and nothing else, and an unreachable
//! unrelated remote must not block the land.
//! The reconcile step owns its own permission and oplog snapshot. The target move itself is not
//! captured by the oplog (see the CLI's undo caveats), matching the prior CLI behavior.
//!
//! A target without an upstream has nothing to fetch or push, so `land_locally` holds one
//! exclusive permission throughout and records the target move and the reconcile as a single
//! oplog entry, making the whole land undoable.

mod deliver;
mod merge;
mod reconcile;

use std::path::Path;

use anyhow::{Context as _, bail};
use but_api_macros::but_api;
use but_core::DryRun;
use but_ctx::Context;
use but_error::bail_precondition;
use but_oplog::legacy::{OperationKind, SnapshotDetails};
use gitbutler_git::GitContextExt as _;
use gix::prelude::ObjectIdExt;
use tracing::instrument;

use crate::WorkspaceState;

/// How many times we re-fetch and re-merge when the target moved underneath us before giving up.
const MAX_PUSH_ATTEMPTS: usize = 5;

/// What `branch_land` ended up doing, used to drive honest end-of-command reporting.
#[derive(Debug, Clone)]
pub enum BranchLandKind {
    /// Every branch was already reachable from the target; nothing was pushed or moved.
    AlreadyIntegrated,
    /// The target advanced to `new_target_oid` (a fast-forward to a branch tip, or a merge commit).
    Updated {
        /// The commit the target now points at.
        new_target_oid: gix::ObjectId,
        /// The commit the target pointed at before landing, for undo recipes.
        prev_target_oid: gix::ObjectId,
    },
}

/// The result of landing branches onto the target.
#[derive(Debug, Clone)]
pub struct BranchLandResult {
    /// What landing did to the target.
    pub landed: BranchLandKind,
    /// The requested branches that were already reachable from the target before landing, so
    /// landing them changed nothing. A branch that lands only because an earlier branch in the
    /// request brought it along (`but merge top bottom`) counts as landed, not as this.
    pub already_integrated: Vec<String>,
    /// The landed branches whose copy on the push remote was deleted after the land (only copies
    /// fully contained in the landed target are deleted).
    pub deleted_remote_branches: Vec<String>,
    /// Whether delivery moved local refs (a target without an upstream, or a `gb-local`
    /// self-remote) rather than pushing to a remote.
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
        /// Every branch was already reachable from the target.
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
        /// The requested branches that were already reachable from the target.
        pub already_integrated: Vec<String>,
        /// The landed branches whose copy on the push remote was deleted after the land.
        pub deleted_remote_branches: Vec<String>,
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
                already_integrated: value.already_integrated,
                deleted_remote_branches: value.deleted_remote_branches,
                local_delivery: value.local_delivery,
                reconcile_skipped: value.reconcile_skipped,
                workspace: value.workspace.try_into()?,
            })
        }
    }
}

/// Reject target configurations that landing cannot handle.
///
/// A target without an upstream is landed locally, so only targets with an upstream are checked.
/// Call this before confirmation or mutation; landing checks it again after reloading the repo.
pub fn ensure_supported_target_upstream(
    project_meta: &but_core::ref_metadata::ProjectMeta,
    repo: &gix::Repository,
) -> anyhow::Result<()> {
    let local = project_meta.target_ref_or_err()?;
    let Some(upstream) = project_meta.remote_target_ref(repo)? else {
        return Ok(());
    };
    let (fetch_remote, upstream_branch) =
        but_core::extract_remote_name_and_short_name(upstream.as_ref(), &repo.remote_names())
            .context("Could not determine the target upstream's remote")?;
    let push_remote = project_meta
        .push_remote
        .as_deref()
        .filter(|name| !name.is_empty())
        .unwrap_or(&fetch_remote);
    if push_remote != fetch_remote {
        bail_precondition!(
            "`but merge` does not yet support triangular remotes (fetch `{fetch_remote}`, \
             push `{push_remote}`). Merge via a pull request instead, or configure a single \
             remote for the target branch."
        );
    }
    if local.shorten() != upstream_branch {
        bail_precondition!(
            "`but merge` requires matching local and upstream branch names (local `{}`, upstream `{upstream_branch}`). Merge via a pull request instead.",
            local.shorten(),
        );
    }
    Ok(())
}

/// Land `branches` directly onto the configured target ref with a single target update.
///
/// `branches` are the short names of the branches to land (their `refs/heads/<branch>` refs), in
/// landing order; duplicates are ignored. Each branch must be the bottom segment of its stack — or,
/// with `whole_stack`, the top segment, which publishes every segment below it as well — and the
/// landed segments must be free of conflicted commits. HEAD must be the managed workspace or, in
/// single-branch mode, the checked-out branch (see [`ensure_landable_checkout`]), and a target
/// remote must be non-triangular.
///
/// `no_ff` always creates a merge commit; `ff_only` refuses to land unless the target can be
/// fast-forwarded to every branch tip in turn. Passing both is an error.
///
/// `expected_tips` is empty, or holds the tip the caller showed the user for each of `branches`, in
/// the same order: the land is refused if any branch moved since, as by an agent committing during
/// the confirmation or the fetch, because the push can't be undone. They're compared under the
/// worktree lock that decides the land.
///
/// This fetches the target, lands the branches one after another onto it locally (each a
/// fast-forward or signed merge commit), publishes the result with one push or ref move (retrying
/// when the target moves underneath us), then reconciles the remaining applied branches onto the
/// moved target. A conflict in any branch fails the whole land before anything is published. The
/// remote push is not undoable; see [`BranchLandResult::reconcile_skipped`] and the
/// workspace state for what to report.
///
/// A target without an upstream is moved locally instead, and the whole land is a single undoable
/// oplog entry, recorded as soon as the target moved so `but undo` reverts even a land whose
/// reconcile failed.
#[but_api(try_from = json::BranchLandResult)]
#[instrument(skip(ctx), err(Debug))]
pub fn branch_land(
    ctx: &mut Context,
    branches: Vec<String>,
    no_ff: bool,
    ff_only: bool,
    whole_stack: bool,
    expected_tips: Vec<gix::ObjectId>,
) -> anyhow::Result<BranchLandResult> {
    if no_ff && ff_only {
        bail!("Cannot land with both `no_ff` and `ff_only`");
    }
    if !expected_tips.is_empty() && expected_tips.len() != branches.len() {
        bail!(
            "Got {} expected tips for {} branches",
            expected_tips.len(),
            branches.len()
        );
    }
    // Drop duplicate branches, together with their expected tips.
    let (branches, expected_tips) = {
        let mut seen = std::collections::HashSet::new();
        let mut unique_branches = Vec::new();
        let mut unique_tips = Vec::new();
        for (idx, branch) in branches.into_iter().enumerate() {
            if seen.insert(branch.clone()) {
                if let Some(tip) = expected_tips.get(idx) {
                    unique_tips.push(*tip);
                }
                unique_branches.push(branch);
            }
        }
        (unique_branches, unique_tips)
    };
    if branches.is_empty() {
        bail!("No branch to land");
    }

    let base_branch = {
        let mut guard = ctx.exclusive_worktree_access();
        let local_target = {
            // A caller may have checked the checkout already, as the CLI does before its
            // confirmation prompt. Reload so this check sees HEAD as it is now.
            ctx.reload_repo_and_invalidate_workspace(guard.write_permission())?;
            let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
            ensure_landable_checkout(&ws)?;
            ensure_supported_target_upstream(&ws.graph.project_meta, &repo)?;
            let project_meta = &ws.graph.project_meta;
            match project_meta.remote_target_ref(&repo)? {
                Some(_) => None,
                None => Some(project_meta.target_ref_or_err()?.clone()),
            }
        };
        if let Some(local_target) = local_target {
            drop(guard);
            return land_locally(
                ctx,
                &branches,
                &expected_tips,
                local_target,
                no_ff,
                ff_only,
                whole_stack,
            );
        }
        crate::legacy::virtual_branches::get_base_branch_data(ctx, guard.write_permission())?
            .ok_or_else(|| anyhow::anyhow!("No base branch configured"))?
    };

    let target_branch_name = base_branch.short_name.clone();
    if target_branch_name.is_empty() {
        bail!("Configured target branch has no branch name");
    }
    let fetch_remote_name = base_branch
        .remote_name
        .clone()
        .context("Merging without a remote isn't supported yet")?;
    let push_remote_name = if base_branch
        .push_remote_name
        .as_ref()
        .context("Merging without a remote isn't supported yet")?
        .is_empty()
    {
        fetch_remote_name.clone()
    } else {
        base_branch
            .push_remote_name
            .as_ref()
            .context("Merging without a remote isn't supported yet")?
            .clone()
    };
    if push_remote_name.is_empty() {
        bail!("Configured target branch has no push remote");
    }

    let target_display = format!("{push_remote_name}/{target_branch_name}");
    let push_remote_url = if base_branch
        .push_remote_url
        .as_ref()
        .context("Merging without a remote isn't supported yet")?
        .is_empty()
    {
        base_branch
            .remote_url
            .as_ref()
            .context("Merging without a remote isn't supported yet")?
    } else {
        base_branch
            .push_remote_url
            .as_ref()
            .context("Merging without a remote isn't supported yet")?
    };
    let update_target_locally = {
        let repo = ctx.repo.get()?;
        remote_points_at_current_repo(&repo, push_remote_url)?
    };

    // Safety guards a non-CLI caller must not be able to bypass: never publish lower stack segments
    // the user did not opt into, and never publish conflicted commits onto the target. Every branch
    // is validated before anything mutates, and the names being landed are captured now: their
    // remote copies are deleted after a successful land.
    let mut landed_branch_names = validate_landing(ctx, &branches, &target_display, whole_stack)?;
    // Landing the target branch's own name must never delete the target on the remote.
    landed_branch_names.retain(|name| *name != target_branch_name);

    // Fetch only the target's remote: landing needs a fresh target tracking ref and nothing else,
    // and an unreachable unrelated remote must not block the land.
    fetch_target_remote(ctx, &fetch_remote_name)?;
    let fetch_target_ref: gix::refs::FullName =
        format!("refs/remotes/{fetch_remote_name}/{target_branch_name}").try_into()?;

    // Land the branches, retrying when the target moves underneath us (optimistic concurrency).
    let mut landed: Option<BranchLandKind> = None;
    let mut already_integrated = Vec::new();
    for attempt in 1..=MAX_PUSH_ATTEMPTS {
        // Recompute the plan against the freshly-fetched target on every attempt — a branch can go
        // from fast-forwardable to divergent between retries.
        let plan = {
            let _guard = ctx.exclusive_worktree_access();
            let repo = ctx.repo.get()?;
            merge::plan_land(
                &repo,
                &branches,
                &expected_tips,
                fetch_target_ref.as_ref(),
                no_ff,
                ff_only,
            )
        }?;
        already_integrated = plan.already_integrated;

        let (new_target_oid, prev_target_oid) = (plan.new_target_oid, plan.target_oid);
        if new_target_oid == prev_target_oid {
            landed = Some(BranchLandKind::AlreadyIntegrated);
            break;
        }

        let push_result = if update_target_locally {
            let repo = ctx.repo.get()?;
            deliver::update_local_target_refs(
                &repo,
                new_target_oid,
                prev_target_oid,
                Some(&push_remote_name),
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
                fetch_target_remote(ctx, &fetch_remote_name)?;
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
        bail!(
            "Failed to land {} onto {target_display}",
            branches.join(", ")
        );
    };

    // On the real-remote path, re-fetch so the tracking ref reflects the landed commit, then verify
    // it actually advanced before reconciling — otherwise the reconcile would be a silent no-op.
    if let BranchLandKind::Updated { new_target_oid, .. } = &landed
        && !update_target_locally
    {
        fetch_target_remote(ctx, &fetch_remote_name)?;

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
                already_integrated,
                deleted_remote_branches: Vec::new(),
                local_delivery: update_target_locally,
                reconcile_skipped: true,
                workspace: current_workspace_state(ctx)?,
            });
        }
    }

    // Drop the cached workspace view so the reconcile's status read peels the freshly-fetched target.
    ctx.invalidate_workspace_cache()?;

    let reconciled = reconcile::reconcile_after_land(ctx)?;

    // The direct-push counterpart of the forge's "delete branch after merge": drop the landed
    // branches' remote copies now that the target contains them. Leaving them behind is what makes
    // a later same-named branch show up as merged upstream and refuse commits. This must run after
    // the reconcile — integration detection reads the remote-tracking refs as evidence — and is
    // skipped when the reconcile was, so the still-applied branches keep that evidence for the
    // later `but pull`.
    let deleted_remote_branches = if reconciled.blocked_by_worktree {
        Vec::new()
    } else {
        let target_tip = {
            let repo = ctx.repo.get()?;
            peel_target_tip(&repo, &fetch_remote_name, &target_branch_name)?
        };
        target_tip
            .map(|tip| {
                deliver::delete_landed_remote_branches(
                    ctx,
                    &landed_branch_names,
                    &push_remote_name,
                    tip,
                    update_target_locally,
                )
            })
            .unwrap_or_default()
    };
    if !deleted_remote_branches.is_empty() {
        // The deletions changed refs after the reconcile's workspace read; drop the cached view so
        // later reads in this process don't serve the deleted remote-tracking refs.
        ctx.invalidate_workspace_cache()?;
    }

    Ok(BranchLandResult {
        landed,
        already_integrated,
        deleted_remote_branches,
        local_delivery: update_target_locally,
        reconcile_skipped: reconciled.blocked_by_worktree,
        workspace: reconciled.workspace,
    })
}

/// Land `branches` onto `target`, a local branch without an upstream: with nothing to fetch or
/// push, move `target` itself and reconcile the remaining branches.
fn land_locally(
    ctx: &mut Context,
    branches: &[String],
    expected_tips: &[gix::ObjectId],
    target: but_core::ref_metadata::LocalRefName,
    no_ff: bool,
    ff_only: bool,
    whole_stack: bool,
) -> anyhow::Result<BranchLandResult> {
    let target_ref = target.as_local_full_name_ref();
    let target_branch_name = target_ref.shorten().to_string();
    validate_landing(ctx, branches, &target_branch_name, whole_stack)?;

    let mut guard = ctx.exclusive_worktree_access();
    let plan = {
        let repo = ctx.repo.get()?;
        merge::plan_land(&repo, branches, expected_tips, target_ref, no_ff, ff_only)?
    };
    let landed = if plan.new_target_oid == plan.target_oid {
        BranchLandKind::AlreadyIntegrated
    } else {
        BranchLandKind::Updated {
            new_target_oid: plan.new_target_oid,
            prev_target_oid: plan.target_oid,
        }
    };

    // Updating the remaining branches deletes any branch whose changes are now on the target, not
    // just the merged ones. So the undo snapshot records every applied branch, plus the target.
    // It is taken before anything changes: undo puts all of them back, and redo deletes the
    // branches the merge deleted.
    let applied_branches = {
        let (_repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
        ws.stacks
            .iter()
            .flat_map(|stack| &stack.segments)
            .flat_map(|segment| segment.ref_names())
            .filter(|name| {
                name.category() == Some(gix::refs::Category::LocalBranch) && *name != target_ref
            })
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>()
    };
    let preserved_refs = std::iter::once(target_ref)
        .chain(applied_branches.iter().map(|name| name.as_ref()))
        .collect::<Vec<_>>();
    let mut snapshot = but_oplog::UnmaterializedOplogSnapshot::from_details_with_refs_and_perm(
        ctx,
        SnapshotDetails::new(OperationKind::MergeBranch),
        &preserved_refs,
        guard.read_permission(),
        DryRun::No,
    );
    if let BranchLandKind::Updated {
        new_target_oid,
        prev_target_oid,
    } = &landed
    {
        {
            let repo = ctx.repo.get()?;
            deliver::update_local_target_refs(
                &repo,
                *new_target_oid,
                *prev_target_oid,
                None,
                &target_branch_name,
            )
            .context("Failed to update local target branch")?;
        }
        // Once the target moved there is something to undo, even if the reconcile fails.
        if let Some(snapshot) = snapshot.take() {
            snapshot.commit(ctx, guard.write_permission()).ok();
        }
    }
    ctx.invalidate_workspace_cache()?;

    let workspace = reconcile::reconcile_after_land_only_with_perm(ctx, guard.write_permission())
        .with_context(|| {
        format!("Failed to update the remaining branches onto `{target_branch_name}`")
    })?;
    if let Some(snapshot) = snapshot {
        snapshot.commit(ctx, guard.write_permission()).ok();
    }

    Ok(BranchLandResult {
        landed,
        already_integrated: plan.already_integrated,
        deleted_remote_branches: Vec::new(),
        local_delivery: true,
        reconcile_skipped: false,
        workspace,
    })
}

/// Refuse landing unless HEAD is the managed workspace or an ordinary checked-out branch.
/// The reconcile after the land rewrites whatever HEAD points at, so a detached HEAD or edit mode
/// must be refused before anything is published. Callers that confirm with the
/// user run this first, so they never ask about a land that can't happen.
pub fn ensure_landable_checkout(ws: &but_graph::Workspace) -> anyhow::Result<()> {
    let single_branch_checkout = ws.ref_name().is_some_and(|name| {
        name.category() == Some(gix::refs::Category::LocalBranch)
            && name.as_bstr() != gitbutler_operating_modes::EDIT_BRANCH_REF
    });
    if !ws.kind.has_managed_ref() && !single_branch_checkout {
        bail_precondition!("`but merge` requires the GitButler workspace or a checked-out branch.");
    }
    Ok(())
}

/// Validate every branch of a land request with [`validate_branch_landing`] before anything
/// mutates. Returns every branch name the land publishes, once each and in landing order: the
/// requested branches and the named segments landing with them. Their remote copies are deleted
/// after a successful land.
fn validate_landing(
    ctx: &mut Context,
    branches: &[String],
    target_display: &str,
    whole_stack: bool,
) -> anyhow::Result<Vec<String>> {
    let mut landed = Vec::new();
    for branch in branches {
        let lower_segments =
            validate_branch_landing(ctx, branch, branches, target_display, whole_stack)?;
        for name in std::iter::once(branch.clone()).chain(lower_segments) {
            if !landed.contains(&name) {
                landed.push(name);
            }
        }
    }
    Ok(landed)
}

/// Refuse landing `branch` if it would publish stack segments the request didn't opt into, or
/// conflicted commits. `requested` is every branch of the land request.
///
/// The segments below `branch` are published along with its tip. That's fine when each of them is
/// itself requested, as in `but merge bottom top`; otherwise `whole_stack` must opt into publishing
/// them. Commits on unnamed segments below (their branch refs were deleted) can't be requested by
/// name, so they always need `whole_stack`.
///
/// With `whole_stack`, `branch` must be the top of its stack, or have only requested segments above
/// it, so `--whole-stack` always means "the entire stack lands", never a partial land that strands
/// the segments above. Conflicted commits are refused in everything the tip would publish (the same
/// guard `but push` applies before sending commits to a remote).
///
/// Returns the named segments that land together with `branch`. A branch outside every lane is
/// refused, as none of these checks could run on it.
fn validate_branch_landing(
    ctx: &mut Context,
    branch: &str,
    requested: &[String],
    target_display: &str,
    whole_stack: bool,
) -> anyhow::Result<Vec<String>> {
    let Some(scan) = scan_stack(ctx, branch)? else {
        bail_precondition!(
            "Refusing to land `{branch}`: it is not in the workspace. Apply or check out the branch first."
        );
    };
    let is_requested = |name: &String| requested.contains(name);

    // Judge "top of the stack" by position, not by names: a segment whose branch ref was deleted
    // still holds commits that "land the entire stack" would have to include.
    let covered_from_above = !scan.unnamed_upper && scan.upper_segments.iter().all(is_requested);
    if whole_stack && scan.has_upper && !covered_from_above {
        if let Some(top) = scan.upper_segments.first() {
            bail_precondition!(
                "Refusing to land `{branch}` with --whole-stack: it is not the top of its stack. \
                 --whole-stack lands the entire stack; name its top segment `{top}` instead.",
            );
        }
        bail_precondition!(
            "Refusing to land `{branch}` with --whole-stack: it is not the top of its stack — \
             unnamed segment(s) with commits sit above it (their branch refs no longer exist), so \
             landing `{branch}` would not land the entire stack.",
        );
    }

    if !whole_stack {
        let unrequested_lower: Vec<&str> = scan
            .lower_segments
            .iter()
            .filter(|name| !is_requested(name))
            .map(String::as_str)
            .collect();
        if let Some(bottom) = scan.lower_segments.last()
            && !unrequested_lower.is_empty()
        {
            bail_precondition!(
                "Refusing to land `{branch}`: it is stacked on top of {} other segment(s) ({}) \
                 whose commits would also be published to {target_display}. Land the bottom \
                 segment `{bottom}`, or pass --whole-stack to land `{branch}` together with \
                 everything below it.",
                unrequested_lower.len(),
                unrequested_lower.join(", "),
            );
        }
        if scan.unnamed_below_commits > 0 {
            bail_precondition!(
                "Refusing to land `{branch}`: {} commit(s) on unnamed segment(s) below it would \
                 also be published to {target_display}. Pass --whole-stack to land `{branch}` \
                 together with everything below it.",
                scan.unnamed_below_commits,
            );
        }
    }

    if !scan.conflicted.is_empty() {
        bail_precondition!(
            "Cannot land `{branch}`: it would publish {} conflicted commit{} ({}). \
             Resolve them first with `but resolve <commit>`.",
            scan.conflicted.len(),
            if scan.conflicted.len() == 1 { "" } else { "s" },
            scan.conflicted.join(", "),
        );
    }
    Ok(scan.lower_segments)
}

/// Fetch the target's fetch remote and record the outcome on the project, mirroring the
/// bookkeeping `fetch_from_remotes` performs so `last_fetched` and auto-fetch scheduling stay
/// accurate for targeted fetches too.
fn fetch_target_remote(ctx: &Context, remote: &str) -> anyhow::Result<()> {
    use anyhow::Context as _;

    let result = ctx.fetch(remote, Some("land".to_string()));
    let timestamp = std::time::SystemTime::now();
    let project_data_last_fetched = match &result {
        Ok(()) => gitbutler_project::FetchResult::Fetched { timestamp },
        Err(err) => gitbutler_project::FetchResult::Error {
            timestamp,
            error: err.to_string(),
        },
    };
    gitbutler_project::update(gitbutler_project::UpdateRequest {
        project_data_last_fetched: Some(project_data_last_fetched),
        ..gitbutler_project::UpdateRequest::default_with_id(ctx.legacy_project.id.clone())
    })
    .context("failed to update project with last fetched timestamp")?;
    result
}

/// What landing `branch` would publish beyond the branch's own segment, for callers that must
/// describe a whole-stack land before invoking [`branch_land`]. Empty when `branch` is the bottom
/// of its stack or not an applied branch.
#[derive(Debug, Clone, Default)]
pub struct LowerStack {
    /// Named segments below `branch`, top to bottom.
    pub segments: Vec<String>,
    /// Commits on unnamed segments below `branch` (their branch refs no longer exist) — published
    /// all the same, so confirmations must disclose them even without a name to show.
    pub unnamed_commits: usize,
}

/// The [`LowerStack`] landing `branch` would publish along with it.
pub fn lower_stack(ctx: &mut Context, branch: &str) -> anyhow::Result<LowerStack> {
    Ok(scan_stack(ctx, branch)?
        .map(|scan| LowerStack {
            segments: scan.lower_segments,
            unnamed_commits: scan.unnamed_below_commits,
        })
        .unwrap_or_default())
}

/// `branch`'s position in its stack as the landing guards see it.
struct StackScan {
    /// Named segments above `branch`, top of the stack first. Non-empty means landing `branch`
    /// would leave these stranded above a moved target.
    upper_segments: Vec<String>,
    /// Whether any segment sits above `branch` in its stack, named or not.
    has_upper: bool,
    /// Whether any segment above `branch` has no name (its branch ref was deleted).
    unnamed_upper: bool,
    /// Every local branch whose tip landing `branch` publishes: the named segments below it, top to
    /// bottom, followed by any other branch the graph places on a published commit.
    lower_segments: Vec<String>,
    /// Commits on unnamed segments below `branch` (their branch refs no longer exist), for
    /// confirmations to disclose.
    unnamed_below_commits: usize,
    /// Short hashes of the conflicted commits among everything the branch's tip publishes.
    conflicted: Vec<String>,
}

/// Locate `branch` in the graph workspace, or `None` when no lane (an applied stack or a linked
/// worktree) has a segment by that name.
///
/// The stack layout (what sits above `branch`, and which segments are unnamed) comes from the
/// workspace projection, and is only used to refuse. What the tip publishes, which authorizes the
/// push, doesn't: it's everything reachable from the branch but not from the target, walked in
/// Git, and the branches landing with it are every local branch in the graph that points into that
/// set. So a branch the lossy projection drops can't slip onto the target unrequested.
fn scan_stack(ctx: &mut Context, branch: &str) -> anyhow::Result<Option<StackScan>> {
    let guard = ctx.exclusive_worktree_access();
    let (repo, ws, _db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    let head_info = but_workspace::graph_to_ref_info(
        &ws,
        &repo,
        but_workspace::ref_info::Options {
            project_meta: ws.graph.project_meta.clone(),
            traversal: but_graph::init::Options::limited(),
            ..Default::default()
        },
    )?;

    // Segments are ordered top of the stack first, so everything after `pos` is published along
    // with the branch's tip and everything before it sits on top of the branch.
    for segments in head_info.lanes().map(|lane| lane.segments) {
        let Some(pos) = segments
            .iter()
            .position(|s| segment_short_name(s).as_deref() == Some(branch))
        else {
            continue;
        };

        let published = published_commits(&repo, &ws.graph.project_meta, branch)?;
        let upper_segments: Vec<String> = segments[..pos]
            .iter()
            .filter_map(segment_short_name)
            .collect();
        let mut lower_segments: Vec<String> = segments[pos + 1..]
            .iter()
            .filter_map(segment_short_name)
            .collect();
        let target_branch = ws
            .graph
            .project_meta
            .target_ref
            .as_ref()
            .map(|target| target.as_local_full_name_ref().shorten().to_string());
        for name in graph_local_branches(&ws.graph) {
            // A branch on the tip itself, like an empty segment stacked above it, publishes
            // nothing the branch doesn't.
            if name == branch
                || target_branch.as_deref() == Some(name.as_str())
                || lower_segments.contains(&name)
                || upper_segments.contains(&name)
            {
                continue;
            }
            if peel_ref(&repo, &format!("refs/heads/{name}"))?
                .is_some_and(|tip| published.first() != Some(&tip) && published.contains(&tip))
            {
                lower_segments.push(name);
            }
        }

        let mut conflicted = Vec::new();
        for id in &published {
            if but_core::Commit::from_id(id.attach(&repo))?.is_conflicted() {
                conflicted.push(id.attach(&repo).shorten_or_id().to_string());
            }
        }

        return Ok(Some(StackScan {
            upper_segments,
            has_upper: pos > 0,
            unnamed_upper: segments[..pos]
                .iter()
                .any(|s| segment_short_name(s).is_none()),
            lower_segments,
            unnamed_below_commits: segments[pos + 1..]
                .iter()
                .filter(|s| segment_short_name(s).is_none())
                .map(|s| s.commits.len())
                .sum(),
            conflicted,
        }));
    }
    Ok(None)
}

/// The commits landing `branch` publishes, tip first: those reachable from it but not from the
/// target as it is now. The target is its remote-tracking ref, or the local target branch when it
/// has no upstream.
fn published_commits(
    repo: &gix::Repository,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    branch: &str,
) -> anyhow::Result<Vec<gix::ObjectId>> {
    let Some(tip) = peel_ref(repo, &format!("refs/heads/{branch}"))? else {
        bail!("Branch {branch} not found");
    };
    let target_ref = match project_meta.remote_target_ref(repo)? {
        Some(remote) => remote,
        None => project_meta
            .target_ref_or_err()?
            .as_local_full_name_ref()
            .to_owned(),
    };
    let target_tip = match repo.try_find_reference(target_ref.as_ref())? {
        Some(reference) => Some(reference.into_fully_peeled_id()?.detach()),
        None => None,
    };
    Ok(repo
        .rev_walk([tip])
        .with_hidden(target_tip)
        .all()?
        .map(|info| info.map(|info| info.id))
        .collect::<Result<_, _>>()?)
}

/// Every local branch the graph knows about, by short name: the names of its segments and the refs
/// on its commits.
fn graph_local_branches(graph: &but_graph::Graph) -> Vec<String> {
    let mut names = Vec::new();
    for sidx in graph.segments() {
        let segment = &graph[sidx];
        let refs = segment.ref_name().into_iter().chain(
            segment
                .commits
                .iter()
                .flat_map(|commit| commit.refs.iter().map(|r| r.ref_name.as_ref())),
        );
        for ref_name in refs {
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let name = ref_name.shorten().to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
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
    let mut meta = ctx.meta()?;
    let guard = ctx.exclusive_worktree_access();
    let (repo, ws, mut db) = ctx.workspace_and_db_mut_with_perm(guard.read_permission())?;
    WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        std::collections::BTreeMap::new(),
        Vec::new(),
        &mut db,
    )
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
    Ok(repo.merge_base(a, b)?.map(gix::Id::detach))
}

#[test]
fn merge_base_distinguishes_unrelated_history_from_missing_objects() -> anyhow::Result<()> {
    let tmp = tempfile::tempdir()?;
    gix::init(tmp.path())?;
    let repo = but_testsupport::open_repo(tmp.path())?;
    let root = |name: &str| {
        repo.commit(
            name,
            name,
            repo.object_hash().empty_tree(),
            std::iter::empty::<gix::ObjectId>(),
        )
        .map(gix::Id::detach)
    };
    let first = root("refs/heads/first")?;
    let second = root("refs/heads/second")?;
    assert_eq!(
        merge_base_opt(&repo, first, second)?,
        None,
        "unrelated roots have no merge base"
    );
    assert!(
        merge_base_opt(&repo, first, repo.object_hash().null()).is_err(),
        "a missing object is a traversal failure, not unrelated history"
    );
    Ok(())
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
