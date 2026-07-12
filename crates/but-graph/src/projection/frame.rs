//! The workspace frame derived from the arena, its stored layout and the context:
//! workspace kind, entrypoint, target, and the upper/lower bounds.

use std::collections::HashSet;

use gix::ObjectId;

use crate::workspace::{GraphContext, WorkspaceKind};
use crate::{CommitFlags, CommitGraph};

/// The commit/name-addressed workspace frame.
#[derive(Debug)]
pub(crate) struct WorkspaceFrame {
    /// What kind of workspace the entrypoint sees, `Managed`'s ref info included.
    pub kind: crate::workspace::WorkspaceKind,
    /// The workspace metadata, in managed views.
    pub metadata: Option<but_core::ref_metadata::Workspace>,
    /// The commit the view's tip rests on: the managed workspace commit, or the
    /// checked-out tip.
    pub tip_commit: Option<ObjectId>,
    /// The tip's naming — the workspace ref or the checked-out branch.
    pub tip_ref_info: Option<crate::RefInfo>,
    /// The entrypoint ref, if any.
    pub entry_ref: Option<gix::refs::FullName>,
    /// The commit the entrypoint rests on.
    pub entry_commit: Option<ObjectId>,
    /// Whether the entrypoint is a checkout INSIDE the managed workspace (a branch
    /// or commit within its reach, distinct from the tip).
    pub entry_inside: bool,
    /// The workspace's lowest base.
    pub lower_bound: Option<ObjectId>,
    /// The name of the branch resting on the bound, if any.
    pub lower_bound_ref_name: Option<gix::refs::FullName>,
    /// The target's name and resolved tip commit.
    pub target_ref: Option<FrameTarget>,
    /// The remembered (or seeded) target commit.
    pub target_commit: Option<ObjectId>,
    /// Target-remote tip commits beyond the target ref — extra target context.
    pub integrated_tip_commits: Vec<ObjectId>,
}

