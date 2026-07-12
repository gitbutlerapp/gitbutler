//! Phase 3 of gather-then-build: author the segment data from the commit-graph and the
//! chain plan; the ref-position derivation then reads the record into the stored layout.

use std::collections::{HashMap, HashSet};

use gix::reference::Category;

use super::facts::Facts;
use super::plan::ChainPlan;
use super::remotes::remote_name_in_play;
use super::{IdMap, IdSet, disambiguated_ref, is_plain_local_branch};
use crate::CommitGraph;

/// Materialize the [`SegmentData`](super::segment_data::SegmentData) — the record — from
/// `b.cg`.
///
/// `b` is the build context every phase reads.
///
/// This is "gather-then-build": everything is decided as data BEFORE any segment exists —
/// the caller authors `f`/`plan`/`layout` via `gather_and_plan`; this function adds the
/// repository/metadata-flavored decisions (the empty-workspace splice name, the advanced-outside
/// branches, the claimed remote names), then [`build`](super::segment_data::build)
/// materializes the record. The record never becomes graph storage: after any ad-hoc
/// rewrites, `derive_ref_layout` reads it into the stored ref layout and `enrich`
/// mines it for commit details — then it is dropped.
#[tracing::instrument(
    name = "graph_from_commit_graph",
    level = "trace",
    skip_all,
    fields(commits = b.cg.node_count())
)]
pub(crate) fn graph_from_commit_graph<T: but_core::RefMetadata>(
    b: &super::BuildInputs<'_>,
    meta: &T,
    f: Facts,
    plan: &ChainPlan,
    layout: &super::plan::LayoutPlan,
) -> super::segment_data::SegmentData {
    let &super::BuildInputs {
        cg,
        workspace_commit,
        entrypoint,
        entrypoint_ref,
        remote_tracking,
        symbolic_remotes,
        stack_branches,
        project_meta,
        options,
        ..
    } = b;
    let Facts {
        in_set,
        ws_is_managed_merge,
        pinned_commits,
        boundaries,
        entrypoint_forced_boundary: _,
        owner_of,
        tips,
    } = f;

    // RefChain-structure decisions: the empty-workspace splice's name and the advanced-outside
    // branches.
    let managed = stack_branches.is_some();
    let ws_empty_ref = (managed && !ws_is_managed_merge)
        .then(|| empty_workspace_ref(cg, workspace_commit))
        .flatten();
    let advanced_outside = if managed {
        let mut named: HashSet<gix::refs::FullName> = tips
            .iter()
            .filter_map(|&tip| plan.tip_name(tip).map(|nt| nt.name))
            .collect();
        named.extend(plan.floats.iter().map(|fl| fl.name.clone()));
        named.extend(ws_empty_ref.clone());
        advanced_outside_decisions(
            cg,
            &in_set,
            &owner_of,
            stack_branches,
            remote_tracking,
            meta,
            project_meta.target_ref.as_ref(),
            &pinned_commits,
            named,
        )
    } else {
        Vec::new()
    };

    let claimed_remote_names = claim_remote_names(
        cg,
        plan,
        &in_set,
        remote_tracking,
        stack_branches,
        symbolic_remotes,
    );
    // The entrypoint is a planned boundary in every region too: a checkout inside a remote's
    // ahead run starts its own segment at creation, never split out after the fact.
    let region_pinned = {
        let mut p = pinned_commits.clone();
        p.insert(entrypoint);
        p
    };
    super::segment_data::build(super::segment_data::GraphInputs {
        cg,
        tips: &tips,
        in_set: &in_set,
        boundaries: &boundaries,
        owner_of: &owner_of,
        plan,
        layout,
        ad_hoc_chain_start: layout.chains.len() - b.ad_hoc_chains.len(),
        workspace_commit,
        ws_empty_ref: ws_empty_ref.as_ref(),
        advanced_outside: &advanced_outside,
        remote_tracking,
        symbolic_remotes,
        stack_branches,
        region_pinned: &region_pinned,
        claimed_remote_names: &claimed_remote_names,
        entrypoint,
        entrypoint_ref,
        target_ref: project_meta.target_ref.as_ref(),
        extra_target: options.extra_target_commit_id,
    })
}

/// Remote refs some creator will consume as a segment name: the region builder cuts its run
/// at interior remote refs only when unclaimed. Plan-modeled names (`remote_used` covers the
/// walk seeds) plus the ahead-case remotes of EVERY boundary-tip local (`add_remote_segments`
/// regions all of them, mirroring its gates) plus explicit-tip remote names.
fn claim_remote_names(
    cg: &CommitGraph,
    plan: &ChainPlan,
    in_set: &IdSet,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    symbolic_remotes: &[String],
) -> HashSet<gix::refs::FullName> {
    let mut claimed_remote_names: HashSet<gix::refs::FullName> = plan.remote_used.clone();
    claimed_remote_names.extend(plan.base_name_of.values().filter_map(|name| {
        let rt = remote_tracking.get(name)?;
        let rt_tip = cg.commit_by_ref(rt.as_ref())?;
        (!in_set.contains(&rt_tip)
            && remote_name_in_play(rt, symbolic_remotes)
            && !super::is_stack_branch(stack_branches, rt))
        .then(|| rt.clone())
    }));
    if cg.explicit_seeds {
        claimed_remote_names.extend(cg.seeds.iter().filter_map(|t| {
            t.ref_name
                .clone()
                .filter(|r| r.as_ref().category() == Some(Category::RemoteBranch))
        }));
    }
    claimed_remote_names
}

