//! The graph builders: every [`Graph`](crate::Graph) is assembled here from a [`CommitGraph`]
//! flattened out of the raw traversal ([`CommitGraph::from_walk`]). The builders reconstruct the
//! FULL segment graph (workspace / branch / anonymous / target / remote segments, their
//! first-parent connections, generations, and remote↔local sibling links) so that everything
//! downstream — projection, renderer, consumers — sees one graph shape regardless of how the
//! build was entered (managed workspace, non-managed checkout, explicit tips, overlays).
//!
//! The build is phased (gather-then-build), one file per phase: [`facts`] (pure boundary/shape
//! reads) → [`plan`] (the chain plan, decided entirely as data) → [`materialize`] (segments
//! minted in their final shape), with [`chains`] and [`remotes`] holding the structure passes
//! materialization drives. Entry points and shared helpers live here.

use std::collections::{BTreeMap, HashMap, HashSet};

use but_core::RefMetadata;

/// `ObjectId`-keyed map/set on the prehashed [`gix::hashtable`] — ids are touched per commit in
/// the hot build loops, where SipHash on 20-byte keys dominates profiles.
type IdMap<V> = gix::hashtable::HashMap<gix::ObjectId, V>;
type IdSet = gix::hashtable::HashSet<gix::ObjectId>;
use gix::reference::Category;

use crate::{
    CommitGraph, SegmentIndex,
    init::overlay::{OverlayMetadata, OverlayRepo},
    segment_graph::{Connection, SegmentGraph},
};

use materialize::graph_from_commit_graph;
use remotes::remote_tracking_from_repository;

mod chains;
mod facts;
mod materialize;
mod plan;
mod position_ir;
mod ref_positions;
mod remotes;

/// Build the managed-workspace segment [`Graph`](crate::Graph) straight from a git `CommitGraph`,
/// deriving the enrichment inputs from `(repo, meta, project_meta)` — the flip entry for
/// [`Graph::from_head`](crate::Graph::from_head).
pub fn graph_from_repository<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<Option<crate::Graph>> {
    graph_from_repository_with_overlay(
        repo,
        meta,
        entrypoint,
        entrypoint_ref,
        project_meta,
        options,
        crate::init::Overlay::default(),
    )
}

/// [`graph_from_repository`] with the workspace projection applied — the flip test seam for
/// asserting projection-level parity. `None` on the same non-managed fall-through.
pub fn workspace_from_repository<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<Option<crate::Workspace>> {
    graph_from_repository(
        repo,
        meta,
        entrypoint,
        entrypoint_ref,
        project_meta,
        options,
    )?
    .map(crate::Graph::into_workspace)
    .transpose()
}

