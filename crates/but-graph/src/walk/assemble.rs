//! Assemble the walk's input: normalize the seeds, decide their queue order, and derive
//! the facts the walker consults while it runs.
//!
//! [`InitialSeeds`] is the product: the ordered seed list plus the auxiliary tables —
//! target/local links, target refs, symbolic remote names — each derived once from the
//! seeds themselves. The behind-target ancestry proof lives here too: it is a seeding
//! decision, made before anything walks.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::ensure;
use but_core::{RefMetadata, extract_remote_name_and_short_name, ref_metadata::ProjectMeta};

use super::seed::{Seed, SeedRole, push_integrated_seed_once, push_seed_once};
use super::types::{Goals, Limit};
use super::utils::{obtain_workspace_infos, try_refname_to_id};
use super::{Overlay, WorktreeTip};
use crate::CommitFlags;
use crate::SegmentMetadata;
use crate::walk::overlay::{OverlayMetadata, OverlayRepo};

/// The complete pre-traversal plan derived from either explicit seeds or
/// workspace metadata.
///
/// `queue_initial_seeds()` consumes this to mint the seed-table records and fill the traversal
/// queue, and provides the auxiliary ref/remote information the traversal and build need. Each
/// seed gets its own (possibly empty) segment during the walk.
pub(super) struct InitialSeeds {
    /// Ordered traversal roots to turn into segments and queue items.
    pub(super) seeds: Vec<Seed>,
    /// Workspace commits used to ensure commits remain owned by the workspace
    /// roots that introduced them.
    pub(super) workspace_seeds: Vec<gix::ObjectId>,
    /// Workspace ref names that should be included while collecting refs by
    /// prefix, even when they are not reachable from the entrypoint yet.
    pub(super) workspace_ref_names: Vec<gix::refs::FullName>,
    /// Remote target refs already scheduled as initial integrated seeds, so
    /// `try_queue_remote_tracking_branches()` won't queue them again when
    /// local branches point at them as upstreams. Derived once from the seeds
    /// at assembly (`target_refs_from_seeds`) — a cache, not parallel state:
    /// the walker consults it per collected commit, where recomputation would
    /// cost O(commits × seeds).
    pub(super) target_refs: Vec<gix::refs::FullName>,
    /// Remote names to try when a local branch has no configured upstream:
    /// `refs/remotes/<remote>/<local-short-name>` is used if that ref exists
    /// and isn't configured for another branch.
    pub(super) symbolic_remote_names: Vec<String>,
    /// Whether metadata-derived workspace/target seeds should be front-loaded
    /// into the traversal queue after their segments are created.
    pub(super) frontload_workspace_related_seeds: bool,
    /// The caller's EXPLICIT extra target (`Options::extra_target_commit_id`) — a deliberate
    /// "connect the view down to here". Only this auxiliary anchor earns an entrypoint goal;
    /// the metadata-recorded target commit is ambient context and must not extend the walk.
    pub(super) explicit_extra_target: Option<gix::ObjectId>,
    /// Target remote/local tracking links inferred from seed refs and branch
    /// config.
    ///
    /// Needed up front because the two sides may share a commit or arrive in
    /// either order: queueing delays the target until the local side has a
    /// segment, then links both as siblings before other seeds can claim
    /// their commits.
    pub(super) target_local_links: TargetLocalLinks,
    /// Anonymous target-remote seeds that are auxiliary traversal context rather
    /// than primary target refs.
    pub(super) auxiliary_integrated_seed_ids: BTreeSet<gix::ObjectId>,
}

/// Bidirectional lookup between target remote refs and their local tracking refs.
#[derive(Default)]
pub(super) struct TargetLocalLinks {
    /// Local tracking ref by target remote ref.
    pub(super) local_by_target: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
    /// Target remote ref by local tracking ref.
    pub(super) target_by_local: BTreeMap<gix::refs::FullName, gix::refs::FullName>,
}

