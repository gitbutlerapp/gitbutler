//! SPIKE (commit-graph-experiment): build the segment [`Graph`] from a [`CommitGraph`] — Route B
//! toward deleting the segment graph. Rather than reproduce the projection's simplified stacks, this
//! reconstructs the FULL segment graph (workspace / branch / anonymous / target / remote segments,
//! their first-parent connections, generations, and remote↔local sibling links) so that everything
//! downstream (projection, renderer, consumers) is unchanged.
//!
//! Verified structurally via `graph_structure` (a commit-id-keyed fingerprint) rather than by segment
//! index, since the id numbering necessarily differs from the walk's. First milestone: the clean linear
//! `single-stack` case (each commit its own segment + a co-located remote root).

#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap, HashSet};

use bstr::ByteSlice;
use but_core::RefMetadata;
use gix::reference::Category;

use crate::{
    Commit, CommitGraph, RefInfo, Segment, SegmentIndex,
    segment_graph::{Connection, SegmentGraph},
};

/// Build the managed-workspace segment [`Graph`](crate::Graph) straight from a git `CommitGraph`,
/// deriving the enrichment inputs from `(repo, meta, project_meta)` — the flip entry for
/// [`Graph::from_head`](crate::Graph::from_head). Mirrors `project_from_repository`'s derivation.
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

/// Like [`graph_from_repository`], but serving `overlay` refs and metadata from memory — the flip
/// counterpart of [`Graph::redo_traversal_with_overlay`](crate::Graph::redo_traversal_with_overlay).
pub fn graph_from_repository_with_overlay<T: but_core::RefMetadata>(
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
    // callers route any repository through here when the flip is enabled.
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
    let mut cg = CommitGraph::from_walk(
        repo,
        meta,
        walk_tip,
        walk_ref.clone(),
        project_meta.clone(),
        options.clone(),
        overlay,
    )?;
    cg.mark_managed_ws_commit_by_message(repo, ws_commit);

    // From here on every ref and metadata read goes through the overlay views, so in-memory
    // previews (apply/unapply) enrich from the future state, not the on-disk one.
    let meta = &overlay_meta;
    let ws_meta = meta.workspace(ws_ref.as_ref())?;
    // Integration marks and `NotInRemote` come from the walk's traversal — no re-flagging needed. The
    // target commit is still resolved from the CALLER's project meta for the builder's boundaries; a
    // default `ProjectMeta` means no target (no hard-coded `origin/main` fallback), like the walk.
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
    // Only IN-WORKSPACE stacks form lanes. An inactive/outside stack's branches never splice as
    // empty segments (`unapplied_branch_on_base`: "This will be an empty workspace") — they
    // contribute only branch METADATA, which names commit-holding segments via the metadata tier
    // of disambiguation. A branch listed in SEVERAL stacks counts once, like the walk, which
    // ignores duplicate stack branch tips.
    let mut seen_branches = HashSet::new();
    let stack_branches: Vec<Vec<gix::refs::FullName>> = ws_meta
        .stacks
        .iter()
        .filter(|s| s.is_in_workspace())
        .map(|s| {
            s.branches
                .iter()
                .map(|b| b.ref_name.clone())
                .filter(|b| seen_branches.insert(b.clone()))
                .collect()
        })
        .collect();
    // Remote-tracking relationships come from git CONFIG plus the caller's project meta — overlay
    // refs don't reshape them.
    let (remote_tracking, symbolic_remotes) =
        crate::commit_graph_projection::remote_tracking_from_repository(repo, &project_meta)?;
    // The main-HEAD referent is the TRAVERSAL ref (like the walk's `graph.entrypoint_ref`): an
    // overlay may override HEAD onto the workspace ref for a future-state preview, which the
    // dispatched `entrypoint_ref` (None for workspace tips) would lose.
    let worktree_by_branch =
        overlay_repo.worktree_branches(walk_ref.as_ref().map(|r| r.as_ref()))?;

    let ep = entrypoint.unwrap_or(ws_commit);
    let graph = graph_from_commit_graph(
        &cg,
        ws_commit,
        ep,
        entrypoint_ref,
        target,
        &remote_tracking,
        &symbolic_remotes,
        Some(&stack_branches),
        true,
        true,
        &worktree_by_branch,
        meta,
        project_meta,
        options,
    );
    // The entrypoint wasn't part of the managed workspace (an adhoc / outside checkout) — this builder
    // doesn't cover that yet, so signal a fall-through to the walk.
    if graph.entrypoint.is_none() {
        return Ok(None);
    }
    Ok(Some(graph))
}