/// Project a PROVIDED `cg` — the write-through seam: an editor-mutated commit graph projects
/// like a fresh walk, with the enrichment (refs, target, metadata) read from the CURRENT
/// `repo`/`meta` state. Dispatches like [`Graph::from_head`](crate::Graph::from_head):
/// managed with `HEAD` as entrypoint when the workspace ref resolves to a commit in `cg`
/// (falling through to non-managed when the entrypoint lands outside the workspace),
/// non-managed from `HEAD` otherwise. `None` when `HEAD` is unborn or unresolvable.
pub fn workspace_from_commit_graph<T: but_core::RefMetadata>(
    mut cg: CommitGraph,
    repo: &gix::Repository,
    meta: &T,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<Option<crate::Workspace>> {
    let (overlay_repo, overlay_meta, _entrypoint) =
        crate::init::Overlay::default().into_parts(repo, meta);
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ws_tip_on_disk = overlay_repo
        .try_find_reference(ws_ref.as_ref())?
        .and_then(|mut r| r.peel_to_commit().ok())
        .map(|c| c.id().detach());
    let ws_commit = ws_tip_on_disk.filter(|c| cg.row(*c).is_some());
    // The walk seeds `InWorkspace` and the project target only for a workspace with METADATA
    // whose ref resolves — a bare `gitbutler/workspace` ref without it walks (and projects)
    // as a plain branch.
    let ws_meta = overlay_meta.workspace_opt(ws_ref.as_ref())?;
    let ws_has_meta = ws_meta.is_some();
    let ws_exists = ws_has_meta && ws_tip_on_disk.is_some();
    // A target tip the editor dropped or rewrote away is still external context on disk —
    // the walk seeds it as an integrated tip whenever the commit exists, so re-represent it.
    // The stored target commit and the target REF tip only count when a workspace exists (the
    // walk pushes them per discovered workspace); the extra target is seeded unconditionally.
    let target_ref_tip = match project_meta.target_ref.as_ref().filter(|_| ws_exists) {
        Some(tr) => overlay_repo
            .try_find_reference(tr.as_ref())?
            .and_then(|mut r| r.peel_to_commit().ok())
            .map(|c| c.id().detach()),
        None => None,
    };
    let target_tips = || {
        project_meta
            .target_commit_id
            .filter(|_| ws_exists)
            .into_iter()
            .chain(options.extra_target_commit_id)
            .chain(target_ref_tip)
    };
    for tip in target_tips() {
        ensure_tip_region(&mut cg, &overlay_repo, tip, crate::CommitFlags::Integrated)?;
    }
    ensure_remote_regions(&mut cg, repo, &overlay_repo, &project_meta)?;
    let head = repo.head()?;
    let head_ref = head.referent_name().map(|n| n.to_owned());
    let head_tip = head.id().map(|id| id.detach());
    // HEAD is external context too: the editor's graph need not contain the checked-out commit
    // (e.g. an edit-mode WIP commit) — the walk always traverses the entrypoint's region.
    if let Some(tip) = head_tip {
        ensure_tip_region(&mut cg, &overlay_repo, tip, crate::CommitFlags::empty())?;
    }
    let head_tip = head_tip.filter(|c| cg.row(*c).is_some());
    // Reconcile edges LAST: the region steps above revive tombstones, and a revival flips
    // effective parents that must then be re-validated against the odb.
    complete_parents_from_odb(&mut cg, &overlay_repo)?;
    cg.recompute_integrated(target_tips());
    cg.recompute_in_workspace(ws_commit.filter(|_| ws_has_meta));
    // `NotInRemote` mirrors the walk's seeding: only tips the walk QUEUES seed it — HEAD,
    // and for a discovered workspace its tip, the target's local tracking branch, and the
    // workspace stack branch refs. A local branch that merely points into remote-reachable
    // history is NOT a seed, and an editor drop must lose the flag entirely (a stale flag
    // would hide the remote region from projection).
    let mut not_in_remote_tips: Vec<gix::ObjectId> = head_tip.into_iter().collect();
    if ws_exists {
        not_in_remote_tips.extend(ws_tip_on_disk);
        if let Some((_, _, Some((_, local_tip)))) =
            crate::init::workspace_target_tip(&overlay_repo, project_meta.target_ref.as_ref())?
        {
            not_in_remote_tips.push(local_tip);
        }
        for branch in ws_meta
            .iter()
            .flat_map(|ws| ws.stacks.iter())
            .filter(|s| s.is_in_workspace())
            .flat_map(|s| s.branches.iter())
        {
            not_in_remote_tips.extend(crate::init::walk::try_refname_to_id(
                &overlay_repo,
                branch.ref_name.as_ref(),
            )?);
        }
    }
    cg.recompute_not_in_remote(not_in_remote_tips);
    cg.recompute_generations();
    // Editor tombstones are seam-internal: reconciliation left no live edge into them, so
    // compaction yields the same graph a fresh walk would — which matters now that the
    // projection's carried graph becomes THE workspace graph downstream consumers reuse.
    cg.compact();
    let ref_prefixes = || {
        ["refs/heads/", "refs/remotes/"]
            .into_iter()
            .chain(options.collect_tags.then_some("refs/tags/"))
    };
    let head_on_ws = head_ref
        .as_ref()
        .is_some_and(|r| but_core::is_workspace_ref_name(r.as_ref()));
    if let Some(ws_commit) = ws_commit {
        // The dispatch's entrypoint rule: HEAD on the workspace ref is the plain case,
        // any other checkout is an entrypoint split within the managed workspace.
        let (entrypoint, entrypoint_ref) = match head_tip {
            Some(tip) if !head_on_ws => (tip, head_ref.clone()),
            _ => (ws_commit, None),
        };
        let main_head_ref = if entrypoint == ws_commit {
            entrypoint_ref.clone().or_else(|| Some(ws_ref.clone()))
        } else {
            entrypoint_ref.clone()
        };
        let mut managed_cg = cg.clone();
        let mut refs_by_id =
            overlay_repo.collect_ref_mapping_by_prefix(ref_prefixes(), &[ws_ref.as_ref()])?;
        let worktree_by_branch =
            overlay_repo.worktree_branches(main_head_ref.as_ref().map(|r| r.as_ref()))?;
        managed_cg.refresh_refs(&mut refs_by_id, &worktree_by_branch);
        let graph = assemble_managed(
            managed_cg,
            repo,
            &overlay_repo,
            &overlay_meta,
            &ws_ref,
            ws_commit,
            entrypoint,
            entrypoint_ref,
            main_head_ref.as_ref(),
            project_meta.clone(),
            options.clone(),
        )?;
        // The entrypoint never made it into a segment — mirror the dispatch's fall-through
        // to the non-managed view.
        if graph.entrypoint.is_some() {
            return graph.into_workspace().map(Some);
        }
    }
    let Some(head_tip) = head_tip else {
        return Ok(None);
    };
    let mut refs_by_id = overlay_repo.collect_ref_mapping_by_prefix(ref_prefixes(), &[])?;
    let worktree_by_branch =
        overlay_repo.worktree_branches(head_ref.as_ref().map(|r| r.as_ref()))?;
    cg.refresh_refs(&mut refs_by_id, &worktree_by_branch);
    assemble_unmanaged(
        cg,
        repo,
        &overlay_repo,
        &overlay_meta,
        head_tip,
        head_ref,
        project_meta,
        options,
    )?
    .into_workspace()
    .map(Some)
}

/// Reconcile every live row's parents with the odb — the write-through seam's edge refresh.
/// After materialization the odb is authoritative for every kept id: an editor pick applied
/// AS-IS carries no arena edges at all (parents implied by the odb), and a `preserved_parents`
/// pick was WRITTEN with parents its arena position never had (edit mode's parent override).
/// A row is kept only when its RAW recorded parents equal its odb parents AND no connected
/// slot targets a tombstone — the projection reads the raw payload, so a stale id left behind
/// by an editor drop is as much a divergence as a wrong edge. Walk cuts (absent slots with
/// odb-true payload) survive. Anything else is rewired to the odb parents, adding (or
/// reviving) missing commits recursively, each reconciled the same way.
fn complete_parents_from_odb(cg: &mut CommitGraph, repo: &OverlayRepo<'_>) -> anyhow::Result<()> {
    let mut queue: Vec<gix::ObjectId> = (0..cg.row_count())
        .filter_map(|idx| cg.row_payload(idx))
        .collect();
    while let Some(id) = queue.pop() {
        let idx = cg.index_of(id).expect("queued ids are live");
        let Ok(commit) = repo.find_commit(id) else {
            continue;
        };
        let mut odb_parents: Vec<_> = commit.parent_ids().map(|p| p.detach()).collect();
        // Collapse exact duplicate parents like the walk and the graph reader do (a
        // workspace merge encodes empty stacks as repeated parents).
        if odb_parents.len() > 1 {
            let mut deduped = Vec::with_capacity(odb_parents.len());
            for p in odb_parents {
                if !deduped.contains(&p) {
                    deduped.push(p);
                }
            }
            odb_parents = deduped;
        }
        if cg.raw_parent_ids(idx) == odb_parents && !cg.has_tombstoned_parent(idx) {
            continue;
        }
        let mut revived = Vec::new();
        let parent_indices = odb_parents
            .iter()
            .map(|&p| {
                if cg.revive(p) {
                    revived.push(p);
                }
                cg.index_of(p).unwrap_or_else(|| {
                    queue.push(p);
                    cg.add_row(Some(p))
                })
            })
            .collect();
        cg.set_parents(idx, parent_indices);
        // A revived row is newly live — it wasn't in the initial sweep, so validate it now.
        // (Its children need no re-queue: any child kept earlier already passed the
        // tombstone-free check, so it can't have been substituting through this row.)
        queue.extend(revived);
    }
    Ok(())
}

/// The write-through seam's external-context refresh: `tip` (a stored/extra target, or a
/// remote-tracking tip) still exists on disk even when the editor dropped its row (tombstoned)
/// or rewrote it in place (the row now holds the rewritten id, while e.g. the remote ref still
/// points at the old commit). Revive tombstones, and append any missing region — walking the odb
/// from `tip` down to commits the graph knows — with `flags` (Integrated for target-seeded
/// tips, empty for remote-ahead regions, the walk's conventions). A stale tip (unresolvable
/// commit) is ignored, like the walk does.
fn ensure_tip_region(
    cg: &mut CommitGraph,
    repo: &OverlayRepo<'_>,
    tip: gix::ObjectId,
    flags: crate::CommitFlags,
) -> anyhow::Result<()> {
    let mut to_add = Vec::new();
    let mut queue = vec![tip];
    let mut seen = HashSet::new();
    while let Some(id) = queue.pop() {
        if !seen.insert(id) || cg.index_of(id).is_some() {
            continue;
        }
        cg.revive(id);
        if cg.index_of(id).is_some() {
            continue;
        }
        let Ok(commit) = repo.find_commit(id) else {
            return Ok(());
        };
        let parents: Vec<_> = commit.parent_ids().map(|p| p.detach()).collect();
        queue.extend(parents.iter().copied());
        to_add.push((id, parents));
    }
    let indices: Vec<_> = to_add.iter().map(|(id, _)| cg.add_row(Some(*id))).collect();
    for ((_, parents), &idx) in to_add.iter().zip(&indices) {
        let parent_indices = parents
            .iter()
            .map(|p| {
                cg.index_of(*p)
                    .expect("parent was added or already present")
            })
            .collect();
        cg.set_parents(idx, parent_indices);
        cg.set_flags(idx, flags);
    }
    Ok(())
}

/// The remote half of the seam's external-context refresh: the walk traverses the AHEAD regions
/// of configuration-implied remote-tracking branches, so when the editor's in-place rewrite made
/// a remote tip's commit vanish from the arena (the local advanced past it), re-append that
/// region from the odb. Only remotes of locals the graph actually contains matter — anything
/// else the walk wouldn't have reached either.
fn ensure_remote_regions(
    cg: &mut CommitGraph,
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<()> {
    let (remote_tracking, _symbolic_remotes) = remote_tracking_from_repository(repo, project_meta)?;
    for (local, remote) in remote_tracking {
        let local_tip = overlay_repo
            .try_find_reference(local.as_ref())?
            .and_then(|mut r| r.peel_to_commit().ok())
            .map(|c| c.id().detach());
        if local_tip.is_none_or(|tip| cg.index_of(tip).is_none()) {
            continue;
        }
        let Some(remote_tip) = overlay_repo
            .try_find_reference(remote.as_ref())?
            .and_then(|mut r| r.peel_to_commit().ok())
            .map(|c| c.id().detach())
        else {
            continue;
        };
        ensure_tip_region(cg, overlay_repo, remote_tip, crate::CommitFlags::empty())?;
    }
    Ok(())
}

/// Like [`graph_from_repository`], but serving `overlay` refs and metadata from memory — the flip
/// counterpart of [`Graph::redo_traversal_with_overlay`](crate::Graph::redo_traversal_with_overlay).
pub(crate) fn graph_from_repository_with_overlay<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
    overlay: crate::init::Overlay,
) -> anyhow::Result<Option<crate::Graph>> {
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.clone().into_parts(repo, meta);
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    // No (usable) workspace ref means no managed workspace — signal fall-through, don't fail:
    // the dispatch routes any repository through here and builds non-managed on `Ok(None)`.
    let Some(ws_commit) = overlay_repo
        .try_find_reference(ws_ref.as_ref())?
        .and_then(|mut r| r.peel_to_commit().ok())
        .map(|c| c.id().detach())
    else {
        return Ok(None);
    };
    // Run the WALK's real traversal (queue, goals, limits, flag propagation) to collect the commits:
    // extents and flags are exactly the walk's, and segments are the derived view built on top.
    let walk_tip = entrypoint.unwrap_or(ws_commit);
    let walk_ref = if entrypoint.is_none() || entrypoint == Some(ws_commit) {
        entrypoint_ref.clone().or(Some(ws_ref.clone()))
    } else {
        entrypoint_ref.clone()
    };
    let cg = CommitGraph::from_walk(
        repo,
        meta,
        walk_tip,
        walk_ref.clone(),
        project_meta.clone(),
        options.clone(),
        overlay,
    )?;
    let ep = entrypoint.unwrap_or(ws_commit);
    let graph = assemble_managed(
        cg,
        repo,
        &overlay_repo,
        &overlay_meta,
        &ws_ref,
        ws_commit,
        ep,
        entrypoint_ref,
        walk_ref.as_ref(),
        project_meta,
        options,
    )?;
    // The entrypoint never made it into a segment — it wasn't reached by the traversal at all
    // (outside entrypoints ARE covered, via their own region). Signal fall-through so the
    // dispatch builds the non-managed view instead of returning an unusable graph.
    if graph.entrypoint.is_none() {
        return Ok(None);
    }
    Ok(Some(graph))
}

/// Build a segment [`Graph`](crate::Graph) for a NON-managed checkout — a plain branch or detached
/// HEAD, with no `gitbutler/workspace` merge. `head_tip` is the checked-out commit (the graph's tip).
/// A detached HEAD is anonymized by `from_head`'s detach pass, not here.
pub(crate) fn graph_from_repository_unmanaged<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    graph_from_repository_unmanaged_with_overlay(
        repo,
        meta,
        head_tip,
        entrypoint_ref,
        project_meta,
        options,
        crate::init::Overlay::default(),
    )
}

