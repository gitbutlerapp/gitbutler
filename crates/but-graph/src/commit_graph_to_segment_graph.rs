//! The graph builders: every [`Graph`](crate::Graph) is assembled here from a [`CommitGraph`]
//! flattened out of the raw traversal ([`CommitGraph::from_walk`]). The builders reconstruct the
//! FULL segment graph (workspace / branch / anonymous / target / remote segments, their
//! first-parent connections, generations, and remote↔local sibling links) so that everything
//! downstream — projection, renderer, consumers — sees one graph shape regardless of how the
//! build was entered (managed workspace, non-managed checkout, explicit tips, overlays).

use std::collections::{BTreeMap, HashMap, HashSet};

use but_core::RefMetadata;
use gix::reference::Category;

use crate::{
    Commit, CommitGraph, RefInfo, Segment, SegmentIndex,
    init::overlay::{OverlayMetadata, OverlayRepo},
    segment_graph::{Connection, SegmentGraph},
};

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
    if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
        eprintln!(
            "FLIP ws_commit={ws_commit} entrypoint={entrypoint:?} entrypoint_ref={:?} overlay={overlay:?}",
            entrypoint_ref.as_ref().map(|r| r.as_bstr()),
        );
    }
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
    if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
        eprintln!(
            "FLIP(unmanaged) head_tip={head_tip} entrypoint_ref={:?} overlay={overlay:?}",
            entrypoint_ref.as_ref().map(|r| r.as_bstr()),
        );
    }
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
        .filter(|c| cg.node(*c).is_some());

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

/// Only IN-WORKSPACE stacks form lanes. An inactive/outside stack's branches never splice as
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

/// Assemble the MANAGED-workspace graph from `cg`: workspace metadata defines the lanes, and the
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
    let mut graph = graph_from_commit_graph(
        &cg,
        ws_commit,
        entrypoint,
        entrypoint_ref,
        inputs.target,
        &inputs.remote_tracking,
        &inputs.symbolic_remotes,
        Some(&stack_branches),
        true,
        &inputs.worktree_by_branch,
        overlay_meta,
        project_meta,
        options,
    );
    graph.commit_graph = Some(cg);
    graph.remote_tracking = inputs.remote_tracking;
    Ok(graph)
}

/// Assemble the NON-managed graph from `cg`: no stack or workspace-ref passes, plus the
/// persisted single-branch ordering.
#[allow(clippy::too_many_arguments)]
fn assemble_unmanaged<T: but_core::RefMetadata>(
    cg: CommitGraph,
    repo: &gix::Repository,
    overlay_repo: &OverlayRepo<'_>,
    overlay_meta: &OverlayMetadata<'_, T>,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    let inputs = enrichment_inputs(repo, overlay_repo, &project_meta, entrypoint_ref.as_ref())?;
    let mut graph = graph_from_commit_graph(
        &cg,
        head_tip,
        head_tip,
        entrypoint_ref,
        inputs.target,
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
    graph.commit_graph = Some(cg);
    graph.remote_tracking = inputs.remote_tracking;
    Ok(graph)
}

/// Everything the build decides BEFORE any segment exists — pure facts over the commit graph,
/// ref positions, and metadata. Phase 1 of gather-then-build: the materialization and every
/// later pass read these; nothing here reads a segment.
struct Facts {
    /// The commit set the LOCAL segments span.
    in_set: HashSet<gix::ObjectId>,
    /// Is the checked-out workspace commit a real GitButler-managed merge?
    ws_is_managed_merge: bool,
    /// Managed, but the ws ref sits on (or advanced past) a plain commit: an empty workspace
    /// segment is spliced in above.
    empty_ws_case: bool,
    /// Stored/extra target positions (and explicit tips): segments must start there.
    pinned_commits: HashSet<gix::ObjectId>,
    /// Commits that START a segment.
    boundaries: HashSet<gix::ObjectId>,
    /// The in-set entrypoint was NOT a boundary on its own and is forced to start a segment
    /// (a checkout inside a stack). Its tip keeps the split's naming precedence: the checked-out
    /// ref first, then plain disambiguation.
    entrypoint_forced_boundary: bool,
    /// Which boundary's first-parent run each in-set commit belongs to.
    owner_of: HashMap<gix::ObjectId, gix::ObjectId>,
    /// The boundaries in materialization order: workspace first, then descending generation, id.
    tips: Vec<gix::ObjectId>,
}

#[allow(clippy::too_many_arguments)]
fn facts<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    managed: bool,
    meta: &T,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    options: &crate::init::Options,
) -> Facts {
    // The commit set the LOCAL segments span: everything reachable from the workspace commit, plus the
    // target's own history WHEN the target has a local branch (it is `NotInRemote`) — e.g. an
    // integrated `main` that sits outside the workspace. A remote-only target (ahead of its local, not
    // `NotInRemote`) is NOT added: it becomes a remote segment instead.
    let mut in_set: HashSet<gix::ObjectId> = ancestors(cg, workspace_commit);
    if let Some(t) = target
        && cg
            .node(t)
            .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::NotInRemote))
    {
        in_set.extend(ancestors(cg, t));
    }

    // In-set children per commit, to detect branch points (a commit reached by >1 child).
    let mut children: HashMap<gix::ObjectId, Vec<gix::ObjectId>> = HashMap::new();
    for &c in &in_set {
        for p in cg.all_parent_ids(c) {
            if in_set.contains(&p) {
                children.entry(p).or_default().push(c);
            }
        }
    }

    // Where each remote-tracking branch rejoins the local graph: the first in-set commit along the
    // remote tip's first-parent spine. These are segment boundaries (the remote connects INTO them).
    // Only remotes whose LOCAL counterpart is itself in the graph count — a remote for a branch that
    // lives outside the workspace (e.g. `origin/A-middle` on an outside `A-middle`) is never surfaced,
    // so its spine crossing an in-set commit must not carve a spurious boundary there.
    // The TARGET's rejoin always counts — it is surfaced regardless of where (or whether) its local
    // branch sits; e.g. `main` inside the target's own ahead region must not stop `origin/main`'s
    // line from carving its meeting point with the workspace.
    let target_tip = project_meta
        .target_ref
        .as_ref()
        .and_then(|tr| cg.commit_by_ref(tr.as_ref()));
    // An entrypoint OUTSIDE the workspace (an adhoc checkout in a repository that has one)
    // rejoins like a remote: the first in-set commit on its first-parent spine is a boundary
    // its outside region connects INTO.
    let entrypoint_outside = (!in_set.contains(&entrypoint)).then_some(entrypoint);

    // EXPLICIT tips (from_commit_traversal_tips) can point anywhere, and validation requires a tip
    // id to be its segment's first commit — so each one is a boundary. Workspace-discovered builds
    // must not carve these: there they are ordinary interior commits.
    let tip_ids: HashSet<gix::ObjectId> = if cg.explicit_tips {
        cg.traversal_tips.iter().map(|t| t.id).collect()
    } else {
        HashSet::new()
    };
    let remote_rejoins: HashSet<gix::ObjectId> = remote_tracking
        .iter()
        .filter(|(local, _)| {
            cg.commit_by_ref(local.as_ref())
                .is_some_and(|c| in_set.contains(&c))
        })
        .filter_map(|(_, r)| cg.commit_by_ref(r.as_ref()))
        .chain(target_tip)
        .chain(entrypoint_outside)
        .filter_map(|tip| {
            let mut c = Some(tip);
            while let Some(id) = c {
                if in_set.contains(&id) {
                    return Some(id);
                }
                c = cg.first_parent(id);
            }
            None
        })
        .collect();

    // Is the checked-out workspace commit a real GitButler-managed merge, or a plain commit the ws ref
    // merely sits on (co-located with a stack tip) or has advanced PAST (an "on-top" commit above the
    // real merge)? Only a real merge is held in the workspace segment with its parents as stack tips;
    // otherwise the workspace segment is empty and spliced in above, and the commit keeps its normal
    // history and segmentation.
    let ws_is_managed_merge = managed && cg.is_managed_ws_commit(workspace_commit);
    let empty_ws_case = managed && !ws_is_managed_merge;

    // The workspace commit's parents are stack tips — always segment boundaries (so the workspace
    // segment holds only the workspace commit, even when a parent is anonymous, e.g. an advanced tip).
    // Only for a real managed merge; a plain checked-out tip, co-located stack tip, or advanced ref has
    // no stack parents to split on.
    let ws_parents: HashSet<gix::ObjectId> = if ws_is_managed_merge {
        cg.parents(workspace_commit).collect()
    } else {
        HashSet::new()
    };

    // A merge commit's segment holds only the merge, so its FIRST parent starts its own segment (the
    // second parent is already a boundary — reached by a non-first-parent edge).
    let merge_first_parents: HashSet<gix::ObjectId> = in_set
        .iter()
        .filter(|&&c| cg.all_parent_ids(c).len() > 1)
        .filter_map(|&c| cg.first_parent(c))
        .filter(|p| in_set.contains(p))
        .collect();

    // Every commit a workspace stack branch points at starts a segment: even when the commit is
    // name-ambiguous (several branches on it, so anonymous), the metadata branches float above it as
    // empty segments, so the commit itself must begin its own (anonymous) segment. A branch that
    // ADVANCED past the workspace anchors at its rejoin point instead — the first in-workspace commit
    // on its first-parent spine — which must equally start a segment (the advanced branch is projected
    // onto it via a sibling link).
    let metadata_commits: HashSet<gix::ObjectId> = stack_branches
        .unwrap_or(&[])
        .iter()
        .flatten()
        .filter_map(|b| cg.commit_by_ref(b.as_ref()))
        .filter_map(|tip| {
            let mut c = Some(tip);
            while let Some(id) = c {
                if in_set.contains(&id) {
                    return Some(id);
                }
                c = cg.first_parent(id);
            }
            None
        })
        .collect();

    // Stored/extra target positions must start their own segment: the projection's
    // `TargetCommit::from_commit` ignores a stored target commit that sits mid-segment, losing the
    // remembered base (and with it the workspace lower bound). Not restricted to the workspace set —
    // an older target position often sits inside the target REMOTE's ahead region.
    let mut pinned_commits: HashSet<gix::ObjectId> = project_meta
        .target_commit_id
        .into_iter()
        .chain(options.extra_target_commit_id)
        .filter(|&c| cg.node(c).is_some())
        .collect();
    // EXPLICIT tips split ahead regions too (e.g. an integrated target riding inside a remote's
    // ahead run must start its own segment there, like the walk's tip-seeded segments).
    pinned_commits.extend(tip_ids.iter().copied());

    // A commit starts a new segment when it carries a disambiguated ref, is the workspace tip, is a
    // merge, or is a convergence/branch point (reached by other than a single first-parent child).
    let is_boundary = |c: gix::ObjectId| -> bool {
        c == workspace_commit
            || ws_parents.contains(&c)
            || merge_first_parents.contains(&c)
            || remote_rejoins.contains(&c)
            || metadata_commits.contains(&c)
            || pinned_commits.contains(&c)
            || tip_ids.contains(&c)
            || disambiguated_ref(
                cg,
                c,
                remote_tracking,
                meta,
                Some(workspace_commit),
                project_meta.target_ref.as_ref(),
            )
            .is_some()
            || cg.all_parent_ids(c).len() > 1
            || {
                let kids = children.get(&c).map(Vec::as_slice).unwrap_or_default();
                // Reached by a non-first-parent edge, or by more than one child.
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && in_set.contains(&k))
            }
    };
    let mut boundaries: HashSet<gix::ObjectId> =
        in_set.iter().copied().filter(|&c| is_boundary(c)).collect();
    // A checkout inside a stack (from_commit_traversal): the entrypoint always starts its own
    // segment — planned here instead of splitting the enclosing segment after the build.
    let entrypoint_forced_boundary = in_set.contains(&entrypoint) && boundaries.insert(entrypoint);

    // Every boundary in the set starts a segment; each segment's commit run is the boundary plus its
    // first-parent tail up to (excluding) the next boundary. These runs partition the set, so assigning
    // each commit in a run to its boundary gives the owner directly — no reverse walk (a run's oldest
    // commit, e.g. a root, has no first-parent path back up to its own boundary).
    let mut owner_of: HashMap<gix::ObjectId, gix::ObjectId> = HashMap::new();
    let mut tips: Vec<gix::ObjectId> = boundaries.iter().copied().collect();
    for &tip in &tips {
        for c in commit_run(cg, tip, &in_set, &|c| boundaries.contains(&c)) {
            owner_of.insert(c.id, tip);
        }
    }

    // Segment tips in a stable order (workspace first, then by descending generation, then id) so the
    // numbering is deterministic even though it need not match the walk's.
    tips.sort_by_key(|&t| {
        (
            t != workspace_commit,
            std::cmp::Reverse(cg.node(t).map(|n| n.generation).unwrap_or(0)),
            t,
        )
    });

    Facts {
        in_set,
        ws_is_managed_merge,
        empty_ws_case,
        pinned_commits,
        boundaries,
        entrypoint_forced_boundary,
        owner_of,
        tips,
    }
}