impl WorkspaceFrame {
    pub(crate) fn derive(cg: &CommitGraph, ctx: &GraphContext) -> Option<Self> {
        let naming = naming_commits(cg);
        let seeds = &cg.seeds;
        let entry_seed = seeds.iter().find(|s| s.is_entrypoint);
        let detached = entry_seed.is_some_and(|s| s.is_detached);
        // The seed carries the ref the entry was REQUESTED as; the arena's
        // entrypoint ref is the segment that OWNED the commit — a fallback.
        let entry_ref = (!detached)
            .then(|| {
                entry_seed
                    .and_then(|s| s.ref_name.clone())
                    .or_else(|| cg.entrypoint_ref().cloned())
            })
            .flatten();
        let entry_commit = entry_ref
            .as_ref()
            .and_then(|name| cg.layout()?.positioned_on(name.as_ref()))
            .or_else(|| cg.entrypoint())
            .or_else(|| entry_seed.map(|s| s.id));
        // The builder resolved the entrypoint through its empty chain already —
        // the verdict rides on the context (None when the chain forks or
        // dead-ends: the ref must point to its commit unambiguously).
        let entry_flags = ctx
            .entry_resolved_commit
            .or(entry_commit)
            .filter(|_| ctx.entry_resolved_commit.is_some())
            .and_then(|id| cg.node(id))
            .map(|n| n.flags)
            .unwrap_or_default();

        let seed_ws = seeds.iter().find_map(|s| match (&s.ref_name, &s.metadata) {
            (Some(name), Some(crate::SegmentMetadata::Workspace(meta))) => {
                Some(crate::workspace::WorkspaceMeta {
                    ref_name: name.clone(),
                    metadata: meta.clone(),
                })
            }
            _ => None,
        });
        let ws = ctx.workspace_meta.clone().or(seed_ws);
        let ws = ws.as_ref();
        // The workspace rests where its ref points — the layout position is a
        // PLACEMENT (an advanced ws ref gets placed as an empty over the base).
        let ws_pos = ws.and_then(|wm| {
            let name = &wm.ref_name;
            seeds
                .iter()
                .find(|s| {
                    matches!(s.role, crate::walk::SeedRole::Workspace)
                        && s.ref_name.as_ref() == Some(name)
                })
                .map(|s| s.id)
                .filter(|id| cg.node(*id).is_some())
                .or_else(|| cg.ref_target(name))
                .or_else(|| position_of(cg, name.as_ref()))
        });
        let (mut kind, mut metadata, mut tip_commit, mut tip_ref, entry_inside) =
            classify_kind(ctx, ws, ws_pos, &entry_ref, entry_commit, entry_flags);

        let Targets {
            configured_target_ref,
            configured_target_commit,
            mut target_ref,
            mut target_commit,
            integrated_tip_commits,
        } = resolve_targets(cg, ctx, &naming);

        // A non-managed view without a configured target auto-adopts the tip's remote
        // as its target — decided HERE, so the bound computation below stays a pure read.
        if !kind.has_managed_ref() && target_ref.is_none() {
            target_ref = tip_ref
                .as_ref()
                .and_then(|name| ctx.remote_tracking.get(name))
                .and_then(|remote| {
                    ctx.branch_details
                        .get(tip_ref.as_ref()?)?
                        .remote_walk_tip
                        .map(|tip| FrameTarget {
                            name: remote.clone(),
                            tip: Some(tip),
                        })
                });
        }
        let mut lower_bound = compute_lower_bound(
            cg,
            &kind,
            tip_commit,
            &target_ref,
            target_commit,
            &integrated_tip_commits,
        );

        // The entrypoint is integrated and has a workspace above it: it gets
        // downgraded back to an ad-hoc view if it turns out to be *at* or *below*
        // the merge-base, i.e. outside the workspace above.
        let entry_is_empty_pos = entry_ref
            .as_ref()
            .and_then(|name| cg.layout()?.facts_of(name.as_ref()))
            .is_some_and(|facts| !facts.names_segment || facts.names_empty_segment);
        if integrated_entry_below_bound(
            cg,
            lower_bound,
            entry_commit,
            entry_inside,
            entry_flags,
            entry_is_empty_pos,
        ) {
            kind = WorkspaceKind::AdHoc;
            metadata = None;
            tip_commit = entry_commit;
            tip_ref = entry_ref.clone();
            // entry_inside stays: the frame keeps its entrypoint through the
            // downgrade.
            target_ref = configured_target_ref;
            target_commit = configured_target_commit;
            lower_bound = None;
        }

        // A workspace ref pointing at anything but a managed commit misses its
        // workspace commit.
        if kind.has_managed_ref()
            && !tip_commit.is_some_and(|id| cg.is_managed_ws_commit(id))
            && let Some(wm) = ws
        {
            kind = WorkspaceKind::ManagedMissingWorkspaceCommit {
                ref_info: ref_info_for(ctx, &wm.ref_name, ws_pos),
            };
        }

        let tip_ref_info = tip_ref.as_ref().map(|name| {
            ref_info_for(
                ctx,
                name,
                tip_commit.or_else(|| position_of(cg, name.as_ref())),
            )
        });
        let lower_bound_ref_name = lower_bound.and_then(|lb| naming_ref_at(cg, lb));

        Some(WorkspaceFrame {
            kind,
            metadata,
            tip_commit,
            tip_ref_info,
            entry_ref,
            entry_commit,
            entry_inside,
            lower_bound,
            lower_bound_ref_name,
            target_ref,
            target_commit,
            integrated_tip_commits,
        })
    }
}

/// Kind: HEAD on the ws ref, a checkout INSIDE the workspace's reach, or ad hoc.
fn classify_kind(
    ctx: &GraphContext,
    ws: Option<&crate::workspace::WorkspaceMeta>,
    ws_pos: Option<ObjectId>,
    entry_ref: &Option<gix::refs::FullName>,
    entry_commit: Option<ObjectId>,
    entry_flags: CommitFlags,
) -> (
    WorkspaceKind,
    Option<but_core::ref_metadata::Workspace>,
    Option<ObjectId>,
    Option<gix::refs::FullName>,
    bool,
) {
    let entry_is_ws = ws.is_some_and(|wm| entry_ref.as_ref() == Some(&wm.ref_name));
    if let Some(wm) = ws.filter(|_| entry_is_ws) {
        (
            WorkspaceKind::Managed {
                ref_info: ref_info_for(ctx, &wm.ref_name, ws_pos),
            },
            Some(wm.metadata.clone()),
            ws_pos,
            Some(wm.ref_name.clone()),
            false,
        )
    } else if let Some(wm) = ws.filter(|_| entry_flags.contains(CommitFlags::InWorkspace)) {
        (
            WorkspaceKind::Managed {
                ref_info: ref_info_for(ctx, &wm.ref_name, ws_pos),
            },
            Some(wm.metadata.clone()),
            ws_pos,
            Some(wm.ref_name.clone()),
            true,
        )
    } else {
        (
            WorkspaceKind::AdHoc,
            None,
            entry_commit,
            entry_ref.clone(),
            false,
        )
    }
}

/// The target as the frame sees it: its ref name, and the tip commit when that ref
/// resolves in this graph.
#[derive(Debug, Clone)]
pub struct FrameTarget {
    pub name: gix::refs::FullName,
    pub tip: Option<ObjectId>,
}