/// Like [`graph_from_repository_unmanaged`], but serving `overlay` refs and metadata from memory.
#[allow(clippy::too_many_arguments)]
pub(crate) fn graph_from_repository_unmanaged_with_overlay<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
    overlay: crate::init::Overlay,
) -> anyhow::Result<crate::Graph> {
    // The walk's real traversal, exactly like the managed builder: extents, limits, flags, and
    // overlay handling are the walk's by construction.
    let cg = CommitGraph::from_walk(
        repo,
        meta,
        head_tip,
        entrypoint_ref.clone(),
        project_meta.clone(),
        options.clone(),
        overlay.clone(),
    )?;
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.into_parts(repo, meta);
    assemble_unmanaged(
        cg,
        repo,
        &overlay_repo,
        &overlay_meta,
        head_tip,
        entrypoint_ref,
        project_meta,
        options,
    )
}

/// Like [`graph_from_repository`], but seeded from explicit `tips` — the flip counterpart of
/// [`Graph::from_commit_traversal_tips`](crate::Graph::from_commit_traversal_tips). The tips'
/// normalized traversal roles are carried onto the returned graph (`traversal_tips`), which the
/// projection reads for tips-built graphs.
pub(crate) fn graph_from_repository_tips<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    tips: Vec<crate::init::Tip>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    let overlay = crate::init::Overlay::default();
    let cg = CommitGraph::from_walk_tips(
        repo,
        meta,
        tips,
        project_meta.clone(),
        options.clone(),
        overlay.clone(),
    )?;
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.into_parts(repo, meta);
    let entrypoint = cg
        .entrypoint()
        .ok_or_else(|| anyhow::anyhow!("explicit tips always contain an entrypoint"))?;
    let entrypoint_ref = cg.entrypoint_ref().cloned();
    let carried_tips = cg.traversal_tips.clone();

    // Managed only when the workspace ref resolves AND the tips traversal actually reached its
    // commit — explicit tips define the graph's extent, they don't discover a workspace on their
    // own.
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ws_commit = overlay_repo
        .try_find_reference(ws_ref.as_ref())?
        .and_then(|mut r| r.peel_to_commit().ok())
        .map(|c| c.id().detach())
        .filter(|c| cg.row(*c).is_some());

    let mut graph = if let Some(ws_commit) = ws_commit {
        // A workspace-ref entrypoint is the plain from_head case: no explicit entrypoint ref.
        let ep_ref = entrypoint_ref
            .clone()
            .filter(|r| !but_core::is_workspace_ref_name(r.as_ref()));
        assemble_managed(
            cg,
            repo,
            &overlay_repo,
            &overlay_meta,
            &ws_ref,
            ws_commit,
            entrypoint,
            ep_ref,
            entrypoint_ref.as_ref(),
            project_meta,
            options,
        )?
    } else {
        assemble_unmanaged(
            cg,
            repo,
            &overlay_repo,
            &overlay_meta,
            entrypoint,
            entrypoint_ref,
            project_meta,
            options,
        )?
    };
    // Tips-built graphs carry their seeds' ROLES — the projection reads them (e.g. integrated
    // tips), unlike graphs discovered from a workspace.
    graph.traversal_tips = carried_tips;
    // A detached entrypoint tip keeps its refs on the commit, like `from_head`'s detach pass.
    if graph
        .traversal_tips
        .iter()
        .any(|t| t.is_entrypoint && t.is_detached)
    {
        graph.detach_entrypoint_segment()?;
    }
    Ok(graph)
}

