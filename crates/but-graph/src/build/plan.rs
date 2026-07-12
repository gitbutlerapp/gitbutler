//! Phase 2 of gather-then-build: the chain plan — names, floats, demotions, and
//! ref order decided purely, before any segment exists.

use std::collections::{HashMap, HashSet};

use gix::reference::Category;

use super::facts::{Facts, facts};
use super::remotes::remote_name_in_play;
use super::{IdMap, IdSet, disambiguated_ref, is_plain_local_branch};
use crate::CommitGraph;

/// The authored ref PLACEMENT table: which references sit on which commit, grouped and
/// ordered, one chain per metadata stack list. Build-internal — the chain-structure pass
/// consumes it directly, and the stored [`RefLayout`](crate::ref_layout::RefLayout) is
/// derived from the segments it shapes.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub(super) struct LayoutPlan {
    /// The groups anchored on each commit, in chain-threading order.
    pub(super) at_commit: HashMap<gix::ObjectId, Vec<LayoutGroup>>,
    /// One chain per metadata stack list, in metadata order.
    pub(super) chains: Vec<RefChain>,
    /// Commits kept anonymous (sorted): a shared base loses its build-time name so every
    /// chain's branches float above it as their own chain.
    pub(super) anonymous_bases: Vec<gix::ObjectId>,
}

impl LayoutPlan {
    /// Where the empty chains rest — the stored layout's
    /// [`empty_chain_anchors`](crate::ref_layout::RefLayout::empty_chain_anchors): each
    /// chain's first non-[`Skipped`](GroupPlacement::Skipped) anchor, when its placement
    /// is a splice (a chain with commits of its own is placed as a run, not an anchor).
    pub(super) fn empty_chain_anchors(&self) -> Vec<crate::ref_layout::EmptyChainAnchor> {
        self.chains
            .iter()
            .filter_map(|chain| {
                let (commit, gi) = chain.anchors.iter().find(|(commit, gi)| {
                    self.at_commit
                        .get(commit)
                        .and_then(|groups| groups.get(*gi))
                        .is_some_and(|g| g.placement != GroupPlacement::Skipped)
                })?;
                match self.at_commit[commit][*gi].placement {
                    GroupPlacement::Splice {
                        into_owning_chain, ..
                    } => Some(crate::ref_layout::EmptyChainAnchor {
                        commit: *commit,
                        joins_owning_chain: into_owning_chain,
                    }),
                    _ => None,
                }
            })
            .collect()
    }
}

/// A tip's planned name: the ref that names the tip's segment, and the commit it names.
#[derive(Clone)]
pub(super) struct NamedTip {
    pub(super) name: gix::refs::FullName,
    pub(super) tip: gix::ObjectId,
}

/// One metadata stack list's groups, in metadata order (top → bottom). Each anchor is
/// `(commit, index into LayoutPlan::at_commit[commit])` — the index keeps chains
/// apart when several chains anchor groups on the same commit.
#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub(super) struct RefChain {
    /// The chain's anchors in metadata order.
    pub(super) anchors: Vec<(gix::ObjectId, usize)>,
}

/// One same-commit group of references anchored on a commit.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct LayoutGroup {
    /// The member naming the anchor commit's segment, when the group names it at all.
    pub(super) naming_ref: Option<NamingRef>,
    /// How the group's members land on the graph.
    pub(super) placement: GroupPlacement,
}

/// The group member that NAMES the anchor commit's segment.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct NamingRef {
    /// The naming reference.
    pub(super) name: gix::refs::FullName,
    /// The metadata-order override: this ref displaced a build-time name belonging to the
    /// group, whose remote link moves to its floated empty segment instead.
    pub(super) clear_remote: bool,
}

