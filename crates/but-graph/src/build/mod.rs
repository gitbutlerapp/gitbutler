//! The graph builders: the workspace's ref layout and build context are authored here onto a
//! [`CommitGraph`] flattened out of the raw traversal ([`CommitGraph::from_walk`]), so
//! everything downstream sees one substrate shape regardless of how the build was entered
//! (managed workspace, non-managed checkout, explicit seeds, overlays). The build's segments
//! are internal scaffolding — they author the layout and never leave the builder.
//!
//! The build is phased (gather-then-build), one file per phase: [`facts`] (pure boundary/shape
//! reads) → [`plan`] (the chain plan, pure data) → [`materialize`] (repository-flavored
//! decisions feeding [`segment_data`]) → [`layout`] (the stored ref layout read off the
//! record), with [`remotes`] holding the remote-tracking claims. Entry points and shared
//! helpers live here.
//!
//! THE STRUCTURAL INVARIANT: no pass rewrites another pass's structure. Every mutation is
//! either additive construction in fresh territory (minting, remote regions) or a declared
//! materializer applying a plan decision (chain splices, anonymous bases, the entrypoint
//! placement). The segment record itself is temporary: decisions go in, the stored layout is
//! read off it, then it is dropped — it never becomes graph storage.

use std::collections::{BTreeMap, HashMap, HashSet};

use but_core::RefMetadata;

/// `ObjectId`-keyed map/set on the prehashed [`gix::hashtable`] — ids are touched per commit in
/// the hot build loops, where SipHash on 20-byte keys dominates profiles.
type IdMap<V> = gix::hashtable::HashMap<gix::ObjectId, V>;
type IdSet = gix::hashtable::HashSet<gix::ObjectId>;
use gix::reference::Category;

use crate::{
    CommitGraph,
    walk::overlay::{OverlayMetadata, OverlayRepo},
    workspace::GraphContext,
};

use materialize::graph_from_commit_graph;
use remotes::remote_tracking_from_repository;

mod ad_hoc;
mod facts;
mod layout;
mod materialize;
mod plan;
mod remote_segments;
mod remotes;
mod segment_data;

