//! Phase 2 of gather-then-build: the chain plan — names, floats, demotions, and
//! ref order decided purely, before any segment exists.

use std::collections::{HashMap, HashSet};

use gix::reference::Category;

use super::facts::{Facts, facts};
use super::remotes::remote_name_in_play;
use super::{IdMap, IdSet, disambiguated_ref, is_plain_local_branch};
use crate::CommitGraph;
use crate::ref_arrangement::{ArrangedGroup, Chain, GroupNamer, GroupPlacement, RefArrangement};

/// Phases 1+2 for one build: the facts, the chain plan decided over them, and the authored
/// ref placement table. Everything here is derived from the commit graph and the enrichment
/// inputs — the artifacts materialization consumes, and the data a commit-graph native
/// position derivation reads.
#[allow(clippy::too_many_arguments)]
pub(super) fn gather_and_plan<T: but_core::RefMetadata>(
    cg: &CommitGraph,
    workspace_commit: gix::ObjectId,
    entrypoint: gix::ObjectId,
    entrypoint_ref: Option<&gix::refs::FullName>,
    target: Option<gix::ObjectId>,
    remote_tracking: &HashMap<gix::refs::FullName, gix::refs::FullName>,
    symbolic_remotes: &[String],
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    managed: bool,
    meta: &T,
    project_meta: &but_core::ref_metadata::ProjectMeta,
    options: &crate::init::Options,
) -> (Facts, ChainPlan, RefArrangement) {
    let f = facts(
        cg,
        workspace_commit,
        entrypoint,
        target,
        remote_tracking,
        stack_branches,
        managed,
        meta,
        project_meta,
        options,
    );
    // The workspace's lower bound — the base all chains and the (stored/extra) target converge
    // on, extended down to an older target position.
    let ws_lower_bound =
        super::chains::effective_lower_bound(cg, workspace_commit, target, project_meta, options);
    let plan = chain_plan(
        cg,
        &f,
        workspace_commit,
        entrypoint,
        entrypoint_ref,
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
    // The ref placement table: stored on the commit graph and consumed by the chain-structure
    // pass. The oracle keeps proving it round-trips the plan it came from.
    let arrangement = author_arrangement(&plan);
    debug_assert_arrangement_matches_plan(&arrangement, &plan);
    (f, plan, arrangement)
}

/// One floated chain placeholder decided by [`chain_plan`]: `tip`'s segment goes anonymous, an
/// empty segment named `name` splices in between the workspace and it, and `displaced` (a
/// build-time name pushed aside by a metadata stack branch) returns to the commit as a passive
/// ref.
pub(super) struct Float {
    /// The commit whose segment goes anonymous so the empty named segment can float above it.
    pub(super) tip: gix::ObjectId,
    /// The name given to the empty segment spliced in between the workspace and `tip`.
    pub(super) name: gix::refs::FullName,
    /// A build-time name pushed aside by a metadata stack branch; it returns to `tip`'s commit as
    /// a passive ref. `None` when nothing was displaced.
    pub(super) displaced_ref_name: Option<gix::refs::FullName>,
}

/// One same-commit group of a metadata stack list — the RefOrder unit. Groups appear in
/// metadata order (top → bottom of the stack); a group's refs all point at `commit`.
#[derive(Debug, PartialEq, Eq, Clone)]
pub(super) struct RefGroup {
    /// The commit every ref of this group points at.
    pub(super) commit: gix::ObjectId,
    /// The members that become empty segments spliced above the anchor, in metadata order.
    /// The group's remaining members either name the anchor or already name another segment.
    pub(super) empties: Vec<gix::refs::FullName>,
    /// How the group lands in the graph.
    pub(super) placement: GroupPlacement,
}

pub(super) struct ChainPlan {
    pub(super) floats: Vec<Float>,
    pub(super) demoted: IdSet,
    /// Group-naming decisions of `insert_empty_branches`, keyed by (stack-list index, group
    /// commit): the anchor takes `name`; `clear_remote` marks the metadata-order override (a
    /// non-bottom namer is displaced, the remote creators link its floated empty instead).
    pub(super) group_names: HashMap<(usize, gix::ObjectId), (gix::refs::FullName, bool)>,
    /// Every boundary tip's MATERIALIZATION name (before floats/demotions suppress it on the
    /// segment). The remote/target passes historically ran before the chain shape existed and
    /// keyed their decisions on these.
    pub(super) base_name_of: IdMap<gix::refs::FullName>,
    /// Names the remote/target/explicit-tip passes give to ANONYMOUS boundary tips (a remote
    /// pointing behind/at an anonymous owner names it; the target and explicit tips likewise).
    /// Modeled here in pass order so materialization can mint segments with their FINAL names;
    /// the passes only add links. The value carries the named ref's actual position (a behind
    /// remote can point mid-run, below the owner's tip).
    pub(super) renames: IdMap<(gix::refs::FullName, gix::ObjectId)>,
    /// Every remote-ref name the remote passes will consume (renames, empty roots, ahead
    /// regions, untracked surfacing, the target). With the chain structure built FIRST, the
    /// empties filter consults this instead of finding the remote segments in the graph.
    pub(super) remote_used: HashSet<gix::refs::FullName>,
    /// The RefOrder: co-located ref-order decisions per metadata stack list, in metadata
    /// order — which refs of each same-commit group become empty segments, and how the group
    /// is placed. Modeled with the same `used`-names state the group naming sees, so
    /// materialization can consume order as DATA instead of re-deriving it from the graph.
    pub(super) ref_order: Vec<Vec<RefGroup>>,
}

impl ChainPlan {
    /// The name `tip`'s segment mints with, and the commit that name resolves to: floated and
    /// demoted tips stay anonymous, otherwise the build-time name or the planned rename.
    pub(super) fn tip_name(
        &self,
        tip: gix::ObjectId,
    ) -> Option<(gix::refs::FullName, gix::ObjectId)> {
        if self.floats.iter().any(|fl| fl.tip == tip) || self.demoted.contains(&tip) {
            return None;
        }
        self.base_name_of
            .get(&tip)
            .map(|n| (n.clone(), tip))
            .or_else(|| self.renames.get(&tip).cloned())
    }
}

/// The managed chain NAME decisions, computed before any segment mutation happens (phase 2 of
/// gather-then-build). Models the naming state the passes would see — materialization names,
/// then the anon-owner renames of the remote/target/explicit-tip passes — and decides purely:
///
/// * which shared workspace-parent tips float their name up as an empty chain placeholder
///   (`float_shared_stack_tips`),
/// * which anchors are DEMOTED to anonymous (a shared base at/below the bound, the lower-bound
///   float) so their stacks' branches form their own chains (`demote_shared_bases`),
/// * the group naming and ref order `insert_empty_branches` consumes as data
///   (`thread_ref_groups`).
#[allow(clippy::too_many_arguments)]
pub(super) fn chain_plan<T: but_core::RefMetadata>(
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
) -> ChainPlan {
    let mut plan = ChainPlan {
        floats: Vec::new(),
        demoted: IdSet::default(),
        group_names: HashMap::new(),
        base_name_of: IdMap::default(),
        renames: IdMap::default(),
        remote_used: HashSet::new(),
        ref_order: Vec::new(),
    };
    // The naming state as the chain passes will see it: materialization names first…
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
    // …then the renames of the remote, untracked-remote, target, and explicit-tip passes, each
    // modeled in pass order.
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

    if !managed {
        plan.remote_used = remote_used;
        return plan;
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

    let Some(lists) = stack_branches else {
        return plan;
    };
    let lists_per_commit = stack_lists_per_commit(cg, lists);
    let at_or_below_bound: Option<IdSet> = ws_lower_bound.map(|lb| cg.ancestor_set(lb));
    demote_shared_bases(
        cg,
        facts,
        lists,
        &lists_per_commit,
        at_or_below_bound.as_ref(),
        ws_lower_bound,
        &mut name_of,
        &mut plan.demoted,
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
        &lists_per_commit,
        at_or_below_bound.as_ref(),
        &mut name_of,
        &mut used,
        &remote_used,
        &mut plan,
    );
    plan.remote_used = remote_used;
    plan
}

/// The naming state as the chain passes will see it, seeded with every boundary tip's
/// materialization name.
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
/// in-set segment names it), in materialization order like the pass. Every remote name the pass
/// consumes — a rename, an empty root, an ahead region — is tracked, because the target block
/// only runs when nothing already used the target ref.
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
            if let gix::hashtable::hash_map::Entry::Vacant(e) = name_of.entry(owner) {
                e.insert(remote_ref.clone());
                renames.insert(owner, (remote_ref.clone(), remote_tip));
            }
            remote_used.insert(remote_ref.clone());
        } else if in_play(remote_ref) && !is_meta_stack_branch(remote_ref) {
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
            && let gix::hashtable::hash_map::Entry::Vacant(e) = name_of.entry(t.id)
        {
            e.insert(ref_name.clone());
            renames.insert(t.id, (ref_name, t.id));
        }
    }
}