/// Build a segment [`Graph`](crate::Graph) for a NON-managed checkout — a plain branch or detached
/// HEAD, with no `gitbutler/workspace` merge. `head_tip` is the checked-out commit (the graph's tip);
/// `head_symbolic` is false for a detached HEAD (forces the tip anonymous, no worktree marker).
pub fn graph_from_repository_unmanaged<T: but_core::RefMetadata>(
    repo: &gix::Repository,
    meta: &T,
    head_tip: gix::ObjectId,
    entrypoint_ref: Option<gix::refs::FullName>,
    head_symbolic: bool,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> anyhow::Result<crate::Graph> {
    let cg = CommitGraph::from_repository_unmanaged(repo, Some(head_tip))?;
    let (remote_tracking, symbolic_remotes) =
        crate::commit_graph_projection::remote_tracking_from_repository(repo, &project_meta)?;
    let worktree_by_branch = {
        let (overlay_repo, _om, _ep) = crate::init::Overlay::default().into_parts(repo, meta);
        overlay_repo.worktree_branches(entrypoint_ref.as_ref().map(|r| r.as_ref()))?
    };
    Ok(graph_from_commit_graph(
        &cg,
        head_tip,
        head_tip,
        entrypoint_ref,
        None,
        &remote_tracking,
        &symbolic_remotes,
        None,
        false,
        head_symbolic,
        &worktree_by_branch,
        meta,
        project_meta,
        options,
    ))
}

/// Build a segment [`Graph`](crate::Graph) from `cg`.
///
/// Inputs mirror the projection's enrichment: the workspace commit, the target that bounds/integrates,
/// and the local→remote tracking map. `project_meta`/`options` are carried onto the `Graph`.
#[allow(clippy::too_many_arguments)]
pub fn graph_from_commit_graph<T: but_core::RefMetadata>(
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
    // Whether HEAD points at a ref (vs detached) — controls the worktree marker and the tip's naming.
    head_symbolic: bool,
    // Which worktree (if any) checks out each ref, keyed by ref name — the main worktree `[🌳]` and any
    // linked worktrees `[📁]`. Mirrors the walk's `RefInfo::from_ref` lookup.
    worktree_by_branch: &BTreeMap<gix::refs::FullName, Vec<crate::Worktree>>,
    meta: &T,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: crate::init::Options,
) -> crate::Graph {
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

    // The walk stops traversing integrated history "once there is nothing interesting left". The
    // clearest such situation: the workspace position ITSELF is integrated (e.g. the ws ref sits
    // directly on the target's commit with no dedicated workspace merge) — then nothing below the
    // goal commits is ever walked, and empty stacks resting on the tip have no base. Clip only in
    // that situation; everywhere else the walk's reach is effectively unbounded.
    if managed
        && cg
            .node(workspace_commit)
            .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::Integrated))
    {
        let mut goals: HashSet<gix::ObjectId> = HashSet::new();
        goals.insert(workspace_commit);
        goals.extend(target);
        goals.extend(options.extra_target_commit_id);
        // Every ref-carrying commit anchors traversal — the walk keeps descending while named
        // positions (branches, tags) remain below.
        for c in cg.commit_ids() {
            if !cg.refs_at(c).is_empty() {
                goals.insert(c);
            }
        }
        for b in stack_branches.into_iter().flatten().flatten() {
            if let Some(c) = cg.commit_by_ref(b.as_ref()) {
                goals.insert(c);
            }
        }
        for (local, remote) in remote_tracking {
            if let Some(c) = cg.commit_by_ref(local.as_ref()) {
                goals.insert(c);
            }
            // The remote's rejoin point: the first in-set commit along its first-parent spine.
            let mut c = cg.commit_by_ref(remote.as_ref());
            while let Some(id) = c {
                if in_set.contains(&id) {
                    goals.insert(id);
                    break;
                }
                c = cg.first_parent(id);
            }
        }
        // Goal-descendants: goals plus everything above them (any commit with a kept parent).
        let mut above_goal: HashSet<gix::ObjectId> = goals.intersection(&in_set).copied().collect();
        let mut changed = true;
        while changed {
            changed = false;
            for &c in &in_set {
                if !above_goal.contains(&c)
                    && cg.all_parent_ids(c).iter().any(|p| above_goal.contains(p))
                {
                    above_goal.insert(c);
                    changed = true;
                }
            }
        }
        // The meeting point survives too: the first integrated commit reached from a non-integrated
        // line (the merge-base a stack rests on) is included, only history BELOW it is cut.
        let integrated = |c: gix::ObjectId| {
            cg.node(c)
                .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::Integrated))
        };
        let mut has_non_integrated_child: HashSet<gix::ObjectId> = HashSet::new();
        for &c in &in_set {
            if !integrated(c) {
                for p in cg.all_parent_ids(c) {
                    has_non_integrated_child.insert(p);
                }
            }
        }
        let mut keep: HashSet<gix::ObjectId> = in_set
            .iter()
            .copied()
            .filter(|&c| {
                !integrated(c) || above_goal.contains(&c) || has_non_integrated_child.contains(&c)
            })
            .collect();
        // Merge-parent resolution: a kept merge's parent lines are fully walked (the walk descends
        // them to their meeting point, and on to the root when that's where they converge).
        let mut stack: Vec<gix::ObjectId> = keep
            .iter()
            .filter(|&&c| cg.all_parent_ids(c).len() > 1)
            .flat_map(|&c| cg.all_parent_ids(c))
            .collect();
        let mut visited: HashSet<gix::ObjectId> = HashSet::new();
        while let Some(p) = stack.pop() {
            if in_set.contains(&p) && visited.insert(p) {
                keep.insert(p);
                stack.extend(cg.all_parent_ids(p));
            }
        }
        in_set = keep;
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
    let remote_rejoins: HashSet<gix::ObjectId> = remote_tracking
        .iter()
        .filter(|(local, _)| {
            cg.commit_by_ref(local.as_ref())
                .is_some_and(|c| in_set.contains(&c))
        })
        .filter_map(|(_, r)| cg.commit_by_ref(r.as_ref()))
        .chain(target_tip)
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
    let pinned_commits: HashSet<gix::ObjectId> = project_meta
        .target_commit_id
        .into_iter()
        .chain(options.extra_target_commit_id)
        .filter(|&c| cg.node(c).is_some())
        .collect();

    // A commit starts a new segment when it carries a disambiguated ref, is the workspace tip, is a
    // merge, or is a convergence/branch point (reached by other than a single first-parent child).
    let is_boundary = |c: gix::ObjectId| -> bool {
        c == workspace_commit
            || ws_parents.contains(&c)
            || merge_first_parents.contains(&c)
            || remote_rejoins.contains(&c)
            || metadata_commits.contains(&c)
            || pinned_commits.contains(&c)
            || disambiguated_ref(cg, c, remote_tracking, meta).is_some()
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

    // Every boundary in the set starts a segment; each segment's commit run is the boundary plus its
    // first-parent tail up to (excluding) the next boundary. These runs partition the set, so assigning
    // each commit in a run to its boundary gives the owner directly — no reverse walk (a run's oldest
    // commit, e.g. a root, has no first-parent path back up to its own boundary).
    let mut owner_of: HashMap<gix::ObjectId, gix::ObjectId> = HashMap::new();
    let mut tips: Vec<gix::ObjectId> = in_set.iter().copied().filter(|&c| is_boundary(c)).collect();
    for &tip in &tips {
        for c in commit_run(cg, tip, &in_set, &is_boundary) {
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

    let mut sg = SegmentGraph::new();
    let mut seg_of_tip: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();

    // Create a local segment per tip, holding its first-parent commit run.
    for &tip in &tips {
        let commits = commit_run(cg, tip, &in_set, &is_boundary);
        // The managed workspace tip is named by the workspace ref itself (a `gitbutler/*` ref that
        // normal disambiguation skips). A non-managed tip is named by disambiguation, unless HEAD is
        // detached — then it is forced anonymous. Every other tip: disambiguated.
        let ref_name = if tip == workspace_commit {
            if ws_is_managed_merge {
                // The real managed merge is named by EXACTLY the workspace ref (which normal
                // disambiguation skips). Other transient `gitbutler/*` refs can be co-located —
                // e.g. `gitbutler/edit` mid edit-mode — and must never name (or join) the
                // workspace. The traversal can drop the ws ref from the commit (a checkout of
                // another co-located ref makes it an empty raw segment) — the caller established
                // it points here, so fall back to the well-known name.
                cg.refs_at(tip)
                    .into_iter()
                    .find(|r| but_core::is_workspace_ref_name(r.as_ref()))
                    .or_else(|| but_core::WORKSPACE_REF_NAME.try_into().ok())
            } else if managed || head_symbolic {
                // Co-located stack tip / advanced ref (managed) or a non-managed symbolic tip: name by
                // disambiguation — a stack branch when present, else anonymous. For the managed cases the
                // empty workspace segment is spliced in above afterward.
                disambiguated_ref(cg, tip, remote_tracking, meta)
            } else {
                None
            }
        } else {
            disambiguated_ref(cg, tip, remote_tracking, meta)
        };
        let ref_info = ref_name.map(|ref_name| RefInfo {
            ref_name,
            commit_id: Some(tip),
            worktree: None,
        });
        let remote_tracking_ref_name = ref_info
            .as_ref()
            .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned());
        let sidx = sg.add_node(Segment {
            id: 0,
            generation: 0,
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

    // Connections: for each segment, its bottom commit's parents point at the segment owning each
    // parent, in first-parent order.
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
                let dst = seg_of_tip[&owner];
                let conn = Connection::new(dst, None, Some(bottom), None, Some(parent));
                sg.add_edge(src, conn);
            }
        }
    }

    // Remote segments: for each local segment with a remote-tracking ref whose remote tip is present,
    // create a remote root segment (holding the remote-ahead commits) that connects into the local
    // segment, doubly-linked via siblings.
    add_remote_segments(
        cg,
        &mut sg,
        &seg_of_tip,
        &in_set,
        &owner_of,
        symbolic_remotes,
        stack_branches,
        &pinned_commits,
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
        && segment_by_ref(&sg, tr).is_none()
        && let Some(tip) = cg.commit_by_ref(tr.as_ref())
    {
        if in_set.contains(&tip) {
            if let Some(owner_sidx) = segment_by_commit(&sg, tip)
                && sg.node(owner_sidx).is_some_and(|s| s.ref_info.is_none())
            {
                if let Some(s) = sg.node_mut(owner_sidx) {
                    s.ref_info = Some(RefInfo {
                        ref_name: tr.clone(),
                        commit_id: Some(tip),
                        worktree: None,
                    });
                }
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
        } else {
            // The target's own (remote) commits: segment its region like any remote's — split at
            // merges, connect every rejoin (including a merge's second parent) back into the
            // workspace — so the projection can find the common base. No tracking local, no links.
            segment_ahead_region(
                cg,
                &mut sg,
                tr,
                tip,
                &in_set,
                &seg_of_tip,
                &owner_of,
                None,
                &pinned_commits,
            );
        }
    }

    // A remote's ahead-run may absorb a lower remote's ref (e.g. `origin/split-segment` sitting inside
    // `origin/main`'s ahead commits): split it out into its own named segment first.
    split_remote_interior_refs(&mut sg);
    // Stacked remotes: a remote whose spine passes through another remote's tip stops there and
    // connects into it, rather than absorbing the lower remote's commits.
    split_stacked_remotes(&mut sg);

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
            generation: 0,
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
                        conn.dst_id = None;
                    }
                }
            }
        }
        sg.add_edge(
            floated,
            Connection::new(ep_sidx, None, None, None, Some(entrypoint)),
        );
    }

    // When the ws commit is not a real managed merge (co-located stack tip or advanced ref), an empty
    // workspace segment sits above it.
    let mut ws_empty_sidx = None;
    if managed {
        // A workspace-stack tip that another stack flows into (via first-parent) is a SHARED commit: it
        // is anonymized into its own segment and its ref floats up as an empty placeholder that the
        // workspace connects to (the dependent-branch pattern).
        anonymize_shared_stack_tips(cg, &mut sg, workspace_commit, target, &seg_of_tip, &in_set);
        // The empty workspace segment must exist BEFORE empty branches are spliced in, so each empty
        // stack routes from it (not from the stack segment the ws ref sits on — which would be degenerate).
        if empty_ws_case {
            ws_empty_sidx =
                insert_empty_workspace_segment(&mut sg, &seg_of_tip, cg, workspace_commit);
        }
        // A metadata stack branch that ADVANCED past the workspace points at commits outside it —
        // surface those as the branch's own segment above, sibling-linked from the in-workspace
        // segment so the projection shows that segment under the advanced branch's name.
        add_advanced_outside_branches(
            &mut sg,
            cg,
            &in_set,
            stack_branches,
            workspace_commit,
            remote_tracking,
            meta,
        );
        // Empty metadata branches (no commits) are spliced in at their place in the stack's branch order,
        // routing from the workspace segment (the empty one when present, else the ws-commit's segment).
        let ws_sidx = ws_empty_sidx.or_else(|| seg_of_tip.get(&workspace_commit).copied());
        // The workspace's lower bound — the base all lanes and the (stored/extra) target converge
        // on, extended down to an older target position. Stack branches resting there float as
        // their own empty lanes instead of naming the boundary segment, when nothing else
        // represents their stack.
        let ws_lower_bound =
            effective_lower_bound(cg, workspace_commit, target, &project_meta, &options);
        insert_empty_branches(
            &mut sg,
            cg,
            ws_sidx,
            stack_branches,
            ws_lower_bound,
            &in_set,
            workspace_commit,
            ws_is_managed_merge,
        );
        // `add_remote_segments` linked each remote to the local that named its commit's segment. When a
        // later pass (anonymize / empty-branch splicing) floats that local up into its own empty segment,
        // the remote's sibling is left pointing at the now-anonymous segment below. Re-establish the
        // walk's invariant — a remote `origin/X` is the sibling of the local segment named `X`.
        // A segment still ANONYMOUS after all naming passes takes the name the RAW WALK gave it, when
        // that name is a plain local branch not already naming another segment. The walk's naming is
        // traversal-order dependent and cannot be reproduced statically; this only ADDS names where
        // the derived passes had no opinion — it never overrides their demotions.
        for sidx in sg.node_indices().collect::<Vec<_>>() {
            let Some(first) = sg
                .node(sidx)
                .filter(|s| s.ref_info.is_none())
                .and_then(|s| s.commits.first().map(|c| c.id))
            else {
                continue;
            };
            if let Some(name) = cg
                .walk_name_of(first)
                .filter(|n| is_plain_local_branch(n))
                .cloned()
                && segment_by_ref(&sg, &name).is_none()
                && let Some(s) = sg.node_mut(sidx)
            {
                s.ref_info = Some(RefInfo {
                    ref_name: name,
                    commit_id: Some(first),
                    worktree: None,
                });
            }
        }
        // Naming passes (anchor naming, metadata-order renames, floats) don't carry remote-tracking
        // names — backfill them so any named local segment knows its remote, as segments named at
        // creation time do.
        for sidx in sg.node_indices().collect::<Vec<_>>() {
            if let Some(s) = sg.node_mut(sidx)
                && s.remote_tracking_ref_name.is_none()
                && let Some(rt) = s
                    .ref_info
                    .as_ref()
                    .filter(|ri| is_plain_local_branch(&ri.ref_name))
                    .and_then(|ri| remote_tracking.get(&ri.ref_name).cloned())
            {
                s.remote_tracking_ref_name = Some(rt);
            }
        }
        reconcile_remote_siblings(&mut sg, remote_tracking);
    }

    // A checkout inside a stack (from_commit_traversal) splits the enclosing segment so the entrypoint
    // begins its own segment — there is always a segment starting at the entrypoint.
    let entrypoint_sidx = if let (Some(ws_seg), None) = (ws_empty_sidx, entrypoint_ref.as_ref()) {
        // from_head into a co-located workspace: the entrypoint is the empty workspace segment.
        Some(ws_seg)
    } else if let Some(named) = entrypoint_ref.as_ref().and_then(|r| segment_by_ref(&sg, r)) {
        // The checked-out ref already names a segment — including an EMPTY one spliced in for a
        // virtual stack branch resting on the workspace base. That segment is the entrypoint, not
        // the segment owning the commit it points to.
        Some(named)
    } else {
        split_at_entrypoint_segment(
            &mut sg,
            cg,
            entrypoint,
            entrypoint_ref.as_ref(),
            remote_tracking,
            meta,
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

    // Normalize every connection's endpoints (src = source's last commit, dst = target's first) so the
    // graph passes `check_edge` validation — we only set the ids while building, not the indices.
    normalize_connections(&mut sg);

    // Generations: longest path from a root (a segment with no incoming connections).
    assign_generations(&mut sg);

    let entrypoint =
        entrypoint_sidx.map(|sidx| (sidx, crate::EntryPointCommit::AtCommit(entrypoint)));

    // Surface the extra target (an older target position) as an integrated traversal tip. The projection
    // derives `target_commit` from the deepest integrated tip and uses it to extend the workspace base
    // down to it — showing the commits integrated since then, exactly as the walk does.
    let mut traversal_tips = Vec::new();
    if let Some(extra) = options.extra_target_commit_id {
        traversal_tips
            .push(crate::init::Tip::new(extra).with_role(crate::init::TipRole::TargetRemote));
    }

    crate::Graph {
        inner: sg,
        entrypoint,
        entrypoint_ref,
        project_meta,
        options,
        traversal_tips,
        ..crate::Graph::default()
    }
}

/// Force a segment boundary at the `entrypoint` commit: the enclosing segment is split so the
/// entrypoint begins its own segment (unless it already starts one). Returns the entrypoint segment.
/// A checked-out `entrypoint_ref` names it; else it is disambiguated (anonymous when ambiguous).
fn split_at_entrypoint_segment<T: but_core::RefMetadata>(
    sg: &mut SegmentGraph,
    cg: &CommitGraph,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
) -> Option<SegmentIndex> {
    let (sidx, pos) = sg.node_indices().find_map(|sidx| {
        sg.node(sidx)
            .and_then(|s| s.commits.iter().position(|c| c.id == entrypoint))
            .map(|pos| (sidx, pos))
    })?;
    if pos == 0 {
        return Some(sidx);
    }
    let lower_commits = sg.node_mut(sidx).expect("present").commits.split_off(pos);
    let moved_conns = std::mem::take(&mut sg.node_mut(sidx).expect("present").connections);
    let name = entrypoint_ref
        .cloned()
        .or_else(|| disambiguated_ref(cg, entrypoint, remote_tracking, meta));
    let ref_info = name.clone().map(|ref_name| RefInfo {
        ref_name,
        commit_id: Some(entrypoint),
        worktree: None,
    });
    let remote_tracking_ref_name = name.and_then(|n| remote_tracking.get(&n).cloned());
    let new = sg.add_node(Segment {
        id: 0,
        generation: 0,
        ref_info,
        remote_tracking_ref_name,
        sibling_segment_id: None,
        remote_tracking_branch_segment_id: None,
        commits: lower_commits,
        metadata: None,
        connections: moved_conns,
    });
    sg.node_mut(new).expect("just added").id = new;
    sg.add_edge(sidx, Connection::new(new, None, None, None, None));
    Some(new)
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

/// Enforce the walk's remote↔local invariant after floats: a named remote segment `origin/X` is the
/// sibling of the local segment named `X`, and that local carries `origin/X` as its remote-tracking ref
/// + segment. Only repoints when such a distinct local segment exists, so a target ref that lives only
/// as a commit ref (no local segment of its own) keeps the owning-segment sibling set for it elsewhere.
fn reconcile_remote_siblings(
    sg: &mut SegmentGraph,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
) {
    let local_of_remote: HashMap<&gix::refs::FullName, &gix::refs::FullName> =
        remote_tracking.iter().map(|(l, r)| (r, l)).collect();
    let mut fixes: Vec<(SegmentIndex, gix::refs::FullName, SegmentIndex)> = Vec::new();
    for remote_sidx in sg.node_indices().collect::<Vec<_>>() {
        let Some(remote_ref) = sg
            .node(remote_sidx)
            .and_then(|s| s.ref_info.as_ref())
            .map(|ri| ri.ref_name.clone())
        else {
            continue;
        };
        if remote_ref.as_ref().category() != Some(Category::RemoteBranch) {
            continue;
        }
        let Some(&local_name) = local_of_remote.get(&remote_ref) else {
            continue;
        };
        let Some(local_sidx) = segment_by_ref(sg, local_name) else {
            continue;
        };
        fixes.push((remote_sidx, remote_ref, local_sidx));
    }
    for (remote_sidx, remote_ref, local_sidx) in fixes {
        if let Some(s) = sg.node_mut(remote_sidx) {
            s.sibling_segment_id = Some(local_sidx);
        }
        if let Some(s) = sg.node_mut(local_sidx) {
            s.remote_tracking_ref_name = Some(remote_ref);
            s.remote_tracking_branch_segment_id = Some(remote_sidx);
        }
    }
}

fn add_remote_segments(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    in_set: &HashSet<gix::ObjectId>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    pinned_commits: &HashSet<gix::ObjectId>,
) {
    let mut locals: Vec<(SegmentIndex, gix::refs::FullName, gix::ObjectId)> = seg_of_tip
        .iter()
        .filter_map(|(&tip, &sidx)| {
            sg.node(sidx)
                .and_then(|s| s.remote_tracking_ref_name.clone())
                .map(|rt| (sidx, rt, tip))
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
            let owner_is_anon = sg.node(owner_sidx).is_some_and(|s| s.ref_info.is_none());
            if owner_is_anon {
                if let Some(s) = sg.node_mut(owner_sidx) {
                    s.ref_info = Some(RefInfo {
                        ref_name: remote_ref.clone(),
                        commit_id: Some(remote_tip),
                        worktree: None,
                    });
                    s.sibling_segment_id = Some(local_sidx);
                }
                sg.node_mut(local_sidx)
                    .expect("present")
                    .remote_tracking_branch_segment_id = Some(owner_sidx);
            } else {
                let remote_sidx = add_empty_remote_root(sg, &remote_ref, remote_tip, local_sidx);
                sg.add_edge(
                    remote_sidx,
                    Connection::new(owner_sidx, None, None, None, Some(remote_tip)),
                );
            }
            continue;
        }

        // The remote is AHEAD: segment its ahead region like the local graph (split at merges and
        // second-parent branches), not as one flat first-parent run. Only for remotes the workspace
        // configuration implies (target/push remote, or a git-configured tracking branch) — and never
        // when the remote ref is ITSELF a workspace-metadata stack branch: then it lives in the
        // workspace as a stack, its commits are the user's own, not an upstream.
        let remote_name_in_play = remote_ref
            .as_bstr()
            .strip_prefix(b"refs/remotes/")
            .is_some_and(|rest| {
                symbolic_remotes.iter().any(|r| {
                    rest.strip_prefix(r.as_bytes())
                        .is_some_and(|s| s.first() == Some(&b'/'))
                })
            });
        let is_metadata_stack_branch = stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| *b == remote_ref);
        if !remote_name_in_play || is_metadata_stack_branch {
            continue;
        }
        segment_ahead_region(
            cg,
            sg,
            &remote_ref,
            remote_tip,
            in_set,
            seg_of_tip,
            owner_of,
            Some(local_sidx),
            pinned_commits,
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
    remote_ref: &gix::refs::FullName,
    remote_tip: gix::ObjectId,
    in_set: &HashSet<gix::ObjectId>,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    owner_of: &HashMap<gix::ObjectId, gix::ObjectId>,
    // The local segment tracking this remote, if any — a TARGET without a tracking local segment
    // builds the same region without the sibling/remote-tracking links.
    local_sidx: Option<SegmentIndex>,
    // Stored/extra target positions must start their own segment even inside a remote's ahead
    // region — the projection's `TargetCommit::from_commit` ignores one that sits mid-segment,
    // silently disabling integration checks against it.
    pinned_commits: &HashSet<gix::ObjectId>,
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
            || {
                let kids = children.get(&c).map(Vec::as_slice).unwrap_or_default();
                kids.len() > 1
                    || kids
                        .iter()
                        .any(|&k| cg.first_parent(k) != Some(c) && ahead_set.contains(&k))
            }
    };

    let tips: Vec<gix::ObjectId> = ahead_set
        .iter()
        .copied()
        .filter(|&c| is_boundary(c))
        .collect();
    let mut ahead_owner: HashMap<gix::ObjectId, gix::ObjectId> = HashMap::new();
    let mut ahead_seg: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();
    for &tip in &tips {
        let commits = commit_run(cg, tip, &ahead_set, &is_boundary);
        for c in &commits {
            ahead_owner.insert(c.id, tip);
        }
        let is_root = tip == remote_tip;
        let sidx = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info: is_root.then(|| RefInfo {
                ref_name: remote_ref.clone(),
                commit_id: Some(remote_tip),
                worktree: None,
            }),
            remote_tracking_ref_name: None,
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
    }

    for &tip in &tips {
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
                sg.add_edge(
                    src,
                    Connection::new(dst, None, Some(bottom), None, Some(parent)),
                );
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
        // `C`/`origin/C` on an ambiguous tip). A remote alone (`origin/A` with no local `A`) is just
        // where the remote is — the walk drops it. `remote_tracking` maps every remote to a local name,
        // so the discriminator is whether that local ref really exists here.
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
                generation: 0,
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
            sg.add_edge(
                remote_sidx,
                Connection::new(owner_sidx, None, None, None, Some(tip)),
            );
        }
    }
}

/// Split a remote segment at any INTERIOR commit carrying its own remote branch ref. When a stacked
/// remote (`origin/split-segment`) sits inside a tracked remote's ahead-run (`origin/main`), the lower
/// part becomes a new segment named by that ref, connected from above. Repeats down the chain.
fn split_remote_interior_refs(sg: &mut SegmentGraph) {
    let is_remote = |sg: &SegmentGraph, sidx: SegmentIndex| {
        sg.node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
    };
    let mut work: Vec<SegmentIndex> = sg.node_indices().filter(|&s| is_remote(sg, s)).collect();
    while let Some(sidx) = work.pop() {
        let commits = sg.node(sidx).map(|s| s.commits.clone()).unwrap_or_default();
        let cut = commits.iter().enumerate().skip(1).find_map(|(i, c)| {
            c.refs
                .iter()
                .find(|ri| {
                    ri.ref_name.as_ref().category() == Some(Category::RemoteBranch)
                        // A ref that already names a segment is handled by `split_stacked_remotes`
                        // (re-point into it); only create a segment for one that has none yet.
                        && segment_by_ref(sg, &ri.ref_name).is_none()
                })
                .map(|ri| (i, c.id, ri.ref_name.clone()))
        });
        let Some((i, cut_id, ref_name)) = cut else {
            continue;
        };
        let lower = sg.node_mut(sidx).expect("present").commits.split_off(i);
        let moved = std::mem::take(&mut sg.node_mut(sidx).expect("present").connections);
        let new = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info: Some(RefInfo {
                ref_name,
                commit_id: Some(cut_id),
                worktree: None,
            }),
            remote_tracking_ref_name: None,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits: lower,
            metadata: None,
            connections: moved,
        });
        sg.node_mut(new).expect("just added").id = new;
        let src_last = sg.node(sidx).and_then(|s| s.commits.last().map(|c| c.id));
        sg.add_edge(
            sidx,
            Connection::new(new, None, src_last, None, Some(cut_id)),
        );
        // The new lower segment may itself carry further interior remote refs.
        work.push(new);
    }
}