/// The enrichment inputs every builder entry derives from `(repo, project_meta)` and the overlay
/// views.
struct EnrichmentInputs {
    /// Integration marks and `NotInRemote` come from the walk's traversal — no re-flagging
    /// needed. The target commit is resolved from the CALLER's project meta for the builder's
    /// boundaries; a default `ProjectMeta` means no target (no hard-coded `origin/main`
    /// fallback), like the walk.
    target: Option<gix::ObjectId>,
    /// Remote-tracking relationships come from git CONFIG plus the caller's project meta —
    /// overlay refs don't reshape them.
    remote_tracking: HashMap<gix::refs::FullName, gix::refs::FullName>,
    symbolic_remotes: Vec<String>,
    /// Which worktree (if any) checks out each ref — the main worktree `[🌳]` and any linked
    /// worktrees `[📁]`, keyed by ref name.
    worktree_by_branch: BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
}

fn enrichment_inputs(
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    // The main-HEAD referent (like the walk's `graph.entrypoint_ref`): an overlay may override
    // HEAD onto the workspace ref for a future-state preview, which the dispatched
    // `entrypoint_ref` (None for workspace tips) would lose.
    main_head_ref: Option<&gix::refs::FullName>,
) -> anyhow::Result<EnrichmentInputs> {
    let target = project_meta.target_ref.clone().and_then(|tr| {
        Some(
            overlay_repo
                .try_find_reference(tr.as_ref())
                .ok()??
                .peel_to_commit()
                .ok()?
                .id()
                .detach(),
        )
    });
    let (remote_tracking, symbolic_remotes) = remote_tracking_from_repository(repo, project_meta)?;
    let worktree_by_branch = overlay_repo.worktree_branches(main_head_ref.map(|r| r.as_ref()))?;
    Ok(EnrichmentInputs {
        target,
        remote_tracking,
        symbolic_remotes,
        worktree_by_branch,
    })
}