/// `anonymize_shared_stack_tips`: a workspace-parent tip whose commit another in-workspace
/// commit builds on goes ANONYMOUS, and its name floats above as an empty chain placeholder —
/// the unique metadata STACK branch when build-time disambiguation picked a non-stack ref
/// (which then returns to the commit as a passive ref).
fn float_shared_stack_tips(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    target: Option<gix::ObjectId>,
    stack_branches: Option<&[Vec<gix::refs::FullName>]>,
    name_of: &mut IdMap<gix::refs::FullName>,
    floats: &mut Vec<Float>,
) {
    let is_stack_branch = |n: &gix::refs::FullName| {
        stack_branches
            .into_iter()
            .flatten()
            .flatten()
            .any(|b| b == n)
    };
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
                    .row(c)
                    .is_some_and(|n| n.commit.flags.contains(crate::CommitFlags::InWorkspace))
        });
        if !shared {
            continue;
        }
        // When build-time disambiguation picked a NON-stack ref, float the unique metadata
        // STACK branch instead and return the displaced name to the commit as a passive ref:
        // an applied-but-empty stack must keep its own chain, or the projection's
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
/// branches float above as their own chain. The workspace LOWER BOUND is where independent
/// stacks rest: an otherwise-unrepresented stack's branch pointing there floats as its own
/// empty chain and the boundary segment stays anonymous.
#[allow(clippy::too_many_arguments)]
fn demote_shared_bases(
    cg: &CommitGraph,
    facts: &Facts,
    lists: &[Vec<gix::refs::FullName>],
    lists_per_commit: &IdMap<usize>,
    at_or_below_bound: Option<&IdSet>,
    ws_lower_bound: Option<gix::ObjectId>,
    name_of: &mut IdMap<gix::refs::FullName>,
    demoted: &mut IdSet,
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
            demoted.insert(anchor);
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
        demoted.insert(lb);
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
                    .row(c)
                    .is_some_and(|n| !n.commit.flags.contains(crate::CommitFlags::Integrated)) =>
            {
                return false;
            }
            _ => {}
        }
    }
    at_lb
}

