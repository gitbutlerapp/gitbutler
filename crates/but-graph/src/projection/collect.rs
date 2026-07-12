//! Derive workspace stacks straight from the arena and its stored ref layout —
//! no segment is read.

use std::collections::{HashMap, HashSet};

use crate::workspace::{GraphContext, Stack, StackCommit, StackSegment};
use crate::{CommitGraph, RefInfo};
use but_core::ref_metadata;

/// How the view anchors; only these three states are legal (a managed COMMIT implies a
/// managed ref).
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewAnchor {
    /// The anchor is a managed workspace merge commit (with metadata).
    ManagedCommit,
    /// A managed workspace REF exists, but its commit is missing or unmanaged.
    ManagedRefOnly,
    /// A plain checkout: no managed ref at all.
    AdHoc,
}

impl ViewAnchor {
    fn new(managed_commit: bool, has_managed_ref: bool) -> Self {
        if managed_commit {
            debug_assert!(has_managed_ref, "a managed commit implies a managed ref");
            Self::ManagedCommit
        } else if has_managed_ref {
            Self::ManagedRefOnly
        } else {
            Self::AdHoc
        }
    }
    /// The view is NOT anchored on a managed merge commit.
    fn ad_hoc(self) -> bool {
        !matches!(self, Self::ManagedCommit)
    }
    /// The anchor IS a managed workspace merge commit.
    fn managed_commit(self) -> bool {
        matches!(self, Self::ManagedCommit)
    }
    /// A managed workspace ref exists.
    fn has_managed_ref(self) -> bool {
        !matches!(self, Self::AdHoc)
    }
}

/// Derive the workspace's stacks from the layout the builder stored on `cg` and the
/// workspace metadata's stack lists.
///
/// The parent array drives: one stack per materialized parent, in parent order —
/// metadata-less parents included. A parent claiming a ref chain (matched by the
/// chain's top position) gets its seed-born empty branches from the metadata list,
/// which is what the stored layout cannot carry. Positioned naming refs carve the
/// commit runs; another chain's naming ref ends a stack's territory, while chainless
/// naming refs (a reachable target local, say) become segments within the reaching
/// stack. `lower_bound` is the workspace frame's bound.
///
/// Returns `None` when the graph has no layout to read (unborn refs) — the caller
/// falls back to a single named empty stack.
pub(crate) fn stacks_from_arena(
    cg: &CommitGraph,
    ctx: &GraphContext,
    lower_bound: Option<gix::ObjectId>,
    ws_meta: Option<&ref_metadata::Workspace>,
    has_managed_ref: bool,
    entry_ref: Option<&gix::refs::FullName>,
    entry_commit: Option<gix::ObjectId>,
) -> Option<Vec<Stack>> {
    let layout = cg.layout()?;
    let (ws_commit, materialized_ws_parents) = resolve_view_anchor(cg, layout, ws_meta, entry_ref)?;
    let in_ws_stacks: Vec<_> = ws_meta
        .map(|meta| {
            meta.stacks
                .iter()
                .filter(|stack| stack.is_in_workspace())
                .collect()
        })
        .unwrap_or_default();

    if entry_is_unapplied_branch(
        cg,
        ws_commit,
        ws_meta,
        &in_ws_stacks,
        has_managed_ref,
        entry_ref,
    ) {
        return Some(Vec::new());
    }
    let anchor = ViewAnchor::new(
        cg.is_managed_ws_commit(ws_commit) && ws_meta.is_some(),
        has_managed_ref,
    );
    // A detached checkout doesn't speak for the branches at its commit.
    let detached = cg.seeds.iter().any(|t| t.is_entrypoint && t.is_detached);
    // The entrypoint commit anchors cutting and steering while it LIVES in the
    // arena; a redone traversal's remembered commit can be stale, and then the
    // entry ref's position speaks instead.
    let effective_entrypoint = entry_commit
        .or_else(|| entry_ref.and_then(|e| layout.positioned_on(e.as_ref())))
        .or_else(|| cg.entrypoint());
    let idx = index_layout(cg, layout, anchor);
    let mut chain_of_name = HashMap::<&gix::refs::FullNameRef, usize>::new();
    for (li, meta_stack) in in_ws_stacks.iter().enumerate() {
        for branch in &meta_stack.branches {
            chain_of_name.insert(branch.ref_name.as_ref(), li);
        }
    }
    let entry_ref = entry_ref.or_else(|| cg.entrypoint_ref());
    let real_ws_parents = if anchor.ad_hoc() {
        vec![ws_commit]
    } else {
        cg.all_parent_ids(ws_commit)
    };
    let (order, mut claimed) = build_run_order(
        cg,
        &in_ws_stacks,
        &idx.pos_by_name,
        ws_commit,
        &real_ws_parents,
        &materialized_ws_parents,
        anchor,
        lower_bound,
    );

    let mut unclaimed_empties_at =
        unclaimed_empties_in_table_order(cg, layout, &idx, &in_ws_stacks, &claimed);

    // A run-owning parent resting ON the bound has first claim to the bound's empties.
    let bound_run_exists = order
        .iter()
        .any(|r| r.owns_run && Some(r.start) == lower_bound);
    // Empties anchored ON a run's start belong to that run's own stack — a sibling
    // stack passing through the commit leaves them alone.
    let owned_run_starts: HashSet<gix::ObjectId> = order
        .iter()
        .filter(|r| r.owns_run)
        .map(|r| r.start)
        .collect();
    let walk = RunWalk {
        cg,
        ctx,
        layout,
        idx,
        chain_of_name,
        in_ws_stacks,
        owned_run_starts,
        bound_run_exists,
        anchor,
        detached,
        entry_ref,
        effective_entrypoint,
        reaches_entrypoint: effective_entrypoint
            .map(|ep| cg.reaches_marks(ep))
            .unwrap_or_default(),
        lower_bound,
        ws_meta,
    };
    let mut stacks = Vec::new();
    // Stack ids already handed out, chain-claimed ones included.
    let mut bound_ids = HashSet::<but_core::ref_metadata::StackId>::new();
    for run in &order {
        stacks.extend(walk.collect_stack(
            run,
            &mut claimed,
            &mut bound_ids,
            &mut unclaimed_empties_at,
        ));
    }
    split_at_bound_empties_from_content_stacks(&mut stacks, &walk, lower_bound, anchor);
    prune_all_integrated_anonymous(&mut stacks);
    // A TRUE ad-hoc view always has its one stack, if only an anonymous shell —
    // a detached checkout of fully integrated territory still IS a checkout.
    // (A managed-ref workspace can legitimately be empty.)
    if anchor == ViewAnchor::AdHoc && stacks.is_empty() {
        stacks.push(Stack::from_base_and_segments_raw(
            vec![StackSegment::default()],
            (!anchor.has_managed_ref()).then(but_core::ref_metadata::StackId::single_branch_id),
        ));
    }
    Some(stacks)
}

/// One run of the workspace: the metadata chain claiming it (if any), the commit
/// the run starts on, and whether this stack owns and walks the run — a chain
/// claiming a REAL parent edge (or the entry commit) owns its run, even a chain of
/// only empty branches, since the builder dedicated that materialized parent to it.
/// Fresh-inserted chains rest on commits owned elsewhere and never walk.
struct Run {
    chain: Option<usize>,
    start: gix::ObjectId,
    owns_run: bool,
}