/// Build a segment [`Graph`](crate::Graph) from `cg`.
///
/// Inputs mirror the projection's enrichment: the workspace commit, the target that bounds/integrates,
/// and the local→remote tracking map. `project_meta`/`options` are carried onto the `Graph`.
///
/// This is "gather-then-build": everything is decided as data BEFORE any segment exists, then
/// materialized in one pass. Roughly, in order:
///
/// 1. **Facts** (`facts`) — the boundaries where segments start, which boundary owns each
///    commit, and the tips in materialization order. Pure facts over `cg`; reads no segment.
/// 2. **Lower bound** — the base all lanes and the target converge on.
/// 3. **Lane plan** (`lane_plan`) — the NAME each tip's segment gets (some go anonymous so an
///    empty named segment can float above them), decided before any segment is built.
/// 4. **Materialize** — one local segment per tip holding its first-parent commit run, then the
///    planned float placeholders (empty named segments) spliced above the anonymized tips.
/// 5. **Connect** — each segment's bottom commit points at the segments owning its parents.
/// 6. **Lane structure** — empty-workspace segment, advanced-outside branches, empty-branch
///    splices. Runs before the remote passes so those link the lane segments at creation.
/// 7. **Remote / target / entrypoint passes** — a remote root segment per local branch whose
///    remote tip is present; the target's own remote segment when no local tracks it; regions for
///    an extra (older) target position, an outside checkout, and any explicit tip left uncovered.
#[allow(clippy::too_many_arguments)]
pub(crate) fn graph_from_commit_graph<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // Remote names implied by the workspace configuration (push remote, target's remote). Only these
    // remotes' AHEAD regions are traversed; a config-only tracking link keeps its name but its remote's
    // own commits stay out of the graph, matching the walk's traversal reach.
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    // A managed workspace (`workspace_commit` is the gitbutler/workspace octopus merge). When false,
    // `workspace_commit` is just the checked-out tip: no stack/ws-ref/anonymize passes.
    managed: bool,
    // Which worktree (if any) checks out each ref, keyed by ref name — the main worktree `[🌳]` and any
    // linked worktrees `[📁]`. Mirrors the walk's `RefInfo::from_ref` lookup.
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
    meta: &T,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> crate::Graph {
    let f = facts(
        cg,
        workspace_commit,
        entrypoint,
        target,
        remote_tracking,
        stack_branches,
        managed,
        meta,
        &project_meta,
        &options,
    );
    // The workspace's lower bound — the base all lanes and the (stored/extra) target converge
    // on, extended down to an older target position.
    let ws_lower_bound =
        effective_lower_bound(cg, workspace_commit, target, &project_meta, &options);
    let plan = lane_plan(
        cg,
        &f,
        workspace_commit,
        entrypoint,
        entrypoint_ref.as_ref(),
        target,
        remote_tracking,
        stack_branches,
        ws_lower_bound,
        managed,
        meta,
        project_meta.target_ref.as_ref(),
        symbolic_remotes,
        options.extra_target_commit_id,
    );
    let Facts {
        in_set,
        ws_is_managed_merge: _,
        empty_ws_case,
        pinned_commits,
        boundaries,
        entrypoint_forced_boundary: _,
        owner_of,
        tips,
    } = f;
    let is_boundary = |c: gix::ObjectId| boundaries.contains(&c);

    let mut sg = SegmentGraph::new();
    let mut seg_of_tip: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();

    // Create a local segment per tip, holding its first-parent commit run. Names come from the
    // plan: floated and demoted tips start ANONYMOUS (their name never touches the segment); a
    // float's displaced build-time name rides on the tip commit as a passive ref.
    let floated: HashMap<gix::ObjectId, &Float> =
        plan.floats.iter().map(|fl| (fl.tip, fl)).collect();
    for &tip in &tips {
        let mut commits = commit_run(cg, tip, &in_set, &is_boundary);
        let suppressed = floated.contains_key(&tip) || plan.demoted.contains(&tip);
        let named = if suppressed {
            None
        } else {
            plan.base_name_of
                .get(&tip)
                .map(|n| (n.clone(), tip))
                .or_else(|| plan.renames.get(&tip).cloned())
        };
        if let Some(displaced) = floated
            .get(&tip)
            .and_then(|fl| fl.displaced_ref_name.as_ref())
            && let Some(c0) = commits.first_mut()
            && !c0.refs.iter().any(|r| r.ref_name == *displaced)
        {
            c0.refs.push(RefInfo {
                ref_name: displaced.clone(),
                commit_id: Some(tip),
                worktree: None,
            });
            c0.refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        }
        let ref_info = named.map(|(ref_name, commit_id)| RefInfo {
            ref_name,
            commit_id: Some(commit_id),
            worktree: None,
        });
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_node(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(sidx).expect("just added").id = sidx;
        seg_of_tip.insert(tip, sidx);
    }
    // The planned float placeholders: empty segments carrying the floated names, spliced between
    // the workspace and the now-anonymous shared tips (edges below).
    let mut placeholder_of: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();
    for float in &plan.floats {
        let sidx = sg.add_node(Segment {
            id: 0,
            ref_info: Some(RefInfo {
                ref_name: float.name.clone(),
                commit_id: Some(float.tip),
                worktree: None,
            }),
            remote_tracking_ref_name: remote_tracking.get(&float.name).cloned(),
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits: Vec::new(),
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(sidx).expect("just added").id = sidx;
        placeholder_of.insert(float.tip, sidx);
    }

    // Connections: for each segment, its bottom commit's parents point at the segment owning each
    // parent, in first-parent order. The workspace's edge to a FLOATED parent routes through the
    // placeholder instead.
    for &tip in &tips {
        let src = seg_of_tip[&tip];
        let bottom = sg
            .node(src)
            .expect("present")
            .commits
            .last()
            .map(|c| c.id)
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            if let Some(&owner) = owner_of.get(&parent) {
                let dst = if tip == workspace_commit
                    && let Some(&ph) = placeholder_of.get(&parent)
                {
                    ph
                } else {
                    seg_of_tip[&owner]
                };
                connect(&mut sg, src, dst);
            }
        }
    }
    // Placeholder → the anonymized shared segment.
    for float in &plan.floats {
        let (Some(&ph), Some(&tip_sidx)) =
            (placeholder_of.get(&float.tip), seg_of_tip.get(&float.tip))
        else {
            continue;
        };
        connect(&mut sg, ph, tip_sidx);
    }

    // The lane STRUCTURE (empty-ws segment, advanced-outside branches, empty-branch splices)
    // precedes the remote passes, which link the lane segments at creation.
    let mut ws_empty_sidx = None;
    let before_lanes: HashSet<SegmentIndex> = sg.node_indices().collect();
    if managed {
        if empty_ws_case {
            ws_empty_sidx =
                insert_empty_workspace_segment(&mut sg, &seg_of_tip, cg, workspace_commit);
        }
        add_advanced_outside_branches(
            &mut sg,
            cg,
            &in_set,
            stack_branches,
            workspace_commit,
            remote_tracking,
            meta,
            project_meta.target_ref.as_ref(),
            &pinned_commits,
        );
        let ws_sidx = ws_empty_sidx.or_else(|| seg_of_tip.get(&workspace_commit).copied());
        insert_empty_branches(&mut sg, ws_sidx, &plan, remote_tracking);
    }
    // Segments the lane pass creates: the coverage gates below (extra target, outside
    // entrypoint, explicit tips) historically evaluated BEFORE any lane existed — they must not
    // be shadowed by lane segments (e.g. an advanced-outside run swallowing the stored target
    // position that the extra-target region must surface).
    let lane_created: HashSet<SegmentIndex> = sg
        .node_indices()
        .filter(|sidx| !before_lanes.contains(sidx))
        .collect();

    // Remote segments: for each local segment with a remote-tracking ref whose remote tip is
    // present, create a remote root segment (holding the remote-ahead commits) that connects into
    // the local segment, doubly-linked via siblings. The remote passes historically ran BEFORE the
    // lane structure and keyed on the pre-lane names — the overlay carries exactly that view
    // (materialization names plus the passes' own renames), so the lane reorder cannot change
    // their decisions.
    let pre_lane_names: HashMap<gix::ObjectId, gix::refs::FullName> = plan.base_name_of.clone();
    // Remote refs some creator will consume as a segment name: the region builder cuts its run
    // at interior remote refs only when unclaimed. Plan-modeled names (`remote_used` covers the
    // walk seeds) plus the ahead-case remotes of EVERY boundary-tip local (`add_remote_segments`
    // regions all of them, mirroring its gates) plus explicit-tip remote names.
    let mut claimed_remote_names: HashSet<gix::refs::FullName> = plan.remote_used.clone();
    claimed_remote_names.extend(pre_lane_names.values().filter_map(|name| {
        let rt = remote_tracking.get(name)?;
        let rt_tip = cg.commit_by_ref(rt.as_ref())?;
        let is_meta_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| b == rt);
        (!in_set.contains(&rt_tip)
            && remote_name_in_play(rt, symbolic_remotes)
            && !is_meta_stack_branch)
            .then(|| rt.clone())
    }));
    if cg.explicit_tips {
        claimed_remote_names.extend(cg.traversal_tips.iter().filter_map(|t| {
            t.ref_name
                .clone()
                .filter(|r| r.as_ref().category() == Some(Category::RemoteBranch))
        }));
    }
    // Connections from a region into another creator's territory (a run stopped at a claimed
    // remote): recorded during region creation, wired once every creator ran.
    let mut pending_edges: Vec<(SegmentIndex, gix::ObjectId)> = Vec::new();
    // The entrypoint is a planned boundary in every region too: a checkout inside a remote's
    // ahead run starts its own segment at creation, never split out after the fact.
    let region_pinned = {
        let mut p = pinned_commits.clone();
        p.insert(entrypoint);
        p
    };
    add_remote_segments(
        cg,
        &mut sg,
        &seg_of_tip,
        &in_set,
        &owner_of,
        symbolic_remotes,
        stack_branches,
        &region_pinned,
        remote_tracking,
        &pre_lane_names,
        &plan.renames,
        &claimed_remote_names,
        &mut pending_edges,
    );
    add_untracked_remote_segments(
        cg,
        &mut sg,
        remote_tracking,
        &seg_of_tip,
        &in_set,
        &owner_of,
    );
    // The TARGET remote must surface as a segment even when no local segment tracks it — its local
    // ref may be a mere commit-ref on a stack commit (e.g. `main` on a stack tip the metadata branch
    // names), or absent entirely. In the workspace, the walk names the target's rejoin segment after
    // the target and links it as sibling of the segment owning the local tracking ref's position.
    // Outside it (ahead or fully disjoint history), the target's own commits become a standalone
    // remote segment.
    if let Some(tr) = project_meta.target_ref.as_ref()
        && tr.as_ref().category() == Some(Category::RemoteBranch)
        && let Some(tip) = cg.commit_by_ref(tr.as_ref())
    {
        if in_set.contains(&tip) {
            let owner_tip = owner_of.get(&tip).copied().unwrap_or(tip);
            // Materialization applied the plan's rename when the target NAMES the (previously
            // anonymous) owner; this pass only adds the sibling link.
            if plan.renames.get(&owner_tip).is_some_and(|(n, _)| n == tr)
                && let Some(owner_sidx) = segment_by_commit(&sg, tip)
            {
                // Sibling: the segment whose FIRST commit is the local tracking ref's position.
                let local_sidx = remote_tracking
                    .iter()
                    .find(|(_, r)| *r == tr)
                    .and_then(|(local, _)| cg.commit_by_ref(local.as_ref()))
                    .and_then(|lc| {
                        segment_by_commit(&sg, lc).filter(|&sidx| {
                            sidx != owner_sidx
                                && sg
                                    .node(sidx)
                                    .is_some_and(|s| s.commits.first().is_some_and(|c| c.id == lc))
                        })
                    });
                if let Some(local_sidx) = local_sidx
                    && let Some(s) = sg.node_mut(owner_sidx)
                {
                    s.sibling_segment_id = Some(local_sidx);
                }
            }
        } else if segment_by_ref(&sg, tr).is_none() {
            // The target's own (remote) commits: segment its region like any remote's — split at
            // merges, connect every rejoin (including a merge's second parent) back into the
            // workspace — so the projection can find the common base. No tracking local, no links.
            segment_ahead_region(
                cg,
                &mut sg,
                Some(tr),
                tip,
                &in_set,
                &seg_of_tip,
                &owner_of,
                remote_tracking,
                None,
                &region_pinned,
                &claimed_remote_names,
                &mut pending_edges,
            );
            // The target's LOCAL tracking branch can sit on the region's tip (a fully disjoint
            // target only reached via the target tip itself). The local owns the commit — remotes
            // never take owned commits — so the local names the segment and the remote becomes an
            // empty segment above it, sibling-linked, exactly like the walk. Target queries then
            // count 0 commits ahead (the remote segment is empty).
            let local_on_tip = remote_tracking
                .iter()
                .find(|(local, r)| *r == tr && cg.commit_by_ref(local.as_ref()) == Some(tip))
                .map(|(local, _)| local.clone());
            if let Some(local) = local_on_tip
                && let Some(owner_sidx) = segment_by_commit(&sg, tip)
                && sg.node(owner_sidx).is_some_and(|s| {
                    s.ref_info.as_ref().is_some_and(|ri| &ri.ref_name == tr)
                        && s.commits.first().is_some_and(|c| c.id == tip)
                })
            {
                if let Some(s) = sg.node_mut(owner_sidx) {
                    s.ref_info = Some(RefInfo {
                        ref_name: local,
                        commit_id: Some(tip),
                        worktree: None,
                    });
                    s.remote_tracking_ref_name = Some(tr.clone());
                }
                let remote_sidx = sg.add_node(Segment {
                    id: 0,
                    ref_info: Some(RefInfo {
                        ref_name: tr.clone(),
                        commit_id: Some(tip),
                        worktree: None,
                    }),
                    remote_tracking_ref_name: None,
                    sibling_segment_id: Some(owner_sidx),
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    metadata: None,
                    connections: Vec::new(),
                });
                sg.node_mut(remote_sidx).expect("just added").id = remote_sidx;
                if let Some(s) = sg.node_mut(owner_sidx) {
                    s.remote_tracking_branch_segment_id = Some(remote_sidx);
                }
                connect(&mut sg, remote_sidx, owner_sidx);
            }
        }
    }

    // An EXTRA TARGET (an older target position) whose commit isn't part of any region so far — e.g.
    // one below the workspace's own history under a traversal cut — is surfaced like a target's
    // region, so the projection can derive `target_commit` from it.
    if let Some(extra) = options.extra_target_commit_id
        && cg.node(extra).is_some()
        && segment_by_commit_excluding(&sg, extra, &lane_created).is_none()
    {
        segment_ahead_region(
            cg,
            &mut sg,
            None,
            extra,
            &in_set,
            &seg_of_tip,
            &owner_of,
            remote_tracking,
            None,
            &region_pinned,
            &claimed_remote_names,
            &mut pending_edges,
        );
    }

    // The entrypoint itself sits OUTSIDE the workspace (an adhoc checkout in a repository that has
    // a managed one): its history becomes a region segmented like a remote's — split at inner
    // merges, connected where it rejoins the workspace (a boundary via `entrypoint_outside`) — so
    // the graph carries both components like the walk, and operations from an outside checkout
    // still see the workspace. The projection downgrades it to the single-branch view.
    if !in_set.contains(&entrypoint)
        && cg.node(entrypoint).is_some()
        && segment_by_commit_excluding(&sg, entrypoint, &lane_created).is_none()
    {
        segment_ahead_region(
            cg,
            &mut sg,
            entrypoint_ref.as_ref(),
            entrypoint,
            &in_set,
            &seg_of_tip,
            &owner_of,
            remote_tracking,
            None,
            &region_pinned,
            &claimed_remote_names,
            &mut pending_edges,
        );
    }

    // The walk seeds a segment per tip, and validation requires every tip to be owned by one.
    // An EXPLICIT traversal tip still uncovered gets its own region, named by the tip's ref; a
    // covered one whose ref names no segment (e.g. an integrated remote target riding on an
    // in-set commit) gets an EMPTY tip-named segment spliced above its commit's owner — the
    // walk's tip-seeded shape, which reachability-based consumers (upstream integration,
    // divergence classification) depend on.
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        if cg.node(t.id).is_none() {
            continue;
        }
        match segment_by_commit_excluding(&sg, t.id, &lane_created) {
            None => segment_ahead_region(
                cg,
                &mut sg,
                t.ref_name.as_ref(),
                t.id,
                &in_set,
                &seg_of_tip,
                &owner_of,
                remote_tracking,
                None,
                &region_pinned,
                &claimed_remote_names,
                &mut pending_edges,
            ),
            Some(owner_sidx) => {
                let Some(ref_name) = t.ref_name.clone() else {
                    continue;
                };
                // Workspace refs are the managed machinery's territory — it names or splices the
                // workspace segment itself (a second one here would violate name uniqueness).
                if but_core::is_workspace_ref_name(ref_name.as_ref()) {
                    continue;
                }
                if segment_by_ref(&sg, &ref_name).is_some()
                    || sg.node(owner_sidx).is_some_and(|s| {
                        s.ref_info
                            .as_ref()
                            .is_some_and(|ri| ri.ref_name == ref_name)
                    })
                {
                    continue;
                }
                // An ANONYMOUS segment starting at the tip takes the tip's name — applied by
                // MATERIALIZATION from the plan's renames; a still-anonymous tip start here
                // means the plan and the build disagree.
                if sg.node(owner_sidx).is_some_and(|s| {
                    s.commits.first().is_some_and(|c| c.id == t.id) && s.ref_info.is_none()
                }) {
                    debug_assert!(
                        false,
                        "the plan names every anonymous tip-started segment ({ref_name} at {})",
                        t.id
                    );
                    continue;
                }
                let empty_sidx = sg.add_node(Segment {
                    id: 0,
                    ref_info: Some(RefInfo {
                        ref_name: ref_name.clone(),
                        commit_id: Some(t.id),
                        worktree: None,
                    }),
                    remote_tracking_ref_name: remote_tracking.get(&ref_name).cloned(),
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    metadata: None,
                    connections: Vec::new(),
                });
                sg.node_mut(empty_sidx).expect("just added").id = empty_sidx;
                connect(&mut sg, empty_sidx, owner_sidx);
            }
        }
    }

    // The target's remote segment may have been created before its LOCAL got a segment (the
    // local can materialize from the extra-target region above) — link them like every other
    // creator does.
    if let Some(tr) = project_meta.target_ref.as_ref()
        && let Some(tr_sidx) = segment_by_ref(&sg, tr)
    {
        let tr = tr.clone();
        link_remote_to_local(&mut sg, tr_sidx, &tr, remote_tracking);
    }

    add_co_located_remote_empties(&mut sg, remote_tracking);
    // Wire the stopped runs into the segments that own their territory — every creator has run,
    // so the target of each pending connection exists (the owning creator's root, a cut segment,
    // or a mid-run commit of one).
    for (src, parent) in pending_edges.drain(..) {
        let Some(dst) = sg.node_indices().find(|&sidx| {
            sg.node(sidx)
                .is_some_and(|s| s.commits.iter().any(|c| c.id == parent))
        }) else {
            continue;
        };
        connect(&mut sg, src, dst);
    }

    // A no-ref checkout at a REMOTE-named segment's tip: the walk's anonymous entrypoint tip owns
    // the commits as a local segment — a remote ref never names it — and the remote's machinery
    // re-establishes the name as an EMPTY segment above. Float the name up so the projection sees
    // a detached view, not the remote segment. A LOCAL name stays: the walk names the entrypoint
    // segment after it.
    if entrypoint_ref.is_none()
        && entrypoint != workspace_commit
        && let Some(ep_sidx) = segment_by_commit(&sg, entrypoint)
        && sg.node(ep_sidx).is_some_and(|s| {
            s.ref_info
                .as_ref()
                .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
                && s.commits.first().is_some_and(|c| c.id == entrypoint)
        })
    {
        let (ref_info, rt_name, sibling, rt_seg) = {
            let s = sg.node_mut(ep_sidx).expect("present");
            (
                s.ref_info.take(),
                s.remote_tracking_ref_name.take(),
                s.sibling_segment_id.take(),
                s.remote_tracking_branch_segment_id.take(),
            )
        };
        let floated = sg.add_node(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name: rt_name,
            sibling_segment_id: sibling,
            remote_tracking_branch_segment_id: rt_seg,
            commits: Vec::new(),
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(floated).expect("just added").id = floated;
        // Links and edges aimed at the named segment now belong to its floated name.
        for sidx in sg.node_indices().collect::<Vec<_>>() {
            if sidx == floated {
                continue;
            }
            if let Some(s) = sg.node_mut(sidx) {
                if s.sibling_segment_id == Some(ep_sidx) {
                    s.sibling_segment_id = Some(floated);
                }
                if s.remote_tracking_branch_segment_id == Some(ep_sidx) {
                    s.remote_tracking_branch_segment_id = Some(floated);
                }
                for conn in &mut s.connections {
                    if conn.target == ep_sidx {
                        conn.target = floated;
                        conn.dst = None;
                        conn.dst_id = None;
                    }
                }
            }
        }
        connect(&mut sg, floated, ep_sidx);
    }

    if managed {
        // The remote/target passes link remotes against the plan's effective names — the
        // floated/demoted name's segment carries the links, so the suppressed tip drops its own.
        for tip in plan
            .floats
            .iter()
            .map(|fl| fl.tip)
            .chain(plan.demoted.iter().copied())
        {
            if let Some(s) = seg_of_tip.get(&tip).and_then(|&sidx| sg.node_mut(sidx)) {
                s.remote_tracking_ref_name = None;
                s.remote_tracking_branch_segment_id = None;
            }
        }
    }

    // A checkout inside a stack (from_commit_traversal) splits the enclosing segment so the entrypoint
    // begins its own segment — there is always a segment starting at the entrypoint.
    let entrypoint_sidx = if let (Some(ws_seg), None, true) = (
        ws_empty_sidx,
        entrypoint_ref.as_ref(),
        entrypoint == workspace_commit,
    ) {
        // from_head into a co-located workspace: the entrypoint is the empty workspace segment.
        // Only when the checkout IS the workspace position — a ref-less checkout elsewhere (e.g.
        // an unapply preview with the branch dropped) must not claim the workspace segment while
        // remembering a commit the graph doesn't hold.
        Some(ws_seg)
    } else if let Some(named) = entrypoint_ref.as_ref().and_then(|r| segment_by_ref(&sg, r)) {
        // The checked-out ref already names a segment — including an EMPTY one spliced in for a
        // virtual stack branch resting on the workspace base. That segment is the entrypoint, not
        // the segment owning the commit it points to.
        Some(named)
    } else {
        name_entrypoint_segment(
            &mut sg,
            entrypoint,
            entrypoint_ref.as_ref(),
            remote_tracking,
        )
    };

    // Classify each named segment by its ref's metadata: the workspace ref → Workspace, a tracked
    // branch → Branch, others → None. Matches the walk's `extract_local_branch_metadata`.
    for sidx in sg.node_indices().collect::<Vec<_>>() {
        let ref_name = sg
            .node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .map(|ri| ri.ref_name.clone());
        if let Some(ref_name) = ref_name {
            let md = segment_metadata(ref_name.as_ref(), meta);
            if let Some(s) = sg.node_mut(sidx) {
                s.metadata = md;
            }
        }
    }

    // A ref that NAMES a segment (or is a segment's remote-tracking ref) lives on that segment, so it is
    // removed from every commit's own ref list — including an empty branch's ref that sits on another
    // segment's commit (the walk does the same, avoiding showing it twice).
    let segment_names: HashSet<gix::refs::FullName> = sg
        .node_indices()
        .flat_map(|sidx| {
            sg.node(sidx)
                .map(|s| {
                    s.ref_info
                        .as_ref()
                        .map(|ri| ri.ref_name.clone())
                        .into_iter()
                        .chain(s.remote_tracking_ref_name.clone())
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
        .collect();
    for sidx in sg.node_indices().collect::<Vec<_>>() {
        if let Some(s) = sg.node_mut(sidx) {
            for commit in &mut s.commits {
                // Also drop remote-tracking refs: a remote is only ever shown as its own segment, never
                // annotated on a commit.
                commit.refs.retain(|ri| {
                    !segment_names.contains(&ri.ref_name)
                        && ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                });
            }
        }
    }

    // Annotate every ref with the worktree that checks it out — the main worktree `[🌳]` (whatever
    // ref HEAD actually points at, including the workspace ref) and linked worktrees `[📁]`. Keyed
    // by ref name, mirroring the walk's `RefInfo::from_ref`. No hardcoded HEAD assumption: marking
    // the workspace-commit segment unconditionally put `[🌳]` on a stack branch when HEAD was on
    // the workspace ref, and vice versa.
    let annotate = |ri: &mut RefInfo| {
        if ri.worktree.is_none()
            && let Some(wt) = worktree_by_branch.get(&ri.ref_name).and_then(|w| w.first())
        {
            ri.worktree = Some(wt.clone());
        }
    };
    for sidx in sg.node_indices().collect::<Vec<_>>() {
        let Some(s) = sg.node_mut(sidx) else { continue };
        if let Some(ri) = s.ref_info.as_mut() {
            annotate(ri);
        }
        for commit in &mut s.commits {
            for ri in &mut commit.refs {
                annotate(ri);
            }
        }
    }

    let entrypoint =
        entrypoint_sidx.map(|sidx| (sidx, crate::EntryPointCommit::AtCommit(entrypoint)));

    // Surface the extra target (an older target position) as an integrated traversal tip. The projection
    // derives `target_commit` from the deepest integrated tip and uses it to extend the workspace base
    // down to it — showing the commits integrated since then, exactly as the walk does. Only when the
    // commit actually made it into a segment — validation requires every tip to be owned by one, and
    // the traversal legitimately never reaches an extra target outside its cut.
    let mut traversal_tips = Vec::new();
    if let Some(extra) = options.extra_target_commit_id
        && segment_by_commit(&sg, extra).is_some()
    {
        traversal_tips
            .push(crate::init::Tip::new(extra).with_role(crate::init::TipRole::TargetRemote));
    }

    let mut graph = crate::Graph {
        inner: sg,
        entrypoint,
        entrypoint_ref,
        project_meta,
        options,
        traversal_tips,
        ..crate::Graph::default()
    };
    // The traversal's hard-limit signal survives the derivation — consumers surface it to the user.
    if cg.hard_limit_hit {
        graph.set_hard_limit_hit();
    }
    graph
}

/// The segment starting at the `entrypoint` commit — which exists by construction: the
/// entrypoint is a planned boundary in materialization and in every region, so no commit run
/// ever contains it mid-run. A checked-out `entrypoint_ref` names it (validation requires it):
/// an anonymous segment takes the name directly; one already named by ANOTHER ref keeps its
/// commits and the entrypoint ref becomes an empty segment spliced in above, like the walk's.
fn name_entrypoint_segment(
    sg: &mut SegmentGraph,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) -> Option<SegmentIndex> {
    let (sidx, pos) = sg.node_indices().find_map(|sidx| {
        sg.node(sidx)
            .and_then(|s| s.commits.iter().position(|c| c.id == entrypoint))
            .map(|pos| (sidx, pos))
    })?;
    debug_assert_eq!(
        pos, 0,
        "the entrypoint {entrypoint} is a planned boundary everywhere, yet sits mid-run in {sidx}"
    );
    if pos != 0 {
        return None;
    }
    if let Some(ep_ref) = entrypoint_ref {
        let current = sg
            .node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .map(|ri| ri.ref_name.clone());
        match current {
            None => {
                if let Some(s) = sg.node_mut(sidx) {
                    s.ref_info = Some(RefInfo {
                        ref_name: ep_ref.clone(),
                        commit_id: Some(entrypoint),
                        worktree: None,
                    });
                    s.remote_tracking_ref_name = remote_tracking.get(ep_ref).cloned();
                }
            }
            Some(existing) if existing != *ep_ref => {
                let empty = sg.add_node(Segment {
                    id: 0,
                    ref_info: Some(RefInfo {
                        ref_name: ep_ref.clone(),
                        commit_id: Some(entrypoint),
                        worktree: None,
                    }),
                    remote_tracking_ref_name: remote_tracking.get(ep_ref).cloned(),
                    sibling_segment_id: None,
                    remote_tracking_branch_segment_id: None,
                    commits: Vec::new(),
                    connections: Vec::new(),
                    metadata: None,
                });
                sg.node_mut(empty).expect("just added").id = empty;
                // Incoming edges now route through the entrypoint's empty segment.
                for other in sg.node_indices().collect::<Vec<_>>() {
                    if other == empty {
                        continue;
                    }
                    if let Some(s) = sg.node_mut(other) {
                        for conn in &mut s.connections {
                            if conn.target == sidx {
                                conn.target = empty;
                                conn.dst = None;
                                conn.dst_id = None;
                            }
                        }
                    }
                }
                connect(sg, empty, sidx);
                return Some(empty);
            }
            Some(_) => {}
        }
    }
    Some(sidx)
}

/// One floated lane placeholder decided by [`lane_plan`]: `tip`'s segment goes anonymous, an
/// empty segment named `name` splices in between the workspace and it, and `displaced` (a
/// build-time name pushed aside by a metadata stack branch) returns to the commit as a passive
/// ref.
struct Float {
    /// The commit whose segment goes anonymous so the empty named segment can float above it.
    tip: gix::ObjectId,
    /// The name given to the empty segment spliced in between the workspace and `tip`.
    name: gix::refs::FullName,
    /// A build-time name pushed aside by a metadata stack branch; it returns to `tip`'s commit as
    /// a passive ref. `None` when nothing was displaced.
    displaced_ref_name: Option<gix::refs::FullName>,
}

/// The managed lane NAME decisions, computed before any segment mutation happens (phase 2 of
/// gather-then-build). Models the naming state the passes would see — materialization names,
/// then the anon-owner renames of the remote/target/explicit-tip passes — and decides purely:
///
/// * which shared workspace-parent tips float their name up as an empty lane placeholder
///   (`anonymize_shared_stack_tips`),
/// * which anchors are DEMOTED to anonymous (a shared base at/below the bound, the lower-bound
///   float) so their stacks' branches form their own lanes (`insert_empty_branches`' demotions).
///
/// The group-naming decisions stay in `insert_empty_branches` for now: their "does this ref
/// already name a segment" checks range over remote segments, which become plan data only when
/// the remote passes are planned too.
/// One same-commit group of a metadata stack list — the RefOrder unit. Groups appear in
/// metadata order (top → bottom of the stack); a group's refs all point at `commit`.
#[derive(Debug, PartialEq, Eq, Clone)]
struct RefGroup {
    /// The commit every ref of this group points at.
    commit: gix::ObjectId,
    /// The members that become empty segments spliced above the anchor, in metadata order.
    /// The group's remaining members either name the anchor or already name another segment.
    empties: Vec<gix::refs::FullName>,
    /// How the group lands in the graph.
    placement: GroupPlacement,
}

/// How a [`RefGroup`] is placed by materialization.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum GroupPlacement {
    /// The group's commit is inside another lane: the empties splice into that chain.
    Dependent,
    /// The group anchors its own lane from the workspace (shared base or integrated anchor).
    OwnLane,
    /// Another stack owns the (non-integrated) commit: the refs stay passive on it.
    Passive,
    /// The group is outside the workspace or co-located with a managed merge commit — nothing
    /// is created. Kept so group ordinals stay aligned between plan and build.
    Skipped,
}

struct LanePlan {
    floats: Vec<Float>,
    demoted: HashSet<gix::ObjectId>,
    /// Group-naming decisions of `insert_empty_branches`, keyed by (stack-list index, group
    /// commit): the anchor takes `name`; `clear_remote` marks the metadata-order override (a
    /// non-bottom namer is displaced, the remote creators link its floated empty instead).
    group_names: HashMap<(usize, gix::ObjectId), (gix::refs::FullName, bool)>,
    /// Every boundary tip's MATERIALIZATION name (before floats/demotions suppress it on the
    /// segment). The remote/target passes historically ran before the lane shape existed and
    /// keyed their decisions on these — they read them through [`LanePlan::effective_name`].
    base_name_of: HashMap<gix::ObjectId, gix::refs::FullName>,
    /// Names the remote/target/explicit-tip passes give to ANONYMOUS boundary tips (a remote
    /// pointing behind/at an anonymous owner names it; the target and explicit tips likewise).
    /// Modeled here in pass order so materialization can mint segments with their FINAL names;
    /// the passes only add links. The value carries the named ref's actual position (a behind
    /// remote can point mid-run, below the owner's tip).
    renames: HashMap<gix::ObjectId, (gix::refs::FullName, gix::ObjectId)>,
    /// Every remote-ref name the remote passes will consume (renames, empty roots, ahead
    /// regions, untracked surfacing, the target). With the lane structure built FIRST, the
    /// empties filter consults this instead of finding the remote segments in the graph.
    remote_used: HashSet<gix::refs::FullName>,
    /// The RefOrder: co-located ref-order decisions per metadata stack list, in metadata
    /// order — which refs of each same-commit group become empty segments, and how the group
    /// is placed. Modeled with the same `used`-names state the group naming sees, so
    /// materialization can consume order as DATA instead of re-deriving it from the graph.
    ref_order: Vec<Vec<RefGroup>>,
}

#[allow(clippy::too_many_arguments)]
fn lane_plan<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    ws_lower_bound: Option<gix::ObjectId>,
    managed: bool,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    symbolic_remotes: &[String],
    extra_target: Option<gix::ObjectId>,
) -> LanePlan {
    let mut plan = LanePlan {
        floats: Vec::new(),
        demoted: HashSet::new(),
        group_names: HashMap::new(),
        base_name_of: HashMap::new(),
        renames: HashMap::new(),
        remote_used: HashSet::new(),
        ref_order: Vec::new(),
    };
    // The naming state as the lane passes will see it: materialization names first…
    let mut name_of: HashMap<gix::ObjectId, gix::refs::FullName> = HashMap::new();
    for &tip in &facts.tips {
        if let Some(name) = materialize_tip_name(
            cg,
            tip,
            workspace_commit,
            facts.ws_is_managed_merge,
            facts.entrypoint_forced_boundary.then_some(entrypoint),
            entrypoint_ref,
            remote_tracking,
            meta,
            target_ref,
        ) {
            name_of.insert(tip, name);
        }
    }
    plan.base_name_of = name_of.clone();
    // …then the anon-owner renames of `add_remote_segments` (a remote pointing BEHIND/at an
    // anonymous in-set segment names it), in materialization order like the pass. Every remote
    // name the pass consumes — a rename, an empty root, an ahead region — is tracked, because
    // the target block only runs when nothing already used the target ref.
    let in_play = |rt: &gix::refs::FullName| remote_name_in_play(rt, symbolic_remotes);
    let is_meta_stack_branch = |r: &gix::refs::FullName| {
        stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| b == r)
    };
    let mut remote_used: HashSet<gix::refs::FullName> = HashSet::new();
    for &tip in &facts.tips {
        let Some(remote_ref) = name_of.get(&tip).and_then(|n| remote_tracking.get(n)) else {
            continue;
        };
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        if facts.in_set.contains(&remote_tip) {
            let owner = facts
                .owner_of
                .get(&remote_tip)
                .copied()
                .unwrap_or(remote_tip);
            if let std::collections::hash_map::Entry::Vacant(e) = name_of.entry(owner) {
                e.insert(remote_ref.clone());
                plan.renames.insert(owner, (remote_ref.clone(), remote_tip));
            }
            remote_used.insert(remote_ref.clone());
        } else if in_play(remote_ref) && !is_meta_stack_branch(remote_ref) {
            remote_used.insert(remote_ref.clone());
        }
    }
    // …the untracked-remote pass surfacing remotes whose local counterpart shares the commit…
    {
        let mut remote_refs: std::collections::BTreeSet<gix::refs::FullName> =
            std::collections::BTreeSet::new();
        for c in cg.commit_ids() {
            for r in cg.refs_at(c) {
                if r.as_ref().category() == Some(Category::RemoteBranch) {
                    remote_refs.insert(r);
                }
            }
        }
        for r in remote_refs {
            if remote_used.contains(&r) || name_of.values().any(|n| *n == r) {
                continue;
            }
            let Some(tip) = cg.commit_by_ref(r.as_ref()) else {
                continue;
            };
            if facts.in_set.contains(&tip)
                && cg
                    .refs_at(tip)
                    .iter()
                    .any(|l| remote_tracking.get(l) == Some(&r))
            {
                remote_used.insert(r);
            }
        }
    }
    // …the target pass naming an anonymous in-set owner after the target ref — only when
    // nothing already used it…
    if let Some(tr) = target_ref
        && tr.as_ref().category() == Some(Category::RemoteBranch)
        && !remote_used.contains(tr)
        && !name_of.values().any(|n| n == tr)
        && let Some(tip) = cg.commit_by_ref(tr.as_ref())
    {
        if facts.in_set.contains(&tip) {
            let owner = facts.owner_of.get(&tip).copied().unwrap_or(tip);
            if let std::collections::hash_map::Entry::Vacant(e) = name_of.entry(owner) {
                e.insert(tr.clone());
                plan.renames.insert(owner, (tr.clone(), tip));
            }
        }
        remote_used.insert(tr.clone());
    }
    // …and the explicit-tip pass naming anonymous segments that START at a tip.
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        let Some(ref_name) = t.ref_name.clone() else {
            continue;
        };
        if but_core::is_workspace_ref_name(ref_name.as_ref())
            || name_of.values().any(|n| *n == ref_name)
        {
            continue;
        }
        if facts.boundaries.contains(&t.id)
            && let std::collections::hash_map::Entry::Vacant(e) = name_of.entry(t.id)
        {
            e.insert(ref_name.clone());
            plan.renames.insert(t.id, (ref_name, t.id));
        }
    }

    if !managed {
        plan.remote_used = remote_used;
        return plan;
    }

    // ── anonymize_shared_stack_tips: which workspace-parent tips float ──
    let is_stack_branch = |n: &gix::refs::FullName| {
        stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| b == n)
    };
    if facts.ws_is_managed_merge {
        for parent in cg.parents(workspace_commit) {
            // The target/base lane keeps its name even when other stacks depend on it.
            if Some(parent) == target || !facts.boundaries.contains(&parent) {
                continue;
            }
            let Some(current) = name_of.get(&parent).cloned() else {
                continue;
            };
            // Shared iff some other IN-WORKSPACE commit's first parent is this tip.
            let shared = facts.in_set.iter().any(|&c| {
                c != workspace_commit
                    && cg.first_parent(c) == Some(parent)
                    && cg
                        .node(c)
                        .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::InWorkspace))
            });
            if !shared {
                continue;
            }
            // When build-time disambiguation picked a NON-stack ref, float the unique metadata
            // STACK branch instead and return the displaced name to the commit as a passive ref:
            // an applied-but-empty stack must keep its own lane, or the projection's
            // integration-prune swallows the whole stack with the shared base it would own.
            let (float_name, displaced) = if is_stack_branch(&current) {
                (current.clone(), None)
            } else {
                let mut stack_refs = cg
                    .refs_at(parent)
                    .into_iter()
                    .filter(|r| is_plain_local_branch(r) && is_stack_branch(r));
                match (stack_refs.next(), stack_refs.next()) {
                    (Some(stack_ref), None)
                        if !name_of.values().any(|n| *n == stack_ref)
                            && !plan.floats.iter().any(|f| f.name == stack_ref) =>
                    {
                        (stack_ref, Some(current.clone()))
                    }
                    _ => (current.clone(), None),
                }
            };
            name_of.remove(&parent);
            plan.floats.push(Float {
                tip: parent,
                name: float_name,
                displaced_ref_name: displaced,
            });
        }
    }

    // ── insert_empty_branches' demotions ──
    let Some(lists) = stack_branches else {
        return plan;
    };
    let mut lists_per_commit: HashMap<gix::ObjectId, usize> = HashMap::new();
    for list in lists {
        let mut seen = HashSet::new();
        for b in list {
            if let Some(c) = cg.commit_by_ref(b.as_ref())
                && seen.insert(c)
            {
                *lists_per_commit.entry(c).or_default() += 1;
            }
        }
    }
    let at_or_below_bound: Option<HashSet<gix::ObjectId>> =
        ws_lower_bound.map(|lb| cg.ancestor_set(lb));
    // A commit pointed at by branches of SEVERAL metadata stacks at/below the bound is a shared
    // base: its segment stays anonymous and every stack's branches float above as their own lane.
    for (&commit, &count) in &lists_per_commit {
        if count <= 1 {
            continue;
        }
        if let Some(below) = &at_or_below_bound
            && !below.contains(&commit)
        {
            continue;
        }
        let anchor = facts.owner_of.get(&commit).copied().unwrap_or(commit);
        if name_of
            .get(&anchor)
            .is_some_and(|n| lists.iter().flatten().any(|b| b == n))
        {
            name_of.remove(&anchor);
            plan.demoted.insert(anchor);
        }
    }
    // The workspace LOWER BOUND is where independent stacks rest: an otherwise-unrepresented
    // stack's branch pointing there floats as its own empty lane and the boundary segment stays
    // anonymous.
    let floats_at_lower_bound = |list: &Vec<gix::refs::FullName>| -> bool {
        let Some(lb) = ws_lower_bound else {
            return false;
        };
        let mut at_lb = false;
        for b in list {
            match cg.commit_by_ref(b.as_ref()) {
                Some(c) if c == lb => at_lb = true,
                Some(c)
                    if cg.node(c).is_some_and(|n| {
                        !n.commit.flags.contains(crate::CommitFlags::Integrated)
                    }) =>
                {
                    return false;
                }
                _ => {}
            }
        }
        at_lb
    };
    if let Some(lb) = ws_lower_bound
        && facts.boundaries.contains(&lb)
        && name_of.get(&lb).is_some_and(|n| {
            lists
                .iter()
                .any(|l| l.contains(n) && floats_at_lower_bound(l))
        })
    {
        name_of.remove(&lb);
        plan.demoted.insert(lb);
    }

    // ── Group naming: the pass's "does this ref already name a segment" ranges over every
    // segment, so model the full set of names in use by insert_empty_branches time — lane names
    // plus everything the remote/target/tip/advanced passes will have created. ──
    let mut used: HashSet<gix::refs::FullName> = name_of.values().cloned().collect();
    used.extend(plan.floats.iter().map(|fl| fl.name.clone()));
    used.extend(remote_used.iter().cloned());
    // The target ref always ends up naming something when it resolves.
    if let Some(tr) = target_ref
        && tr.as_ref().category() == Some(Category::RemoteBranch)
        && cg.commit_by_ref(tr.as_ref()).is_some()
    {
        used.insert(tr.clone());
    }
    // Explicit traversal tips name a segment (an anon owner, an empty splice, or a region tip).
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        if let Some(rn) = t.ref_name.clone()
            && !but_core::is_workspace_ref_name(rn.as_ref())
            && cg.node(t.id).is_some()
        {
            used.insert(rn);
        }
    }
    // An extra target outside every region is surfaced named by the unique plain local on it.
    if let Some(extra) = extra_target
        && cg.node(extra).is_some()
        && !facts.in_set.contains(&extra)
    {
        let mut locals = cg.refs_at(extra).into_iter().filter(is_plain_local_branch);
        if let (Some(l), None) = (locals.next(), locals.next()) {
            used.insert(l);
        }
    }
    // Advanced-outside branches (`add_advanced_outside_branches`), deduped by outside tip.
    {
        let mut adv_seen: HashSet<gix::ObjectId> = HashSet::new();
        for b in lists.iter().flatten() {
            if !is_plain_local_branch(b) || used.contains(b) {
                continue;
            }
            let Some(tip) = cg.commit_by_ref(b.as_ref()) else {
                continue;
            };
            if facts.in_set.contains(&tip) || !adv_seen.insert(tip) {
                continue;
            }
            let mut cursor = Some(tip);
            let mut any_outside = false;
            let mut rejoin = false;
            while let Some(id) = cursor {
                if facts.in_set.contains(&id) {
                    rejoin = true;
                    break;
                }
                any_outside = true;
                cursor = cg.first_parent(id);
            }
            if !(rejoin && any_outside) {
                continue;
            }
            if let Some(name) = disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref)
            {
                used.insert(name);
            }
        }
    }

    // The group threading, mirroring `insert_empty_branches` exactly: per stack list, groups of
    // consecutive branches on one commit; the bottom-most member names an anonymous anchor, and
    // metadata order overrides a build-time name that belongs to the group.
    for (li, list) in lists.iter().enumerate() {
        let list: Vec<gix::refs::FullName> = list
            .iter()
            .filter(|b| cg.commit_by_ref(b.as_ref()).is_some())
            .cloned()
            .collect();
        plan.ref_order.push(Vec::new());
        let mut i = 0;
        while i < list.len() {
            let commit = cg.commit_by_ref(list[i].as_ref());
            let start = i;
            while i < list.len() && cg.commit_by_ref(list[i].as_ref()) == commit {
                i += 1;
            }
            let group = &list[start..i];
            let Some(commit) = commit else { continue };
            if !facts.in_set.contains(&commit)
                || (commit == workspace_commit && facts.ws_is_managed_merge)
            {
                plan.ref_order[li].push(RefGroup {
                    commit,
                    empties: Vec::new(),
                    placement: GroupPlacement::Skipped,
                });
                continue;
            }
            let anchor = facts.owner_of.get(&commit).copied().unwrap_or(commit);
            let shared_commit_above_bound = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && at_or_below_bound
                    .as_ref()
                    .is_some_and(|below| !below.contains(&commit));
            if !name_of.contains_key(&anchor)
                && (lists_per_commit.get(&commit).copied().unwrap_or(0) <= 1
                    || shared_commit_above_bound)
                && !(Some(commit) == ws_lower_bound && floats_at_lower_bound(&list))
                && let Some(namer) = group.last()
                && !used.contains(namer)
            {
                name_of.insert(anchor, namer.clone());
                used.insert(namer.clone());
                plan.group_names
                    .insert((li, commit), (namer.clone(), false));
            }
            if let Some(namer) = group.last()
                && name_of
                    .get(&anchor)
                    .is_some_and(|n| n != namer && group.contains(n))
            {
                // The override DISPLACES the anchor's build-time name: it no longer names any
                // segment, so it re-enters the pool and splices as an empty group member.
                if let Some(displaced) = name_of.get(&anchor) {
                    used.remove(displaced);
                }
                name_of.insert(anchor, namer.clone());
                used.insert(namer.clone());
                plan.group_names.insert((li, commit), (namer.clone(), true));
            }
            // Every group member ends placed (naming the anchor or spliced as an empty) when the
            // empties path runs; the cross-stack-owned skip leaves them passive instead.
            let cross_stack_owned = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && name_of
                    .get(&anchor)
                    .is_some_and(|n| !list.contains(n) && lists.iter().any(|l| l.contains(n)));
            let anchor_not_integrated = cg
                .node(anchor)
                .is_some_and(|n| !n.commit.flags.contains(crate::CommitFlags::Integrated));
            // ── The RefOrder: which members become empties, and how the group lands. The
            // `used` set at THIS point models materialization's "already names a segment"
            // gate (the group namer included — it names the anchor, not an empty). ──
            let shared_base = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && at_or_below_bound
                    .as_ref()
                    .is_none_or(|below| below.contains(&commit));
            let placement = if cross_stack_owned && anchor_not_integrated {
                GroupPlacement::Passive
            } else if !shared_base && anchor_not_integrated {
                GroupPlacement::Dependent
            } else {
                GroupPlacement::OwnLane
            };
            plan.ref_order[li].push(RefGroup {
                commit,
                empties: group
                    .iter()
                    .filter(|b| !used.contains(*b) && !remote_used.contains(*b))
                    .cloned()
                    .collect(),
                placement,
            });
            if !(cross_stack_owned && anchor_not_integrated) {
                used.extend(group.iter().cloned());
            }
        }
    }
    plan.remote_used = remote_used;
    plan
}