/// Project a PROVIDED `cg` — the write-through seam: an editor-mutated commit graph projects
/// like a fresh walk, with enrichment (refs, target, metadata) read from `repo`/`meta` amended
/// by `overlay` (default overlay = current state, a materialized rebase; a rebase's overlay
/// serves pending ref edits and entrypoint from memory, a preview). Dispatches like
/// `Graph::from_head`: managed when the workspace ref resolves to a
/// commit in `cg` (falling through to non-managed when the entrypoint lands outside the
/// workspace), non-managed otherwise. `None` when the entrypoint is unborn or unresolvable.
pub(crate) fn workspace_from_commit_graph<T: but_core::RefMetadata>(
    mut cg: CommitGraph,
    repo: &gix::Repository,
    meta: &T,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::walk::Options,
    overlay: crate::walk::Overlay,
) -> anyhow::Result<Option<crate::Workspace>> {
    let (overlay_repo, overlay_meta, overlay_entrypoint) = overlay.into_parts(repo, meta);
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ws_tip_on_disk = resolve_ref_tip(&overlay_repo, ws_ref.as_ref())?;
    let ws_commit = ws_tip_on_disk.filter(|c| cg.node(*c).is_some());
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
        Some(tr) => resolve_ref_tip(&overlay_repo, tr.as_ref())?,
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
    let (head_tip, head_ref) = match overlay_entrypoint {
        Some((id, ref_name)) => (Some(id), ref_name),
        None => {
            let head = repo.head()?;
            (
                head.id().map(|id| id.detach()),
                head.referent_name().map(|n| n.to_owned()),
            )
        }
    };
    // HEAD is external context too: the editor's graph need not contain the checked-out commit
    // (e.g. an edit-mode WIP commit) — the walk always traverses the entrypoint's region.
    if let Some(tip) = head_tip {
        ensure_tip_region(&mut cg, &overlay_repo, tip, crate::CommitFlags::empty())?;
    }
    let head_tip = head_tip.filter(|c| cg.node(*c).is_some());
    // Reconcile edges LAST: the region steps above revive tombstones, and a revival flips
    // effective parents that must then be re-validated against the odb.
    cg.complete_parents_from_odb(&overlay_repo)?;
    // `Integrated` = target-reachability. `InWorkspace` follows the walk's rule: only the
    // workspace tip seeds it (`None` clears it everywhere).
    cg.set_flag_on_ancestors(crate::CommitFlags::Integrated, target_tips());
    cg.set_flag_on_ancestors(
        crate::CommitFlags::InWorkspace,
        ws_commit.filter(|_| ws_has_meta),
    );
    // `NotInRemote` mirrors the walk's seeding: only tips the walk QUEUES seed it — HEAD,
    // and for a discovered workspace its tip, the target's local tracking branch, and the
    // workspace stack branch refs. A local branch that merely points into remote-reachable
    // history is NOT a seed, and an editor drop must lose the flag entirely (a stale flag
    // would hide the remote region from projection).
    let mut not_in_remote_tips: Vec<gix::ObjectId> = head_tip.into_iter().collect();
    if ws_exists {
        not_in_remote_tips.extend(ws_tip_on_disk);
        if let Some((_, _, Some((_, local_tip)))) =
            crate::walk::workspace_target_tip(&overlay_repo, project_meta.target_ref.as_ref())?
        {
            not_in_remote_tips.push(local_tip);
        }
        for branch in ws_meta
            .iter()
            .flat_map(|ws| ws.stacks.iter())
            .filter(|s| s.is_in_workspace())
            .flat_map(|s| s.branches.iter())
        {
            not_in_remote_tips.extend(crate::walk::utils::try_refname_to_id(
                &overlay_repo,
                branch.ref_name.as_ref(),
            )?);
        }
    }
    cg.set_flag_on_ancestors(crate::CommitFlags::NotInRemote, not_in_remote_tips);
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
        let mut refs_by_id =
            overlay_repo.collect_ref_mapping_by_prefix(ref_prefixes(), &[ws_ref.as_ref()])?;
        let worktree_by_branch =
            overlay_repo.worktree_branches(main_head_ref.as_ref().map(|r| r.as_ref()))?;
        cg.refresh_refs(&mut refs_by_id, &worktree_by_branch);
        let (managed_cg, ctx, entrypoint_reached) = assemble_managed(
            cg,
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
        // Entrypoint never made it into a segment — fall through to the non-managed view. The
        // carried commit graph is reclaimed as-is: the non-managed pass overwrites everything
        // the managed attempt touched (refs re-refreshed, layout re-authored), and the stray
        // managed-ws mark is only ever read for the entrypoint, provably a different commit here.
        if entrypoint_reached {
            return crate::workspace::project_workspace(managed_cg, ctx).map(Some);
        }
        cg = managed_cg;
    }
    let Some(head_tip) = head_tip else {
        return Ok(None);
    };
    let mut refs_by_id = overlay_repo.collect_ref_mapping_by_prefix(ref_prefixes(), &[])?;
    let worktree_by_branch =
        overlay_repo.worktree_branches(head_ref.as_ref().map(|r| r.as_ref()))?;
    cg.refresh_refs(&mut refs_by_id, &worktree_by_branch);
    let (cg, ctx) = assemble_unmanaged(
        cg,
        repo,
        &overlay_repo,
        &overlay_meta,
        head_tip,
        head_ref,
        project_meta,
        options,
    )?;
    crate::workspace::project_workspace(cg, ctx).map(Some)
}