/// The projection's per-name layout index: each known ref's facts and, when positioned,
/// the commit it stands on.
type PosByName<'a> =
    HashMap<&'a gix::refs::FullNameRef, (&'a crate::ref_layout::RefFacts, Option<gix::ObjectId>)>;

/// A chain claims a run when one of its positioned branches sits on the run's
/// start — metadata branch lists don't reliably order top-to-bottom, so this
/// checks them all. Real branches speak for the chain when it has any; the
/// anchors of its empties count only for a chain of nothing but empties, lest a
/// dependent empty resting on another stack's commit steal that stack's run.
fn chain_claims(
    pos_by_name: &PosByName<'_>,
    meta_stack: &but_core::ref_metadata::WorkspaceStack,
    key: gix::ObjectId,
) -> bool {
    let mut positions = meta_stack.branches.iter().filter_map(|b| {
        pos_by_name
            .get(b.ref_name.as_ref())
            .filter(|(facts, _)| facts.names_segment)
            .map(|&(facts, on)| (facts.names_empty_segment, on))
    });
    let has_real = positions.clone().any(|(empty, _)| !empty);
    positions.any(|(empty, on)| empty != has_real && on == Some(key))
}

/// The stack sequence mirrors how the builder wires the workspace's edges: the
/// commit's real parents in parent order (a chain topping out on a parent replaces
/// that edge in place), then chains without a parent edge INSERT at their metadata
/// index — the builder's fresh-connection rule.
///
/// A managed workspace commit is a pure merge whose parents are the stacks; an
/// UNMANAGED entrypoint commit (a bare `gitbutler/workspace` ref, a plain branch
/// shape, or a managed commit with NO workspace metadata — legacy drift) is
/// itself stack content — the single walk starts on it.
///
/// `real_ws_parents` is the merge commit's actual parent array; `materialized_ws_parents`
/// (the layout's stored `materialized_ws_parents`) is the parent list the metadata
/// implies — one entry per chain, top→bottom, duplicates where empty chains share a
/// base and fresh entries where the builder wired a chain without a real edge. The
/// two differ exactly on the stacks a git parent array cannot express.
///
/// Returns the runs plus which metadata chains were claimed.
///
/// This matching CANNOT move to the builder: a stored parent order can disagree with metadata order
/// (the merge on disk predates a metadata reorder), and in ws-commit-less configs
/// the run candidates come from the frame's view anchor — entry-inside, seed and
/// bound knowledge that only exists at projection time. (An attempt to compute
/// this at build time ended up re-implementing the frame's anchor choice case by case,
/// so the matching stays here.)
#[expect(clippy::too_many_arguments)]
fn build_run_order(
    cg: &CommitGraph,
    in_ws_stacks: &[&ref_metadata::WorkspaceStack],
    pos_by_name: &PosByName<'_>,
    ws_commit: gix::ObjectId,
    real_ws_parents: &[gix::ObjectId],
    materialized_ws_parents: &[gix::ObjectId],
    anchor: ViewAnchor,
    lower_bound: Option<gix::ObjectId>,
) -> (Vec<Run>, Vec<bool>) {
    let mut claims = RunClaims {
        cg,
        in_ws_stacks,
        pos_by_name,
        lower_bound,
        order: Vec::new(),
        claimed: vec![false; in_ws_stacks.len()],
        claimed_names: HashSet::new(),
    };
    if anchor.managed_commit() {
        claims.claim_chains_topping_the_entry(ws_commit);
    }
    claims.claim_real_parents(real_ws_parents);
    claims.claim_surplus_materialized_parents(materialized_ws_parents, real_ws_parents);
    // Unclaimed chains resting on the bound still surface as stacks at their
    // metadata index, with or without stored chain parents. A workspace REF makes this
    // a multi-stack workspace even without a managed commit or metadata reconciliation.
    if anchor.has_managed_ref() {
        claims.note_names_of_claimed_chains();
        if materialized_ws_parents.is_empty() {
            claims.claim_duplicate_runs();
        }
        claims.claim_at_bound_empties();
    }
    (claims.order, claims.claimed)
}

/// Local empties whose chain never claimed a run (or that no chain lists) still splice
/// in wherever their anchor is walked — the first stack through the commit takes them,
/// in positioned order.
///
/// Table order, not group order: the first stack through a commit takes its empties in
/// the order the layout registered them. Group order LOOKS equivalent but is not —
/// same-commit empties from different chains flip their vertical order under it.
fn unclaimed_empties_in_table_order<'a>(
    cg: &CommitGraph,
    layout: &'a crate::ref_layout::RefLayout,
    idx: &LayoutIndexes<'_>,
    in_ws_stacks: &[&ref_metadata::WorkspaceStack],
    claimed: &[bool],
) -> HashMap<gix::ObjectId, Vec<&'a gix::refs::FullName>> {
    let mut unclaimed_empties_at = HashMap::<gix::ObjectId, Vec<&gix::refs::FullName>>::new();
    for (name, _) in layout.facts.iter().filter(|(name, facts)| {
        facts.names_segment
            && facts.names_empty_segment
            && name.category() == Some(gix::reference::Category::LocalBranch)
            && !is_implementation_ref(cg, name.as_ref())
    }) {
        if in_ws_stacks.iter().enumerate().any(|(li, m)| {
            claimed[li]
                && m.branches
                    .iter()
                    .any(|b| b.ref_name.as_ref() == name.as_ref())
        }) {
            continue;
        }
        let Some(&(_, Some(on))) = idx.pos_by_name.get(name.as_ref()) else {
            continue;
        };
        unclaimed_empties_at.entry(on).or_default().push(name);
    }
    unclaimed_empties_at
}

/// The claim passes of [`build_run_order`], in call order, over the shared claim state:
/// each pass matches unclaimed metadata chains to run-start commits its own way.
struct RunClaims<'a> {
    cg: &'a CommitGraph,
    in_ws_stacks: &'a [&'a ref_metadata::WorkspaceStack],
    pos_by_name: &'a PosByName<'a>,
    lower_bound: Option<gix::ObjectId>,
    // (chain, run start, owns_run): a chain claiming a REAL parent edge (or the entry
    // commit) owns its run and walks it — even a chain of only empty branches, since
    // the builder dedicated that materialized parent to it. Fresh-inserted chains rest on
    // commits owned elsewhere and never walk.
    order: Vec<Run>,
    claimed: Vec<bool>,
    claimed_names: HashSet<&'a gix::refs::FullNameRef>,
}

impl<'a> RunClaims<'a> {
    /// A metadata chain topping out ON the entry commit itself (a workspace ref listed
    /// as its own stack branch — unreconciled legacy shapes) claims a run that starts
    /// there; the parent walks below then find everything already collected.
    fn claim_chains_topping_the_entry(&mut self, ws_commit: gix::ObjectId) {
        for (li, meta) in self.in_ws_stacks.iter().enumerate() {
            if !self.claimed[li] && chain_claims(self.pos_by_name, meta, ws_commit) {
                self.claimed[li] = true;
                self.order.push(Run {
                    chain: Some(li),
                    start: ws_commit,
                    owns_run: true,
                });
            }
        }
    }