/// The name a boundary tip gets at MATERIALIZATION — shared with `lane_plan`'s modeling so plan
/// and build cannot drift. The managed workspace tip is named by the workspace ref itself (a
/// `gitbutler/*` ref that normal disambiguation skips); a forced entrypoint boundary keeps the
/// split's precedence (checked-out ref first); every other tip is named by disambiguation. A
/// truly detached HEAD is anonymized afterwards by `from_head`'s detach pass, never here.
#[allow(clippy::too_many_arguments)]
fn materialize_tip_name<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    workspace_commit: gix::ObjectId,
    ws_is_managed_merge: bool,
    forced_entrypoint: Option<gix::ObjectId>,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    if tip == workspace_commit {
        if ws_is_managed_merge {
            // The real managed merge is named by EXACTLY the workspace ref. Other transient
            // `gitbutler/*` refs can be co-located — e.g. `gitbutler/edit` mid edit-mode — and
            // must never name (or join) the workspace. The traversal can drop the ws ref from
            // the commit — the caller established it points here, so fall back to the
            // well-known name.
            cg.refs_at(tip)
                .into_iter()
                .find(|r| but_core::is_workspace_ref_name(r.as_ref()))
                .or_else(|| but_core::WORKSPACE_REF_NAME.try_into().ok())
        } else {
            // Co-located stack tip / advanced ref (managed) or a non-managed tip: name by
            // disambiguation; the empty workspace segment is spliced in above afterward.
            disambiguated_ref(
                cg,
                tip,
                remote_tracking,
                meta,
                Some(workspace_commit),
                target_ref,
            )
        }
    } else if forced_entrypoint == Some(tip) {
        entrypoint_ref
            .cloned()
            .or_else(|| disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref))
    } else {
        disambiguated_ref(
            cg,
            tip,
            remote_tracking,
            meta,
            Some(workspace_commit),
            target_ref,
        )
    }
}