/// Only IN-WORKSPACE stacks form chains. An inactive/outside stack's branches never splice as
/// empty segments (`unapplied_branch_on_base`: "This will be an empty workspace") — they
/// contribute only branch METADATA, which names commit-holding segments via the metadata tier
/// of disambiguation. A branch listed in SEVERAL stacks counts once, like the walk, which
/// ignores duplicate stack branch tips.
fn in_workspace_stack_branches(
    ws: &but_core::ref_metadata::Workspace,
) -> Vec<Vec<gix::refs::FullName>> {
    let mut seen_branches = HashSet::new();
    ws.stacks
        .iter()
        .filter(|s| s.is_in_workspace())
        .map(|s| {
            s.branches
                .iter()
                .map(|b| b.ref_name.clone())
                .filter(|b| seen_branches.insert(b.clone()))
                .collect()
        })
        .collect()
}

/// Assemble the MANAGED-workspace graph from `cg`: workspace metadata defines the chains, and the
/// enrichment reads go through the overlay views so in-memory previews (apply/unapply) see the
/// future state, not the on-disk one.
#[allow(clippy::too_many_arguments)]
fn assemble_managed<T: but_core::RefMetadata>(
    mut cg: CommitGraph,
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    overlay_meta: &OverlayMetadata<'_, T>,
    ws_ref: &gix::refs::FullName,
    ws_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    main_head_ref: Option<&gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    cg.mark_managed_ws_commit_by_message(repo, ws_commit);
    let ws_meta = overlay_meta.workspace(ws_ref.as_ref())?;
    let stack_branches = in_workspace_stack_branches(&ws_meta);
    let inputs = enrichment_inputs(repo, overlay_repo, &project_meta, main_head_ref)?;
    let (f, plan, mut arrangement) = plan::gather_and_plan(
        &cg,
        ws_commit,
        entrypoint,
        entrypoint_ref.as_ref(),
        inputs.target,
        &inputs.remote_tracking,
        &inputs.symbolic_remotes,
        Some(&stack_branches),
        true,
        overlay_meta,
        &project_meta,
        &options,
    );
    let (mut graph, native) = graph_from_commit_graph(
        &cg,
        f,
        &plan,
        &arrangement,
        ws_commit,
        entrypoint,
        entrypoint_ref,
        &inputs.remote_tracking,
        &inputs.symbolic_remotes,
        Some(&stack_branches),
        true,
        &inputs.worktree_by_branch,
        overlay_meta,
        project_meta,
        options,
    );
    let positions = ref_positions::author_positions(&native, &graph, repo)?;
    if cfg!(debug_assertions) {
        position_ir::debug_walk_positions_parity(&graph, repo, &positions);
    }
    arrangement.positions = Some(positions);
    cg.arrangement = Some(arrangement);
    graph.commit_graph = Some(cg);
    graph.remote_tracking = inputs.remote_tracking;
    Ok(graph)
}