/// The group naming's "does this ref already name a segment" ranges over every segment, so
/// model the full set of names in use by `insert_empty_branches` time — chain names plus
/// everything the remote/target/tip/advanced passes will have created.
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
    // Explicit traversal tips name a segment (an anon owner, an empty splice, or a region tip).
    for t in cg.traversal_tips.iter().filter(|_| cg.explicit_tips) {
        if let Some(rn) = t.ref_name.clone()
            && !but_core::is_workspace_ref_name(rn.as_ref())
            && cg.row(t.id).is_some()
        {
            used.insert(rn);
        }
    }
    // An extra target outside every region is surfaced named by the unique plain local on it.
    if let Some(extra) = extra_target
        && cg.row(extra).is_some()
        && !facts.in_set.contains(&extra)
    {
        let mut locals = cg.refs_at(extra).into_iter().filter(is_plain_local_branch);
        if let (Some(l), None) = (locals.next(), locals.next()) {
            used.insert(l);
        }
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
        if let Some(name) = disambiguated_ref(cg, tip, remote_tracking, meta, None, target_ref) {
            used.insert(name);
        }
    }
    used
}

/// The group threading, mirroring `insert_empty_branches` exactly: per stack list, groups of
/// consecutive branches on one commit; the bottom-most member names an anonymous anchor, and
/// metadata order overrides a build-time name that belongs to the group. Fills the plan's
/// `group_names` and `ref_order`.
#[allow(clippy::too_many_arguments)]
fn thread_ref_groups(
    cg: &CommitGraph,
    facts: &Facts,
    workspace_commit: gix::ObjectId,
    ws_lower_bound: Option<gix::ObjectId>,
    lists: &[Vec<gix::refs::FullName>],
    lists_per_commit: &IdMap<usize>,
    at_or_below_bound: Option<&IdSet>,
    name_of: &mut IdMap<gix::refs::FullName>,
    used: &mut HashSet<gix::refs::FullName>,
    remote_used: &HashSet<gix::refs::FullName>,
    plan: &mut ChainPlan,
) {
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
                .row(anchor)
                .is_some_and(|n| !n.commit.flags.contains(crate::CommitFlags::Integrated));
            // ── The RefOrder: which members become empties, and how the group lands. The
            // `used` set at THIS point models materialization's "already names a segment"
            // gate (the group namer included — it names the anchor, not an empty). ──
            let shared_base = lists_per_commit.get(&commit).copied().unwrap_or(0) > 1
                && at_or_below_bound.is_none_or(|below| below.contains(&commit));
            let placement = if cross_stack_owned && anchor_not_integrated {
                GroupPlacement::Passive
            } else if !shared_base && anchor_not_integrated {
                GroupPlacement::Dependent
            } else {
                GroupPlacement::OwnChain
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
}

/// The name a boundary tip gets at MATERIALIZATION — shared with `chain_plan`'s modeling so plan
/// and build cannot drift. The managed workspace tip is named by the workspace ref itself (a
/// `gitbutler/*` ref that normal disambiguation skips); a forced entrypoint boundary keeps the
/// split's precedence (checked-out ref first); every other tip is named by disambiguation. A
/// truly detached HEAD is anonymized afterwards by `from_head`'s detach pass, never here.
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

/// Project the plan's ref decisions into the STORED table shape (see `crate::ref_arrangement`):
/// per-chain group threading becomes commit-keyed groups plus chain anchors. Stored on the
/// commit graph and consumed by the chain-structure pass —
/// `debug_assert_arrangement_matches_plan` proves the shape carries the plan losslessly on
/// every build.
pub(super) fn author_arrangement(plan: &ChainPlan) -> RefArrangement {
    let mut arrangement = RefArrangement::default();
    for (li, chain) in plan.ref_order.iter().enumerate() {
        let mut stored = Chain::default();
        for group in chain {
            let namer = plan
                .group_names
                .get(&(li, group.commit))
                .map(|(name, clear_remote)| GroupNamer {
                    name: name.clone(),
                    clear_remote: *clear_remote,
                });
            let groups = arrangement.at_commit.entry(group.commit).or_default();
            groups.push(ArrangedGroup {
                namer,
                empties: group.empties.clone(),
                placement: group.placement,
            });
            stored.anchors.push((group.commit, groups.len() - 1));
        }
        arrangement.chains.push(stored);
    }
    arrangement.demoted = plan.demoted.iter().copied().collect();
    arrangement.demoted.sort();
    arrangement
}

/// The stage-A parity oracle: reconstruct the plan's ref decisions FROM the stored table and
/// require exact equality — chain count and threading order, per-group empties and placement,
/// namers (with their displacement flag), and demotions. Non-trivial precisely where the shape
/// changes: several chains anchoring groups on the SAME commit must stay apart via the anchor
/// index, and no stored group may be orphaned from every chain.
pub(super) fn debug_assert_arrangement_matches_plan(
    arrangement: &RefArrangement,
    plan: &ChainPlan,
) {
    if !cfg!(debug_assertions) {
        return;
    }
    assert_eq!(
        arrangement.chains.len(),
        plan.ref_order.len(),
        "arrangement chains must mirror the plan's stack lists"
    );
    let mut group_names = HashMap::new();
    for (li, stored) in arrangement.chains.iter().enumerate() {
        let chain: Vec<RefGroup> = stored
            .anchors
            .iter()
            .map(|&(commit, gi)| {
                let group = &arrangement.at_commit[&commit][gi];
                if let Some(namer) = &group.namer {
                    group_names.insert((li, commit), (namer.name.clone(), namer.clear_remote));
                }
                RefGroup {
                    commit,
                    empties: group.empties.clone(),
                    placement: group.placement,
                }
            })
            .collect();
        assert_eq!(
            chain, plan.ref_order[li],
            "chain {li} must reconstruct the plan's group threading"
        );
    }
    assert_eq!(
        group_names, plan.group_names,
        "chain anchors must reconstruct the plan's group naming"
    );
    let mut demoted: Vec<_> = plan.demoted.iter().copied().collect();
    demoted.sort();
    assert_eq!(
        arrangement.demoted, demoted,
        "the table's demotions must match the plan's"
    );
    let anchored: usize = arrangement.chains.iter().map(|c| c.anchors.len()).sum();
    let stored: usize = arrangement.at_commit.values().map(Vec::len).sum();
    assert_eq!(anchored, stored, "every stored group belongs to a chain");
}