/// The first-parent commit run owned by `tip`: `tip` and each first-parent descendant-in-history until
/// the next boundary (exclusive) or the set edge.
fn commit_run(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &HashSet<gix::ObjectId>,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
) -> Vec<Commit> {
    let mut out = Vec::new();
    let mut id = Some(tip);
    while let Some(c) = id {
        if !in_set.contains(&c) {
            break;
        }
        if c != tip && is_boundary(c) {
            break;
        }
        if let Some(node) = cg.node(c) {
            out.push(node.commit.clone());
        }
        id = cg.first_parent(c).filter(|p| in_set.contains(p));
    }
    out
}

/// Link a just-created remote-named segment to the local segment named by its tracking
/// counterpart: the remote's sibling points at the local, and the local carries the remote's
/// name and segment id. A no-op when no such local exists (e.g. the local ref only rides a
/// commit).
fn link_remote_to_local(
    sg: &mut SegmentGraph,
    remote_sidx: SegmentIndex,
    remote_ref: &gix::refs::FullName,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let Some(local_name) = remote_tracking
        .iter()
        .find_map(|(l, r)| (r == remote_ref).then_some(l))
    else {
        return;
    };
    let Some(local_sidx) = segment_by_ref(sg, local_name) else {
        return;
    };
    if let Some(s) = sg.node_mut(remote_sidx) {
        s.sibling_segment_id = Some(local_sidx);
    }
    if let Some(s) = sg.node_mut(local_sidx) {
        s.remote_tracking_ref_name = Some(remote_ref.clone());
        s.remote_tracking_branch_segment_id = Some(remote_sidx);
    }
}