    /// Every real parent edge is a run in parent order, claimed by the chain whose
    /// positioned branch sits on it (anonymous when none does).
    fn claim_real_parents(&mut self, real_ws_parents: &[gix::ObjectId]) {
        for parent in real_ws_parents {
            let chain = self.in_ws_stacks.iter().enumerate().find_map(|(li, meta)| {
                (!self.claimed[li] && chain_claims(self.pos_by_name, meta, *parent)).then_some(li)
            });
            if let Some(li) = chain {
                self.claimed[li] = true;
            }
            self.order.push(Run {
                chain,
                start: *parent,
                owns_run: true,
            });
        }
    }

    /// The chain parents ARE the resolved set of runs: every real parent plus a fresh
    /// connection per chain the builder wired in besides them (duplicates meaning two
    /// chains share one run). Each surplus entry claims its chain and inserts at the
    /// chain's metadata index — the builder's fresh-connection order.
    fn claim_surplus_materialized_parents(
        &mut self,
        materialized_ws_parents: &[gix::ObjectId],
        real_ws_parents: &[gix::ObjectId],
    ) {
        let mut surplus = materialized_ws_parents.to_vec();
        for parent in real_ws_parents {
            if let Some(i) = surplus.iter().position(|s| s == parent) {
                surplus.remove(i);
            }
        }
        for parent in surplus {
            let Some(li) = self.in_ws_stacks.iter().enumerate().find_map(|(li, meta)| {
                (!self.claimed[li] && chain_claims(self.pos_by_name, meta, parent)).then_some(li)
            }) else {
                continue;
            };
            self.insert_claim(
                li,
                Run {
                    chain: Some(li),
                    start: parent,
                    owns_run: true,
                },
            );
        }
    }

    /// Seed `claimed_names` from the chains claimed so far — the later passes skip
    /// branches another chain already materialized.
    fn note_names_of_claimed_chains(&mut self) {
        for (li, meta) in self.in_ws_stacks.iter().enumerate() {
            if self.claimed[li] {
                self.claimed_names
                    .extend(meta.branches.iter().map(|b| b.ref_name.as_ref()));
            }
        }
    }

    /// A second chain keyed on an already-claimed run start duplicates that
    /// run — the analog of a duplicated chain parent when none are stored. A chain
    /// whose real branch rests elsewhere gets a fresh run from that position. Only
    /// without stored chain parents: with them, real branches already claimed their
    /// runs.
    fn claim_duplicate_runs(&mut self) {
        for (li, meta) in self.in_ws_stacks.iter().enumerate() {
            if self.claimed[li]
                || meta
                    .branches
                    .iter()
                    .all(|b| self.claimed_names.contains(b.ref_name.as_ref()))
            {
                continue;
            }
            let key = self
                .order
                .iter()
                .map(|r| r.start)
                .find(|k| chain_claims(self.pos_by_name, meta, *k))
                .or_else(|| {
                    meta.branches
                        .iter()
                        .find_map(|b| {
                            self.pos_by_name
                                .get(b.ref_name.as_ref())
                                .filter(|(facts, _)| {
                                    facts.names_segment
                                        && !facts.names_empty_segment
                                        && facts.reachable
                                })
                                .and_then(|&(_, on)| on)
                        })
                        // ... but only OUTSIDE every other stack's reach; a position
                        // some stack walks over is that stack's segment, not its own stack.
                        .filter(|on| !self.order.iter().any(|r| reaches(self.cg, r.start, *on)))
                });
            let Some(key) = key else { continue };
            self.claimed_names
                .extend(meta.branches.iter().map(|b| b.ref_name.as_ref()));
            self.insert_claim(
                li,
                Run {
                    chain: Some(li),
                    start: key,
                    owns_run: true,
                },
            );
        }
    }

    /// Last resort: an unclaimed chain of nothing but empties resting ON the
    /// exclusive bound surfaces as its own (non-walking) stack. Mid-run anchors
    /// splice into the stack that walks them instead.
    fn claim_at_bound_empties(&mut self) {
        for (li, meta) in self.in_ws_stacks.iter().enumerate() {
            if self.claimed[li] {
                continue;
            }
            // Metadata can list one branch in several stacks mid-operation; the
            // first stack that materialized it keeps it.
            if meta
                .branches
                .iter()
                .all(|b| self.claimed_names.contains(b.ref_name.as_ref()))
            {
                continue;
            }
            // A branch whose commit IS the exclusive bound has an empty segment too
            // (nothing sits strictly above the bound), even where the builder marked
            // it non-empty for naming a real commit — or gave naming rights to a
            // sibling ref on the same commit.
            let is_empty_pos = |facts: &crate::ref_layout::RefFacts, on: Option<gix::ObjectId>| {
                (facts.names_segment && facts.names_empty_segment)
                    || (on.is_some() && on == self.lower_bound)
            };
            let all_empty_anchor = meta.branches.iter().find_map(|b| {
                let &(facts, on) = self.pos_by_name.get(b.ref_name.as_ref())?;
                is_empty_pos(facts, on).then_some(on).flatten()
            });
            let any_real = meta.branches.iter().any(|b| {
                self.pos_by_name
                    .get(b.ref_name.as_ref())
                    .is_some_and(|&(facts, on)| facts.names_segment && !is_empty_pos(facts, on))
            });
            let Some(anchor) = all_empty_anchor
                .filter(|_| !any_real)
                .filter(|a| Some(*a) == self.lower_bound)
            else {
                continue;
            };
            self.claimed_names
                .extend(meta.branches.iter().map(|b| b.ref_name.as_ref()));
            self.insert_claim(
                li,
                Run {
                    chain: Some(li),
                    start: anchor,
                    owns_run: false,
                },
            );
        }
    }

    /// Claim chain `li` and insert its run at the chain's metadata index.
    fn insert_claim(&mut self, li: usize, run: Run) {
        self.claimed[li] = true;
        let at = li.min(self.order.len());
        self.order.insert(at, run);
    }
}