/// Assemble the NON-managed graph from `cg`: no stack or workspace-ref passes, plus the
/// persisted single-branch ordering.
#[allow(clippy::too_many_arguments)]
fn assemble_unmanaged<T: but_core::RefMetadata>(
    mut cg: CommitGraph,
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    overlay_meta: &OverlayMetadata<'_, T>,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    let inputs = enrichment_inputs(repo, overlay_repo, &project_meta, entrypoint_ref.as_ref())?;
    let (f, plan, mut arrangement) = plan::gather_and_plan(
        &cg,
        head_tip,
        head_tip,
        entrypoint_ref.as_ref(),
        inputs.target,
        &inputs.remote_tracking,
        &inputs.symbolic_remotes,
        None,
        false,
        overlay_meta,
        &project_meta,
        &options,
    );
    let (mut graph, mut native) = graph_from_commit_graph(
        &cg,
        f,
        &plan,
        &arrangement,
        head_tip,
        head_tip,
        entrypoint_ref,
        &inputs.remote_tracking,
        &inputs.symbolic_remotes,
        None,
        false,
        &inputs.worktree_by_branch,
        overlay_meta,
        project_meta,
        options,
    );
    graph.ad_hoc_branch_stack_upgrades(overlay_repo, overlay_meta, &inputs.worktree_by_branch)?;
    position_ir::replay_ad_hoc(&mut native, &cg, &graph.ad_hoc_branch_stack_orders);
    if cfg!(debug_assertions) {
        position_ir::debug_check(&native, &graph.inner, "ad_hoc");
    }
    let positions = ref_positions::author_positions(&native, &graph, repo)?;
    if cfg!(debug_assertions) {
        position_ir::debug_walk_positions_parity(&graph, repo, &positions);
    }
    arrangement.positions = Some(positions);
    cg.arrangement = Some(arrangement);
    graph.commit_graph = Some(cg);
    graph.remote_tracking = inputs.remote_tracking;
    Ok(graph)
}