#[expect(clippy::too_many_arguments)]
fn add_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    in_set: &HashSet<gix::ObjectId>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    pinned_commits: &HashSet<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    pre_lane_names: &HashMap<gix::ObjectId, gix::refs::FullName>,
    renames: &HashMap<gix::ObjectId, (gix::refs::FullName, gix::ObjectId)>,
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    // Locals are keyed on the PRE-LANE names: this pass historically ran before the lane shape
    // was applied and saw every build-time name. The LINKS however belong to whichever segment
    // finally carries the name — a lane placeholder, a spliced empty, a group-named anchor —
    // which exists already (lanes precede this pass), so no reconciliation is needed after.
    let mut locals: Vec<(SegmentIndex, gix::refs::FullName, gix::ObjectId)> = seg_of_tip
        .iter()
        .filter_map(|(&tip, &sidx)| {
            let name = pre_lane_names.get(&tip)?;
            let rt = remote_tracking.get(name).cloned()?;
            let link_sidx = segment_by_ref(sg, name).unwrap_or(sidx);
            Some((link_sidx, rt, tip))
        })
        .collect();
    // Deterministic remote-segment ids: `seg_of_tip` is a HashMap, its order varies per process.
    locals.sort_by_key(|&(sidx, ..)| sidx);
    for (local_sidx, remote_ref, _local_tip) in locals {
        let Some(remote_tip) = cg.commit_by_ref(remote_ref.as_ref()) else {
            continue;
        };
        // The remote points BEHIND/at an in-set commit: it names that commit's segment rather than
        // forming its own root. If the segment is anonymous, the remote ref names it directly; if it is
        // already named (e.g. the target `main`), a separate empty remote root points into it.
        if in_set.contains(&remote_tip) {
            let owner = owner_of.get(&remote_tip).copied().unwrap_or(remote_tip);
            let owner_sidx = seg_of_tip[&owner];
            // Materialization applied the plan's rename when this remote NAMES the (previously
            // anonymous) owner; this pass only adds the links.
            let named_by_this = renames
                .get(&owner)
                .is_some_and(|(name, _)| name == &remote_ref);
            if named_by_this {
                if let Some(s) = sg.node_mut(owner_sidx) {
                    s.sibling_segment_id = Some(local_sidx);
                }
                sg.node_mut(local_sidx)
                    .expect("present")
                    .remote_tracking_branch_segment_id = Some(owner_sidx);
            } else {
                let remote_sidx = add_empty_remote_root(sg, &remote_ref, remote_tip, local_sidx);
                connect(sg, remote_sidx, owner_sidx);
            }
            continue;
        }

        // The remote is AHEAD: segment its ahead region like the local graph (split at merges and
        // second-parent branches), not as one flat first-parent run. Only for remotes the workspace
        // configuration implies (target/push remote, or a git-configured tracking branch) — and never
        // when the remote ref is ITSELF a workspace-metadata stack branch: then it lives in the
        // workspace as a stack, its commits are the user's own, not an upstream.
        let in_play = remote_name_in_play(&remote_ref, symbolic_remotes);
        let is_metadata_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| *b == remote_ref);
        if !in_play || is_metadata_stack_branch {
            continue;
        }
        segment_ahead_region(
            cg,
            sg,
            Some(&remote_ref),
            remote_tip,
            in_set,
            seg_of_tip,
            owner_of,
            remote_tracking,
            Some(local_sidx),
            pinned_commits,
            claimed_remote_names,
            pending_edges,
        );
    }
}