/// The frame's target picture, configured and seeded.
struct Targets {
    configured_target_ref: Option<FrameTarget>,
    configured_target_commit: Option<ObjectId>,
    target_ref: Option<FrameTarget>,
    target_commit: Option<ObjectId>,
    integrated_tip_commits: Vec<ObjectId>,
}

/// Targets: configured, else seeded target-remote context. Only EXPLICIT
/// seeds count in full; ordinary builds contribute just the extra target.
fn resolve_targets(cg: &CommitGraph, ctx: &GraphContext, naming: &HashSet<ObjectId>) -> Targets {
    let effective_seeds: Vec<crate::walk::Seed> = if cg.explicit_seeds {
        cg.seeds.clone()
    } else {
        cg.extra_target
            .filter(|extra| cg.node(*extra).is_some())
            .map(|extra| {
                crate::walk::Seed::new(extra).with_role(crate::walk::SeedRole::TargetRemote)
            })
            .into_iter()
            .collect()
    };
    let has_ws_seed = effective_seeds
        .iter()
        .any(|s| matches!(s.metadata, Some(crate::SegmentMetadata::Workspace(_))));
    let target_remote_seeds = || effective_seeds.iter().filter(|s| s.role.is_integrated());
    let configured_target_ref = ctx.project_meta.target_ref.as_ref().and_then(|name| {
        let pos = position_of(cg, name.as_ref())?;
        Some(FrameTarget {
            name: name.clone(),
            tip: Some(pos),
        })
    });
    let target_commit_valid =
        |id: ObjectId| cg.node(id).is_some() && is_segment_start(cg, naming, id);
    let configured_target_commit = ctx
        .project_meta
        .target_commit_id
        .filter(|id| target_commit_valid(*id));
    let target_ref = configured_target_ref.clone().or_else(|| {
        if has_ws_seed {
            return None;
        }
        target_remote_seeds()
            .filter_map(|s| s.ref_name.clone())
            .find_map(|name| {
                let pos = position_of(cg, name.as_ref())?;
                Some(FrameTarget {
                    name,
                    tip: Some(pos),
                })
            })
    });
    let target_points_to = |target: &Option<FrameTarget>, id: ObjectId| {
        target.as_ref().is_some_and(|t| t.tip == Some(id))
    };
    let target_commit = configured_target_commit.or_else(|| {
        target_remote_seeds()
            .map(|s| s.id)
            .filter(|id| target_commit_valid(*id))
            .filter(|id| !target_points_to(&target_ref, *id))
            .max_by_key(|id| cg.generation_of(*id).unwrap_or_default())
    });
    let integrated_tip_commits: Vec<ObjectId> = target_remote_seeds()
        .map(|s| s.id)
        .filter(|id| target_commit_valid(*id))
        .filter(|id| !target_points_to(&target_ref, *id))
        .fold(Vec::new(), |mut acc, id| {
            if !acc.contains(&id) {
                acc.push(id);
            }
            acc
        });
    Targets {
        configured_target_ref,
        configured_target_commit,
        target_ref,
        target_commit,
        integrated_tip_commits,
    }
}