/// Add one seed per linked-worktree tip, so a branch only a linked worktree checks out is
/// still walked — and, when it has one, still NAMES its segment: an edit can only make a
/// worktree follow a rewrite if the ref it follows is addressable.
fn append_worktree_seeds(
    repo: &OverlayRepo<'_>,
    seeds: &mut Vec<Seed>,
    worktree_tips: Vec<WorktreeTip>,
) {
    let mut seeded: BTreeSet<_> = seeds.iter().map(|seed| seed.id).collect();
    let named: BTreeSet<_> = seeds
        .iter()
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    for tip in worktree_tips {
        let id = match &tip.ref_name {
            // Gone from the ref store (dropped by an overlay, or deleted): its recorded
            // tip is stale, and resurrecting it would show history that no ref claims.
            Some(name) => match repo.try_find_reference(name.as_ref()) {
                Ok(Some(mut reference)) => match reference.peel_to_id() {
                    Ok(id) => id.detach(),
                    Err(_) => continue,
                },
                _ => continue,
            },
            // A detached worktree has no ref to carry; only its commit matters.
            None => tip.id,
        };
        if seeded.insert(id) {
            let name = tip.ref_name.filter(|name| !named.contains(name));
            seeds.push(Seed::new(id).with_ref_name(name));
        }
    }
}

/// Build auxiliary traversal inputs from normalized seeds.
pub(super) fn assemble_initial_seeds(
    repo: &OverlayRepo<'_>,
    mut seeds: Vec<Seed>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
    worktree_tips: Vec<WorktreeTip>,
) -> InitialSeeds {
    append_worktree_seeds(repo, &mut seeds, worktree_tips);
    let mut auxiliary_integrated_seed_ids = BTreeSet::new();
    if let Some(extra_target) = extra_target_commit_id {
        auxiliary_integrated_seed_ids.insert(extra_target);
        push_integrated_seed_once(&mut seeds, extra_target);
    }
    let frontload_workspace_related_seeds = has_workspace_related_seeds(&seeds);
    if frontload_workspace_related_seeds {
        auxiliary_integrated_seed_ids.extend(seeds.iter().filter_map(|seed| {
            seed.is_anonymous_integrated_target_context()
                .then_some(seed.id)
        }));
    }
    collapse_anonymous_integrated_seeds_into_named_targets(&mut seeds);
    let seeds = seeds_in_queue_order(seeds, &auxiliary_integrated_seed_ids);
    let workspace_seeds = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::Workspace))
        .map(|seed| seed.id)
        .collect();
    let workspace_ref_names = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::Workspace))
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    let include_seed_refs = !seeds
        .iter()
        .any(|seed| matches!(seed.metadata, Some(SegmentMetadata::Workspace(_))));
    let target_refs = target_refs_from_seeds(&seeds, project_meta, include_seed_refs);
    let symbolic_remote_names =
        symbolic_remote_names_from_seeds(repo, &seeds, project_meta, include_seed_refs);
    let target_local_links = target_local_links_from_seeds(repo, &seeds);

    InitialSeeds {
        explicit_extra_target: extra_target_commit_id,
        seeds,
        workspace_seeds,
        workspace_ref_names,
        target_refs,
        symbolic_remote_names,
        frontload_workspace_related_seeds,
        target_local_links,
        auxiliary_integrated_seed_ids,
    }
}

/// Drop anonymous integrated target seeds whose commit a named integrated
/// target already covers — the named segment should own the commit.
fn collapse_anonymous_integrated_seeds_into_named_targets(seeds: &mut Vec<Seed>) {
    let named_integrated_target_ids = seeds
        .iter()
        .filter_map(|seed| {
            (matches!(seed.role, SeedRole::TargetRemote) && seed.ref_name.is_some())
                .then_some(seed.id)
        })
        .collect::<BTreeSet<_>>();
    seeds.retain(|seed| !seed.collapses_into_named_integrated_target(&named_integrated_target_ids));
}