/// A chain member that is EMPTY and anchored on the exclusive bound defects from
/// its chain: it rests on base territory below the chain's real content, so it
/// stands as its own stack (keeping the chain's identity and position), while the
/// content remainder becomes a new stack appended at the bottom. All-empty chains
/// stay whole — there is no content to separate from. Operations then persist the
/// split back to metadata, converging on two chains.
fn split_at_bound_empties_from_content_stacks(
    stacks: &mut Vec<Stack>,
    walk: &RunWalk<'_>,
    lower_bound: Option<gix::ObjectId>,
    anchor: ViewAnchor,
) {
    if anchor.ad_hoc() || lower_bound.is_none() {
        return;
    }
    let mut appended = Vec::new();
    for stack in stacks.iter_mut() {
        if stack.segments.len() < 2 {
            continue;
        }
        // The stack's metadata chain gives each branch its declared rank (0 = top).
        let Some(chain) = stack
            .id
            .and_then(|id| walk.in_ws_stacks.iter().find(|m| m.id == id))
        else {
            continue;
        };
        let rank_of = |name: &gix::refs::FullNameRef| {
            chain
                .branches
                .iter()
                .position(|b| b.ref_name.as_ref() == name)
        };
        // The highest-ranked (closest to top) content-bearing member.
        let Some(top_content_rank) = stack
            .segments
            .iter()
            .filter(|seg| !seg.commits.is_empty())
            .filter_map(|seg| seg.ref_name().and_then(rank_of))
            .min()
        else {
            continue;
        };
        // A member defects when it is empty, rests ON the bound, and its metadata
        // rank puts it ABOVE the content — an inversion the chain layout cannot
        // express (physically it sits on base territory BELOW that content). A
        // dependent empty declared below its content is consistent and stays.
        let defects = |seg: &StackSegment| {
            seg.commits.is_empty()
                && seg.ref_name().is_some_and(|name| {
                    walk.idx
                        .pos_by_name
                        .get(name)
                        .is_some_and(|&(_, on)| on.is_some() && on == lower_bound)
                        && rank_of(name).is_some_and(|rank| rank < top_content_rank)
                })
        };
        if !stack.segments.iter().any(&defects) {
            continue;
        }
        let (defectors, remainder): (Vec<_>, Vec<_>) =
            stack.segments.drain(..).partition(|seg| defects(seg));
        // The defector keeps the chain's stack identity and position; the content
        // remainder is the newly-arranged entity and settles at the bottom.
        let remainder_stack = Stack::from_base_and_segments_raw(remainder, None);
        stack.segments = defectors;
        appended.push(remainder_stack);
    }
    stacks.extend(appended);
}

/// Everything the per-run walk reads. The mutable claim state (claimed chains,
/// bound stack ids, unclaimed empties) is passed into [`Self::collect_stack`]
/// separately so the borrows stay obvious.
struct RunWalk<'a> {
    cg: &'a CommitGraph,
    ctx: &'a GraphContext,
    layout: &'a crate::ref_layout::RefLayout,
    idx: LayoutIndexes<'a>,
    chain_of_name: HashMap<&'a gix::refs::FullNameRef, usize>,
    in_ws_stacks: Vec<&'a ref_metadata::WorkspaceStack>,
    owned_run_starts: HashSet<gix::ObjectId>,
    bound_run_exists: bool,
    anchor: ViewAnchor,
    detached: bool,
    entry_ref: Option<&'a gix::refs::FullName>,
    effective_entrypoint: Option<gix::ObjectId>,
    /// Per node: whether the entrypoint is among its ancestors — the merge-steering
    /// question, answered once for the whole arena instead of per merge.
    reaches_entrypoint: Vec<bool>,
    lower_bound: Option<gix::ObjectId>,
    ws_meta: Option<&'a ref_metadata::Workspace>,
}

/// The named-segment assembly a run walk accumulates: finished segments, the
/// names already emitted (a name never names two segments in one stack), and
/// the segment currently being filled.
struct SegmentsInProgress<'a> {
    segments: Vec<StackSegment>,
    emitted: HashSet<&'a gix::refs::FullNameRef>,
    current: Option<StackSegment>,
}

impl SegmentsInProgress<'_> {
    fn new() -> Self {
        SegmentsInProgress {
            segments: Vec::new(),
            emitted: HashSet::new(),
            current: None,
        }
    }

    /// Close `current`, keeping it unless it is an anonymous shell that only
    /// existed to carry outside commits and never received any.
    fn flush_current(&mut self) {
        if let Some(seg) = self.current.take()
            && (!seg.commits.is_empty() || seg.commits_outside.is_none())
        {
            self.segments.push(seg);
        }
    }
}

/// What the naming pass decided for one walked commit.
enum SegmentCut<'a, 's> {
    /// The commit is named by a branch whose chain claimed another stack —
    /// this run's territory ends here.
    ForeignTerritory,
    /// The commit starts a new named segment; `projected_outside` carries the
    /// outside commits when the name came from an outside projection.
    Named {
        name: &'a gix::refs::FullName,
        projected_outside: Option<&'s Vec<StackCommit>>,
    },
    /// No name speaks for this commit — it rides in the current (or a fresh
    /// anonymous) segment.
    Anonymous,
}

impl<'a> RunWalk<'a> {
    /// Walk one run into a stack: this chain's (or chainless) naming refs cut
    /// segments, another stack's naming ref or the workspace bound ends the
    /// territory, and leftover empties settle at the stack's bottom in metadata
    /// order. Returns `None` when the run contributed nothing.
    fn collect_stack(
        &self,
        run: &Run,
        claimed: &mut [bool],
        bound_ids: &mut HashSet<but_core::ref_metadata::StackId>,
        unclaimed_empties_at: &mut HashMap<gix::ObjectId, Vec<&'a gix::refs::FullName>>,
    ) -> Option<Stack> {
        let (li, chain_id, branch_names) = self.claim_chain(run, claimed, bound_ids);

        // The chain's branch list in metadata order (top → bottom): empties splice in
        // wherever their anchor lies — above the run, between runs, or below the
        // bound. The walk emits each named branch's run when it reaches it; whatever
        // never got reached (anchors at or below the bound) settles as empty
        // segments afterwards, in metadata order.
        let mut pending = self.pending_empties(&branch_names);
        let mut bound_rest: Vec<&gix::refs::FullName> = Vec::new();
        let mut sip = SegmentsInProgress::new();

        let last_base = self.walk_run(
            run,
            li,
            &branch_names,
            claimed,
            unclaimed_empties_at,
            &mut pending,
            &mut bound_rest,
            &mut sip,
        );
        sip.flush_current();

        self.settle_leftover_empties(pending, bound_rest, &mut sip);

        let last_base = if run.owns_run {
            last_base
        } else {
            Some(run.start).filter(|id| self.cg.node(*id).is_some())
        };
        if let Some(seg) = sip.segments.last_mut() {
            seg.base = last_base;
        }

        let id = self.bind_stack_identity(chain_id, &sip.segments, bound_ids);
        let mut segments = sip.segments;

        // An entrypoint gitbutler/* ref may NAME a run (absorbing its commits in
        // legacy bare-checkout shapes) but never rests as an empty segment.
        segments.retain(|seg| {
            seg.commits_outside.is_some()
                || !seg.commits.is_empty()
                || !seg.ref_name().is_some_and(in_gitbutler_namespace)
        });
        if segments.is_empty() {
            if li.is_some() {
                self.emit_collapsed_chain_last_trace(&branch_names, &mut segments);
            }
            if segments.is_empty() {
                // Fully empty stacks are removed — a parent whose run was consumed
                // elsewhere contributes nothing.
                return None;
            }
        }
        Some(Stack::from_base_and_segments_raw(segments, id))
    }