/// How a group's members land relative to the commit the group anchors on.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) enum GroupPlacement {
    /// The group is outside the workspace or co-located with a managed merge commit — nothing
    /// is created, naming ref included. Kept so group ordinals stay aligned between plan and
    /// build.
    Skipped,
    /// Another chain owns the (non-integrated) commit: the members stay passive on it.
    Passive(Vec<gix::refs::FullName>),
    /// The non-naming members become empty segments spliced above the anchor.
    Splice {
        /// The spliced members, in metadata order.
        members: Vec<gix::refs::FullName>,
        /// The anchor commit is inside another chain, so the empties splice into it; otherwise
        /// the group anchors its own chain from the workspace (shared base or integrated
        /// anchor).
        into_owning_chain: bool,
    },
}

/// Phases 1+2 for one build: the facts, the chain plan decided over them, and the authored
/// ref placement table — the artifacts materialization consumes.
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn gather_and_plan<T: but_core::RefMetadata>(
    b: &super::BuildInputs<'_>,
    meta: &T,
) -> (Facts, ChainPlan, LayoutPlan) {
    let f = facts(b, meta);
    // The base all chains and the (stored/extra) target converge on, extended down to an
    // older target position.
    let ws_lower_bound = effective_lower_bound(
        b.cg,
        b.workspace_commit,
        b.target,
        b.project_meta,
        b.options,
    );
    let (plan, layout) = chain_plan(b, &f, ws_lower_bound, meta);
    (f, plan, layout)
}

/// One floated chain placeholder decided by [`chain_plan`].
pub(super) struct Float {
    /// The commit whose segment goes anonymous so the empty named segment can float above it.
    pub(super) tip: gix::ObjectId,
    /// The name given to the empty segment spliced in between the workspace and `tip`.
    pub(super) name: gix::refs::FullName,
    /// A build-time name pushed aside by a metadata stack branch; it returns to `tip`'s
    /// commit as a passive ref. `None` when nothing was displaced.
    pub(super) displaced_ref_name: Option<gix::refs::FullName>,
}

pub(super) struct ChainPlan {
    pub(super) floats: Vec<Float>,
    pub(super) anonymous_bases: IdSet,
    /// Every boundary tip's MATERIALIZATION name, before floats/demotions suppress it. The
    /// remote/target passes key their decisions on these — they evaluate the pre-chain view.
    pub(super) base_name_of: IdMap<gix::refs::FullName>,
    /// Names the remote/target/explicit-tip passes give to ANONYMOUS boundary tips, modeled
    /// in pass order so materialization can mint segments with their FINAL names; the passes
    /// only add links. The value carries the named ref's actual position (a behind remote can
    /// point mid-run, below the owner's tip).
    pub(super) renames: IdMap<(gix::refs::FullName, gix::ObjectId)>,
    /// Every remote-ref name the remote passes will consume (renames, empty roots, ahead
    /// regions, untracked surfacing, the target). With the chain structure built FIRST, the
    /// empties filter consults this instead of finding the remote segments in the graph.
    pub(super) remote_used: HashSet<gix::refs::FullName>,
}

impl ChainPlan {
    /// The name `tip`'s segment mints with, and the commit that name resolves to: floated and
    /// anonymized tips stay anonymous, otherwise the build-time name or the planned rename.
    pub(super) fn tip_name(&self, tip: gix::ObjectId) -> Option<NamedTip> {
        if self.floats.iter().any(|fl| fl.tip == tip) || self.anonymous_bases.contains(&tip) {
            return None;
        }
        self.base_name_of
            .get(&tip)
            .map(|n| NamedTip {
                name: n.clone(),
                tip,
            })
            .or_else(|| {
                let (name, tip) = self.renames.get(&tip).cloned()?;
                Some(NamedTip { name, tip })
            })
    }
}