/// Order validated seeds deterministically — queue order matters because the
/// first item to reach a commit owns its segment.
///
/// Role priority sets the broad shape, workspace metadata restores stack and
/// branch order where available, and stable tie-breakers make equivalent
/// inputs independent of caller order. Non-workspace traversals keep caller
/// order among equal priorities.
fn seeds_in_queue_order(
    seeds: Vec<Seed>,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> Vec<Seed> {
    let has_workspace_related_seeds = has_workspace_related_seeds(&seeds);
    let workspace_branch_order = workspace_branch_order_from_seeds(&seeds);
    let mut seeds: Vec<_> = seeds.into_iter().enumerate().collect();
    seeds.sort_by(|(a_idx, a), (b_idx, b)| {
        seed_queue_priority(
            a,
            has_workspace_related_seeds,
            auxiliary_integrated_seed_ids,
        )
        .cmp(&seed_queue_priority(
            b,
            has_workspace_related_seeds,
            auxiliary_integrated_seed_ids,
        ))
        .then_with(|| {
            seed_metadata_order(a, &workspace_branch_order)
                .cmp(&seed_metadata_order(b, &workspace_branch_order))
        })
        .then_with(|| {
            if has_workspace_related_seeds {
                seed_sort_name(a).cmp(&seed_sort_name(b))
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| {
            if has_workspace_related_seeds {
                a.id.cmp(&b.id)
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .then_with(|| a_idx.cmp(b_idx))
    });
    seeds.into_iter().map(|(_, seed)| seed).collect()
}

/// Whether seed ordering has to emulate workspace metadata traversal: the
/// relative order of workspace-related seeds decides commit ownership and how
/// virtual workspace/stack segments are minted, so their presence switches
/// sorting from "preserve caller order" to "rebuild metadata order".
fn has_workspace_related_seeds(seeds: &[Seed]) -> bool {
    seeds.iter().any(|seed| {
        matches!(
            seed.role,
            SeedRole::Workspace
                | SeedRole::TargetLocal { .. }
                | SeedRole::WorkspaceStackBranch { .. }
        ) || matches!(seed.metadata, Some(SegmentMetadata::Workspace(_)))
    })
}

/// Primary sort key for initial seeds. For workspace-related traversals,
/// recreate the metadata-derived segment creation order:
///
/// 1. A non-workspace reachable entrypoint, if there is one.
/// 2. The workspace ref, the traversal anchor.
/// 3. The integrated target ref, then its local tracking branch, so they can
///    be linked as siblings and agree on target ownership.
/// 4. Synthetic integrated targets, like extra target commits.
/// 5. Workspace stack branches (order refined later from metadata).
/// 6. Other reachable roots.
///
/// For non-workspace traversals there is no metadata order to recover:
/// integrated context first, non-entry reachable roots next, the entrypoint
/// last. Synthetic integrated seeds stay last — auxiliary limits, not roots.
fn seed_queue_priority(
    seed: &Seed,
    has_workspace_related_seeds: bool,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> usize {
    if has_workspace_related_seeds {
        match &seed.role {
            SeedRole::Reachable if seed.is_entrypoint => 0,
            SeedRole::Workspace => 1,
            SeedRole::TargetRemote if seed.ref_name.is_some() => 2,
            SeedRole::TargetLocal { .. } => 3,
            SeedRole::TargetRemote
                if seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids) =>
            {
                4
            }
            SeedRole::TargetRemote => 2,
            SeedRole::WorkspaceStackBranch { .. } => 5,
            SeedRole::Reachable => 6,
        }
    } else {
        match &seed.role {
            SeedRole::TargetRemote
                if seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids) =>
            {
                3
            }
            SeedRole::TargetRemote => 0,
            SeedRole::TargetLocal { .. } => 0,
            SeedRole::Reachable | SeedRole::Workspace | SeedRole::WorkspaceStackBranch { .. } => {
                if seed.is_entrypoint {
                    2
                } else {
                    1
                }
            }
        }
    }
}

/// Recover stack-branch order from workspace metadata — the only reliable way
/// for scrambled explicit input to produce the same graph and projection as
/// `from_tip()`.
///
/// Maps a branch ref to `(workspace_order, stack_order, branch_order)`:
/// workspaces sorted by their optional ref name (deterministic for
/// multi-workspace input), stacks counted among in-workspace stacks only,
/// branches by position in their stack. Refs not in the map fall back to
/// later tie-breakers; on duplicates the first metadata occurrence wins,
/// matching "first configured stack owns the branch".
fn workspace_branch_order_from_seeds(
    seeds: &[Seed],
) -> BTreeMap<gix::refs::FullName, (usize, usize, usize)> {
    let mut workspaces: Vec<_> = seeds
        .iter()
        .filter_map(|seed| match seed.metadata.as_ref() {
            Some(SegmentMetadata::Workspace(data)) => Some((seed.ref_name.as_ref(), data)),
            Some(SegmentMetadata::Branch(_)) | None => None,
        })
        .collect();
    workspaces.sort_by_key(|(ref_name, _)| *ref_name);

    let mut out = BTreeMap::new();
    for (workspace_order, (_ref_name, data)) in workspaces.into_iter().enumerate() {
        for (stack_order, stack) in data
            .stacks
            .iter()
            .filter(|stack| stack.is_in_workspace())
            .enumerate()
        {
            for (branch_order, branch) in stack.branches.iter().enumerate() {
                out.entry(branch.ref_name.clone()).or_insert((
                    workspace_order,
                    stack_order,
                    branch_order,
                ));
            }
        }
    }
    out
}

/// The metadata order for a workspace stack branch seed; `None` for other
/// roles, which are governed by role priority and later tie-breakers.
fn seed_metadata_order(
    seed: &Seed,
    workspace_branch_order: &BTreeMap<gix::refs::FullName, (usize, usize, usize)>,
) -> Option<(usize, usize, usize)> {
    match &seed.role {
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            workspace_branch_order.get(desired_ref_name).copied()
        }
        SeedRole::Reachable
        | SeedRole::Workspace
        | SeedRole::TargetRemote
        | SeedRole::TargetLocal { .. } => None,
    }
}

/// Stable name tie-breaker for workspace-related sorting: sorting by the ref
/// that will name the segment keeps caller input order irrelevant. Ignored
/// for non-workspace traversals, which preserve caller order.
fn seed_sort_name(seed: &Seed) -> Option<String> {
    match &seed.role {
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            Some(desired_ref_name.as_bstr().to_string())
        }
        SeedRole::TargetLocal { local_ref_name, .. } => Some(local_ref_name.as_bstr().to_string()),
        SeedRole::Reachable | SeedRole::Workspace | SeedRole::TargetRemote => {
            seed.ref_name.as_ref().map(|ref_name| ref_name.to_string())
        }
    }
}

/// The seeds a build from `entrypoint` would start out with on its own: the workspace,
/// the branches it declares, and the target.
///
/// [`Workspace::from_head`](crate::Workspace::from_head) resolves these internally, so
/// most callers never need them. Callers that want these AND tips of their own — a branch
/// listing, which must also reach branches outside the workspace — build the union and
/// hand it to [`Workspace::from_seeds`](crate::Workspace::from_seeds). Feeding
/// `from_seeds` extra tips ALONE would skip the metadata seeds entirely, leaving declared
/// branches without a segment to name.
pub fn seeds_from_workspace_metadata<T: RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<Vec<Seed>> {
    let (overlay_repo, overlay_meta, _entrypoint) = Overlay::default().into_parts(repo, meta);
    initial_seeds_from_workspace_metadata(
        &overlay_repo,
        &overlay_meta,
        entrypoint,
        entrypoint_ref,
        project_meta,
        extra_target_commit_id,
    )
}

/// Discover workspaces, targets, local tracking branches, and workspace stack
/// branch refs and turn them into initial traversal seeds.
pub(crate) fn initial_seeds_from_workspace_metadata<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    project_meta: &ProjectMeta,
    extra_target_commit_id: Option<gix::ObjectId>,
) -> anyhow::Result<Vec<Seed>> {
    let workspaces = obtain_workspace_infos(
        repo,
        entrypoint_ref.map(|rn| rn.as_ref()),
        entrypoint,
        project_meta,
        meta,
    )?;
    let tip_ref_matches_ws_ref = workspaces
        .iter()
        .find_map(|(ws_tip, ws_rn, _)| (Some(ws_rn) == entrypoint_ref).then_some(ws_tip));

    let mut seeds = Vec::new();
    let mut workspace_metas = Vec::new();
    // The metadata's target commit only provides target context for workspaces.
    let metadata_target_commit_id = if workspaces.is_empty() {
        None
    } else {
        project_meta.target_commit_id
    };
    let mut queued_ids = Vec::new();

    match tip_ref_matches_ws_ref {
        None => {
            // We don't name the seed of the entrypoint as we want the segment
            // naming to be handled by seeds created from metadata.
            seeds.push(Seed::entrypoint(entrypoint, None));
            queued_ids.push(entrypoint);
        }
        Some(ws_tip) => {
            ensure!(
                *ws_tip == entrypoint,
                format!(
                    "BUG:: {entrypoint_ref:?} points to {ws_tip}, but the caller claimed it points to {entrypoint}"
                )
            );
        }
    }

    for (ws_tip, ws_ref, ws_meta) in workspaces {
        workspace_metas.push(ws_meta.clone());
        seeds.push(
            Seed::new(ws_tip)
                .with_ref_name(Some(ws_ref.clone()))
                .with_role(SeedRole::Workspace)
                .with_metadata(SegmentMetadata::Workspace(ws_meta.clone()))
                .with_is_entrypoint(Some(&ws_ref) == entrypoint_ref),
        );

        let target = if let Some((target_ref, target_ref_id, local_info)) =
            workspace_target_tip(repo, project_meta.target_ref.as_ref())?
        {
            let local_info =
                local_info.filter(|(_local_ref_name, local_tip)| !queued_ids.contains(local_tip));
            seeds.push(
                Seed::new(target_ref_id)
                    .with_ref_name(Some(target_ref))
                    .with_role(SeedRole::TargetRemote),
            );
            if let Some((local_ref_name, local_tip)) = local_info.clone() {
                // A local strictly BEHIND its target converges at its own tip — an ancestry
                // fact, not a walk. Pairing goals here would make the target's lane walk the
                // whole corridor down to the stale tip (measured: 98% of a 4.6k-commit walk
                // for a 300 hint). The seed still rides along as data.
                let behind_target = local_tip != target_ref_id
                    && tracing::debug_span!("prove_local_behind_target").in_scope(|| {
                        generation_cutoff_reaches(repo, target_ref_id, local_tip).unwrap_or_else(
                            || {
                                // No commit-graph cutoff for the local: the full merge-base
                                // proves instead. Its cost is bounded by the corridor the
                                // OLD pairing would have walked anyway.
                                repo.for_find_only()
                                    .merge_base(local_tip, target_ref_id)
                                    .ok()
                                    .is_some_and(|base| base == local_tip)
                            },
                        )
                    });
                seeds.push(Seed::new(local_tip).with_role(SeedRole::TargetLocal {
                    local_ref_name,
                    behind_target,
                }));
            }
            Some((
                target_ref_id,
                local_info.map(|(_local_ref_name, local_tip)| local_tip),
            ))
        } else {
            None
        };
        queued_ids.push(ws_tip);
        if let Some((target_ref_id, local_tip)) = target {
            queued_ids.push(target_ref_id);
            if let Some(local_tip) = local_tip {
                queued_ids.push(local_tip);
            }
        }
    }

    if let Some(extra_target) = extra_target_commit_id {
        push_integrated_seed_once(&mut seeds, extra_target);
    }

    if let Some(target_commit_id) = metadata_target_commit_id {
        // Metadata may be stale — the commit might not exist (anymore). Ignore if that's the case.
        if let Err(err) = repo.find_commit(target_commit_id) {
            tracing::warn!(
                ?target_commit_id,
                ?err,
                "Ignoring stale target commit id as it didn't exist"
            );
        } else {
            push_integrated_seed_once(&mut seeds, target_commit_id);
        }
    }

    // In ad-hoc/single-branch mode the persisted branch order plays the role workspace
    // metadata plays for managed stacks: its members must start segments so the ordered
    // chain can be planned over boundaries instead of repaired after the fact. Seed them
    // like stack branches — reversed, so when several ordered refs share one tip, the
    // run's bottom-most ref (the commit owner) provides the segment name.
    if let Some(entrypoint_ref) =
        entrypoint_ref.filter(|r| r.category() == Some(gix::reference::Category::LocalBranch))
        && let Some(branch_order) = meta.branch_stack_order(entrypoint_ref.as_ref())?
    {
        for branch in branch_order.into_iter().rev() {
            if branch.category() != Some(gix::reference::Category::LocalBranch) {
                continue;
            }
            let Some(tip) = try_refname_to_id(repo, branch.as_ref())? else {
                continue;
            };
            push_seed_once(
                &mut seeds,
                Seed::new(tip).with_role(SeedRole::WorkspaceStackBranch {
                    desired_ref_name: branch,
                }),
            );
        }
    }

    // Queue workspace stack branch refs that may have advanced since the
    // workspace commit was written, and thus would not be reached from that
    // commit alone.
    for ws_metadata in workspace_metas {
        for segment in ws_metadata
            .stacks
            .into_iter()
            .filter(|s| s.is_in_workspace())
            .flat_map(|s| s.branches.into_iter())
        {
            // An unborn declared branch has no tip to seed from — it is a name waiting for
            // a commit, not a position in history.
            let Some(segment_tip) = try_refname_to_id(repo, segment.ref_name.as_ref())? else {
                continue;
            };
            push_seed_once(
                &mut seeds,
                Seed::new(segment_tip).with_role(SeedRole::WorkspaceStackBranch {
                    desired_ref_name: segment.ref_name,
                }),
            );
        }
    }

    Ok(seeds)
}

/// Does `tip` reach `ancestor` — a single-side ancestry walk from `tip`, pruned by
/// commit-graph generation: nothing whose generation is at or below `ancestor`'s can
/// still reach it, so the walk is bounded by the corridor between the two plus the
/// young fringe the commit-graph file does not cover (parsed, never pruned). `None`
/// when there is no commit-graph file or it does not cover `ancestor` — no cutoff,
/// no bound — leaving the caller to prove differently.
///
/// This replaces a full merge-base for the is-ancestor question: merge-base paints
/// from both sides at roughly twice the per-commit cost (measured ~190ms vs the
/// ~90ms corridor it saved on a 22k-stale local).
fn generation_cutoff_reaches(
    repo: &OverlayRepo<'_>,
    tip: gix::ObjectId,
    ancestor: gix::ObjectId,
) -> Option<bool> {
    let cache = repo.commit_graph_if_enabled().ok().flatten()?;
    let cutoff = cache.commit_by_id(ancestor)?.generation();
    let mut seen = gix::hashtable::HashSet::default();
    let mut stack = vec![tip];
    while let Some(id) = stack.pop() {
        if id == ancestor {
            return Some(true);
        }
        if !seen.insert(id) {
            continue;
        }
        match cache.commit_by_id(id) {
            Some(commit) => {
                if commit.generation() <= cutoff {
                    continue;
                }
                for parent in commit.iter_parents() {
                    // A corrupt entry voids the proof, not the build.
                    let pos = parent.ok()?;
                    stack.push(cache.id_at(pos).into());
                }
            }
            None => {
                let commit = repo.find_commit(id).ok()?;
                stack.extend(commit.parent_ids().map(|id| id.detach()));
            }
        }
    }
    Some(false)
}

/// A local branch ref and the commit it points to, when it tracks a workspace
/// target ref.
pub(crate) type LocalTrackingTip = (gix::refs::FullName, gix::ObjectId);

/// A workspace target ref, its commit, and optionally the local branch tracking it.
pub(crate) type WorkspaceTargetTip = (gix::refs::FullName, gix::ObjectId, Option<LocalTrackingTip>);

/// Resolve a workspace target ref and, when possible, its local tracking branch
/// tip.
pub(crate) fn workspace_target_tip(
    repo: &OverlayRepo<'_>,
    target_ref: Option<&gix::refs::FullName>,
) -> anyhow::Result<Option<WorkspaceTargetTip>> {
    let Some(target_ref) = target_ref else {
        return Ok(None);
    };
    let target_ref_id = match try_refname_to_id(repo, target_ref.as_ref()).map_err(|err| {
        tracing::warn!("Ignoring non-existing target branch {target_ref}: {err}");
        err
    }) {
        Ok(Some(target_ref_id)) => target_ref_id,
        Ok(None) | Err(_) => return Ok(None),
    };
    let local_info = repo
        .upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())
        .ok()
        .flatten()
        .and_then(|(local_tracking_name, _remote_name)| {
            let target_local_tip = try_refname_to_id(repo, local_tracking_name.as_ref()).ok()??;
            Some((local_tracking_name, target_local_tip))
        });
    Ok(Some((target_ref.clone(), target_ref_id, local_info)))
}

/// Remote target refs already represented by initial seeds, so
/// remote-tracking discovery won't queue them again. Workspace traversals
/// take these from the project metadata target ref; explicit traversals fall
/// back to named integrated seeds when `include_integrated_seed_refs` is set.
fn target_refs_from_seeds(
    seeds: &[Seed],
    project_meta: &ProjectMeta,
    include_integrated_seed_refs: bool,
) -> Vec<gix::refs::FullName> {
    let has_workspace_metadata_seed = seeds
        .iter()
        .any(|seed| matches!(seed.metadata, Some(SegmentMetadata::Workspace(_))));
    let mut target_refs: Vec<_> = seeds
        .iter()
        .filter(|seed| include_integrated_seed_refs && seed.role.is_integrated())
        .filter_map(|seed| seed.ref_name.clone())
        .chain(
            has_workspace_metadata_seed
                .then(|| project_meta.target_ref.clone())
                .flatten(),
        )
        .collect();
    target_refs.sort();
    target_refs.dedup();
    target_refs
}

/// Infer target remote/local tracking links without exposing correlation ids
/// on public seeds: a named [`SeedRole::TargetRemote`] pairs with the
/// [`SeedRole::TargetLocal`] whose ref is configured to track it. If either
/// side is absent, no sibling link is prepared up front.
pub(super) fn target_local_links_from_seeds(
    repo: &OverlayRepo<'_>,
    seeds: &[Seed],
) -> TargetLocalLinks {
    let remote_target_refs: Vec<_> = seeds
        .iter()
        .filter(|seed| matches!(seed.role, SeedRole::TargetRemote))
        .filter_map(|seed| seed.ref_name.clone())
        .collect();
    let local_refs: BTreeSet<_> = seeds
        .iter()
        .filter_map(|seed| match &seed.role {
            SeedRole::TargetLocal {
                local_ref_name,
                behind_target: false,
            } => Some(local_ref_name.clone()),
            SeedRole::TargetLocal {
                behind_target: true,
                ..
            } => None,
            SeedRole::Reachable
            | SeedRole::Workspace
            | SeedRole::WorkspaceStackBranch { .. }
            | SeedRole::TargetRemote => None,
        })
        .collect();

    let mut links = TargetLocalLinks::default();
    for target_ref in remote_target_refs {
        let Some((local_ref, _remote_name)) = repo
            .upstream_branch_and_remote_for_tracking_branch(target_ref.as_ref())
            .ok()
            .flatten()
        else {
            continue;
        };
        if !local_refs.contains(&local_ref) {
            continue;
        }
        links
            .local_by_target
            .insert(target_ref.clone(), local_ref.clone());
        links.target_by_local.insert(local_ref, target_ref);
    }
    links
}

/// Collect symbolic remote names implied by seed refs, workspace target refs,
/// workspace `push_remote` settings, and stack branch refs.
fn symbolic_remote_names_from_seeds(
    repo: &OverlayRepo<'_>,
    seeds: &[Seed],
    project_meta: &ProjectMeta,
    include_seed_refs: bool,
) -> Vec<String> {
    let remote_names = repo.remote_names();
    let refs = seeds
        .iter()
        .filter_map(|seed| {
            include_seed_refs
                .then_some(seed.ref_name.as_ref())
                .flatten()
        })
        .filter_map({
            let remote_names = &remote_names;
            move |ref_name| {
                extract_remote_name_and_short_name(ref_name.as_ref(), remote_names)
                    .map(|(remote, _short_name)| (1, remote))
            }
        });
    let workspace_metadata_names = seeds
        .iter()
        .filter_map(|seed| match seed.metadata.as_ref() {
            Some(SegmentMetadata::Workspace(data)) => Some(data),
            Some(SegmentMetadata::Branch(_)) | None => None,
        })
        .flat_map(|data| {
            data.stacks.iter().flat_map(|s| {
                s.branches.iter().flat_map(|b| {
                    extract_remote_name_and_short_name(b.ref_name.as_ref(), &remote_names)
                        .map(|(remote, _short_name)| (1, remote))
                })
            })
        });
    let desired_refs = seeds.iter().filter_map(|seed| match &seed.role {
        _ if !include_seed_refs => None,
        SeedRole::WorkspaceStackBranch { desired_ref_name } => {
            extract_remote_name_and_short_name(desired_ref_name.as_ref(), &remote_names)
                .map(|(remote, _short_name)| (1, remote))
        }
        SeedRole::Reachable
        | SeedRole::Workspace
        | SeedRole::TargetLocal { .. }
        | SeedRole::TargetRemote => None,
    });
    let target_ref = project_meta.target_ref.as_ref().and_then(|target_ref| {
        extract_remote_name_and_short_name(target_ref.as_ref(), &remote_names)
            .map(|(remote, _short_name)| (1, remote))
    });
    let push_remote = project_meta
        .push_remote
        .as_ref()
        .map(|push_remote| (0, push_remote.clone()));
    sorted_symbolic_remote_names(
        refs.chain(workspace_metadata_names)
            .chain(desired_refs)
            .chain(target_ref)
            .chain(push_remote),
    )
}

/// Sort and deduplicate remote names, preserving explicit push remotes before
/// remotes inferred from refs with the same name.
fn sorted_symbolic_remote_names(names: impl Iterator<Item = (usize, String)>) -> Vec<String> {
    let mut names: Vec<_> = names.collect();
    names.sort();
    names.dedup();
    names.into_iter().map(|(_order, remote)| remote).collect()
}

/// The second half of the ordering heuristic: `seeds_in_queue_order()` fixes
/// segment creation order, but some roles must also be *visited* first so
/// they own shared commits.
///
/// Synthetic integrated seeds always front-load (limits, not user roots);
/// workspace, target, and target-local seeds front-load so target ownership
/// and sibling links settle before stack branches can claim shared commits.
/// Stack branches deliberately are not front-loaded — their traversal work
/// should follow the workspace/target context.
pub(super) fn queue_should_frontload_seed(
    seed: &Seed,
    frontload_workspace_related_seeds: bool,
    auxiliary_integrated_seed_ids: &BTreeSet<gix::ObjectId>,
) -> bool {
    seed.is_auxiliary_integrated_seed(auxiliary_integrated_seed_ids)
        || (frontload_workspace_related_seeds
            && matches!(
                seed.role,
                SeedRole::Workspace | SeedRole::TargetRemote | SeedRole::TargetLocal { .. }
            ))
}

/// Return the flags and limit used by a reachable seed seeking the entrypoint.
pub(super) fn reachable_seed_flags_and_limit(
    seed: gix::ObjectId,
    entrypoint: gix::ObjectId,
    max_limit: Limit,
    goals: &mut Goals,
) -> (CommitFlags, Limit) {
    let limit = if seed == entrypoint {
        max_limit
    } else {
        max_limit.with_indirect_goal(entrypoint, goals)
    };
    (CommitFlags::NotInRemote, limit)
}
