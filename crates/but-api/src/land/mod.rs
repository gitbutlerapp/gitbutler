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
//! throughout: it interleaves fetching the target remote with the retry loop, so it acquires and
//! releases worktree access per step, like the legacy public APIs. Only the target's fetch remote
//! is fetched — landing needs a fresh target tracking ref and nothing else, and an unreachable
//! unrelated remote must not block the land.
//! The reconcile step owns its own permission and oplog snapshot. The target move itself is not
//! captured by the oplog (see the CLI's undo caveats), matching the prior CLI behavior.

mod deliver;
mod merge;
mod reconcile;

use std::path::Path;

use anyhow::bail;
use but_api_macros::but_api;
use but_ctx::Context;
use gitbutler_git::GitContextExt as _;
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
    /// The landed branches whose copy on the push remote was deleted after the land (only copies
    /// fully contained in the landed target are deleted).
    pub deleted_remote_branches: Vec<String>,
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
                deleted_remote_branches: value.deleted_remote_branches,
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
/// must be the bottom segment of its stack — or, with `whole_stack`, the top segment, which
/// publishes every segment below it as well — and the landed segments must be free of conflicted
/// commits. The workspace must be a managed GitButler workspace with a configured, non-triangular
/// target remote.
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
    whole_stack: bool,
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
    // the user did not opt into, and never publish conflicted commits onto the target. The names
    // being landed are captured before anything mutates: their remote copies are deleted after a
    // successful land.
    let lower_segments = validate_branch_landing(ctx, &branch, &target_display, whole_stack)?;
    let mut landed_branch_names = vec![branch.clone()];
    landed_branch_names.extend(lower_segments);
    // Landing the target branch's own name must never delete the target on the remote.
    landed_branch_names.retain(|name| *name != target_branch_name);

    // Fetch only the target's remote: landing needs a fresh target tracking ref and nothing else,
    // and an unreachable unrelated remote must not block the land.
    fetch_target_remote(ctx, &fetch_remote_name)?;

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
        bail!("Failed to land {branch} onto {target_display}");
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
        deleted_remote_branches,
        local_delivery: update_target_locally,
        reconcile_skipped: reconciled.blocked_by_worktree,
        workspace: reconciled.workspace,
    })
}

/// Refuse landing a non-bottom stack segment unless `whole_stack` opts into publishing the lower
/// segments — and even then only for the top segment, so `--whole-stack` always means "the entire
/// stack lands", never a partial land that strands the segments above. Also refuse conflicted
/// commits in any segment that would be published (the same guard `but push` applies before
/// sending commits to a remote). All computed from the graph workspace, not stack projections.
/// Returns the named lower segments that land together with `branch` — non-empty only for a
/// validated `--whole-stack` land.
fn validate_branch_landing(
    ctx: &mut Context,
    branch: &str,
    target_display: &str,
    whole_stack: bool,
) -> anyhow::Result<Vec<String>> {
    let Some(scan) = scan_stack(ctx, branch)? else {
        return Ok(Vec::new());
    };

    if whole_stack && scan.has_upper {
        // Judge "top of the stack" by position, not by names: a segment whose branch ref was
        // deleted still holds commits that "land the entire stack" would have to include.
        if let Some(top) = scan.upper_segments.first() {
            bail!(
                "Refusing to land `{branch}` with --whole-stack: it is not the top of its stack. \
                 --whole-stack lands the entire stack; name its top segment `{top}` instead.",
            );
        }
        bail!(
            "Refusing to land `{branch}` with --whole-stack: it is not the top of its stack — \
             unnamed segment(s) with commits sit above it (their branch refs no longer exist), so \
             landing `{branch}` would not land the entire stack.",
        );
    }
    // Key off the commits below, not the segment names: segments whose branch ref was deleted are
    // unnamed but their commits would be published all the same.
    let publishes_below = scan.commits_below > 0 || !scan.lower_segments.is_empty();
    if publishes_below && !whole_stack {
        if scan.lower_segments.is_empty() {
            bail!(
                "Refusing to land `{branch}`: {} commit(s) on unnamed segment(s) below it would \
                 also be published to {target_display}. Pass --whole-stack to land `{branch}` \
                 together with everything below it.",
                scan.commits_below,
            );
        }
        bail!(
            "Refusing to land `{branch}`: it is stacked on top of {} other segment(s) ({}) \
             whose commits would also be published to {target_display}. Land the bottom segment \
             `{}`, or pass --whole-stack to land `{branch}` together with everything below it.",
            scan.lower_segments.len(),
            scan.lower_segments.join(", "),
            scan.lower_segments.last().expect("non-empty checked above"),
        );
    }

    if !scan.conflicted.is_empty() {
        bail!(
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
    /// Named segments below `branch`, top to bottom. Non-empty means landing `branch` also
    /// publishes their commits.
    lower_segments: Vec<String>,
    /// Commits in all segments below `branch`, named or not — everything its tip would publish
    /// beyond its own segment.
    commits_below: usize,
    /// The subset of `commits_below` sitting on unnamed segments, for confirmations to disclose.
    unnamed_below_commits: usize,
    /// Short hashes of conflicted commits in `branch`'s segment and every segment below it — every
    /// commit the branch's tip would publish.
    conflicted: Vec<String>,
}

/// Locate `branch` in the graph workspace, or `None` when no applied stack has a segment by that
/// name.
fn scan_stack(ctx: &mut Context, branch: &str) -> anyhow::Result<Option<StackScan>> {
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

    // Segments are ordered top of the stack first, so everything after `pos` is published along
    // with the branch's tip and everything before it sits on top of the branch.
    for stack in &head_info.stacks {
        let Some(pos) = stack
            .segments
            .iter()
            .position(|s| segment_short_name(s).as_deref() == Some(branch))
        else {
            continue;
        };
        return Ok(Some(StackScan {
            upper_segments: stack.segments[..pos]
                .iter()
                .filter_map(segment_short_name)
                .collect(),
            has_upper: pos > 0,
            lower_segments: stack.segments[pos + 1..]
                .iter()
                .filter_map(segment_short_name)
                .collect(),
            commits_below: stack.segments[pos + 1..]
                .iter()
                .map(|s| s.commits.len())
                .sum(),
            unnamed_below_commits: stack.segments[pos + 1..]
                .iter()
                .filter(|s| segment_short_name(s).is_none())
                .map(|s| s.commits.len())
                .sum(),
            conflicted: stack.segments[pos..]
                .iter()
                .flat_map(|s| &s.commits)
                .filter(|c| c.has_conflicts)
                .map(|c| c.id.attach(&repo).shorten_or_id().to_string())
                .collect(),
        }));
    }
    Ok(None)
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
    let (repo, ws, db) = ctx.workspace_and_db_with_perm(guard.read_permission())?;
    WorkspaceState::from_workspace_with_db(
        &ws,
        &mut meta,
        &repo,
        std::collections::BTreeMap::new(),
        &db,
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