/// Build the managed-workspace's ref layout and context onto its `CommitGraph`, deriving the
/// enrichment inputs from `(repo, meta, project_meta)`. `overlay` refs and metadata are served
/// from memory (the [`Workspace::redo`](crate::Workspace::redo) path).
pub fn graph_from_repository<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::walk::Options,
    overlay: crate::walk::Overlay,
) -> anyhow::Result<Option<(CommitGraph, GraphContext)>> {
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.into_parts(repo, meta);
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    // No (usable) workspace ref means no managed workspace — signal fall-through, don't fail:
    // the dispatch routes any repository through here and builds non-managed on `Ok(None)`.
    let Some(ws_commit) = resolve_ref_tip(&overlay_repo, ws_ref.as_ref())? else {
        return Ok(None);
    };
    let walk_tip = entrypoint.unwrap_or(ws_commit);
    let walk_ref = if entrypoint.is_none() || entrypoint == Some(ws_commit) {
        entrypoint_ref.clone().or(Some(ws_ref.clone()))
    } else {
        entrypoint_ref.clone()
    };
    let cg = CommitGraph::from_walk(
        &overlay_repo,
        &overlay_meta,
        walk_tip,
        walk_ref.clone(),
        project_meta.clone(),
        options.clone(),
    )?;
    let ep = entrypoint.unwrap_or(ws_commit);
    let (cg, ctx, entrypoint_reached) = assemble_managed(
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
    if !entrypoint_reached {
        return Ok(None);
    }
    Ok(Some((cg, ctx)))
}

/// Build the ref layout and context for a NON-managed checkout — a plain branch or detached
/// HEAD, with no `gitbutler/workspace` merge. `head_tip` is the checked-out commit (the graph's tip).
/// A detached HEAD is anonymized by `from_head`'s detach pass, not here.
#[allow(clippy::too_many_arguments)]
pub(crate) fn graph_from_repository_unmanaged<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::walk::Options,
    overlay: crate::walk::Overlay,
) -> anyhow::Result<(CommitGraph, GraphContext)> {
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.into_parts(repo, meta);
    let cg = CommitGraph::from_walk(
        &overlay_repo,
        &overlay_meta,
        head_tip,
        entrypoint_ref.clone(),
        project_meta.clone(),
        options.clone(),
    )?;
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

/// Like [`graph_from_repository`], but seeded from explicit `tips`. The tips'
/// normalized traversal roles are carried onto the returned graph (`seeds`), which the
/// projection reads for tips-built graphs.
pub(crate) fn graph_from_repository_seeds<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    tips: Vec<crate::walk::Seed>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::walk::Options,
) -> anyhow::Result<(CommitGraph, GraphContext)> {
    let overlay = crate::walk::Overlay::default();
    let (overlay_repo, overlay_meta, _overlay_entrypoint) = overlay.into_parts(repo, meta);
    let cg = CommitGraph::from_walk_seeds(
        &overlay_repo,
        &overlay_meta,
        tips,
        project_meta.clone(),
        options.clone(),
    )?;
    let entrypoint = cg
        .entrypoint()
        .ok_or_else(|| anyhow::anyhow!("explicit seeds always contain an entrypoint"))?;
    let entrypoint_ref = cg.entrypoint_ref().cloned();

    // Managed only when the workspace ref resolves AND the tips traversal actually reached its
    // commit — explicit seeds define the graph's extent, they don't discover a workspace on their
    // own.
    let ws_ref: gix::refs::FullName = but_core::WORKSPACE_REF_NAME.try_into()?;
    let ws_commit =
        resolve_ref_tip(&overlay_repo, ws_ref.as_ref())?.filter(|c| cg.node(*c).is_some());

    // Detachment travels on the seeds the walker carried over — the projection
    // reads it from there, no segment pass needed.
    if let Some(ws_commit) = ws_commit {
        // A workspace-ref entrypoint is the plain from_head case: no explicit entrypoint ref.
        let ep_ref = entrypoint_ref
            .clone()
            .filter(|r| !but_core::is_workspace_ref_name(r.as_ref()));
        let (cg, ctx, _entrypoint_reached) = assemble_managed(
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
        )?;
        Ok((cg, ctx))
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
        )
    }
}