/// Segment a remote's AHEAD region (commits reachable from `remote_tip` that are not in-set) the same
/// way the local graph is segmented — splitting at merges and their second-parent branches — instead
/// of collapsing it into one flat first-parent run. The tip segment is named `remote_ref` (sibling
/// `local_sidx`); interior merges and second-parent branches become their own anonymous remote
/// segments. Bottom-of-segment parents connect to the owning ahead segment, or to the local segment
/// where the region rejoins the graph.
#[allow(clippy::too_many_arguments)]
fn segment_ahead_region(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    // `None` for a bare EXTRA TARGET commit id — the tip segment is then named by the unique
    // plain local branch on it, if any.
    remote_ref: Option<&gix::refs::FullName>,
    remote_tip: gix::ObjectId,
    in_set: &HashSet<gix::ObjectId>,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // The local segment tracking this remote, if any — a TARGET without a tracking local segment
    // builds the same region without the sibling/remote-tracking links.
    local_sidx: Option<SegmentIndex>,
    // Stored/extra target positions must start their own segment even inside a remote's ahead
    // region — the projection's `TargetCommit::from_commit` ignores one that sits mid-segment,
    // silently disabling integration checks against it.
    pinned_commits: &HashSet<gix::ObjectId>,
    // Remote refs some creator will consume as a segment name (tracked roots, untracked
    // surfacing, the target, explicit tips): an interior remote ref cuts the root run only
    // when unclaimed — a claimed one STOPS it (that creator's region owns the territory).
    claimed_remote_names: &HashSet<gix::refs::FullName>,
    // Connections into another creator's region, recorded as (source segment, parent commit)
    // and wired by the caller once every creator ran — the target segment may not exist yet.
    pending_edges: &mut Vec<(SegmentIndex, gix::ObjectId)>,
) {
    // Commits the remote is ahead by: ancestors of the tip that stop at the in-set boundary.
    let mut ahead_set: HashSet<gix::ObjectId> = HashSet::new();
    let mut stack = vec![remote_tip];
    while let Some(id) = stack.pop() {
        if in_set.contains(&id) || !ahead_set.insert(id) {
            continue;
        }
        stack.extend(cg.all_parent_ids(id));
    }

    let mut children: HashMap<gix::ObjectId, Vec<gix::ObjectId>> = HashMap::new();
    for &c in &ahead_set {
        for p in cg.all_parent_ids(c) {
            if ahead_set.contains(&p) {
                children.entry(p).or_default().push(c);
            }
        }
    }
    let merge_first_parents: HashSet<gix::ObjectId> = ahead_set
        .iter()
        .filter(|&&c| cg.all_parent_ids(c).len() > 1)
        .filter_map(|&c| cg.first_parent(c))
        .filter(|p| ahead_set.contains(p))
        .collect();
    let is_boundary = |c: gix::ObjectId| -> bool {
        c == remote_tip
            || pinned_commits.contains(&c)
            || cg.all_parent_ids(c).len() > 1
            || merge_first_parents.contains(&c)
            || cg.refs_at(c).iter().any(is_plain_local_branch)
            || {
                let kids = children.get(&c).map(Vec::as_slice).unwrap_or_default();
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && ahead_set.contains(&k))
            }
    };

    // Remote refs riding non-boundary commits of a remote-named root's run resolve at creation
    // (formerly the post-hoc surgery of `split_remote_interior_refs`/`split_stacked_remotes`):
    // an UNCLAIMED ref cuts the run — the commit starts its own segment named by that ref; a
    // CLAIMED ref (another creator's root, built or not) STOPS the run — its territory belongs
    // to that creator's region, and the connection into it is wired once every creator ran.
    let root_is_remote =
        remote_ref.is_some_and(|r| r.as_ref().category() == Some(Category::RemoteBranch));
    let mut interior_cuts: HashMap<gix::ObjectId, gix::refs::FullName> = HashMap::new();
    let mut stop: Option<gix::ObjectId> = None;
    if root_is_remote {
        let existing_remote_tip = |c: gix::ObjectId| {
            sg.node_indices().any(|sidx| {
                sg.node(sidx).is_some_and(|s| {
                    s.commits.first().is_some_and(|f| f.id == c)
                        && s.ref_info.as_ref().is_some_and(|ri| {
                            ri.ref_name.as_ref().category() == Some(Category::RemoteBranch)
                        })
                })
            })
        };
        let mut id = cg
            .first_parent(remote_tip)
            .filter(|p| ahead_set.contains(p));
        while let Some(c) = id {
            if is_boundary(c) {
                break;
            }
            if cg
                .refs_at(c)
                .iter()
                .any(|r| claimed_remote_names.contains(r))
                || existing_remote_tip(c)
            {
                stop = Some(c);
                break;
            }
            if let Some(r) = cg.refs_at(c).into_iter().find(|r| {
                r.as_ref().category() == Some(Category::RemoteBranch)
                    && !claimed_remote_names.contains(r)
                    && segment_by_ref(sg, r).is_none()
            }) {
                interior_cuts.insert(c, r);
            }
            id = cg.first_parent(c).filter(|p| ahead_set.contains(p));
        }
    }
    let is_boundary =
        |c: gix::ObjectId| is_boundary(c) || interior_cuts.contains_key(&c) || stop == Some(c);

    let mut tips: Vec<gix::ObjectId> = ahead_set
        .iter()
        .copied()
        .filter(|&c| is_boundary(c))
        .collect();
    // Deterministic segment ids: `ahead_set` is a HashSet, its order varies per process.
    tips.sort_by_key(|&t| {
        (
            t != remote_tip,
            std::cmp::Reverse(cg.node(t).map(|n| n.generation).unwrap_or(0)),
            t,
        )
    });
    let mut ahead_owner: HashMap<gix::ObjectId, gix::ObjectId> = HashMap::new();
    let mut ahead_seg: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();
    let mut reused: HashSet<gix::ObjectId> = HashSet::new();
    for &tip in &tips {
        // The stop commit is another creator's root: this region neither mints nor owns it —
        // the connection into that segment is a pending edge resolved after every creator ran.
        if stop == Some(tip) {
            continue;
        }
        let commits = commit_run(cg, tip, &ahead_set, &is_boundary);
        for c in &commits {
            ahead_owner.insert(c.id, tip);
        }
        let is_root = tip == remote_tip;
        // Overlapping regions can split at the same boundary (two stacked remotes above `main`):
        // a segment starting at this commit may already exist. Reuse it rather than minting a
        // dangling twin. Roots keep their own identity (their name and sibling links belong to
        // THIS region's ref).
        if !is_root
            && let Some(existing) = sg.node_indices().find(|&sidx| {
                sg.node(sidx)
                    .is_some_and(|s| s.commits.first().is_some_and(|c| c.id == tip))
            })
        {
            ahead_seg.insert(tip, existing);
            reused.insert(tip);
            continue;
        }
        let root_name = || {
            remote_ref.cloned().or_else(|| {
                let mut it = cg
                    .refs_at(remote_tip)
                    .into_iter()
                    .filter(is_plain_local_branch);
                it.next().filter(|_| it.next().is_none())
            })
        };
        // Interior segments are named by the cutting remote ref, else by the unique plain local
        // branch at their boundary, like the local graph's ref-driven segmentation; ambiguity
        // keeps them anonymous.
        let interior_name = || {
            interior_cuts.get(&tip).cloned().or_else(|| {
                let mut it = cg.refs_at(tip).into_iter().filter(is_plain_local_branch);
                it.next().filter(|_| it.next().is_none())
            })
        };
        let ref_info = if is_root {
            root_name().map(|ref_name| RefInfo {
                ref_name,
                commit_id: Some(remote_tip),
                worktree: None,
            })
        } else {
            interior_name().map(|ref_name| RefInfo {
                ref_name,
                commit_id: Some(tip),
                worktree: None,
            })
        };
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .filter(|ri| is_plain_local_branch(&ri.ref_name))
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_node(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: if is_root { local_sidx } else { None },
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(sidx).expect("just added").id = sidx;
        ahead_seg.insert(tip, sidx);
        if is_root && let Some(local_sidx) = local_sidx {
            sg.node_mut(local_sidx)
                .expect("present")
                .remote_tracking_branch_segment_id = Some(sidx);
        }
        // A cut segment is remote-named: link it to its tracking local like every remote creator.
        if let Some(cut_ref) = interior_cuts.get(&tip) {
            let cut_ref = cut_ref.clone();
            link_remote_to_local(sg, sidx, &cut_ref, remote_tracking);
        }
    }

    for &tip in &tips {
        // A reused segment already carries its own outgoing connections; the stop commit was
        // never minted here.
        if reused.contains(&tip) || stop == Some(tip) {
            continue;
        }
        let src = ahead_seg[&tip];
        let bottom = sg
            .node(src)
            .and_then(|s| s.commits.last().map(|c| c.id))
            .unwrap_or(tip);
        for parent in cg.all_parent_ids(bottom) {
            let dst = if ahead_set.contains(&parent) {
                ahead_owner
                    .get(&parent)
                    .and_then(|o| ahead_seg.get(o))
                    .copied()
            } else {
                owner_of
                    .get(&parent)
                    .and_then(|o| seg_of_tip.get(o))
                    .copied()
            };
            if let Some(dst) = dst {
                connect(sg, src, dst);
            } else if ahead_set.contains(&parent) {
                // The parent is beyond the stop: another creator's region owns it, and its
                // segment may not exist yet — wire once every creator ran.
                pending_edges.push((src, parent));
            }
        }
    }
}