    /// PHASE 1: claim the run's metadata chain (if any) and dedupe its branch
    /// list — metadata written mid-operation can list a branch twice; only the
    /// first occurrence shapes the stack.
    fn claim_chain(
        &self,
        run: &Run,
        claimed: &mut [bool],
        bound_ids: &mut HashSet<but_core::ref_metadata::StackId>,
    ) -> (
        Option<usize>,
        Option<but_core::ref_metadata::StackId>,
        Vec<&'a gix::refs::FullName>,
    ) {
        let Some((li, meta_stack)) = run.chain.map(|li| (li, self.in_ws_stacks[li])) else {
            return (None, None, Vec::new());
        };
        claimed[li] = true;
        let mut names: Vec<&gix::refs::FullName> = Vec::new();
        for b in &meta_stack.branches {
            if !names.iter().any(|n| **n == b.ref_name) {
                names.push(&b.ref_name);
            }
        }
        bound_ids.insert(meta_stack.id);
        (Some(li), Some(meta_stack.id), names)
    }

    /// PHASE 2: the chain's empty branches, in metadata order — they splice in
    /// wherever the walk crosses their anchor.
    fn pending_empties(
        &self,
        branch_names: &[&'a gix::refs::FullName],
    ) -> Vec<&'a gix::refs::FullName> {
        branch_names
            .iter()
            .filter(|n| {
                self.idx
                    .pos_by_name
                    .get(n.as_ref())
                    .is_some_and(|(facts, _)| facts.names_segment && facts.names_empty_segment)
            })
            .copied()
            .collect()
    }