/// Find the segment named exactly `ref_name`, if any.
fn segment_by_ref(sg: &SegmentGraph, ref_name: &gix::refs::FullName) -> Option<SegmentIndex> {
    sg.segment_ids().find(|&sidx| {
        sg.segment(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| &ri.ref_name == ref_name)
    })
}

/// Like [`segment_by_commit`], but ignoring `exclude`d segments — the pre-chain coverage view
/// for gates that historically ran before the chain structure existed.
fn segment_by_commit_excluding(
    sg: &SegmentGraph,
    commit: gix::ObjectId,
    exclude: &HashSet<SegmentIndex>,
) -> Option<SegmentIndex> {
    sg.segment_ids().find(|&sidx| {
        !exclude.contains(&sidx)
            && sg
                .segment(sidx)
                .is_some_and(|s| s.commits.iter().any(|c| c.id == commit))
    })
}

/// Find the segment that holds `commit`, if any.
fn segment_by_commit(sg: &SegmentGraph, commit: gix::ObjectId) -> Option<SegmentIndex> {
    sg.segment_ids().find(|&sidx| {
        sg.segment(sidx)
            .is_some_and(|s| s.commits.iter().any(|c| c.id == commit))
    })
}

/// Connect `src` → `dst` with final endpoints: the source's last commit and the target's
/// first. Every builder edge is created after both segments hold their final commits, so
/// endpoints never need repair.
fn connect(sg: &mut SegmentGraph, src: SegmentIndex, dst: SegmentIndex) {
    let conn = Connection::new(dst, None, None, None, None).adjusted_for(src, dst, sg);
    sg.add_edge(src, conn);
}

/// All ancestors of `start` (inclusive) present in the graph, walking every parent.
fn ancestors(cg: &CommitGraph, start: gix::ObjectId) -> IdSet {
    let mut seen = IdSet::default();
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        if cg.row(c).is_none() {
            continue;
        }
        if seen.insert(c) {
            stack.extend(cg.all_parent_ids(c));
        }
    }
    seen
}