/// Create segments for remote-tracking refs that no local segment claimed (untracked/orphan remotes,
/// e.g. `origin/C` pointing at an anonymous commit). Each becomes an empty root connecting to the
/// segment owning its tip, with no sibling.
fn add_untracked_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    in_set: &HashSet<gix::ObjectId>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
) {
    let mut remote_refs: std::collections::BTreeSet<gix::refs::FullName> =
        std::collections::BTreeSet::new();
    for c in cg.commit_ids() {
        for r in cg.refs_at(c) {
            if r.as_ref().category() == Some(Category::RemoteBranch) {
                remote_refs.insert(r);
            }
        }
    }
    for r in remote_refs {
        if segment_by_ref(sg, &r).is_some() {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(r.as_ref()) else {
            continue;
        };
        // Only surface a remote whose LOCAL counterpart actually sits on the same commit (e.g.
        // `C`/`origin/C` on an ambiguous tip). An ORPHAN remote (`origin/A` with no local `A`)
        // has no lane to pair with and is deliberately invisible: the traversal never walks it
        // (remotes are queued off encountered LOCAL refs only), and a metadata branch whose local
        // ref is missing is skipped at seeding. When the local ref (re)appears — e.g. an anonymous
        // segment renamed back to `A` — the next build pairs the remote again automatically.
        // Probed 2026-07-03: surfacing orphans leaks into apply/ad-hoc behavior, not just display.
        // `remote_tracking` maps every remote to a local name, so the discriminator is whether
        // that local ref really exists here.
        let has_local_counterpart = cg
            .refs_at(tip)
            .iter()
            .any(|l| remote_tracking.get(l) == Some(&r));
        if !has_local_counterpart {
            continue;
        }
        // Only the behind/in-set case for now: an empty root into the segment owning the tip.
        if in_set.contains(&tip)
            && let Some(&owner) = owner_of.get(&tip)
            && let Some(&owner_sidx) = seg_of_tip.get(&owner)
        {
            let remote_sidx = sg.add_node(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: r.clone(),
                    commit_id: Some(tip),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.node_mut(remote_sidx).expect("just added").id = remote_sidx;
            connect(sg, remote_sidx, owner_sidx);
            link_remote_to_local(sg, remote_sidx, &r, remote_tracking);
        }
    }
}

/// Several remote refs can share ONE commit (e.g. `origin/A`+`origin/B`+`origin/C` after squash
/// merges rewrote their locals) — the first named the commit-holding remote segment; every other
/// becomes an EMPTY segment pointing at it, so each remote ref resolves to a segment. A pure
/// creator over final names: adds empties + links, mutates nothing existing.
fn add_co_located_remote_empties(
    sg: &mut SegmentGraph,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let is_remote = |sg: &SegmentGraph, sidx: SegmentIndex| {
        sg.node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
    };
    for sidx in sg.node_indices().collect::<Vec<_>>() {
        if !is_remote(sg, sidx) {
            continue;
        }
        let Some(first) = sg.node(sidx).and_then(|s| s.commits.first().cloned()) else {
            continue;
        };
        for ri in &first.refs {
            if ri.ref_name.as_ref().category() != Some(Category::RemoteBranch)
                || segment_by_ref(sg, &ri.ref_name).is_some()
            {
                continue;
            }
            let empty = sg.add_node(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: ri.ref_name.clone(),
                    commit_id: Some(first.id),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.node_mut(empty).expect("just added").id = empty;
            connect(sg, empty, sidx);
            let name = ri.ref_name.clone();
            link_remote_to_local(sg, empty, &name, remote_tracking);
        }
    }
}

/// Create an empty remote root segment named `remote_ref`, sibling-linked to `local_sidx` (and set the
/// local's `remote_tracking_branch_segment_id`).
fn add_empty_remote_root(
    sg: &mut SegmentGraph,
    remote_ref: &gix::refs::FullName,
    remote_tip: gix::ObjectId,
    local_sidx: SegmentIndex,
) -> SegmentIndex {
    let remote_sidx = sg.add_node(Segment {
        id: 0,
        ref_info: Some(RefInfo {
            ref_name: remote_ref.clone(),
            commit_id: Some(remote_tip),
            worktree: None,
        }),
        remote_tracking_ref_name: None,
        sibling_segment_id: Some(local_sidx),
        remote_tracking_branch_segment_id: None,
        commits: Vec::new(),
        metadata: None,
        connections: Vec::new(),
    });
    sg.node_mut(remote_sidx).expect("just added").id = remote_sidx;
    sg.node_mut(local_sidx)
        .expect("present")
        .remote_tracking_branch_segment_id = Some(remote_sidx);
    remote_sidx
}

/// Splice an empty `gitbutler/workspace` segment above the stack tip the workspace ref is co-located
/// with (no dedicated merge commit). It holds no commits, carries the main worktree, and connects into
/// the stack segment that owns `workspace_commit`.
fn insert_empty_workspace_segment(
    sg: &mut SegmentGraph,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
) -> Option<SegmentIndex> {
    let stack_sidx = *seg_of_tip.get(&workspace_commit)?;
    // The traversal may have dropped the special workspace ref from the commit's refs when a stack
    // branch on the same commit named its raw segment — the caller established the ref points here,
    // so fall back to the well-known name rather than silently skipping the workspace segment.
    let ws_ref = cg
        .refs_at(workspace_commit)
        .into_iter()
        .find(|r| but_core::is_workspace_ref_name(r.as_ref()))
        .or_else(|| but_core::WORKSPACE_REF_NAME.try_into().ok())?;
    // The worktree annotation comes from the shared `worktree_by_branch` pass — HEAD may well be on
    // a stack branch, not the workspace ref.
    let ws_seg = sg.add_node(Segment {
        id: 0,
        ref_info: Some(RefInfo {
            ref_name: ws_ref,
            commit_id: Some(workspace_commit),
            worktree: None,
        }),
        remote_tracking_ref_name: None,
        sibling_segment_id: None,
        remote_tracking_branch_segment_id: None,
        commits: Vec::new(),
        metadata: None,
        connections: Vec::new(),
    });
    sg.node_mut(ws_seg).expect("just added").id = ws_seg;
    connect(sg, ws_seg, stack_sidx);
    Some(ws_seg)
}

/// Find the segment named exactly `ref_name`, if any.
fn segment_by_ref(sg: &SegmentGraph, ref_name: &gix::refs::FullName) -> Option<SegmentIndex> {
    sg.node_indices().find(|&sidx| {
        sg.node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| &ri.ref_name == ref_name)
    })
}

/// A metadata stack branch pointing at a commit OUTSIDE the workspace has advanced past it. Surface
/// its outside commits as a segment named after the branch: the first-parent run from its tip down to
/// the first in-workspace commit, connected into the segment owning that commit. That owning segment
/// gets a sibling link so the projection can display it under the advanced branch's name.
#[expect(clippy::too_many_arguments)]
fn add_advanced_outside_branches<T: but_core::RefMetadata>(
    sg: &mut SegmentGraph,
    cg: &CommitGraph,
    in_set: &HashSet<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    pinned_commits: &HashSet<gix::ObjectId>,
) {
    for b in stack_branches.into_iter().flatten().flatten() {
        // Only LOCAL branches advance past a workspace; metadata can also list remote refs as stack
        // branches, and those are handled by the remote passes.
        if !is_plain_local_branch(b) || segment_by_ref(sg, b).is_some() {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(b.as_ref()) else {
            continue;
        };
        if in_set.contains(&tip) {
            continue;
        }
        // A PINNED commit (a stored/extra target position) must start its own segment via the
        // extra-target region — the projection derives the remembered base from it. When lanes
        // run before the remote passes this pass would otherwise swallow it into the branch's
        // outside run first.
        if pinned_commits.contains(&tip) {
            continue;
        }
        // The branch's outside commits, down to where it rejoins the workspace.
        let mut commits: Vec<Commit> = Vec::new();
        let mut cursor = Some(tip);
        let mut rejoin = None;
        while let Some(id) = cursor {
            if in_set.contains(&id) {
                rejoin = Some(id);
                break;
            }
            if let Some(node) = cg.node(id) {
                commits.push(node.commit.clone());
            }
            cursor = cg.first_parent(id);
        }
        let (Some(rejoin), false) = (rejoin, commits.is_empty()) else {
            continue;
        };
        // Several stack branches can share the outside tip (e.g. an applied-branch preview where
        // `E` and `D` rest on the same not-yet-merged commit) — the run is materialized ONCE.
        if segment_by_commit(sg, tip).is_some() {
            continue;
        }
        let Some(owner_sidx) = segment_by_commit(sg, rejoin) else {
            continue;
        };
        // Named like any tip: ambiguous refs keep the segment anonymous (the walk's floating
        // `►D, ►E` run), a unique branch names it (the advanced `B` above its own lane).
        let ref_info =
            disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref).map(|ref_name| {
                RefInfo {
                    ref_name,
                    commit_id: Some(tip),
                    worktree: None,
                }
            });
        let named = ref_info.is_some();
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let seg = sg.add_node(Segment {
            id: 0,
            ref_info,
            remote_tracking_ref_name,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(seg).expect("just added").id = seg;
        connect(sg, seg, owner_sidx);
        // Only a NAMED advanced branch is the in-workspace segment's sibling (the projection shows
        // that segment under the advanced branch's name); a floating anonymous run stays unlinked,
        // and the workspace position itself never links to outside content.
        if named
            && rejoin != workspace_commit
            && let Some(owner) = sg.node_mut(owner_sidx)
            && owner.sibling_segment_id.is_none()
        {
            owner.sibling_segment_id = Some(seg);
        }
    }
}

/// Materialize the plan's [RefOrder](LanePlan::ref_order): per metadata stack list, thread the
/// same-commit groups top→bottom — the plan-decided namer takes the anchor, the plan-decided
/// empties splice above it in metadata order — producing
/// `ws → [empties] → seg(c1) → [empties] → seg(c2) → … → [empties] → base`.
/// Which refs become empties and how a group lands (dependent splice, own lane, passive) is
/// plan DATA; this pass only looks up anchors and splices.
fn insert_empty_branches(
    sg: &mut SegmentGraph,
    ws_sidx: Option<SegmentIndex>,
    plan: &LanePlan,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    // DEMOTIONS, decided by `lane_plan`: a shared base at/below the bound stays anonymous while
    // every stack's branches float above as their own lane; the lower-bound anchor of an
    // otherwise-unrepresented stack floats likewise. Remote links of a demoted name are
    // established on the floated segment by the remote creators.
    for &tip in &plan.demoted {
        let Some(anchor) = segment_by_commit(sg, tip) else {
            continue;
        };
        if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
            eprintln!("FLIP lane-plan demotion at {tip}");
        }
        if let Some(s) = sg.node_mut(anchor) {
            s.ref_info = None;
            s.remote_tracking_ref_name = None;
            s.remote_tracking_branch_segment_id = None;
        }
    }
    for (li, lane) in plan.ref_order.iter().enumerate() {
        // `from_sidx` feeds the top of the stack: the workspace segment for the first group, then each
        // group's anchor for the next (so its empties splice into the edge coming from above).
        let mut from_sidx = ws_sidx;
        for group in lane {
            // Outside the workspace or co-located with a managed merge commit: nothing to place.
            if group.placement == GroupPlacement::Skipped {
                continue;
            }
            let Some(anchor) = segment_by_commit(sg, group.commit) else {
                continue;
            };
            // GROUP NAMING, decided by `lane_plan`: the bottom-most branch names an anonymous
            // anchor; metadata order overrides a build-time name that belongs to the group (its
            // remote links are cleared, the remote creators link its floated empty instead).
            if let Some((namer, clear_remote)) = plan.group_names.get(&(li, group.commit))
                && let Some(s) = sg.node_mut(anchor)
            {
                s.ref_info = Some(RefInfo {
                    ref_name: namer.clone(),
                    commit_id: Some(group.commit),
                    worktree: None,
                });
                s.remote_tracking_ref_name = remote_tracking.get(namer).cloned();
                if *clear_remote {
                    s.remote_tracking_branch_segment_id = None;
                }
            }
            if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
                eprintln!(
                    "EMPTIES li={li} commit={} empties={:?}",
                    group.commit, group.empties
                );
            }
            if !group.empties.is_empty() {
                // ANOTHER stack owns the (non-integrated) commit: these branches stay PASSIVE
                // refs — consumers (apply) discover them on the commit and record them as
                // dependent branches in THIS stack's metadata, after which the same-list path
                // splices them.
                if group.placement == GroupPlacement::Passive {
                    from_sidx = Some(anchor);
                    continue;
                }
                let dependent = group.placement == GroupPlacement::Dependent;
                insert_empty_chain_above(
                    sg,
                    from_sidx,
                    anchor,
                    &group.empties,
                    remote_tracking,
                    group.commit,
                    dependent,
                    dependent,
                );
            }
            from_sidx = Some(anchor);
        }
    }
}

/// Does the segment name a remote-tracking branch?
fn is_remote_segment(sg: &SegmentGraph, sidx: SegmentIndex) -> bool {
    sg.node(sidx)
        .and_then(|s| s.ref_info.as_ref())
        .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
}

/// Like [`segment_by_commit`], but ignoring `exclude`d segments — the pre-lane coverage view
/// for gates that historically ran before the lane structure existed.
fn segment_by_commit_excluding(
    sg: &SegmentGraph,
    commit: gix::ObjectId,
    exclude: &HashSet<SegmentIndex>,
) -> Option<SegmentIndex> {
    sg.node_indices().find(|&sidx| {
        !exclude.contains(&sidx)
            && sg
                .node(sidx)
                .is_some_and(|s| s.commits.iter().any(|c| c.id == commit))
    })
}

/// Find the segment that holds `commit`, if any.
fn segment_by_commit(sg: &SegmentGraph, commit: gix::ObjectId) -> Option<SegmentIndex> {
    sg.node_indices().find(|&sidx| {
        sg.node(sidx)
            .is_some_and(|s| s.commits.iter().any(|c| c.id == commit))
    })
}

/// The workspace's LOWER BOUND: the nearest commit common to the target and EVERY workspace lane
/// (the walk's `compute_lowest_base` — the base all stacks and the target converge on). BFS from the
/// workspace over all parents, so the nearest such commit wins.
fn workspace_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: gix::ObjectId,
) -> Option<gix::ObjectId> {
    let mut common = cg.ancestor_set(target);
    for parent in cg.all_parent_ids(workspace_commit) {
        let lane = cg.ancestor_set(parent);
        common.retain(|c| lane.contains(c));
    }
    let mut seen = HashSet::new();
    let mut queue = std::collections::VecDeque::from([workspace_commit]);
    while let Some(c) = queue.pop_front() {
        if common.contains(&c) {
            return Some(c);
        }
        if seen.insert(c) {
            queue.extend(cg.all_parent_ids(c));
        }
    }
    None
}

/// The lower bound the PROJECTION will use: the merge base with the target, extended DOWN to a
/// stored/extra target position lying below it — an older target location keeps the commits
/// integrated since then visible, so stacks resting between the bound and the merge base are real
/// (kept) stacks, not empty floats.
fn effective_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    options: &crate::init::Options,
) -> Option<gix::ObjectId> {
    let mut lb = target
        .or(project_meta.target_commit_id)
        .or(options.extra_target_commit_id)
        .and_then(|t| workspace_lower_bound(cg, workspace_commit, t))?;
    for candidate in [
        project_meta.target_commit_id,
        options.extra_target_commit_id,
    ]
    .into_iter()
    .flatten()
    {
        if candidate != lb && cg.ancestor_set(lb).contains(&candidate) {
            lb = candidate;
        }
    }
    Some(lb)
}