    /// PHASE 3: walk the run commit by commit. This chain's (or chainless)
    /// naming refs cut segments; another STACK's naming ref or the workspace
    /// bound ends the territory. Returns the base the walk stopped on —
    /// `None` when it ended on another stack's territory (per the sibling-base rule
    /// (see `hide_bases_on_sibling_territory`), resting on a sibling shows no base).
    #[expect(clippy::too_many_arguments)]
    fn walk_run(
        &self,
        run: &Run,
        li: Option<usize>,
        branch_names: &[&'a gix::refs::FullName],
        claimed: &[bool],
        unclaimed_empties_at: &mut HashMap<gix::ObjectId, Vec<&'a gix::refs::FullName>>,
        pending: &mut Vec<&'a gix::refs::FullName>,
        bound_rest: &mut Vec<&'a gix::refs::FullName>,
        sip: &mut SegmentsInProgress<'a>,
    ) -> Option<gix::ObjectId> {
        let run_start = run.owns_run.then_some(run.start);
        let mut last_base = None;
        let mut cursor = run_start;
        while let Some(id) = cursor {
            if self.stops_at_bound(id, run_start, li, unclaimed_empties_at, bound_rest) {
                last_base = Some(id);
                break;
            }
            let Some(node) = self.cg.node(id) else { break };
            self.splice_empties_at(id, run_start, li, pending, unclaimed_empties_at, sip);
            self.flush_for_entrypoint(id, run_start, sip);
            let detached_start =
                self.anchor.ad_hoc() && li.is_none() && self.detached && Some(id) == run_start;
            match self.segment_cut_at(id, run_start, li, branch_names, claimed, &sip.emitted) {
                SegmentCut::ForeignTerritory => {
                    // (A run's START always belongs to its stack, even when named by
                    // a branch whose chain claimed elsewhere — metadata written
                    // mid-move splits a chain across two parents.)
                    // Another STACK's territory (a chain that got its own stack): per the
                    // sibling-base rule (see `hide_bases_on_sibling_territory`), resting on
                    // a sibling shows no base.
                    last_base = None;
                    break;
                }
                SegmentCut::Named {
                    name,
                    projected_outside,
                } => {
                    sip.flush_current();
                    sip.emitted.insert(name.as_ref());
                    let mut seg = named_segment(name.clone(), Some(id), self.ctx);
                    if let Some(outside) = projected_outside {
                        seg.name_projected_from_outside = true;
                        if !outside.is_empty() {
                            seg.commits_outside = Some(outside.clone());
                        }
                    }
                    sip.current = Some(seg);
                }
                SegmentCut::Anonymous => {
                    if sip.current.is_none() {
                        sip.current = Some(StackSegment::default());
                    }
                }
            }
            let appended_inside = self.append_commit(node, id, li, detached_start, sip);
            last_base = next_parent(self.cg, id, &self.reaches_entrypoint);
            cursor = last_base;
            // A commit whose raw parents were never walked ends the run early —
            // the traversal's limit, worn by the last collected commit.
            if appended_inside
                && cursor.is_none()
                && !node.parent_ids.is_empty()
                && !node.flags.contains(crate::CommitFlags::ShallowBoundary)
                && let Some(commit) = sip.current.as_mut().and_then(|seg| seg.commits.last_mut())
            {
                commit.flags |= crate::workspace::StackCommitFlags::EarlyEnd;
            }
        }
        last_base
    }

    /// The walk stops when it reaches the workspace bound. Chainless empties
    /// resting ON the bound settle at this stack's bottom, after its own
    /// leftovers — a stack whose own run start IS the bound has first claim on them.
    /// (An AD-HOC stack STARTING on the bound walks right through — a checkout's
    /// own segment is always shown.)
    fn stops_at_bound(
        &self,
        id: gix::ObjectId,
        run_start: Option<gix::ObjectId>,
        li: Option<usize>,
        unclaimed_empties_at: &mut HashMap<gix::ObjectId, Vec<&'a gix::refs::FullName>>,
        bound_rest: &mut Vec<&'a gix::refs::FullName>,
    ) -> bool {
        if Some(id) != self.lower_bound {
            return false;
        }
        let walks_through = self.anchor.ad_hoc()
            && li.is_none()
            && entry_walks_through(
                self.ctx,
                self.layout,
                &self.idx.pos_by_name,
                self.entry_ref,
                id,
                run_start,
            );
        if walks_through {
            return false;
        }
        if !self.bound_run_exists || run_start == Some(id) {
            *bound_rest = unclaimed_empties_at.remove(&id).unwrap_or_default();
            if self.anchor.ad_hoc() && li.is_none() && Some(id) == run_start {
                retain_ordered_after_entry(bound_rest, self.ctx, self.entry_ref);
            }
        }
        true
    }

    /// The entrypoint commit starts its own segment even without a naming ref.
    fn flush_for_entrypoint(
        &self,
        id: gix::ObjectId,
        run_start: Option<gix::ObjectId>,
        sip: &mut SegmentsInProgress<'_>,
    ) {
        if Some(id) == self.effective_entrypoint
            && Some(id) != run_start
            && sip
                .current
                .as_ref()
                .is_some_and(|seg| !seg.commits.is_empty())
            && sip.current.as_ref().and_then(|seg| seg.ref_name())
                != self.entry_ref.map(|r| r.as_ref())
        {
            sip.flush_current();
        }
    }

    /// Add the walked commit to the current segment. Commits the workspace
    /// doesn't own (an advanced tip that left the workspace) ride OUTSIDE the
    /// segment until workspace territory starts; returns whether the commit
    /// landed inside.
    fn append_commit(
        &self,
        node: &crate::Commit,
        id: gix::ObjectId,
        li: Option<usize>,
        detached_start: bool,
        sip: &mut SegmentsInProgress<'_>,
    ) -> bool {
        let Some(seg) = sip.current.as_mut() else {
            return false;
        };
        if li.is_some()
            && !node.flags.contains(crate::CommitFlags::InWorkspace)
            && seg.commits.is_empty()
        {
            let mut commit = StackCommit::from_graph_commit(node);
            strip_structural_refs(&mut commit, seg.ref_name(), &self.idx.names_empty);
            seg.commits_outside
                .get_or_insert_with(Vec::new)
                .push(commit);
            return false;
        }
        let mut commit = StackCommit::from_graph_commit(node);
        strip_structural_refs(&mut commit, seg.ref_name(), &self.idx.names_empty);
        // A detached checkout hands its name back to the commit LAST — after any
        // tags that already rode there.
        if detached_start
            && let Some(&name) = self.idx.naming_at.get(&id)
            && let Some(i) = commit
                .refs
                .iter()
                .position(|ri| ri.ref_name.as_ref() == name.as_ref())
        {
            let ri = commit.refs.remove(i);
            commit.refs.push(ri);
        }
        seg.commits.push(commit);
        true
    }

    /// PHASE 3a: empties anchored on THIS commit splice in before its segment.
    fn splice_empties_at(
        &self,
        id: gix::ObjectId,
        run_start: Option<gix::ObjectId>,
        li: Option<usize>,
        pending: &mut Vec<&'a gix::refs::FullName>,
        unclaimed_empties_at: &mut HashMap<gix::ObjectId, Vec<&'a gix::refs::FullName>>,
        sip: &mut SegmentsInProgress<'a>,
    ) {
        let anchor_of =
            |name: &gix::refs::FullNameRef| self.idx.pos_by_name.get(name).and_then(|&(_, on)| on);
        let mut here: Vec<&gix::refs::FullName> = Vec::new();
        pending.retain(|n| {
            if anchor_of(n.as_ref()) == Some(id) {
                here.push(n);
                false
            } else {
                true
            }
        });
        if (Some(id) == run_start || !self.owned_run_starts.contains(&id))
            && let Some(more) = unclaimed_empties_at.remove(&id)
        {
            here.extend(more);
        }
        // An ad-hoc view starts AT the entrypoint ref: everything ABOVE the
        // entry in its persisted order is out of view; the rest splice in order.
        if self.anchor.ad_hoc() && li.is_none() && Some(id) == run_start {
            retain_ordered_after_entry(&mut here, self.ctx, self.entry_ref);
        }
        if here.is_empty() {
            return;
        }
        sip.flush_current();
        // A commit with no naming ref of its own takes the bottom-most
        // empty anchored on it as its segment name and is absorbed by it;
        // the rest stay empty above it.
        let absorbs = self
            .idx
            .naming_at
            .get(&id)
            .filter(|n| {
                li.is_none() || n.category() != Some(gix::reference::Category::RemoteBranch)
            })
            .is_none()
            && !self.idx.projected_at.contains_key(&id);
        let absorber = if absorbs { here.pop() } else { None };
        for n in here {
            sip.emitted.insert(n.as_ref());
            // An empty segment points at no commit of its own — `commit_id`
            // is the branch's OWN tip, and consumers (e.g. squash's
            // empty-target refusal) rely on None meaning empty.
            sip.segments
                .push(named_segment((*n).clone(), None, self.ctx));
        }
        if let Some(n) = absorber {
            sip.emitted.insert(n.as_ref());
            sip.current = Some(named_segment((*n).clone(), Some(id), self.ctx));
        }
    }

    /// PHASE 3b: decide what names this commit — a segment cut, foreign
    /// territory, or nothing.
    fn segment_cut_at<'s>(
        &'s self,
        id: gix::ObjectId,
        run_start: Option<gix::ObjectId>,
        li: Option<usize>,
        branch_names: &[&'a gix::refs::FullName],
        claimed: &[bool],
        emitted: &HashSet<&gix::refs::FullNameRef>,
    ) -> SegmentCut<'a, 's> {
        let mut direct = self.idx.naming_at.get(&id).copied();
        // Remote names only speak in CHAINLESS stacks; a chain-claimed run keeps
        // its commits under the chain's own segments.
        if li.is_some()
            && direct.is_some_and(|n| n.category() == Some(gix::reference::Category::RemoteBranch))
        {
            direct = None;
        }
        // A name this stack already emitted never names a second segment.
        if direct.is_some_and(|n| emitted.contains(n.as_ref())) {
            direct = None;
        }
        // A DETACHED head doesn't speak for the branches at its commit: the
        // run starts anonymous and they ride along.
        let detached_start =
            self.anchor.ad_hoc() && li.is_none() && self.detached && Some(id) == run_start;
        if detached_start {
            direct = None;
        } else if direct.is_none() && Some(id) == run_start {
            // A claiming chain's own positioned branch names its run start even
            // when the position was too ambiguous to cut runs generally.
            direct = branch_names
                .iter()
                .find(|n| {
                    self.idx
                        .pos_by_name
                        .get(n.as_ref())
                        .is_some_and(|&(facts, on)| {
                            facts.names_segment && !facts.names_empty_segment && on == Some(id)
                        })
                })
                .copied();
        }
        let projected = (direct.is_none() && !detached_start)
            .then(|| self.idx.projected_at.get(&id))
            .flatten()
            .filter(|(name, _)| !emitted.contains(name.as_ref()));
        match direct.or(projected.map(|(name, _)| *name)) {
            Some(name)
                if Some(id) != run_start
                    && self
                        .chain_of_name
                        .get(name.as_ref())
                        .is_some_and(|&c| Some(c) != li && claimed[c]) =>
            {
                SegmentCut::ForeignTerritory
            }
            Some(name) => SegmentCut::Named {
                name,
                projected_outside: projected.map(|(_, outside)| outside),
            },
            None => SegmentCut::Anonymous,
        }
    }

    /// PHASE 4: leftover empties (anchors at or below the bound, or on unwalked
    /// commits) settle at the stack's bottom in metadata order.
    fn settle_leftover_empties(
        &self,
        pending: Vec<&'a gix::refs::FullName>,
        bound_rest: Vec<&'a gix::refs::FullName>,
        sip: &mut SegmentsInProgress<'a>,
    ) {
        for n in pending.into_iter().chain(bound_rest) {
            if !sip.emitted.contains(n.as_ref()) {
                // Empty leftovers own no commit — None, not their anchor.
                sip.segments.push(named_segment(n.clone(), None, self.ctx));
            }
        }
    }

    /// PHASE 5: natural stacks still bind their metadata identity when any of
    /// their segments carries a metadata stack's branch name (the weightless
    /// first-match) — INACTIVE stacks included: identity outlives application.
    fn bind_stack_identity(
        &self,
        chain_id: Option<but_core::ref_metadata::StackId>,
        segments: &[StackSegment],
        bound_ids: &mut HashSet<but_core::ref_metadata::StackId>,
    ) -> Option<but_core::ref_metadata::StackId> {
        chain_id
            .or_else(|| {
                segments
                    .iter()
                    .flat_map(|s| {
                        s.ref_name().into_iter().chain(
                            s.commits
                                .iter()
                                .flat_map(|c| c.refs.iter().map(|ri| ri.ref_name.as_ref())),
                        )
                    })
                    .find_map(|name| {
                        self.ws_meta?.stacks.iter().find_map(|meta| {
                            (!bound_ids.contains(&meta.id)
                                && meta.branches.iter().any(|b| b.ref_name.as_ref() == name))
                            .then(|| {
                                bound_ids.insert(meta.id);
                                meta.id
                            })
                        })
                    })
            })
            // The single ad-hoc stack carries the fixed single-branch id; a
            // managed-REF workspace (whatever its state) binds from metadata alone.
            .or_else(|| {
                (self.anchor == ViewAnchor::AdHoc)
                    .then(but_core::ref_metadata::StackId::single_branch_id)
            })
    }

    /// PHASE 6: a CHAIN-claimed run that collapsed to nothing (its walk broke at
    /// the bound immediately, e.g. a branch merged upstream whose commit IS the
    /// bound) still emits its empty/at-bound branches: this run is the chain's
    /// only projection, and a vanished applied chain would be un-applied by the
    /// next write.
    fn emit_collapsed_chain_last_trace(
        &self,
        branch_names: &[&gix::refs::FullName],
        segments: &mut Vec<StackSegment>,
    ) {
        for n in branch_names {
            let pos = self.idx.pos_by_name.get(n.as_ref());
            // Naming rights don't matter here — the chain vanishes otherwise,
            // so any empty or at-bound position is trace enough.
            let at_bound_or_empty = pos.is_some_and(|&(facts, on)| {
                facts.names_empty_segment || (on.is_some() && on == self.lower_bound)
            });
            if at_bound_or_empty {
                let mut seg = named_segment((*n).clone(), None, self.ctx);
                // The anchor is this empty branch's base — operations
                // rebuilding the workspace commit fall back to it when
                // there is no tip.
                seg.base = pos.and_then(|&(_, on)| on);
                segments.push(seg);
            }
        }
    }
}

/// A workspace whose every stack is anonymous, identityless and fully integrated
/// shows nothing at all — there is no un-integrated work to lay out.
fn prune_all_integrated_anonymous(stacks: &mut Vec<Stack>) {
    if !stacks.is_empty()
        && stacks.iter().all(|s| s.id.is_none())
        && stacks
            .iter()
            .flat_map(|s| &s.segments)
            .all(|seg| seg.ref_name().is_none())
        && stacks
            .iter()
            .flat_map(|s| s.segments.iter().flat_map(|seg| &seg.commits))
            .next()
            .is_some()
        && stacks
            .iter()
            .flat_map(|s| s.segments.iter().flat_map(|seg| &seg.commits))
            .all(|c| {
                c.flags
                    .contains(crate::workspace::StackCommitFlags::Integrated)
            })
    {
        stacks.clear();
    }
}

/// The commit the workspace view anchors on, plus the materialized parents.
///
/// Without materialized parents, a positioned gitbutler/* ref still pointing at
/// a managed workspace commit defines the workspace view (a branch checkout
/// inside a workspace); otherwise the entrypoint commit is the single run and
/// the ad-hoc discriminator does the rest.
fn resolve_view_anchor(
    cg: &CommitGraph,
    layout: &crate::ref_layout::RefLayout,
    ws_meta: Option<&ref_metadata::Workspace>,
    entry_ref: Option<&gix::refs::FullName>,
) -> Option<(gix::ObjectId, Vec<gix::ObjectId>)> {
    layout
        .materialized_ws_parents
        .clone()
        .map(|m| (m.commit, m.parents))
        .or_else(|| {
            // ... and like everywhere else, workspace semantics require the
            // metadata to exist; without it the entrypoint's view wins.
            ws_meta?;
            let on = layout.placements().find_map(|(name, on)| {
                (in_gitbutler_namespace(name.as_ref()) && cg.is_managed_ws_commit(on)).then_some(on)
            })?;
            Some((on, Vec::new()))
        })
        .or_else(|| {
            // A redone traversal can remember a stale entrypoint commit; the
            // entry REF's live position anchors the run instead.
            let entry = entry_ref
                .map(|r| r.as_ref())
                .or_else(|| cg.entrypoint_ref().map(|r| r.as_ref()))?;
            let on = layout.positioned_on(entry)?;
            Some((on, Vec::new()))
        })
        .or_else(|| Some((cg.entrypoint()?, Vec::new())))
}

/// A workspace REF with metadata but NO in-workspace stacks and no managed
/// commit is a defined-but-empty workspace: a checkout of an unapplied branch
/// doesn't become an ad-hoc stack inside it.
fn entry_is_unapplied_branch(
    cg: &CommitGraph,
    ws_commit: gix::ObjectId,
    ws_meta: Option<&ref_metadata::Workspace>,
    in_ws_stacks: &[&ref_metadata::WorkspaceStack],
    has_managed_ref: bool,
    entry_ref: Option<&gix::refs::FullName>,
) -> bool {
    has_managed_ref
        && !cg.is_managed_ws_commit(ws_commit)
        && ws_meta.is_some_and(|meta| !meta.stacks.is_empty())
        && in_ws_stacks.is_empty()
        && entry_ref
            .or_else(|| cg.entrypoint_ref())
            .zip(ws_meta)
            .is_some_and(|(entry, meta)| {
                meta.stacks.iter().any(|stack| {
                    !stack.is_in_workspace()
                        && stack
                            .branches
                            .iter()
                            .any(|b| b.ref_name.as_ref() == entry.as_ref())
                })
            })
}

/// The layout read three ways: positioned refs by name, run-naming refs by commit,
/// empty-segment names, and out-of-workspace name projections.
struct LayoutIndexes<'a> {
    pos_by_name: PosByName<'a>,
    naming_at: HashMap<gix::ObjectId, &'a gix::refs::FullName>,
    names_empty: HashSet<&'a gix::refs::FullNameRef>,
    /// A naming ref positioned OUTSIDE the workspace projects onto its first
    /// in-workspace first-parent ancestor: the segment there takes its name, and the
    /// outside prefix rides along as commits_outside.
    projected_at: HashMap<gix::ObjectId, (&'a gix::refs::FullName, Vec<StackCommit>)>,
}

/// The `refs/heads/gitbutler/` namespace: refs GitButler creates for itself (the
/// workspace ref, targets) rather than the user's branches.
fn in_gitbutler_namespace(name: &gix::refs::FullNameRef) -> bool {
    name.as_bstr().starts_with(b"refs/heads/gitbutler/")
}

/// Implementation refs never shape user-visible stacks: they neither name segments
/// nor ride on commits.
fn is_implementation_ref(cg: &CommitGraph, name: &gix::refs::FullNameRef) -> bool {
    in_gitbutler_namespace(name) && cg.entrypoint_ref().map(|r| r.as_ref()) != Some(name)
}

fn index_layout<'a>(
    cg: &CommitGraph,
    layout: &'a crate::ref_layout::RefLayout,
    anchor: ViewAnchor,
) -> LayoutIndexes<'a> {
    let placed: HashMap<&gix::refs::FullNameRef, gix::ObjectId> = layout
        .placements()
        .map(|(name, on)| (name.as_ref(), on))
        .collect();
    let mut pos_by_name = PosByName::new();
    for (name, facts) in &layout.facts {
        pos_by_name.insert(name.as_ref(), (facts, placed.get(name.as_ref()).copied()));
    }
    let mut naming_at = HashMap::<gix::ObjectId, &gix::refs::FullName>::new();
    let mut names_empty = HashSet::<&gix::refs::FullNameRef>::new();
    let mut projected_at =
        HashMap::<gix::ObjectId, (&gix::refs::FullName, Vec<StackCommit>)>::new();
    let in_ws = |id: gix::ObjectId| {
        cg.node(id)
            .is_some_and(|n| n.flags.contains(crate::CommitFlags::InWorkspace))
    };
    for (name, facts) in layout
        .facts
        .iter()
        .filter(|(name, facts)| facts.names_segment && !is_implementation_ref(cg, name.as_ref()))
    {
        if facts.names_empty_segment {
            names_empty.insert(name.as_ref());
            continue;
        }
        let Some(pos_on) = placed.get(name.as_ref()).copied() else {
            continue;
        };
        if name.category() == Some(gix::reference::Category::RemoteBranch)
            && anchor.managed_commit()
        {
            // Remote positions never carve a WORKSPACE stack's runs — locals name
            // segments, remotes only pair up as sidebands. An ad-hoc stack without
            // local names does read them.
            continue;
        }
        if in_ws(pos_on) || name.category() != Some(gix::reference::Category::LocalBranch) {
            naming_at.insert(pos_on, name);
            continue;
        }
        let mut outside = Vec::new();
        let mut cursor = Some(pos_on);
        while let Some(id) = cursor {
            let Some(node) = cg.node(id) else { break };
            if node.flags.contains(crate::CommitFlags::InWorkspace) {
                projected_at.entry(id).or_insert((name, outside));
                break;
            }
            if node.flags.contains(crate::CommitFlags::Integrated) {
                // Integrated territory below the workspace is not an advanced tip —
                // nothing projects from there.
                break;
            }
            let mut commit = StackCommit::from_graph_commit(node);
            strip_structural_refs(&mut commit, Some(name.as_ref()), &names_empty);
            outside.push(commit);
            cursor = cg.all_parent_ids(id).first().copied();
        }
        // A ref whose line never reaches the workspace stays where it is — the walk
        // may still meet it directly at a run top.
        naming_at.insert(pos_on, name);
    }
    LayoutIndexes {
        pos_by_name,
        naming_at,
        names_empty,
        projected_at,
    }
}