/// The managed chain NAME decisions, computed before any segment mutation (phase 2 of
/// gather-then-build). Models the naming state the passes would see and decides purely: which
/// shared workspace-parent tips float their name up as an empty placeholder
/// (`float_shared_stack_tips`), which anchors are DEMOTED to anonymous so their stacks'
/// branches form their own chains (`anonymize_shared_bases`), and the group naming and ref
/// order `insert_empty_branches` consumes as data (`thread_ref_groups`).
#[tracing::instrument(level = "trace", skip_all)]
pub(super) fn chain_plan<T: but_core::RefMetadata>(
    b: &super::BuildInputs<'_>,
    facts: &Facts,
    ws_lower_bound: Option<gix::ObjectId>,
    meta: &T,
) -> (ChainPlan, LayoutPlan) {
    let &super::BuildInputs {
        cg,
        workspace_commit,
        entrypoint,
        entrypoint_ref,
        target,
        remote_tracking,
        symbolic_remotes,
        stack_branches,
        project_meta,
        options,
        ..
    } = b;
    let target_ref = project_meta.target_ref.as_ref();
    let extra_target = options.extra_target_commit_id;
    let mut plan = ChainPlan {
        floats: Vec::new(),
        anonymous_bases: IdSet::default(),
        base_name_of: IdMap::default(),
        renames: IdMap::default(),
        remote_used: HashSet::new(),
    };
    let mut layout = LayoutPlan::default();
    // Materialization names first…
    let mut name_of = materialization_names(
        cg,
        facts,
        workspace_commit,
        entrypoint,
        entrypoint_ref,
        remote_tracking,
        meta,
        target_ref,
    );
    plan.base_name_of = name_of.clone();
    // …then the remote, untracked-remote, target, and explicit-tip renames, in pass order.
    let mut remote_used = model_remote_renames(
        cg,
        facts,
        &mut name_of,
        &mut plan.renames,
        remote_tracking,
        stack_branches,
        symbolic_remotes,
    );
    model_untracked_remotes(cg, facts, &name_of, remote_tracking, &mut remote_used);
    model_target_rename(
        cg,
        facts,
        &mut name_of,
        &mut plan.renames,
        target_ref,
        &mut remote_used,
    );
    model_explicit_tip_renames(cg, facts, &mut name_of, &mut plan.renames);

    if stack_branches.is_none() && b.ad_hoc_chains.is_empty() {
        plan.remote_used = remote_used;
        layout.anonymous_bases = plan.anonymous_bases.iter().copied().collect();
        layout.anonymous_bases.sort();
        return (plan, layout);
    }

    float_shared_stack_tips(
        cg,
        facts,
        workspace_commit,
        target,
        stack_branches,
        &mut name_of,
        &mut plan.floats,
    );

    let lists = stack_branches.unwrap_or(&[]);
    let combined: Vec<Vec<gix::refs::FullName>> = lists
        .iter()
        .chain(b.ad_hoc_chains.iter())
        .cloned()
        .collect();
    let lists_per_commit = stack_lists_per_commit(cg, &combined);
    let at_or_below_bound: Option<IdSet> = ws_lower_bound.map(|lb| cg.ancestor_set(lb));
    anonymize_shared_bases(
        cg,
        facts,
        lists,
        &lists_per_commit,
        at_or_below_bound.as_ref(),
        ws_lower_bound,
        &mut name_of,
        &mut plan.anonymous_bases,
    );
    let mut used = names_in_use(
        cg,
        facts,
        &name_of,
        &plan.floats,
        &remote_used,
        lists,
        remote_tracking,
        meta,
        target_ref,
        extra_target,
    );
    thread_ref_groups(
        cg,
        facts,
        workspace_commit,
        ws_lower_bound,
        lists,
        b.ad_hoc_chains,
        &lists_per_commit,
        at_or_below_bound.as_ref(),
        &mut name_of,
        &mut used,
        &remote_used,
        &mut layout,
    );
    plan.remote_used = remote_used;
    layout.anonymous_bases = plan.anonymous_bases.iter().copied().collect();
    layout.anonymous_bases.sort();
    (plan, layout)
}

/// Every boundary tip's materialization name — the naming state the chain passes start from.
#[allow(clippy::too_many_arguments)]
fn materialization_names<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
) -> IdMap<gix::refs::FullName> {
    let mut name_of: IdMap<gix::refs::FullName> = IdMap::default();
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
    name_of
}