/// Splice `empties` as a chain of empty segments ABOVE `anchor`, routing `from_sidx` to `anchor`
/// through them. If `from_sidx` already has edge(s) into `anchor` (including a merge's duplicate
/// parents), they are moved onto the chain top; if it has none — because a sibling empty stack already
/// consumed the shared edge to `anchor` (two empty stacks on the same base) — a fresh edge is added.
/// Other stacks' and remotes' edges into `anchor` are untouched. Produces `top_empty → … → anchor`.
#[expect(clippy::too_many_arguments)]
fn insert_empty_chain_above(
    sg: &mut SegmentGraph,
    from_sidx: Option<SegmentIndex>,
    anchor: SegmentIndex,
    empties: &[gix::refs::FullName],
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    // The commit every empty branch points at (the group's commit — empty segments still have a
    // ref TARGET, like the walk's).
    commit_id: gix::ObjectId,
    // The anchor commit sits strictly inside another stack's lane (not at/below the base): splice into
    // that chain's existing edge rather than adding a fresh workspace lane.
    dependent: bool,
    // Route EVERY incoming edge to the anchor through the chain (a splice INTO the lane, above the
    // bound): both the workspace's parent edge and the chain edge from the commit-holding segment
    // above enter at the chain top — the walk's inline-splice shape. `false` keeps other stacks'
    // direct edges (a true shared base where each stack has its own lane).
    redirect_all: bool,
) {
    let seg_ids: Vec<SegmentIndex> = empties
        .iter()
        .map(|b| {
            let s = sg.add_node(Segment {
                id: 0,
                ref_info: Some(RefInfo {
                    ref_name: b.clone(),
                    commit_id: Some(commit_id),
                    worktree: None,
                }),
                remote_tracking_ref_name: remote_tracking.get(b).cloned(),
                sibling_segment_id: None,
                remote_tracking_branch_segment_id: None,
                commits: Vec::new(),
                metadata: None,
                connections: Vec::new(),
            });
            sg.node_mut(s).expect("just added").id = s;
            s
        })
        .collect();
    let Some(&top) = seg_ids.first() else {
        return;
    };
    // Move `from_sidx`'s edge(s) into the anchor onto the chain top; other stacks and remotes that also
    // reach the anchor keep their direct edges. If it has none, the anchor may sit MID-CHAIN of another
    // stack (dependent branches, e.g. `D`/`E` pointing into `S1`'s spine): splice into the existing
    // incoming edge from the commit-holding local segment above, matching the walk — a fresh workspace
    // edge would mint a duplicate lane showing the anchor's commits twice. Only when no such chain
    // parent exists (a sibling empty stack already took the shared edge to this base) does a fresh
    // edge connect this stack from above.
    if let Some(from_sidx) = from_sidx {
        let mut redirected = false;
        let redirect_sources: Vec<SegmentIndex> = if redirect_all {
            sg.node_indices()
                .filter(|&s| !seg_ids.contains(&s) && !is_remote_segment(sg, s))
                .collect()
        } else {
            vec![from_sidx]
        };
        for source in redirect_sources {
            if let Some(from) = sg.node_mut(source) {
                for conn in &mut from.connections {
                    if conn.target == anchor {
                        conn.target = top;
                        conn.dst = None;
                        conn.dst_id = None;
                        redirected = true;
                    }
                }
            }
        }
        if !redirected {
            // Prefer a commit-holding chain parent (the dependent-branch pattern); an EMPTY one —
            // another stack's branch already spliced above the same anchor — also carries the
            // chain, so a further dependent branch slots in underneath it rather than minting a
            // fresh lane.
            let find_parent = |require_commits: bool| {
                sg.node_indices().find(|&sidx| {
                    sidx != from_sidx
                        && !is_remote_segment(sg, sidx)
                        && sg.node(sidx).is_some_and(|s| {
                            (!require_commits || !s.commits.is_empty())
                                && s.connections.iter().any(|c| c.target == anchor)
                        })
                })
            };
            let chain_parent = dependent
                .then(|| find_parent(true).or_else(|| find_parent(false)))
                .flatten();
            match chain_parent {
                Some(parent) => {
                    if let Some(parent) = sg.node_mut(parent) {
                        for conn in &mut parent.connections {
                            if conn.target == anchor {
                                conn.target = top;
                                conn.dst = None;
                                conn.dst_id = None;
                            }
                        }
                    }
                }
                None => {
                    connect(sg, from_sidx, top);
                }
            }
        }
    }
    for i in 0..seg_ids.len() {
        let next = seg_ids.get(i + 1).copied().unwrap_or(anchor);
        connect(sg, seg_ids[i], next);
    }
}

/// Connect `src` → `dst` with final endpoints: the source's last commit and the target's
/// first. Every builder edge is created after both segments hold their final commits, so
/// endpoints never need repair.
fn connect(sg: &mut SegmentGraph, src: SegmentIndex, dst: SegmentIndex) {
    let conn = Connection::new(dst, None, None, None, None).adjusted_for(src, dst, sg);
    sg.add_edge(src, conn);
}

/// All ancestors of `start` (inclusive) present in the graph, walking every parent.
fn ancestors(cg: &CommitGraph, start: gix::ObjectId) -> HashSet<gix::ObjectId> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        if cg.node(c).is_none() {
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
    // tie-break applies only to its direct parents (lane tops).
    workspace_commit: Option<gix::ObjectId>,
    target_ref: Option<&gix::refs::FullName>,
) -> Option<gix::refs::FullName> {
    let branches: Vec<gix::refs::FullName> = cg
        .refs_at(c)
        .into_iter()
        .filter(is_plain_local_branch)
        .collect();
    let unique = |pred: &dyn Fn(&gix::refs::FullName) -> bool| {
        let mut it = branches.iter().filter(|r| pred(r));
        it.next().filter(|_| it.next().is_none()).cloned()
    };
    let integrated = cg
        .node(c)
        .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::Integrated));
    (!integrated)
        .then(|| unique(&|r| segment_metadata(r.as_ref(), meta).is_some()))
        .flatten()
        .or_else(|| unique(&|r| remote_tracking.contains_key(r)))
        // Several remote-tracked branches on a LANE TOP (a direct parent of the workspace merge):
        // a unique branch WITH metadata wins even when integrated (it is the lane the user works
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

/// Is `remote_ref` on a remote the workspace configuration implies (target/push remote, or a
/// git-configured tracking branch)? Only such remotes' ahead regions are traversed.
fn remote_name_in_play(remote_ref: &gix::refs::FullName, symbolic_remotes: &[String]) -> bool {
    remote_ref
        .as_bstr()
        .strip_prefix(b"refs/remotes/".as_ref())
        .is_some_and(|rest| {
            symbolic_remotes.iter().any(|r| {
                rest.strip_prefix(r.as_bytes())
                    .is_some_and(|s| s.first() == Some(&b'/'))
            })
        })
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

/// Local branch -> its remote-tracking branch, mirroring the walk's
/// `lookup_remote_tracking_branch_or_deduce_it`, plus the SYMBOLIC remote names in play:
/// 1. A branch CONFIGURED in git (`branch.<name>.remote`/`merge`) tracks that remote branch.
/// 2. Otherwise the relationship is deduced by name (`refs/remotes/<remote>/<X>` for `refs/heads/<X>`),
///    but ONLY against remotes the workspace configuration implies — the `push_remote` (highest
///    priority: "the push-remote overrides the remote we use for listing, even if a fetch remote is
///    available"), then the remote of the configured `target_ref`. A workspace with neither deduces
///    NO name-based relationships at all.
///
/// The returned symbolic names also gate which remotes' AHEAD regions the graph traverses — a
/// config-only tracking link keeps its name, but its remote's own commits stay out of the graph,
/// matching what the walk's traversal reaches.
pub(crate) fn remote_tracking_from_repository(
    repo: &gix::Repository,
    project_meta: &but_core::ref_metadata::ProjectMeta,
) -> anyhow::Result<(
    HashMap<gix::refs::FullName, gix::refs::FullName>,
    Vec<String>,
)> {
    let mut remotes: Vec<String> = Vec::new();
    if let Some(push_remote) = project_meta.push_remote.as_deref() {
        remotes.push(push_remote.to_string());
    }
    if let Some(target_ref) = project_meta.target_ref.as_ref()
        && let Some((remote, _short)) =
            but_core::extract_remote_name_and_short_name(target_ref.as_ref(), &repo.remote_names())
        && !remotes.contains(&remote)
    {
        remotes.push(remote);
    }

    let remote_refs: Vec<gix::refs::FullName> = repo
        .references()?
        .all()?
        .filter_map(Result::ok)
        .filter(|r| r.name().as_bstr().starts_with(b"refs/remotes/"))
        .map(|r| r.name().to_owned())
        .collect();
    let mut map = HashMap::new();
    // Name-deduction against the symbolic remotes.
    for remote in &remotes {
        let prefix = format!("refs/remotes/{remote}/");
        for name in &remote_refs {
            if let Some(short) = name.as_bstr().strip_prefix(prefix.as_bytes()) {
                let local = format!("refs/heads/{}", String::from_utf8_lossy(short));
                if let Ok(local_ref) = gix::refs::FullName::try_from(local) {
                    // The first (highest-priority) remote to claim a local branch wins.
                    map.entry(local_ref).or_insert_with(|| name.clone());
                }
            }
        }
    }
    // Git-configured tracking branches win over name-deduction.
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        // The configured NAME counts even when the remote ref does not exist (yet) — the link is
        // name-only then, and passes that need the remote's commits skip unresolvable refs anyway.
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
        {
            let rt = rt.into_owned();
            // The walk also traverses the remotes of git-configured tracking branches — their remote
            // names join the eligibility set.
            let rest = &rt.as_bstr()[b"refs/remotes/".len()..];
            if let Some(slash) = rest.iter().position(|&b| b == b'/') {
                let remote = String::from_utf8_lossy(&rest[..slash]).into_owned();
                if !remotes.contains(&remote) {
                    remotes.push(remote);
                }
            }
            map.insert(local, rt);
        }
    }
    // A remote tracks ONE local: a git-CONFIGURED binding evicts a name-deduced pair for the same
    // remote (e.g. `base-of-A` configured to track `origin/A` after `A` was rebased away from it —
    // `A` no longer tracks anything).
    let mut config_bound: HashMap<gix::refs::FullName, gix::refs::FullName> = HashMap::new();
    for reference in repo.references()?.local_branches()?.filter_map(Result::ok) {
        let local = reference.name().to_owned();
        if let Some(Ok(rt)) =
            repo.branch_remote_tracking_ref_name(local.as_ref(), gix::remote::Direction::Fetch)
        {
            config_bound.insert(rt.into_owned(), local);
        }
    }
    map.retain(|local, rt| {
        config_bound
            .get(rt)
            .is_none_or(|config_local| config_local == local)
    });
    Ok((map, remotes))
}