/// Truncate any remote segment whose commit run passes through ANOTHER remote segment's tip, and
/// re-point it there (stacked remotes: `origin/B` on top of `origin/A`).
fn split_stacked_remotes(sg: &mut SegmentGraph) {
    let is_remote = |sg: &SegmentGraph, sidx: SegmentIndex| {
        sg.node(sidx)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| ri.ref_name.as_ref().category() == Some(Category::RemoteBranch))
    };
    let mut remote_tip_sidx: HashMap<gix::ObjectId, SegmentIndex> = HashMap::new();
    for sidx in sg.node_indices() {
        if is_remote(sg, sidx)
            && let Some(first) = sg.node(sidx).and_then(|s| s.commits.first())
        {
            remote_tip_sidx.insert(first.id, sidx);
        }
    }
    for sidx in sg.node_indices().collect::<Vec<_>>() {
        if !is_remote(sg, sidx) {
            continue;
        }
        let commits = sg.node(sidx).map(|s| s.commits.clone()).unwrap_or_default();
        let cut = commits.iter().enumerate().skip(1).find_map(|(i, c)| {
            remote_tip_sidx
                .get(&c.id)
                .filter(|&&t| t != sidx)
                .map(|&t| (i, c.id, t))
        });
        if let Some((i, cut_id, target_sidx)) = cut {
            let s = sg.node_mut(sidx).expect("present");
            s.commits.truncate(i);
            s.connections.clear();
            let src_last = s.commits.last().map(|c| c.id);
            sg.add_edge(
                sidx,
                Connection::new(target_sidx, None, src_last, None, Some(cut_id)),
            );
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
        generation: 0,
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

/// For each workspace-stack tip that another stack flows into via first-parent, anonymize the tip
/// segment (drop its ref) and insert an empty segment carrying that ref between the workspace and the
/// now-anonymous segment. This reproduces the dependent-branch shape (empty A → anon(shared) ← B).
fn anonymize_shared_stack_tips(
    cg: &CommitGraph,
    sg: &mut SegmentGraph,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    seg_of_tip: &HashMap<gix::ObjectId, SegmentIndex>,
    in_set: &HashSet<gix::ObjectId>,
) {
    let Some(&ws_sidx) = seg_of_tip.get(&workspace_commit) else {
        return;
    };
    for parent in cg.parents(workspace_commit) {
        // The target/base lane keeps its name even when other stacks depend on it — don't anonymize it.
        if Some(parent) == target {
            continue;
        }
        let Some(&p_sidx) = seg_of_tip.get(&parent) else {
            continue;
        };
        let has_ref = sg.node(p_sidx).is_some_and(|s| s.ref_info.is_some());
        // Shared iff some other IN-WORKSPACE commit's first parent is this tip (another stack
        // depends on it). The in-set also carries the target's own local history (e.g. `main`
        // merging the stack back) — the target flowing into a tip doesn't make it shared.
        let shared = in_set.iter().any(|&c| {
            c != workspace_commit
                && cg.first_parent(c) == Some(parent)
                && cg
                    .node(c)
                    .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::InWorkspace))
        });
        if !has_ref || !shared {
            continue;
        }
        if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
            eprintln!("FLIP anonymize_shared_stack_tips floats ref off segment of {parent}");
        }
        // Float the ref onto a new empty placeholder segment. The walk floats the TIP-SEEDED
        // (traversal) name; when build-time disambiguation picked a different ref (e.g. the
        // remote-tracked `main` over the stack's `lane`), float the walk's choice and return
        // the displaced name to the commit as a passive ref.
        let mut ref_info = sg.node_mut(p_sidx).expect("present").ref_info.take();
        if let Some(walk_name) = cg
            .walk_name_of(parent)
            .filter(|n| {
                is_plain_local_branch(n) && ref_info.as_ref().is_none_or(|ri| ri.ref_name != **n)
            })
            .cloned()
            && segment_by_ref(sg, &walk_name).is_none()
        {
            let displaced = ref_info.replace(RefInfo {
                ref_name: walk_name,
                commit_id: Some(parent),
                worktree: None,
            });
            if let Some(displaced) = displaced
                && let Some(c0) = sg.node_mut(p_sidx).and_then(|s| s.commits.first_mut())
                && !c0.refs.iter().any(|r| r.ref_name == displaced.ref_name)
            {
                c0.refs.push(displaced);
                c0.refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
            }
        }
        if let Some(s) = sg.node_mut(p_sidx) {
            s.remote_tracking_ref_name = None;
            s.remote_tracking_branch_segment_id = None;
        }
        let placeholder = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info,
            remote_tracking_ref_name: None,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits: Vec::new(),
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(placeholder).expect("just added").id = placeholder;
        // Workspace now connects to the placeholder instead of directly to the shared segment.
        if let Some(ws) = sg.node_mut(ws_sidx) {
            for conn in &mut ws.connections {
                if conn.target == p_sidx {
                    conn.target = placeholder;
                    conn.dst_id = None;
                }
            }
        }
        // Placeholder → the anonymized shared segment.
        sg.add_edge(
            placeholder,
            Connection::new(p_sidx, None, None, None, Some(parent)),
        );
    }
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
        generation: 0,
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
    sg.add_edge(
        ws_seg,
        Connection::new(stack_sidx, None, None, None, Some(workspace_commit)),
    );
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
fn add_advanced_outside_branches<T: but_core::RefMetadata>(
    sg: &mut SegmentGraph,
    cg: &CommitGraph,
    in_set: &HashSet<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    workspace_commit: gix::ObjectId,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
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
        let ref_info = disambiguated_ref(cg, tip, remote_tracking, meta).map(|ref_name| RefInfo {
            ref_name,
            commit_id: Some(tip),
            worktree: None,
        });
        let named = ref_info.is_some();
        let seg = sg.add_node(Segment {
            id: 0,
            generation: 0,
            ref_info,
            remote_tracking_ref_name: None,
            sibling_segment_id: None,
            remote_tracking_branch_segment_id: None,
            commits,
            metadata: None,
            connections: Vec::new(),
        });
        sg.node_mut(seg).expect("just added").id = seg;
        sg.add_edge(
            seg,
            Connection::new(owner_sidx, None, None, None, Some(rejoin)),
        );
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

/// Splice each stack's empty metadata branches (no commits of their own) into the segment chain at
/// their metadata position. Each branch points at a commit (`cg.commit_by_ref`); consecutive branches
/// on the same commit form a group whose segment (the anchor) already exists — the metadata-pointed
/// commit was made a boundary. Any branch in a group that does not NAME the anchor is an empty segment
/// stacked above it, in list order. Groups are threaded top→bottom so the chain interleaves
/// `ws → [empties] → seg(c1) → [empties] → seg(c2) → … → [empties] → base`.
fn insert_empty_branches(
    sg: &mut SegmentGraph,
    cg: &CommitGraph,
    ws_sidx: Option<SegmentIndex>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    ws_lower_bound: Option<gix::ObjectId>,
    in_set: &HashSet<gix::ObjectId>,
    workspace_commit: gix::ObjectId,
    ws_is_managed_merge: bool,
) {
    let Some(lists) = stack_branches else {
        return;
    };
    // Commits pointed at by branches from MORE THAN ONE stack are the shared base/convergence; they
    // keep their anonymity (each stack's empty branch floats above), whereas a commit owned by a single
    // stack is named by that stack's bottom-most branch.
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
    // A commit pointed at by branches of SEVERAL metadata stacks is a shared base: its segment stays
    // anonymous and every stack's branches float above as their own lane. Build-time disambiguation
    // may have named it after one of those branches (e.g. the remote-tracked `main`) — demote that
    // name so it floats like its peers; its remote links are re-established on the floated segment
    // by `reconcile_remote_siblings`.
    for (&commit, &count) in &lists_per_commit {
        if count <= 1 {
            continue;
        }
        let Some(anchor) = segment_by_commit(sg, commit) else {
            continue;
        };
        if sg
            .node(anchor)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| lists.iter().flatten().any(|b| *b == ri.ref_name))
            && let Some(s) = sg.node_mut(anchor)
        {
            s.ref_info = None;
            s.remote_tracking_ref_name = None;
            s.remote_tracking_branch_segment_id = None;
        }
    }
    // The workspace LOWER BOUND (the base all lanes and the target converge on) is where independent
    // stacks rest: the walk floats a stack branch pointing there up as its own empty lane — preserving
    // the metadata stack entry — and keeps the boundary segment anonymous (`workspace_upgrades` →
    // `create_independent_segments`). Only OTHERWISE-UNREPRESENTED stacks float: a stack with another
    // branch alive above the bound is already visible, and its bound-resting branch stays put.
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
        && let Some(anchor) = segment_by_commit(sg, lb)
        && sg
            .node(anchor)
            .is_some_and(|s| s.commits.first().is_some_and(|c| c.id == lb))
        && sg
            .node(anchor)
            .and_then(|s| s.ref_info.as_ref())
            .is_some_and(|ri| {
                lists
                    .iter()
                    .any(|l| l.contains(&ri.ref_name) && floats_at_lower_bound(l))
            })
        && let Some(s) = sg.node_mut(anchor)
    {
        if std::env::var_os("BUT_GRAPH_FLIP_DEBUG").is_some() {
            eprintln!("FLIP lower-bound demotion at {ws_lower_bound:?}");
        }
        s.ref_info = None;
        s.remote_tracking_ref_name = None;
        s.remote_tracking_branch_segment_id = None;
    }
    for list in lists {
        // Branches whose ref does not resolve contribute nothing — and must not SPLIT a same-commit
        // group (e.g. a nonexistent branch listed between two branches on the same commit would
        // otherwise break the group in two, mis-naming the anchor and losing the empties).
        let list: Vec<gix::refs::FullName> = list
            .iter()
            .filter(|b| cg.commit_by_ref(b.as_ref()).is_some())
            .cloned()
            .collect();
        // `from_sidx` feeds the top of the stack: the workspace segment for the first group, then each
        // group's anchor for the next (so its empties splice into the edge coming from above).
        let mut from_sidx = ws_sidx;
        let mut i = 0;
        while i < list.len() {
            let commit = cg.commit_by_ref(list[i].as_ref());
            let start = i;
            while i < list.len() && cg.commit_by_ref(list[i].as_ref()) == commit {
                i += 1;
            }
            let group = &list[start..i];
            let (Some(commit), Some(anchor)) =
                (commit, commit.and_then(|c| segment_by_commit(sg, c)))
            else {
                continue;
            };
            // A branch resting OUTSIDE the workspace (e.g. above the workspace position in an
            // apply preview) is not part of any lane — the walk leaves it a passive commit ref.
            // Same for one resting ON a real managed MERGE commit (e.g. a stack tip co-located
            // with it mid edit-mode): it cannot be a lane above the workspace, and splicing it
            // there would cycle the workspace segment into its own child. A CO-LOCATED workspace
            // position (no managed commit) is different: that is exactly where empty stacks live.
            if !in_set.contains(&commit) || (commit == workspace_commit && ws_is_managed_merge) {
                continue;
            }
            // When several branches of a SINGLE stack share a commit its segment is name-ambiguous
            // (anonymous). The bottom-most branch (adjacent to the commit) NAMES that segment; the ones
            // above it are the empties. Skip if it already has a segment (a placeholder floated by
            // anonymize) or if the commit is a shared base owned by more than one stack (stays anon).
            let anchor_is_anon = sg.node(anchor).is_some_and(|s| s.ref_info.is_none());
            if anchor_is_anon
                && lists_per_commit.get(&commit).copied().unwrap_or(0) <= 1
                && !(Some(commit) == ws_lower_bound && floats_at_lower_bound(&list))
                && let Some(namer) = group.last()
                && segment_by_ref(sg, namer).is_none()
                && let Some(s) = sg.node_mut(anchor)
            {
                s.ref_info = Some(RefInfo {
                    ref_name: namer.clone(),
                    commit_id: Some(commit),
                    worktree: None,
                });
            }
            // Metadata order wins over build-time disambiguation: when the anchor is named by a
            // NON-bottom group member (e.g. the remote-tracked `advanced-lane` named the segment, but
            // metadata stacks it ABOVE `dependent`), the bottom-most branch takes over the commit's
            // segment and the previous namer floats above as an empty. Its remote links are cleared
            // here and re-established on the floated segment by `reconcile_remote_siblings`.
            if let Some(namer) = group.last()
                && sg
                    .node(anchor)
                    .and_then(|s| s.ref_info.as_ref())
                    .is_some_and(|ri| ri.ref_name != *namer && group.contains(&ri.ref_name))
                && let Some(s) = sg.node_mut(anchor)
            {
                s.ref_info = Some(RefInfo {
                    ref_name: namer.clone(),
                    commit_id: Some(commit),
                    worktree: None,
                });
                s.remote_tracking_ref_name = None;
                s.remote_tracking_branch_segment_id = None;
            }
            // Only branches without any segment yet become empties — one that already names a segment
            // (its own materialised segment, the anchor just named above, or a placeholder floated by
            // `anonymize_shared_stack_tips`) is already placed.
            let empties: Vec<gix::refs::FullName> = group
                .iter()
                .filter(|b| segment_by_ref(sg, b).is_none())
                .cloned()
                .collect();
            if !empties.is_empty() {
                // Dependent-branch splice vs own lane: a commit at/below the base (Integrated) or
                // shared by several metadata stacks gets its own lane from the workspace; a commit
                // strictly inside another stack's lane means these branches are DEPENDENT and must
                // splice into that chain instead of minting a duplicate lane.
                let dependent = lists_per_commit.get(&commit).copied().unwrap_or(0) <= 1
                    && sg.node(anchor).is_some_and(|s| {
                        s.commits
                            .first()
                            .is_some_and(|c| !c.flags.contains(crate::CommitFlags::Integrated))
                    });
                insert_empty_chain_above(sg, from_sidx, anchor, &empties, commit, dependent);
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

/// Find the segment that holds `commit`, if any.
fn segment_by_commit(sg: &SegmentGraph, commit: gix::ObjectId) -> Option<SegmentIndex> {
    sg.node_indices().find(|&sidx| {
        sg.node(sidx)
            .is_some_and(|s| s.commits.iter().any(|c| c.id == commit))
    })
}

/// All ancestors of `tip` (inclusive), over all parents.
fn ancestor_set(cg: &CommitGraph, tip: gix::ObjectId) -> HashSet<gix::ObjectId> {
    let mut set = HashSet::new();
    let mut queue = std::collections::VecDeque::from([tip]);
    while let Some(c) = queue.pop_front() {
        if set.insert(c) {
            queue.extend(cg.all_parent_ids(c));
        }
    }
    set
}

/// The workspace's LOWER BOUND: the nearest commit common to the target and EVERY workspace lane
/// (the walk's `compute_lowest_base` — the base all stacks and the target converge on). BFS from the
/// workspace over all parents, so the nearest such commit wins.
fn workspace_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: gix::ObjectId,
) -> Option<gix::ObjectId> {
    let mut common = ancestor_set(cg, target);
    for parent in cg.all_parent_ids(workspace_commit) {
        let lane = ancestor_set(cg, parent);
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
        if candidate != lb && ancestor_set(cg, lb).contains(&candidate) {
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
fn insert_empty_chain_above(
    sg: &mut SegmentGraph,
    from_sidx: Option<SegmentIndex>,
    anchor: SegmentIndex,
    empties: &[gix::refs::FullName],
    // The commit every empty branch points at (the group's commit — empty segments still have a
    // ref TARGET, like the walk's).
    commit_id: gix::ObjectId,
    // The anchor commit sits strictly inside another stack's lane (not at/below the base): splice into
    // that chain's existing edge rather than adding a fresh workspace lane.
    dependent: bool,
) {
    let seg_ids: Vec<SegmentIndex> = empties
        .iter()
        .map(|b| {
            let s = sg.add_node(Segment {
                id: 0,
                generation: 0,
                ref_info: Some(RefInfo {
                    ref_name: b.clone(),
                    commit_id: Some(commit_id),
                    worktree: None,
                }),
                remote_tracking_ref_name: None,
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
        if let Some(from) = sg.node_mut(from_sidx) {
            for conn in &mut from.connections {
                if conn.target == anchor {
                    conn.target = top;
                    conn.dst_id = None;
                    redirected = true;
                }
            }
        }
        if !redirected {
            let chain_parent = dependent
                .then(|| {
                    sg.node_indices().find(|&sidx| {
                        sidx != from_sidx
                            && !is_remote_segment(sg, sidx)
                            && sg.node(sidx).is_some_and(|s| {
                                !s.commits.is_empty()
                                    && s.connections.iter().any(|c| c.target == anchor)
                            })
                    })
                })
                .flatten();
            match chain_parent {
                Some(parent) => {
                    if let Some(parent) = sg.node_mut(parent) {
                        for conn in &mut parent.connections {
                            if conn.target == anchor {
                                conn.target = top;
                                conn.dst_id = None;
                            }
                        }
                    }
                }
                None => {
                    sg.add_edge(from_sidx, Connection::new(top, None, None, None, None));
                }
            }
        }
    }
    for i in 0..seg_ids.len() {
        let next = seg_ids.get(i + 1).copied().unwrap_or(anchor);
        sg.add_edge(seg_ids[i], Connection::new(next, None, None, None, None));
    }
}

/// Re-normalize each connection's endpoints against the final segments (src = source's last commit,
/// dst = target's first), matching what `check_edge` validates.
fn normalize_connections(sg: &mut SegmentGraph) {
    let mut updates: Vec<(SegmentIndex, usize, Connection)> = Vec::new();
    for src in sg.node_indices().collect::<Vec<_>>() {
        let conns = sg
            .node(src)
            .map(|s| s.connections.clone())
            .unwrap_or_default();
        for (i, c) in conns.into_iter().enumerate() {
            let target = c.target;
            updates.push((src, i, c.adjusted_for(src, target, sg)));
        }
    }
    for (src, i, adj) in updates {
        if let Some(s) = sg.node_mut(src) {
            s.connections[i] = adj;
        }
    }
}

/// Longest path from a root (segment with no incoming connection); roots are generation 0.
fn assign_generations(sg: &mut SegmentGraph) {
    let order = sg.toposort();
    // toposort yields sources-before-targets; connections point tip→base, so a base's generation is
    // 1 + max over its incoming sources.
    let mut depth: HashMap<SegmentIndex, usize> = HashMap::new();
    for sidx in &order {
        depth.entry(*sidx).or_insert(0);
    }
    for sidx in order {
        let g = depth[&sidx];
        let targets: Vec<SegmentIndex> = sg
            .node(sidx)
            .map(|s| s.connections.iter().map(|c| c.target).collect())
            .unwrap_or_default();
        for t in targets {
            let e = depth.entry(t).or_insert(0);
            *e = (*e).max(g + 1);
        }
    }
    for (sidx, g) in depth {
        if let Some(s) = sg.node_mut(sidx) {
            s.generation = g;
        }
    }
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
    if let Ok(Some(branch)) = meta.branch_opt(ref_name) {
        return Some(crate::SegmentMetadata::Branch((*branch).clone()));
    }
    if let Ok(Some(ws)) = meta.workspace_opt(ref_name) {
        return Some(crate::SegmentMetadata::Workspace((*ws).clone()));
    }
    None
}