/// The build context every phase reads — one bundle from dispatch to materialization.
/// Phase artifacts (facts, plan, layout) travel separately, since later phases borrow them
/// alongside this context.
pub(super) struct BuildInputs<'a> {
    pub cg: &'a CommitGraph,
    /// The workspace tip in managed builds; the checked-out tip otherwise.
    pub workspace_commit: gix::ObjectId,
    pub entrypoint: gix::ObjectId,
    pub entrypoint_ref: Option<&'a gix::refs::FullName>,
    /// The target ref's resolved tip.
    pub target: Option<gix::ObjectId>,
    pub remote_tracking: &'a HashMap<gix::refs::FullName, gix::refs::FullName>,
    pub symbolic_remotes: &'a [String],
    /// The in-workspace stack branch lists (managed builds only).
    pub stack_branches: Option<&'a [Vec<gix::refs::FullName>]>,
    /// Ad-hoc same-tip branch orders routed through the chain plan.
    pub ad_hoc_chains: &'a [Vec<gix::refs::FullName>],
    pub project_meta: &'a but_core::ref_metadata::ProjectMeta,
    pub options: &'a crate::walk::Options,
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
    options: crate::walk::Options,
) -> anyhow::Result<(CommitGraph, GraphContext, bool)> {
    // Two candidates, usually the same commit: the facts' managed-merge check reads the
    // workspace tip, the ref layout's ws anchor reads the (checkout) entrypoint.
    cg.mark_managed_ws_commit_by_message(repo, ws_commit);
    cg.mark_managed_ws_commit_by_message(repo, entrypoint);
    let ws_meta = overlay_meta.workspace(ws_ref.as_ref())?;
    let stack_branches = in_workspace_stack_branches(&ws_meta);
    let inputs = enrichment_inputs(repo, overlay_repo, &project_meta, main_head_ref)?;
    assemble(
        cg,
        overlay_repo,
        overlay_meta,
        ws_commit,
        entrypoint,
        entrypoint_ref,
        Some(&stack_branches),
        inputs,
        project_meta,
        options,
    )
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
    options: crate::walk::Options,
) -> anyhow::Result<(CommitGraph, GraphContext)> {
    cg.mark_managed_ws_commit_by_message(repo, head_tip);
    let inputs = enrichment_inputs(repo, overlay_repo, &project_meta, entrypoint_ref.as_ref())?;
    let (cg, ctx, _entrypoint_reached) = assemble(
        cg,
        overlay_repo,
        overlay_meta,
        head_tip,
        head_tip,
        entrypoint_ref,
        None,
        inputs,
        project_meta,
        options,
    )?;
    Ok((cg, ctx))
}