/// The anon-owner renames of `add_remote_segments` (a remote pointing BEHIND/at an anonymous
/// in-set segment names it), in materialization order like the pass. Every remote name the
/// pass consumes is tracked, because the target block only runs when nothing already used the
/// target ref.
fn model_remote_renames(
    cg: &CommitGraph,
    facts: &Facts,
    name_of: &mut IdMap<gix::refs::FullName>,
    renames: &mut IdMap<(gix::refs::FullName, gix::ObjectId)>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    symbolic_remotes: &[String],
) -> HashSet<gix::refs::FullName> {
    let in_play = |rt: &gix::refs::FullName| remote_name_in_play(rt, symbolic_remotes);
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
            if let gix::hashtable::hash_map::Entry::Vacant(e) = name_of.entry(owner) {
                e.insert(remote_ref.clone());
                renames.insert(owner, (remote_ref.clone(), remote_tip));
            }
            remote_used.insert(remote_ref.clone());
        } else if in_play(remote_ref) && !super::is_stack_branch(stack_branches, remote_ref) {
            remote_used.insert(remote_ref.clone());
        }
    }
    remote_used
}

/// The untracked-remote pass surfacing remotes whose local counterpart shares the commit.
fn model_untracked_remotes(
    cg: &CommitGraph,
    facts: &Facts,
    name_of: &IdMap<gix::refs::FullName>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    remote_used: &mut HashSet<gix::refs::FullName>,
) {
    let named: HashSet<&gix::refs::FullName> = name_of.values().collect();
    for r in super::remote_refs(cg) {
        if remote_used.contains(&r) || named.contains(&r) {
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

/// The target pass naming an anonymous in-set owner after the target ref — only when nothing
/// already used it.
fn model_target_rename(
    cg: &CommitGraph,
    facts: &Facts,
    name_of: &mut IdMap<gix::refs::FullName>,
    renames: &mut IdMap<(gix::refs::FullName, gix::ObjectId)>,
    target_ref: Option<&gix::refs::FullName>,
    remote_used: &mut HashSet<gix::refs::FullName>,
) {
    if let Some(tr) = target_ref
        && tr.as_ref().category() == Some(Category::RemoteBranch)
        && !remote_used.contains(tr)
        && !name_of.values().any(|n| n == tr)
        && let Some(tip) = cg.commit_by_ref(tr.as_ref())
    {
        if facts.in_set.contains(&tip) {
            let owner = facts.owner_of.get(&tip).copied().unwrap_or(tip);
            if let gix::hashtable::hash_map::Entry::Vacant(e) = name_of.entry(owner) {
                e.insert(tr.clone());
                renames.insert(owner, (tr.clone(), tip));
            }
        }
        remote_used.insert(tr.clone());
    }
}

/// The explicit-tip pass naming anonymous segments that START at a tip.
fn model_explicit_tip_renames(
    cg: &CommitGraph,
    facts: &Facts,
    name_of: &mut IdMap<gix::refs::FullName>,
    renames: &mut IdMap<(gix::refs::FullName, gix::ObjectId)>,
) {
    for t in cg.seeds.iter().filter(|_| cg.explicit_seeds) {
        let Some(ref_name) = t.ref_name.clone() else {
            continue;
        };
        if but_core::is_workspace_ref_name(ref_name.as_ref())
            || name_of.values().any(|n| *n == ref_name)
        {
            continue;
        }
        if facts.boundaries.contains(&t.id)
            && let gix::hashtable::hash_map::Entry::Vacant(e) = name_of.entry(t.id)
        {
            e.insert(ref_name.clone());
            renames.insert(t.id, (ref_name, t.id));
        }
    }
}

/// A workspace-parent tip whose commit another in-workspace commit builds on goes ANONYMOUS,
/// its name floating above as an empty chain placeholder — the unique metadata STACK branch
/// when build-time disambiguation picked a non-stack ref (which then returns to the commit as
/// a passive ref).
fn float_shared_stack_tips(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    name_of: &mut IdMap<gix::refs::FullName>,
    floats: &mut Vec<Float>,
) {
    if !facts.ws_is_managed_merge {
        return;
    }
    for parent in cg.parents(workspace_commit) {
        // The target/base chain keeps its name even when other stacks depend on it.
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
                    .is_some_and(|n| n.flags.contains(crate::CommitFlags::InWorkspace))
        });
        if !shared {
            continue;
        }
        // Float the unique metadata STACK branch over a build-time non-stack pick: an
        // applied-but-empty stack must keep its own chain, or the projection's
        // integration-prune swallows the whole stack with the shared base it would own.
        let (float_name, displaced) = if super::is_stack_branch(stack_branches, &current) {
            (current.clone(), None)
        } else {
            let mut stack_refs = cg
                .refs_at(parent)
                .into_iter()
                .filter(|r| is_plain_local_branch(r) && super::is_stack_branch(stack_branches, r));
            match (stack_refs.next(), stack_refs.next()) {
                (Some(stack_ref), None)
                    if !name_of.values().any(|n| *n == stack_ref)
                        && !floats.iter().any(|f| f.name == stack_ref) =>
                {
                    (stack_ref, Some(current.clone()))
                }
                _ => (current.clone(), None),
            }
        };
        name_of.remove(&parent);
        floats.push(Float {
            tip: parent,
            name: float_name,
            displaced_ref_name: displaced,
        });
    }
}

/// How many metadata stack lists point (via any of their branches) at each commit.
fn stack_lists_per_commit(cg: &CommitGraph, lists: &[Vec<gix::refs::FullName>]) -> IdMap<usize> {
    let mut lists_per_commit: IdMap<usize> = IdMap::default();
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
    lists_per_commit
}

/// `insert_empty_branches`' demotions. A commit pointed at by branches of SEVERAL metadata
/// stacks at/below the bound is a shared base: its segment stays anonymous and every stack's
/// branches float above as their own chain. Likewise at the workspace LOWER BOUND, where
/// independent stacks rest: an otherwise-unrepresented stack's branch pointing there floats
/// as its own empty chain.
#[allow(clippy::too_many_arguments)]
fn anonymize_shared_bases(
    cg: &CommitGraph,
    facts: &Facts,
    lists: &[Vec<gix::refs::FullName>],
    lists_per_commit: &IdMap<usize>,
    at_or_below_bound: Option<&IdSet>,
    ws_lower_bound: Option<gix::ObjectId>,
    name_of: &mut IdMap<gix::refs::FullName>,
    anonymous_bases: &mut IdSet,
) {
    for (&commit, &count) in lists_per_commit {
        if count <= 1 {
            continue;
        }
        if let Some(below) = at_or_below_bound
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
            anonymous_bases.insert(anchor);
        }
    }
    if let Some(lb) = ws_lower_bound
        && facts.boundaries.contains(&lb)
        && name_of.get(&lb).is_some_and(|n| {
            lists
                .iter()
                .any(|l| l.contains(n) && floats_at_lower_bound(cg, ws_lower_bound, l))
        })
    {
        name_of.remove(&lb);
        anonymous_bases.insert(lb);
    }
}