/// The lowest base: merge-base folds over the view's tips and target context.
fn compute_lower_bound(
    cg: &CommitGraph,
    kind: &WorkspaceKind,
    tip_commit: Option<ObjectId>,
    target_ref: &Option<FrameTarget>,
    target_commit: Option<ObjectId>,
    integrated_tip_commits: &[ObjectId],
) -> Option<ObjectId> {
    let base_context = |target_ref: &Option<FrameTarget>, target_commit: &Option<ObjectId>| {
        target_ref
            .iter()
            .filter_map(|t| t.tip)
            .chain(*target_commit)
            .chain(integrated_tip_commits.iter().copied())
            .collect::<Vec<_>>()
    };
    if kind.has_managed_ref() {
        // The stack tips: a managed commit's parents, or — when the workspace
        // commit is missing — the metadata chains' positions.
        let tips: Vec<ObjectId> = if tip_commit.is_some_and(|ws| cg.is_managed_ws_commit(ws)) {
            // A stack can sit outside the merge's cone entirely — its chain
            // then anchors FROM the workspace (a non-owning splice), and the
            // base must still lie below it.
            let anchors: Vec<ObjectId> = cg
                .layout()
                .map(|l| {
                    l.empty_chain_anchors
                        .iter()
                        .filter(|anchor| !anchor.joins_owning_chain)
                        .map(|anchor| anchor.commit)
                        .collect()
                })
                .unwrap_or_default();
            let mut tips = tip_commit
                .map(|ws| cg.all_parent_ids(ws))
                .unwrap_or_default();
            for anchor in anchors {
                if !tips.contains(&anchor) {
                    tips.push(anchor);
                }
            }
            tips
        } else {
            // A commitless workspace segment only gets FRESH connections for
            // spliced empty chains — chains with real segments hang off
            // history, not the workspace ref. Those splices are the tips.
            cg.layout()
                .map(|l| {
                    l.empty_chain_anchors
                        .iter()
                        .map(|anchor| anchor.commit)
                        .collect()
                })
                .unwrap_or_default()
        };
        // Chains of nothing but archived branches leave no positions; the
        // workspace ref's own anchor stands in.
        let tips = if tips.is_empty() && !tip_commit.is_some_and(|ws| cg.is_managed_ws_commit(ws)) {
            tip_commit.into_iter().collect()
        } else {
            tips
        };
        let (base, count) = best_effort_base(
            cg,
            tips.iter()
                .copied()
                .chain(base_context(target_ref, &target_commit)),
        );
        // A MANAGED merge can't rest on itself; a missing-commit workspace
        // legitimately sits right on its base.
        let tip_is_managed = tip_commit.is_some_and(|ws| cg.is_managed_ws_commit(ws));
        base.filter(|b| count >= 2 && (!tip_is_managed || Some(*b) != tip_commit))
            .or_else(|| {
                // target not available? Try the base of the workspace itself.
                (tips.len() > 1)
                    .then(|| best_effort_base(cg, tips.iter().copied()).0)
                    .flatten()
            })
    } else {
        let context = base_context(target_ref, &target_commit);
        if context.is_empty() {
            None
        } else {
            // In single-branch mode a checkout directly reachable from its
            // target keeps the base — the view is just empty then.
            best_effort_base(cg, tip_commit.into_iter().chain(context)).0
        }
    }
}

/// Whether an integrated entrypoint inside the workspace sits at or below the
/// lower bound, i.e. outside the workspace above.
fn integrated_entry_below_bound(
    cg: &CommitGraph,
    lower_bound: Option<ObjectId>,
    entry_commit: Option<ObjectId>,
    entry_inside: bool,
    entry_flags: CommitFlags,
    entry_is_empty_pos: bool,
) -> bool {
    let Some((bound, entry)) = lower_bound
        .zip(entry_commit)
        .filter(|_| entry_inside && entry_flags.contains(CommitFlags::Integrated))
    else {
        return false;
    };
    // An EMPTY entry ABOVE the bound commit is its own place — only a
    // real entry sitting ON the bound is 'at' it.
    (entry == bound && !entry_is_empty_pos) || !reaches_along_first_parent(cg, entry, bound)
}

/// The ref info for `name` resting on `commit`, worktree details included.
fn ref_info_for(
    ctx: &GraphContext,
    name: &gix::refs::FullName,
    commit: Option<ObjectId>,
) -> crate::RefInfo {
    crate::RefInfo {
        ref_name: name.clone(),
        commit_id: commit,
        worktree: ctx
            .branch_details
            .get(name)
            .and_then(|d| d.worktree.clone()),
    }
}

/// A commit STARTS a segment when a positioned ref names it, when it fans out or
/// tips (child count differs from one), or when its only child reaches it through
/// a non-first-parent edge — the walk's segmentation marks.
fn is_segment_start(cg: &CommitGraph, naming: &HashSet<ObjectId>, id: ObjectId) -> bool {
    if naming.contains(&id) || cg.is_managed_ws_commit(id) {
        return true;
    }
    // Every traversal seed starts its own segment.
    if cg.seeds.iter().any(|s| s.id == id) || cg.extra_target == Some(id) {
        return true;
    }
    let Some(idx) = cg.index_of(id) else {
        return true;
    };
    let children = cg.children_at(idx);
    let [child] = children else { return true };
    let child = cg.node_at(*child);
    if cg.all_parent_ids(child.id).first() != Some(&id) {
        return true;
    }
    // Flag transitions mark boundaries too: workspace and integration territory
    // starts a fresh segment.
    let relevant = CommitFlags::InWorkspace | CommitFlags::Integrated;
    let own = cg.node_at(idx).flags.intersection(relevant);
    own != child.flags.intersection(relevant)
}

/// The layout position of `name`, if positioned.
fn position_of(cg: &CommitGraph, name: &gix::refs::FullNameRef) -> Option<ObjectId> {
    cg.layout()?.positioned_on(name)
}