/// The build pipeline both entries share: plan, author the segments, apply persisted
/// ad-hoc orders, derive and install the stored ref positions, stamp the REQUESTED
/// entrypoint ref on the seed (it names the entry even where the walker nulled it for
/// ambiguity), enrich — and capture the segments' two verdicts before they are
/// dropped: the entry's empty-chain resolution and whether the entrypoint landed in
/// a segment at all (the managed fall-through signal). `stack_branches` being present
/// is what MAKES a build managed.
#[allow(clippy::too_many_arguments)]
fn assemble<T: but_core::RefMetadata>(
    mut cg: CommitGraph,
    overlay_repo: &OverlayRepo<'_>,
    overlay_meta: &OverlayMetadata<'_, T>,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    inputs: EnrichmentInputs,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::walk::Options,
) -> anyhow::Result<(CommitGraph, GraphContext, bool)> {
    // The persisted ad-hoc branch order is a CHAIN: it threads through the same plan
    // machinery as workspace metadata stacks.
    let ad_hoc_orders = ad_hoc::ad_hoc_branch_stack_upgrades(
        entrypoint_ref.as_ref(),
        entrypoint_ref
            .as_ref()
            .and_then(|r| cg.commit_by_ref(r.as_ref()))
            .is_some(),
        overlay_repo,
        overlay_meta,
    )?;
    let b = BuildInputs {
        cg: &cg,
        workspace_commit,
        entrypoint,
        entrypoint_ref: entrypoint_ref.as_ref(),
        target: inputs.target,
        remote_tracking: &inputs.remote_tracking,
        symbolic_remotes: &inputs.symbolic_remotes,
        stack_branches,
        ad_hoc_chains: &ad_hoc_orders.same_tip_chains,
        project_meta: &project_meta,
        options: &options,
    };
    let (f, plan, layout_plan) = plan::gather_and_plan(&b, overlay_meta);
    let segment_data = graph_from_commit_graph(&b, overlay_meta, f, &plan, &layout_plan);
    let ad_hoc_branch_stack_orders = ad_hoc_orders.full_orders;
    let mut layout = layout::derive_ref_layout(&segment_data, &cg)?;
    layout.empty_chain_anchors = layout_plan.empty_chain_anchors();
    cg.layout = Some(layout);
    if let Some(name) = entrypoint_ref.as_ref()
        && let Some(seed) = cg
            .seeds
            .iter_mut()
            .find(|s| s.is_entrypoint && !s.is_detached)
    {
        seed.ref_name = Some(name.clone());
    }
    let (branch_details, workspace_meta) =
        segment_data::enrich(&cg, &segment_data, overlay_meta, &inputs.worktree_by_branch);
    let entry_resolved_commit = segment_data.resolve_entrypoint_commit(&cg);
    let entrypoint_reached = segment_data.entrypoint_sidx.is_some();
    let ctx = GraphContext {
        options,
        project_meta,
        symbolic_remote_names: inputs.symbolic_remotes,
        remote_tracking: inputs.remote_tracking,
        ad_hoc_branch_stack_orders,
        branch_details,
        workspace_meta,
        entry_resolved_commit,
    };
    Ok((cg, ctx, entrypoint_reached))
}

/// The write-through seam's external-context refresh: `tip` (a stored/extra target, or a
/// remote-tracking tip) still exists on disk even when the editor dropped its node or rewrote
/// it in place. Revive tombstones and append any missing region — walking the odb from `tip`
/// down to commits the graph knows — with `flags` per the walk's conventions (Integrated for
/// target-seeded tips, empty for remote-ahead regions). A stale (unresolvable) tip is ignored,
/// like the walk does.
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
    let indices: Vec<_> = to_add
        .iter()
        .map(|(id, _)| cg.add_node(Some(*id)))
        .collect();
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
/// of remote-tracking branches, so when an editor rewrite made a remote tip's commit vanish from
/// the arena (the local advanced past it), re-append that region from the odb. Only remotes of
/// locals the graph actually contains matter — anything else the walk wouldn't reach either.
fn ensure_remote_regions(
    cg: &mut CommitGraph,
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<()> {
    let (remote_tracking, _symbolic_remotes) =
        remote_tracking_from_repository(repo, overlay_repo, project_meta)?;
    for (local, remote) in remote_tracking {
        let local_tip = resolve_ref_tip(overlay_repo, local.as_ref())?;
        if local_tip.is_none_or(|tip| cg.index_of(tip).is_none()) {
            continue;
        }
        let Some(remote_tip) = resolve_ref_tip(overlay_repo, remote.as_ref())? else {
            continue;
        };
        ensure_tip_region(cg, overlay_repo, remote_tip, crate::CommitFlags::empty())?;
    }
    Ok(())
}

/// The commit `name` peels to through the overlay, `None` when the ref is absent
/// or unpeelable.
fn resolve_ref_tip(
    repo: &OverlayRepo<'_>,
    name: &gix::refs::FullNameRef,
) -> anyhow::Result<Option<gix::ObjectId>> {
    Ok(repo
        .try_find_reference(name)?
        .and_then(|mut r| r.peel_to_commit().ok())
        .map(|c| c.id().detach()))
}

/// The enrichment inputs every builder entry derives from `(repo, project_meta)` and the overlay
/// views.
struct EnrichmentInputs {
    /// The target commit, resolved from the CALLER's project meta for the builder's boundaries;
    /// a default `ProjectMeta` means no target (no hard-coded `origin/main` fallback), like the
    /// walk. Integration marks and `NotInRemote` already come from the walk — no re-flagging.
    target: Option<gix::ObjectId>,
    /// Remote-tracking relationships come from git CONFIG plus the caller's project meta —
    /// overlay refs don't reshape them.
    remote_tracking: HashMap<gix::refs::FullName, gix::refs::FullName>,
    symbolic_remotes: Vec<String>,
    /// Which worktree (if any) checks out each ref — the main worktree `[🌳]` and any linked
    /// worktrees `[📁]`, keyed by ref name.
    worktree_by_branch: BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
}

#[tracing::instrument(level = "trace", skip_all)]
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
    let (remote_tracking, symbolic_remotes) =
        remote_tracking_from_repository(repo, overlay_repo, project_meta)?;
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

/// The local branch that names the segment at `c`, mirroring the walk's tiers: ABOVE the base
/// the unique branch with GitButler METADATA wins (a stack branch beats the target's local ref,
/// e.g. `A` over `main`); at/below the base (Integrated) the target's local position wins
/// instead. Then the unique REMOTE-TRACKED branch (e.g. `main` over a plain `new-A`), then the
/// only branch, else anonymous.
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
    disambiguated_ref_at(
        cg,
        cg.index_of(c)?,
        remote_tracking,
        meta,
        workspace_commit,
        target_ref,
    )
}