/// The unambiguous local-branch at `c`: prefer the single branch with a remote-tracking branch, else
/// the single branch overall (mirrors the projection's remote-tiered disambiguation).
/// Pick the local branch that names the segment at `c`, mirroring the walk's tiers: ABOVE the base the
/// unique branch with GitButler METADATA wins (`disambiguate_refs_by_branch_metadata` — a stack branch
/// beats the target's local ref, e.g. `A` over `main`); at/below the base (Integrated) the target's
/// local position wins instead (e.g. `main` over the stack's empty `below`, which floats above). Then
/// the unique REMOTE-TRACKED branch (the walk's remote-local-tracking naming, e.g. `main` over a plain
/// `new-A`), then the only branch, else anonymous.
fn disambiguated_ref<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    c: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    // The workspace commit, when naming happens in a managed workspace: the target-local
    // tie-break applies only to its direct parents (chain tops).
    workspace_commit: Option<gix::ObjectId>,
    target_ref: Option<&gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let row = cg.row(c)?;
    let branches: Vec<&gix::refs::FullName> = row
        .commit
        .refs
        .iter()
        .map(|r| &r.ref_name)
        .filter(|rn| is_plain_local_branch(rn))
        .collect();
    // Most commits carry no refs at all — this runs per commit in the boundary scan.
    if branches.is_empty() {
        return None;
    }
    let unique = |pred: &dyn Fn(&gix::refs::FullName) -> bool| {
        let mut it = branches.iter().copied().filter(|r| pred(r));
        it.next().filter(|_| it.next().is_none()).cloned()
    };
    let integrated = row.commit.flags.contains(crate::CommitFlags::Integrated);
    (!integrated)
        .then(|| unique(&|r| segment_metadata(r.as_ref(), meta).is_some()))
        .flatten()
        .or_else(|| unique(&|r| remote_tracking.contains_key(r)))
        // Several remote-tracked branches on a LANE TOP (a direct parent of the workspace merge):
        // a unique branch WITH metadata wins even when integrated (it is the chain the user works
        // in, e.g. `first-branch` next to a target-local `main` in gb-local mode); among several
        // metadata branches the TARGET's own local wins (e.g. `main` next to a just-applied
        // branch, both resting on the target's tip). Deeper commits stay anonymous like the walk's.
        .or_else(|| {
            workspace_commit
                .is_some_and(|ws| cg.children(c).any(|k| k == ws))
                .then(|| {
                    unique(&|r| segment_metadata(r.as_ref(), meta).is_some())
                        .or_else(|| unique(&|r| remote_tracking.get(r) == target_ref))
                })
                .flatten()
        })
        .or_else(|| unique(&|_| true))
}

fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    // Only the workspace ref itself is special; other `gitbutler/*` refs (e.g. `gitbutler/target`)
    // name segments like any branch, matching the walk.
    rn.category() == Some(Category::LocalBranch) && !but_core::is_workspace_ref_name(rn)
}

/// The segment metadata for a ref: `Branch` for a tracked branch, `Workspace` for the workspace ref,
/// `None` otherwise (mirrors `extract_local_branch_metadata`).
fn segment_metadata<T: but_core::RefMetadata>(
    ref_name: &gix::refs::FullNameRef,
    meta: &T,
) -> Option<crate::SegmentMetadata> {
    if ref_name.category() != Some(Category::LocalBranch) {
        return None;
    }
    // The workspace ref is a WORKSPACE, never a branch — stray branch metadata under its name
    // (e.g. an overlay that pre-writes branch data for a ref about to be created) must not
    // reclassify the workspace segment, which the projection finds by its metadata.
    if but_core::is_workspace_ref_name(ref_name) {
        return meta
            .workspace_opt(ref_name)
            .ok()
            .flatten()
            .map(|ws| crate::SegmentMetadata::Workspace((*ws).clone()));
    }
    if let Ok(Some(branch)) = meta.branch_opt(ref_name) {
        return Some(crate::SegmentMetadata::Branch((*branch).clone()));
    }
    if let Ok(Some(ws)) = meta.workspace_opt(ref_name) {
        return Some(crate::SegmentMetadata::Workspace((*ws).clone()));
    }
    None
}