/// Does this metadata stack list rest at the workspace lower bound — some branch at the bound
/// itself and every other resolved branch integrated?
fn floats_at_lower_bound(
    cg: &CommitGraph,
    ws_lower_bound: Option<gix::ObjectId>,
    list: &[gix::refs::FullName],
) -> bool {
    let Some(lb) = ws_lower_bound else {
        return false;
    };
    let mut at_lb = false;
    for b in list {
        match cg.commit_by_ref(b.as_ref()) {
            Some(c) if c == lb => at_lb = true,
            Some(c)
                if cg
                    .node(c)
                    .is_some_and(|n| !n.flags.contains(crate::CommitFlags::Integrated)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    at_lb
}

/// The full set of names in use by `insert_empty_branches` time — chain names plus everything
/// the remote/target/tip/advanced passes will have created — because the group naming's "does
/// this ref already name a segment" ranges over every segment.
#[allow(clippy::too_many_arguments)]
fn names_in_use<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    facts: &Facts,
    name_of: &IdMap<gix::refs::FullName>,
    floats: &[Float],
    remote_used: &HashSet<gix::refs::FullName>,
    lists: &[Vec<gix::refs::FullName>],
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    meta: &T,
    target_ref: Option<&gix::refs::FullName>,
    extra_target: Option<gix::ObjectId>,
) -> HashSet<gix::refs::FullName> {
    let mut used: HashSet<gix::refs::FullName> = name_of.values().cloned().collect();
    used.extend(floats.iter().map(|fl| fl.name.clone()));
    used.extend(remote_used.iter().cloned());
    // The target ref always ends up naming something when it resolves.
    if let Some(tr) = target_ref
        && tr.as_ref().category() == Some(Category::RemoteBranch)
        && cg.commit_by_ref(tr.as_ref()).is_some()
    {
        used.insert(tr.clone());
    }
    // Explicit traversal seeds name a segment (an anon owner, an empty splice, or a region tip).
    for t in cg.seeds.iter().filter(|_| cg.explicit_seeds) {
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
        && let Some(l) = super::remotes::unique_plain_local(cg, extra)
    {
        used.insert(l);
    }
    // Advanced-outside branches (`add_advanced_outside_branches`), deduped by outside tip.
    let mut adv_seen: IdSet = IdSet::default();
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
        // The tip is outside (guarded above); it counts only when its spine rejoins the set.
        if cg
            .first_on_spine(tip, |c| facts.in_set.contains(&cg.id_at(c)))
            .is_none()
        {
            continue;
        }
        if let Some(name) = disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref) {
            used.insert(name);
        }
    }
    used
}

/// The group threading, mirroring `insert_empty_branches` exactly: per stack list, groups of
/// consecutive branches on one commit; the bottom-most member names an anonymous anchor, and
/// metadata order overrides a build-time name that belongs to the group. Authors the stored
/// layout's chains and commit-keyed groups directly.
#[allow(clippy::too_many_arguments)]
fn thread_ref_groups(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    ws_lower_bound: Option<gix::ObjectId>,
    lists: &[Vec<gix::refs::FullName>],
    ad_hoc_lists: &[Vec<gix::refs::FullName>],
    lists_per_commit: &IdMap<usize>,
    at_or_below_bound: Option<&IdSet>,
    name_of: &mut IdMap<gix::refs::FullName>,
    used: &mut HashSet<gix::refs::FullName>,
    remote_used: &HashSet<gix::refs::FullName>,
    layout: &mut LayoutPlan,
) {
    let push_group = |layout: &mut LayoutPlan,
                      stored: &mut RefChain,
                      commit: gix::ObjectId,
                      naming_ref: Option<NamingRef>,
                      placement: GroupPlacement| {
        let groups = layout.at_commit.entry(commit).or_default();
        groups.push(LayoutGroup {
            naming_ref,
            placement,
        });
        stored.anchors.push((commit, groups.len() - 1));
    };
    let all_lists = lists
        .iter()
        .map(|l| (l, false))
        .chain(ad_hoc_lists.iter().map(|l| (l, true)));
    for (list, is_ad_hoc) in all_lists {
        let list: Vec<gix::refs::FullName> = list
            .iter()
            .filter(|b| cg.commit_by_ref(b.as_ref()).is_some())
            .cloned()
            .collect();
        let mut stored = RefChain::default();
        let mut i = 0;
        while i < list.len() {
            let commit = cg.commit_by_ref(list[i].as_ref());
            let start = i;
            while i < list.len() && cg.commit_by_ref(list[i].as_ref()) == commit {
                i += 1;
            }
            let group = &list[start..i];
            let Some(commit) = commit else { continue };
            // Ad-hoc entry chains live in the ENTRY region, which may sit outside the
            // workspace in-set — their placement domain is wherever the walk put them.
            if (!is_ad_hoc && !facts.in_set.contains(&commit))
                || (commit == workspace_commit && facts.ws_is_managed_merge)
            {
                push_group(layout, &mut stored, commit, None, GroupPlacement::Skipped);
                continue;
            }
            let anchor = facts.owner_of.get(&commit).copied().unwrap_or(commit);
            let mut naming = None;
            let shared_commit_above_bound = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && at_or_below_bound.is_some_and(|below| !below.contains(&commit));
            if !name_of.contains_key(&anchor)
                && (lists_per_commit.get(&commit).copied().unwrap_or(0) <= 1
                    || shared_commit_above_bound)
                && !(Some(commit) == ws_lower_bound
                    && floats_at_lower_bound(cg, ws_lower_bound, &list))
                && let Some(namer) = group.last()
                && !used.contains(namer)
            {
                name_of.insert(anchor, namer.clone());
                used.insert(namer.clone());
                naming = Some(NamingRef {
                    name: namer.clone(),
                    clear_remote: false,
                });
            }
            if let Some(namer) = group.last()
                && name_of
                    .get(&anchor)
                    .is_some_and(|n| n != namer && group.contains(n))
            {
                // The override DISPLACES the anchor's build-time name: it re-enters the pool
                // and splices as an empty group member.
                if let Some(displaced) = name_of.get(&anchor) {
                    used.remove(displaced);
                }
                name_of.insert(anchor, namer.clone());
                used.insert(namer.clone());
                naming = Some(NamingRef {
                    name: namer.clone(),
                    clear_remote: true,
                });
            }
            // Every group member ends placed (naming the anchor or spliced as an empty) when the
            // empties path runs; the cross-stack-owned skip leaves them passive instead.
            let cross_stack_owned = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && name_of
                    .get(&anchor)
                    .is_some_and(|n| !list.contains(n) && lists.iter().any(|l| l.contains(n)));
            let anchor_not_integrated = cg
                .node(anchor)
                .is_some_and(|n| !n.flags.contains(crate::CommitFlags::Integrated));
            // The RefOrder: which members become empties, and how the group lands. `used` at
            // THIS point models materialization's "already names a segment" gate (the group
            // naming ref included — it names the anchor, not an empty).
            let shared_base = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && at_or_below_bound.is_none_or(|below| below.contains(&commit));
            let members: Vec<gix::refs::FullName> = group
                .iter()
                .filter(|b| !used.contains(*b) && !remote_used.contains(*b))
                .cloned()
                .collect();
            let placement = if cross_stack_owned && anchor_not_integrated {
                GroupPlacement::Passive(members)
            } else {
                used.extend(group.iter().cloned());
                GroupPlacement::Splice {
                    members,
                    into_owning_chain: !shared_base && anchor_not_integrated,
                }
            };
            push_group(layout, &mut stored, commit, naming, placement);
        }
        layout.chains.push(stored);
    }
}

/// The name a boundary tip gets at MATERIALIZATION — shared with `chain_plan`'s modeling so
/// plan and build cannot drift. The managed workspace tip is named by the workspace ref
/// itself; a forced entrypoint boundary keeps the split's precedence (checked-out ref first);
/// every other tip is named by disambiguation. A truly detached HEAD is anonymized afterwards
/// by `from_head`'s detach pass, never here.
#[allow(clippy::too_many_arguments)]
pub(super) fn materialize_tip_name<T: but_core::RefMetadata>(
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
            // Named by EXACTLY the workspace ref: co-located transient `gitbutler/*` refs
            // (e.g. `gitbutler/edit` mid edit-mode) must never name or join the workspace.
            super::materialize::empty_workspace_ref(cg, tip)
        } else {
            // Name by disambiguation; the empty workspace segment is spliced in above later.
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

/// The lower bound the PROJECTION will use: the merge base with the target, extended DOWN to a
/// stored/extra target position lying below it — an older target location keeps the commits
/// integrated since then visible, so stacks resting between the bound and the merge base are real
/// (kept) stacks, not empty floats.
pub(super) fn effective_lower_bound(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    options: &crate::walk::Options,
) -> Option<gix::ObjectId> {
    let mut lb = target
        .or(project_meta.target_commit_id)
        .or(options.extra_target_commit_id)
        .and_then(|t| cg.lowest_common_base(workspace_commit, t))?;
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