/// [`disambiguated_ref`] by node handle — the boundary scan calls this once per in-set commit.
fn disambiguated_ref_at<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    c: usize,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    workspace_commit: Option<gix::ObjectId>,
    target_ref: Option<&gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let node = cg.node_at(c);
    let branches: Vec<&gix::refs::FullName> = node
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
    let integrated = node.flags.contains(crate::CommitFlags::Integrated);
    (!integrated)
        .then(|| unique(&|r| segment_metadata(r.as_ref(), meta).is_some()))
        .flatten()
        .or_else(|| unique(&|r| remote_tracking.contains_key(r)))
        // Several remote-tracked branches on a STACK TOP (a direct parent of the workspace merge):
        // a unique branch WITH metadata wins even when integrated (it is the chain the user works
        // in, e.g. `first-branch` next to a target-local `main` in gb-local mode); among several
        // metadata branches the TARGET's own local wins (e.g. `main` next to a just-applied
        // branch, both resting on the target's tip). Deeper commits stay anonymous like the walk's.
        .or_else(|| {
            workspace_commit
                .is_some_and(|ws| cg.children_at(c).iter().any(|&k| cg.id_at(k) == ws))
                .then(|| {
                    unique(&|r| segment_metadata(r.as_ref(), meta).is_some())
                        .or_else(|| unique(&|r| remote_tracking.get(r) == target_ref))
                })
                .flatten()
        })
        .or_else(|| unique(&|_| true))
}

/// Whether `name` is one of the workspace metadata's stack branches.
pub(super) fn is_stack_branch(
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    name: &gix::refs::FullName,
) -> bool {
    stack_branches
        .into_iter()
        .flatten()
        .flatten()
        .any(|b| b == name)
}

fn is_plain_local_branch(rn: &gix::refs::FullName) -> bool {
    let rn = rn.as_ref();
    // Only the workspace ref itself is special; other `gitbutler/*` refs (e.g. `gitbutler/target`)
    // name segments like any branch, matching the walk.
    rn.category() == Some(Category::LocalBranch) && !but_core::is_workspace_ref_name(rn)
}

/// All remote-tracking refs on commits in the graph, in name order.
fn remote_refs(cg: &CommitGraph) -> std::collections::BTreeSet<gix::refs::FullName> {
    cg.commit_ids()
        .flat_map(|c| cg.refs_at(c))
        .filter(|r| r.as_ref().category() == Some(Category::RemoteBranch))
        .collect()
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