/// The per-tip minting decision: the tip's commit run (a float's displaced build-time name
/// riding on the tip commit as a passive ref) and its planned name — `None` for floated/anonymized
/// tips, which start ANONYMOUS.
pub(super) fn tip_run_and_name(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &IdSet,
    boundaries: &IdSet,
    plan: &ChainPlan,
    visit: impl FnMut(usize),
) -> TipRun {
    let is_boundary = |c: gix::ObjectId| boundaries.contains(&c);
    let float = plan.floats.iter().find(|fl| fl.tip == tip);
    let commits = commit_run(cg, tip, in_set, &is_boundary, visit);
    let named = plan.tip_name(tip);
    let displaced = float
        .and_then(|fl| fl.displaced_ref_name.as_ref())
        .filter(|displaced| {
            commits.first().is_some_and(|&c0| {
                !cg.node_at(c0)
                    .refs
                    .iter()
                    .any(|r| r.ref_name == **displaced)
            })
        })
        .cloned();
    TipRun {
        commits,
        named,
        displaced,
    }
}

/// One tip's minting decision, in arena handles.
pub(super) struct TipRun {
    pub commits: Vec<usize>,
    pub named: Option<super::plan::NamedTip>,
    /// A float's displaced build-time name, riding on the tip commit as a passive ref.
    pub displaced: Option<gix::refs::FullName>,
}

/// The first-parent commit run owned by `tip`: `tip` and each first-parent descendant-in-history until
/// the next boundary (exclusive) or the set edge. `visit` sees each member's handle.
pub(super) fn commit_run(
    cg: &CommitGraph,
    tip: gix::ObjectId,
    in_set: &IdSet,
    is_boundary: &impl Fn(gix::ObjectId) -> bool,
    mut visit: impl FnMut(usize),
) -> Vec<usize> {
    // Handle space: one `by_id` lookup for the tip, plain vec reads after.
    let mut out = Vec::new();
    let mut cursor = cg.index_of(tip);
    while let Some(c) = cursor {
        let id = cg.id_at(c);
        if !in_set.contains(&id) {
            break;
        }
        if id != tip && is_boundary(id) {
            break;
        }
        visit(c);
        out.push(c);
        cursor = cg.first_parent_at(c);
    }
    out
}

/// The name an empty-workspace splice carries. The traversal may have
/// dropped the special workspace ref from the commit's refs when a stack branch on the same
/// commit named its raw segment — the caller established the ref points here, so fall back to
/// the well-known name rather than silently skipping the workspace segment.
pub(super) fn empty_workspace_ref(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
) -> Option<gix::refs::FullName> {
    cg.refs_at(workspace_commit)
        .into_iter()
        .find(|r| but_core::is_workspace_ref_name(r.as_ref()))
        .or_else(|| but_core::WORKSPACE_REF_NAME.try_into().ok())
}

/// A metadata stack branch pointing at a commit OUTSIDE the workspace that has advanced past it,
/// decided as data: its outside run, the in-workspace commit it rejoins,
/// and its disambiguated name.
pub(super) struct AdvancedOutside {
    pub(super) tip: gix::ObjectId,
    pub(super) name: Option<gix::refs::FullName>,
    /// The branch's outside run, in arena handles.
    pub(super) commits: Vec<usize>,
    pub(super) rejoin: gix::ObjectId,
}

/// The advanced-outside decisions, in metadata-stack order. `named` carries every ref name a
/// segment holds when the pass starts (mint names, float placeholders, the empty-workspace
/// splice) — a branch that already names a segment does not advance.
#[expect(clippy::too_many_arguments)]
pub(super) fn advanced_outside_decisions<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    in_set: &IdSet,
    owner_of: &IdMap<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    pinned_commits: &IdSet,
    mut named: HashSet<gix::refs::FullName>,
) -> Vec<AdvancedOutside> {
    let mut decisions: Vec<AdvancedOutside> = Vec::new();
    let mut outside_owned = IdSet::default();
    for b in stack_branches.into_iter().flatten().flatten() {
        // Only LOCAL branches advance past a workspace; metadata can also list remote refs as stack
        // branches, and those are handled by the remote passes.
        if !is_plain_local_branch(b) || named.contains(b) {
            continue;
        }
        let Some(tip) = cg.commit_by_ref(b.as_ref()) else {
            continue;
        };
        if in_set.contains(&tip) {
            continue;
        }
        // A PINNED commit (a stored/extra target position) must start its own segment via the
        // extra-target region — the projection derives the remembered base from it. When chains
        // run before the remote passes this pass would otherwise swallow it into the branch's
        // outside run first.
        if pinned_commits.contains(&tip) {
            continue;
        }
        // The branch's outside commits, down to where it rejoins the workspace.
        let mut commits: Vec<usize> = Vec::new();
        let mut cursor = Some(tip);
        let mut rejoin = None;
        while let Some(id) = cursor {
            if in_set.contains(&id) {
                rejoin = Some(id);
                break;
            }
            if let Some(h) = cg.index_of(id) {
                commits.push(h);
            }
            cursor = cg.first_parent(id);
        }
        let (Some(rejoin), false) = (rejoin, commits.is_empty()) else {
            continue;
        };
        // Several stack branches can share the outside tip (e.g. an applied-branch preview where
        // `E` and `D` rest on the same not-yet-merged commit) — the run is materialized ONCE.
        if outside_owned.contains(&tip) || !owner_of.contains_key(&rejoin) {
            continue;
        }
        // Named like any tip: ambiguous refs keep the segment anonymous (the walk's floating
        // `►D, ►E` run), a unique branch names it (the advanced `B` above its own chain).
        let name = disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref);
        outside_owned.extend(commits.iter().map(|&h| cg.id_at(h)));
        named.extend(name.clone());
        decisions.push(AdvancedOutside {
            tip,
            name,
            commits,
            rejoin,
        });
    }
    decisions
}