/// All commits that carry a segment-naming position.
fn naming_commits(cg: &CommitGraph) -> HashSet<ObjectId> {
    cg.layout()
        .map(|l| l.segment_naming_placements().map(|(_, on)| on).collect())
        .unwrap_or_default()
}

/// The name of a local branch naming `id`, if any — remotes only when no local does.
fn naming_ref_at(cg: &CommitGraph, id: ObjectId) -> Option<gix::refs::FullName> {
    let layout = cg.layout()?;
    let at = |pred: &dyn Fn(&gix::refs::FullName) -> bool| {
        layout
            .placements()
            .find(|(name, on)| {
                *on == id
                    && pred(name)
                    && layout
                        .facts_of(name.as_ref())
                        .is_some_and(|facts| facts.names_segment && !facts.names_empty_segment)
            })
            .map(|(name, _)| name.clone())
    };
    at(&|n| n.category() == Some(gix::reference::Category::LocalBranch)).or_else(|| at(&|_| true))
}

/// Whether `to` is reachable from `from` along FIRST parents only.
fn reaches_along_first_parent(cg: &CommitGraph, from: ObjectId, to: ObjectId) -> bool {
    let mut seen = HashSet::new();
    let mut cursor = Some(from);
    while let Some(id) = cursor.take() {
        if id == to {
            return true;
        }
        if !seen.insert(id) {
            return false;
        }
        cursor = cg.all_parent_ids(id).first().copied();
    }
    false
}

/// Fold pairwise merge-bases, best-effort: disjoint inputs keep the previous
/// candidate instead of clearing the bound.
fn best_effort_base(
    cg: &CommitGraph,
    commits: impl IntoIterator<Item = ObjectId>,
) -> (Option<ObjectId>, usize) {
    let mut count = 0;
    let base = commits
        .into_iter()
        .inspect(|_| count += 1)
        .reduce(|base, commit| cg.merge_base(base, commit).unwrap_or(base));
    (base, count)
}

/// The commits below the framed tip, in traversal order: skip the tip's own run
/// to the next segmentation boundary (entrypoint splits included), then walk,
/// bounded below by the lower bound's generation. This is workspace CONTENT, not
/// a framing fact — derived from the frame's anchors once the frame is settled.
pub(super) fn commits_below_tip(cg: &CommitGraph, frame: &WorkspaceFrame) -> Vec<ObjectId> {
    let naming = naming_commits(cg);
    let tip_ref = frame.tip_ref_info.as_ref().map(|ri| ri.ref_name.clone());
    let bound_generation = frame.lower_bound.and_then(|lb| cg.generation_of(lb));
    let entry_split = frame.entry_inside.then_some(frame.entry_commit).flatten();
    let boundary = |id: ObjectId| is_segment_start(cg, &naming, id) || Some(id) == entry_split;
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut runs = std::collections::VecDeque::new();
    // Skip the tip's own run first.
    // A tip ref that names no REAL segment holds no commits: nothing
    // skips, the territory below starts at the anchor itself.
    let tip_is_empty = tip_ref
        .as_ref()
        .and_then(|name| cg.layout()?.facts_of(name.as_ref()))
        .is_some_and(|facts| !facts.names_segment || facts.names_empty_segment);
    let mut skip_cursor = frame.tip_commit;
    if tip_is_empty {
        skip_cursor = None;
        runs.extend(frame.tip_commit);
    } else if let Some(tip) = frame.tip_commit {
        seen.insert(tip);
    }
    while let Some(id) = skip_cursor.take() {
        let parents = cg.all_parent_ids(id);
        runs.extend(parents.iter().skip(1).copied());
        match parents.first() {
            Some(p) if boundary(*p) => runs.push_back(*p),
            Some(p) => skip_cursor = Some(*p),
            None => {}
        }
    }
    // Then visit run-wise: a run prunes at its START when it begins beyond
    // the bound; commits below the bound inside a surviving run stay.
    while let Some(start) = runs.pop_front() {
        if seen.contains(&start) {
            continue;
        }
        if bound_generation
            .zip(cg.generation_of(start))
            .is_some_and(|(bound, generation)| generation < bound)
        {
            continue;
        }
        let mut cursor = Some(start);
        while let Some(id) = cursor.take() {
            if !seen.insert(id) {
                break;
            }
            if cg.node(id).is_none() {
                break;
            }
            out.push(id);
            let parents = cg.all_parent_ids(id);
            runs.extend(parents.iter().skip(1).copied());
            match parents.first() {
                Some(p) if boundary(*p) => runs.push_back(*p),
                Some(p) => cursor = Some(*p),
                None => {}
            }
        }
    }
    out
}