/// Ad-hoc entries keep only order-mates at or below the entry in a persisted
/// ad-hoc stack order, sorted to that order; without one, ambiguous same-commit
/// peers don't materialize — only the entry itself does.
fn retain_ordered_after_entry(
    names: &mut Vec<&gix::refs::FullName>,
    ctx: &GraphContext,
    entry_ref: Option<&gix::refs::FullName>,
) {
    if let Some((order, ei)) = entry_ref.and_then(|entry| {
        ctx.ad_hoc_branch_stack_orders.iter().find_map(|order| {
            order
                .iter()
                .position(|n| n.as_ref() == entry.as_ref())
                .map(|ei| (order, ei))
        })
    }) {
        let pos_of = |n: &gix::refs::FullNameRef| order.iter().position(|m| m.as_ref() == n);
        names.retain(|n| pos_of(n.as_ref()).is_some_and(|i| i >= ei));
        names.sort_by_key(|n| pos_of(n.as_ref()).unwrap_or(usize::MAX));
    } else {
        names.retain(|n| Some(n.as_ref()) == entry_ref.map(|e| e.as_ref()));
    }
}

/// Old ad-hoc semantics at the bound: a run STARTING on the bound with the
/// entrypoint ref really placed there walks through, and a run reaching a bound
/// whose naming branch sits BELOW the entry branch in a persisted ad-hoc stack
/// order includes that territory too.
fn entry_walks_through(
    ctx: &GraphContext,
    layout: &crate::ref_layout::RefLayout,
    pos_by_name: &PosByName<'_>,
    entry_ref: Option<&gix::refs::FullName>,
    id: gix::ObjectId,
    run_start: Option<gix::ObjectId>,
) -> bool {
    Some(id) == run_start
        && entry_ref.is_some_and(|ep| {
            pos_by_name.get(ep.as_ref()).is_some_and(|&(facts, on)| {
                facts.names_segment && !facts.names_empty_segment && on == Some(id)
            })
        })
        || (!ctx.ad_hoc_branch_stack_orders.is_empty()
            && entry_ref.is_some_and(|entry| {
                // Any positioned ref ON the bound counts — a run continues
                // into ordered territory whether its branch is empty or not.
                layout
                    .segment_naming_placements()
                    .filter(|&(_, on)| on == id)
                    .any(|(r, _)| {
                        ctx.ad_hoc_branch_stack_orders.iter().any(|order| {
                            let ei = order.iter().position(|n| n.as_ref() == entry.as_ref());
                            let bi = order.iter().position(|n| n.as_ref() == r.as_ref());
                            ei.zip(bi).is_some_and(|(ei, bi)| ei < bi)
                        })
                    })
            }))
}

/// Whether `to` is reachable from `from` along any parent line.
fn reaches(cg: &CommitGraph, from: gix::ObjectId, to: gix::ObjectId) -> bool {
    let mut seen = HashSet::new();
    let mut queue = vec![from];
    while let Some(c) = queue.pop() {
        if c == to {
            return true;
        }
        if seen.insert(c) {
            queue.extend(cg.all_parent_ids(c));
        }
    }
    false
}

/// The next commit on a run: the first parent, except a merge steers toward the
/// entrypoint when a parent line reaches it (`reaches_entrypoint` is the precomputed
/// answer; empty when there is no entrypoint).
fn next_parent(
    cg: &CommitGraph,
    id: gix::ObjectId,
    reaches_entrypoint: &[bool],
) -> Option<gix::ObjectId> {
    let parents = cg.all_parent_ids(id);
    if parents.len() > 1
        && let Some(p) = parents.iter().find(|p| {
            cg.index_of(**p)
                .is_some_and(|i| reaches_entrypoint.get(i).copied().unwrap_or_default())
        })
    {
        return Some(*p);
    }
    parents.first().copied()
}

/// Refs consumed as structure stay off the commit: the segment's own naming ref,
/// refs naming empty segments, remote-category refs, and gitbutler/* refs.
fn strip_structural_refs(
    commit: &mut StackCommit,
    own_name: Option<&gix::refs::FullNameRef>,
    names_empty: &HashSet<&gix::refs::FullNameRef>,
) {
    commit.refs.retain(|ri| {
        let name = ri.ref_name.as_ref();
        use gix::reference::Category;
        name.category() != Some(Category::RemoteBranch)
            && !in_gitbutler_namespace(name)
            && Some(name) != own_name
            && !names_empty.contains(name)
    });
}

pub(crate) fn named_segment(
    name: gix::refs::FullName,
    commit_id: Option<gix::ObjectId>,
    ctx: &GraphContext,
) -> StackSegment {
    StackSegment {
        remote_tracking_ref_name: ctx.remote_tracking.get(&name).cloned(),
        ref_info: Some(RefInfo {
            ref_name: name,
            commit_id,
            worktree: None,
        }),
        ..Default::default()
    }
}
