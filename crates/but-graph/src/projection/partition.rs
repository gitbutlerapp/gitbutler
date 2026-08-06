//! The partition engine: the workspace's single producer.
//!
//! Metadata DECLARES a partition of branches into stacks
//! ([`DeclaredStack`](crate::ref_layout::DeclaredStack)); this DERIVES one from the graph, and
//! the two are the same kind of answer arrived at from opposite ends. That is the whole job, and
//! the reason to name it for what it produces rather than for the criterion it applies.
//!
//! It is one graph colouring. Colour each
//! commit for the stack tip that owns it and the partition falls out of the graph itself, so
//! stackhood is a graph fact — seeds converging above the integration base are one multi-tip
//! stack — rather than something reconstructed afterwards from stored shape.
//!
//! The engine is TOTAL: every view it is handed yields stacks. A layoutless graph, an anchorless
//! arena, a merge whose every parent rests on the bound — each still names what it holds, because
//! a caller that could be told "nothing" would need a second producer to cover the difference.
//!
//! READING ORDER: [`derive_partition`] is the table of contents — the driver reads as its
//! phases, in execution order. [`Derivation`] carries the view's facts and answers every
//! phase's questions; its methods appear in the order the driver asks them. The free
//! functions after it are the phase helpers the driver calls directly, and
//! [`expand_lanes`] at the bottom is a separate concern entirely: the display shim that
//! re-materializes today's duplicated-lane UI from the stored multi-tip shape.

use std::collections::{HashMap, HashSet};

use crate::CommitGraph;
use crate::workspace::GraphContext;
use but_core::ref_metadata;

use super::anchor::{ViewAnchor, index_layout, named_segment, resolve_view_anchor};
use super::stack::{Stack, StackSegment};
// ── The driver: the derivation, phase by phase ──

/// THE PARTITION ENGINE, for every kind of view — managed, managed-ref-only and ad-hoc alike.
///
/// Union-colour the tips into classes, then emit ONE [`Stack`] per class: each tip's EXCLUSIVE
/// lane (commits it alone reaches) above the convergence point, plus the SHARED tail (commits
/// ≥2 tips reach) stored ONCE, with each lane's bottom edging into the shared tail's top. The
/// duplicated-lane view the UI shows is derived from that per tip by first-parent traversal and
/// never stored; `branch_parents` carries the convergence. Enrichment — metadata, remote and
/// entrypoint — rides the shared name-keyed passes afterwards.
pub(crate) fn derive_partition(
    cg: &CommitGraph,
    ctx: &GraphContext,
    ws_meta: Option<&ref_metadata::Workspace>,
    has_managed_ref: bool,
    entry_ref: Option<&gix::refs::FullName>,
    // What the view is OF, which the entry alone cannot say: HEAD can sit on an ordinary branch
    // while the view is still of the workspace ref.
    tip_ref: Option<&gix::refs::FullName>,
    lower_bound: Option<gix::ObjectId>,
) -> Vec<Stack> {
    let Some(layout) = cg.layout() else {
        // A LAYOUTLESS graph — an unborn ref — has no refs to place and no commits to colour, but
        // it is still a view: the tip names one empty stack. Producing it here rather than
        // declining keeps the engine total, so no caller needs a second producer for the shape
        // where there is simply nothing to derive.
        return lone_empty_stack(ctx, tip_ref);
    };
    // No anchor means no entrypoint and no positioned ref to stand on — an arena the editor built
    // without one. There is nothing to colour, but the tip still names its view, exactly as for a
    // layoutless graph. Declining here instead would hand the caller an empty workspace with no way
    // to tell "nothing to show" from "the producer gave up".
    let Some((ws_commit, amended)) = resolve_view_anchor(cg, layout, ws_meta, entry_ref) else {
        return lone_empty_stack(ctx, tip_ref);
    };
    let anchor = ViewAnchor::new(
        cg.is_managed_ws_commit(ws_commit) && ws_meta.is_some(),
        has_managed_ref,
    );

    let d = Derivation::new(
        cg,
        ctx,
        ws_meta,
        layout,
        anchor,
        entry_ref,
        tip_ref,
        lower_bound,
        ws_commit,
        amended,
    );
    let in_ws_stacks = &d.in_ws_stacks;
    let (anchor, ad_hoc) = (d.anchor, d.ad_hoc);

    let tips = seed_tips(cg, layout, in_ws_stacks, anchor, ad_hoc, ws_commit);
    let reachable = d.reach(&tips);
    let mut parent = converge(&tips, &reachable, lower_bound);

    // Group tips into classes, in parent order (a class ranks by its earliest member tip).
    let ordered_classes = class_members(&mut parent, tips.len());

    // Per class, the territory it EXCLUSIVELY reaches. Shared/base commits belong to nobody here:
    // ownership is only meaningful where exactly one class can reach, which is what makes it safe
    // to home a stray empty branch by its anchor.
    let class_reach: Vec<HashSet<gix::ObjectId>> = ordered_classes
        .iter()
        .map(|members| {
            members
                .iter()
                .flat_map(|&m| reachable[m].iter().copied())
                .collect()
        })
        .collect();

    // Chains a class speaks for; an empty declared by any OTHER chain is homeless and looks for
    // the stack whose exclusive territory its anchor lands in.
    let claimed_chains: HashSet<usize> = ordered_classes
        .iter()
        .flat_map(|members| members.iter().filter_map(|&m| d.chain_of(&reachable, m)))
        .collect();

    let mut stacks: Vec<(usize, usize, usize, Stack)> = Vec::new();
    // Which declared chain each lane represents, by chain index — identity is the chain's
    // position in the declared partition. `worn_ids` tracks, separately, which META stack ids
    // some lane already wears: the id is an output stamp, not identity, and its bookkeeping
    // only exists so the adoption pass never hands out a worn stamp twice.
    let mut lane_of_chain: HashMap<usize, usize> = HashMap::new();
    let mut worn_ids: HashSet<ref_metadata::StackId> = HashSet::new();
    for (class_idx, members) in ordered_classes.iter().enumerate() {
        let Some((key, parent_idx, declared, declared_idx, stack)) = d.collect_class(
            class_idx,
            members,
            &tips,
            &reachable,
            &claimed_chains,
            &class_reach,
        ) else {
            continue;
        };
        if let Some(ci) = declared_idx {
            lane_of_chain.insert(ci, stacks.len());
        }
        if let Some(id) = stack.id {
            worn_ids.insert(id);
        }
        stacks.push((key, parent_idx, declared, stack));
    }

    // Empty-only stacks: declared chains no class represents, resting on the bound.
    let mut projected: HashSet<gix::refs::FullName> = stacks
        .iter()
        .flat_map(|(_, _, _, s)| s.segments.iter())
        .filter_map(|seg| seg.ref_name().map(ToOwned::to_owned))
        .collect();
    d.adopt_identity(&mut stacks, &mut lane_of_chain, &mut worn_ids);
    d.settle_represented(&mut stacks, &lane_of_chain, &mut projected);
    d.empty_only_lanes(&mut stacks, &lane_of_chain, &projected);
    // LANE ORDER: the amended list's position where it places a lane, then REAL PARENT order — which
    // is authoritative (ruling 4) and covers empty lanes too, since an empty resting on a real
    // parent occupies that parent's position — then the declared index, which is all a lane that is no
    // parent at all has to go on.
    // A view of a plain checkout ALWAYS has its one lane, even when nothing named or collected
    // anything — a checkout of fully integrated territory is still a checkout, and an anonymous
    // shell says so honestly.
    if ad_hoc && stacks.is_empty() {
        stacks.push((
            usize::MAX,
            usize::MAX,
            usize::MAX,
            Stack {
                id: matches!(anchor, ViewAnchor::AdHoc)
                    .then(ref_metadata::StackId::single_branch_id),
                segments: vec![StackSegment::default()],
                branch_parents: None,
            },
        ));
    }
    // NAME WHAT THE MERGE HOLDS (see but-workspace's crate docs). Every parent of the merge is an
    // applied branch, but a parent resting ON the bound owns no commits, so the walk finds nothing
    // to give it and the view comes out holding branches while reporting none. Naming the parents
    // keeps "applied" and "shown" the same fact — without it the branch is invisible to everything
    // downstream, which is how an applied branch that had landed survived `but pull`.
    //
    // This is about VISIBILITY, not emptiness. It does not promise a lane: where nothing names a
    // parent the view stays empty, which a workspace emptied before that became impossible, or a
    // walk truncated by a limit, both legitimately are.
    if !ad_hoc && stacks.is_empty() {
        let sole = d.amended.len() == 1;
        for (position, parent) in d.amended.iter().enumerate() {
            let Some(name) = d.namer_at(*parent) else {
                continue;
            };
            let mut seg = named_segment(name.clone(), None, ctx);
            seg.base = Some(*parent);
            stacks.push((
                position,
                position,
                usize::MAX,
                Stack::from_base_and_segments_raw(
                    vec![seg],
                    sole.then(ref_metadata::StackId::single_branch_id),
                ),
            ));
        }
    }
    // LANE ORDER, after the amended list's position has its say. Real parent order is authoritative
    // (ruling 4) — but it can only order lanes that ARE parents. The moment any lane is not one
    // (an empty branch inserted between lanes, say), parent positions no longer cover the set and the
    // DECLARED order is the only one that ranks every lane; parent positions then break its ties.
    let all_are_parents = stacks
        .iter()
        .all(|(_, position, _, _)| *position != usize::MAX);
    if all_are_parents {
        stacks.sort_by_key(|(key, position, declared, _)| (*key, *position, *declared));
    } else {
        // METADATA ORDER IS AUTHORITATIVE once any lane is not a merge parent. An empty branch has
        // no commits and no parent position, so the merge cannot carry a reorder of it — the
        // declaration is the only place that intent can live, and reading position back off the
        // amended list instead made an emptied branch jump ahead of a sibling that still has
        // commits. A lane the declaration does NOT rank (an anonymous merge parent, seen
        // mid-operation) keeps its place ahead of the declared sequence, ordered among its own
        // kind by parent position.
        stacks.sort_by_key(|(key, position, declared, _)| {
            (*declared != usize::MAX, *declared, *key, *position)
        });
    }
    stacks.into_iter().map(|(_, _, _, s)| s).collect()
}

// ── The context: the view's facts, and the questions every phase asks ──

/// The derivation's shared context: the view's facts and the predicates every phase asks,
/// built once after the trivial views are answered. Phase state that grows during the
/// derivation (tips, reachability, classes, the produced stacks) stays in the driver and
/// is passed to the methods that need it.
/// A produced lane with its three sort keys (amended position, parent position, declared
/// index).
type KeyedStack = (usize, usize, usize, Stack);

struct Derivation<'a> {
    cg: &'a CommitGraph,
    ws_meta: Option<&'a ref_metadata::Workspace>,
    ctx: &'a GraphContext,
    layout: &'a crate::ref_layout::RefLayout,
    idx: super::anchor::LayoutIndexes<'a>,
    /// The declared in-workspace partition; chain identity is the index here.
    in_ws_stacks: Vec<&'a crate::ref_layout::DeclaredStack>,
    lower_bound: Option<gix::ObjectId>,
    /// The target ref, which nothing may rest on or be named by.
    suppress_target: Option<gix::refs::FullName>,
    /// Membership is the InWorkspace flag only where a merge DEFINES it (see
    /// [`Self::in_view`]).
    require_in_ws: bool,
    /// A branch resting ON the bound is still a member; see [`Self::in_cone`].
    entry_rests_on_bound: bool,
    /// First-parent steering: which commits reach the effective entrypoint.
    reaches: Vec<bool>,
    anchor: ViewAnchor,
    ad_hoc: bool,
    detached: bool,
    entry_ref: Option<&'a gix::refs::FullName>,
    /// The target's LOCAL counterpart — never anybody's floor.
    target_local: Option<gix::refs::FullName>,
    /// Chain `i`'s OUTPUT stamp: the i-th in-workspace metadata stack id.
    meta_ids: Vec<ref_metadata::StackId>,
    /// The recorded lane sequence (amended workspace parents).
    amended: Vec<gix::ObjectId>,
    /// The merge's parent list — the authority on lane order.
    merge_parents: Vec<gix::ObjectId>,
    /// A PLAIN CHECKOUT is its own view: standing on an ordinary branch, that branch IS
    /// the lane, and leftover declared stacks do not get to wear its identity. Only a
    /// view OF the workspace ref hands the lane to the declaration — and the entry alone
    /// cannot say which, since HEAD may sit on an ordinary branch while the view is
    /// still of the workspace, so this is decided from the TIP.
    checkout_defines_lane: bool,
}

impl<'a> Derivation<'a> {
    /// Assemble the view's context: index the layout, reduce the declaration, and settle
    /// every fact the phases will ask.
    #[allow(clippy::too_many_arguments)]
    fn new(
        cg: &'a CommitGraph,
        ctx: &'a GraphContext,
        ws_meta: Option<&'a ref_metadata::Workspace>,
        layout: &'a crate::ref_layout::RefLayout,
        anchor: ViewAnchor,
        entry_ref: Option<&'a gix::refs::FullName>,
        tip_ref: Option<&'a gix::refs::FullName>,
        lower_bound: Option<gix::ObjectId>,
        ws_commit: gix::ObjectId,
        amended: Vec<gix::ObjectId>,
    ) -> Self {
        let idx = index_layout(cg, layout, anchor);
        let in_ws_stacks: Vec<&crate::ref_layout::DeclaredStack> = layout.stacks.iter().collect();
        // The OUTPUT stamp for chain `i`: the metadata id of the i-th in-workspace stack — the
        // layout's partition preserves that order. Identity inside the derivation is the chain
        // index; the id exists only to be worn by the produced lane at the boundary.
        let meta_ids: Vec<ref_metadata::StackId> = ws_meta
            .map(|ws| {
                ws.stacks
                    .iter()
                    .filter(|s| s.is_in_workspace())
                    .map(|s| s.id)
                    .collect()
            })
            .unwrap_or_default();
        // The target lives on the PROJECT metadata, not the workspace metadata — the frame reads it
        // from the same place.
        let target = ctx.project_meta.target_ref.clone();
        // AD-HOC rules the managed view never needs. A DETACHED checkout does not speak for the
        // branches at its commit: its start stays anonymous and those refs ride the commit as
        // decorations. And a view with no declared chain lets REMOTE refs name segments, where a chain
        // keeps its own names — so a remote namer is only silenced when a chain speaks here.
        let ad_hoc = !matches!(anchor, ViewAnchor::ManagedCommit);
        let detached = cg.seeds.iter().any(|t| t.is_entrypoint && t.is_detached);
        let suppress_target = target.clone();
        // The target's LOCAL counterpart — `main` to `origin/main`. Nothing may come to rest ON it by
        // accident: standing anywhere above the base would otherwise pick it up as the segment
        // underneath, reading as "your branch is stacked on main" when the user never said so. It can
        // still be a lane of its own; what it cannot be is somebody else's floor.
        // Sourced from the PROJECT, not the workspace: an ad-hoc view carries no workspace metadata,
        // yet it still has a target — and it is exactly the view where a stray floor shows up.
        let target_local: Option<gix::refs::FullName> = suppress_target
            .clone()
            .or_else(|| ctx.project_meta.target_ref.clone())
            .and_then(|t| {
                ctx.remote_tracking
                    .iter()
                    .find_map(|(local, remote)| (*remote == t).then(|| local.clone()))
            });

        // MEMBERSHIP OF THIS VIEW — one predicate, asked by everything that needs it.
        //
        // `CommitFlags::InWorkspace` answers a narrower question than its name suggests: it is stamped
        // from the workspace MERGE (`apply_posthoc_flags` seeds it from `SeedRole::Workspace`), so it
        // means "inside that merge's cone", not "inside what this view shows". Where no merge stamped
        // it — a plain checkout, or a merge not yet rebuilt mid-apply — the flag is on nothing, and
        // demanding it excludes every commit: the projection comes out empty while metadata declares
        // stacks.
        //
        // So membership is the flag only where a merge DEFINES it, and unconditional otherwise. Do NOT
        // "fix" this by seeding the flag from the entrypoint instead: that makes a detached HEAD's
        // commits claim `🏘` membership in a workspace that does not exist (measured — 110 snapshots
        // move, asserting exactly that). The two questions are genuinely different; this names the one
        // the projection actually asks, so the three sites that need it stop each inventing a dodge.
        let require_in_ws =
            matches!(anchor, ViewAnchor::ManagedCommit) && !cg.all_parent_ids(ws_commit).is_empty();

        // The bound is normally where a lane stops. But when the ENTRY REF itself rests there naming a
        // real segment, the checkout IS that commit — the view would otherwise show nothing of what is
        // checked out — so the lane includes it. Its parents lie below the bound and still stop the walk.
        // The bound is the floor for DISCOVERING content — nothing below it belongs to a lane — but it is
        // not a wall against members that live exactly there. A branch resting ON it is still a member of
        // its stack: the ENTRY, because the checkout IS that commit, and any branch the workspace
        // DECLARES, because an applied branch has to be projected. Excluding them is what left the
        // projection incomplete and made a rescue pass look necessary.
        let rests_on_bound_pre = |name: &gix::refs::FullNameRef| {
            lower_bound.is_some_and(|bound| {
                layout
                    .facts_of(name)
                    .is_some_and(|f| f.names_segment && !f.names_empty_segment)
                    && layout.positioned_on(name) == Some(bound)
            })
        };
        let entry_rests_on_bound = entry_ref.is_some_and(|e| rests_on_bound_pre(e.as_ref()))
            || in_ws_stacks
                .iter()
                .any(|m| m.branches.iter().any(|b| rests_on_bound_pre(b.as_ref())));
        // First-parent steering target.
        let effective_entrypoint = entry_ref
            .and_then(|e| layout.positioned_on(e.as_ref()))
            .or_else(|| cg.entrypoint());
        let merge_parents = if matches!(anchor, ViewAnchor::ManagedCommit) {
            cg.all_parent_ids(ws_commit)
        } else {
            Vec::new()
        };
        Self {
            cg,
            ws_meta,
            ctx,
            layout,
            idx,
            in_ws_stacks,
            lower_bound,
            anchor,
            ad_hoc,
            detached,
            entry_ref,
            target_local,
            meta_ids,
            amended,
            merge_parents,
            checkout_defines_lane: anchor == ViewAnchor::AdHoc
                && tip_ref.is_some_and(|t| !crate::ref_layout::in_gitbutler_namespace(t.as_ref())),
            suppress_target: suppress_target.clone(),
            require_in_ws,
            entry_rests_on_bound,
            reaches: effective_entrypoint
                .map(|ep| cg.reaches_marks(ep))
                .unwrap_or_default(),
        }
    }

    // ── Predicates: membership, bounds, naming, and first-parent steering. ──

    /// The local branch naming `id` — layout naming rights first, with the DECLARATION
    /// naming what the merge does not reach yet (the other half of the fresh-connection
    /// rule): first in declaration order names it, siblings surface as empties.
    fn namer_at(&self, id: gix::ObjectId) -> Option<&'a gix::refs::FullName> {
        self.idx
            .naming_at
            .get(&id)
            .filter(|&&n| Some(n) != self.suppress_target.as_ref())
            .copied()
            .or_else(|| {
                let unreached = self
                    .cg
                    .node(id)
                    .is_some_and(|n| !n.flags.contains(crate::CommitFlags::InWorkspace));
                unreached
                    .then(|| {
                        self.in_ws_stacks
                            .iter()
                            .flat_map(|m| m.branches.iter())
                            .find(|b| {
                                Some(*b) != self.suppress_target.as_ref()
                                    && self.layout.positioned_on(b.as_ref()) == Some(id)
                            })
                    })
                    .flatten()
            })
    }

    /// MEMBERSHIP OF THIS VIEW — the flag only where a merge defines it, unconditional
    /// otherwise. `CommitFlags::InWorkspace` means "inside the merge's cone", not "inside
    /// what this view shows"; where no merge stamped it, demanding it empties the view.
    fn in_view(&self, id: gix::ObjectId) -> bool {
        self.cg.node(id).is_some_and(|n| {
            !self.require_in_ws || n.flags.contains(crate::CommitFlags::InWorkspace)
        })
    }

    /// The bound is the floor for DISCOVERING content, not a wall against members living
    /// exactly there: the entry (the checkout IS that commit) and declared branches
    /// resting on it stay members.
    fn in_cone(&self, id: gix::ObjectId) -> bool {
        self.in_view(id)
            && self
                .cg
                .node(id)
                .is_some_and(|n| !n.flags.contains(crate::CommitFlags::BelowBound))
            && (Some(id) != self.lower_bound || self.entry_rests_on_bound)
    }

    /// Does this ref head a real segment of its own — a commit no sibling took, that the
    /// walk can collect? Then some class projects it, and no other pass has to.
    fn names_own_segment(&self, name: &gix::refs::FullNameRef) -> bool {
        self.layout
            .facts_of(name)
            .is_some_and(|f| f.names_segment && !f.names_empty_segment)
    }

    fn rests_on_bound(&self, name: &gix::refs::FullNameRef) -> bool {
        self.lower_bound.is_some_and(|bound| {
            self.names_own_segment(name) && self.layout.positioned_on(name) == Some(bound)
        })
    }

    /// First-parent, steered toward the entrypoint at merges.
    fn next_fp(&self, id: gix::ObjectId) -> Option<gix::ObjectId> {
        let ps = self.cg.all_parent_ids(id);
        if ps.len() > 1
            && let Some(p) = ps.iter().find(|p| {
                self.cg
                    .index_of(**p)
                    .is_some_and(|i| self.reaches.get(i).copied().unwrap_or_default())
            })
        {
            return Some(*p);
        }
        ps.first().copied()
    }

    fn anchor_of(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.layout.positioned_on(name)
    }

    /// Per-tip reachability (bounded at the base) — the convergence relation.
    fn reach(&self, tips: &[gix::ObjectId]) -> Vec<HashSet<gix::ObjectId>> {
        tips.iter()
            .map(|&t| {
                let mut seen = HashSet::new();
                let mut q = vec![t];
                while let Some(id) = q.pop() {
                    if !self.in_cone(id) || !seen.insert(id) {
                        continue;
                    }
                    q.extend(self.cg.all_parent_ids(id));
                }
                seen
            })
            .collect()
    }

    // ── Lane order and ownership. ──

    /// THE LANE ORDER, from recorded intent: `amended` lists the workspace's parents in
    /// CHAIN order — real parents plus one entry per empty chain — the declared lane
    /// sequence. Anything unlisted keeps discovery order at the end.
    fn lane_pos(&self, anchor: Option<gix::ObjectId>) -> usize {
        anchor
            .and_then(|a| self.amended.iter().position(|p| *p == a))
            .unwrap_or(usize::MAX)
    }

    /// A lane's PARENT POSITION: where the workspace merge lists the commit it hangs off —
    /// a graph fact, and the authority on lane order (unlike the recorded intent, always
    /// available).
    fn parent_position(&self, on: Option<gix::ObjectId>) -> usize {
        on.and_then(|on| self.merge_parents.iter().position(|p| *p == on))
            .unwrap_or(usize::MAX)
    }

    /// The one class that reaches `on` EXCLUSIVELY — shared/base commits belong to nobody,
    /// which is what makes it safe to home a stray empty branch by its anchor.
    fn exclusive_owner(
        &self,
        class_reach: &[HashSet<gix::ObjectId>],
        on: gix::ObjectId,
    ) -> Option<usize> {
        let mut found = None;
        for (ci, reach) in class_reach.iter().enumerate() {
            if reach.contains(&on) {
                if found.is_some() {
                    return None;
                }
                found = Some(ci);
            }
        }
        found
    }

    /// THE PRESENCE RULE — an empty surfaces only on incorporated territory, judged by
    /// the same membership question everything else asks.
    fn surfaces(&self, on: Option<gix::ObjectId>) -> bool {
        let terr = super::presence::Territory::of(self.cg, on, |id| self.in_view(id));
        super::presence::leftover_presence(
            terr,
            true,
            on.is_some() && on == self.lower_bound,
            false,
        )
        .surface
    }

    /// The declared chain owning tip `m`: the first whose positioned branch the tip
    /// reaches — unless the checkout defines the lane, where no declaration speaks.
    fn chain_of(&self, reachable: &[HashSet<gix::ObjectId>], m: usize) -> Option<usize> {
        if self.checkout_defines_lane {
            return None;
        }
        self.in_ws_stacks.iter().position(|c| {
            c.branches.iter().any(|b| {
                self.anchor_of(b.as_ref())
                    .is_some_and(|on| reachable[m].contains(&on))
            })
        })
    }

    // ── Segmentation and per-class collection. ──

    /// Segment a linear run of commit ids by the clean naming rule, splicing the class's
    /// empty branches at their anchors — splice-and-absorb — as the run passes them. A
    /// detached HEAD's own start names nothing; a chain silences remote namers. The
    /// NAMING CUT: a ref whose tip advanced OUTSIDE the workspace is legitimately
    /// outside — its name does not project back in, so the in-workspace ancestor stays
    /// anonymous here.
    fn segment_run(
        &self,
        ids: &[gix::ObjectId],
        pending: &mut Vec<(&'a gix::refs::FullName, gix::ObjectId)>,
        emitted: &mut HashSet<gix::refs::FullName>,
        detached_start: Option<gix::ObjectId>,
        chain_speaks: bool,
    ) -> Vec<StackSegment> {
        let namer_at = |id: gix::ObjectId| -> Option<&gix::refs::FullName> {
            if detached_start == Some(id) {
                return None;
            }
            self.namer_at(id).filter(|n| {
                !chain_speaks || n.category() != Some(gix::reference::Category::RemoteBranch)
            })
        };
        let mut segs: Vec<StackSegment> = Vec::new();
        let mut seg = StackSegment::default();
        for &id in ids {
            // Empties anchored here splice above; the bottom-most absorbs an unnamed commit.
            let mut here: Vec<gix::refs::FullName> = Vec::new();
            pending.retain(|&(name, on)| {
                if on == id && emitted.insert(name.clone()) {
                    here.push(name.clone());
                    false
                } else {
                    on != id
                }
            });
            if !here.is_empty() {
                if seg.ref_name().is_some() || !seg.commits.is_empty() {
                    seg.base = Some(id);
                    segs.push(std::mem::take(&mut seg));
                }
                let absorber = (namer_at(id).is_none()).then(|| here.pop()).flatten();
                for name in here {
                    segs.push(named_segment(name, None, self.ctx));
                }
                if let Some(name) = absorber {
                    if seg.ref_name().is_some() || !seg.commits.is_empty() {
                        segs.push(std::mem::take(&mut seg));
                    }
                    seg = named_segment(name, Some(id), self.ctx);
                }
            }
            if let Some(name) = namer_at(id)
                && seg.ref_name() != Some(name.as_ref())
                && !emitted.contains(name)
            {
                if seg.ref_name().is_some() || !seg.commits.is_empty() {
                    seg.base = Some(id);
                    segs.push(std::mem::take(&mut seg));
                }
                seg = named_segment(name.clone(), Some(id), self.ctx);
                emitted.insert(name.clone());
                pending.retain(|&(n, _)| n != name);
            }
            // Join: the commit belongs to the segment being built. Structure only — which
            // refs a commit shows, and in what order, is DECORATION, and every caller of
            // the derivation reduces it to ids before anyone can see it.
            if let Some(node) = self.cg.node(id) {
                seg.commits
                    .push(super::stack::StackCommit::from_graph_commit(node));
            }
        }
        if seg.ref_name().is_some() || !seg.commits.is_empty() {
            segs.push(seg);
        }
        segs
    }
    /// One class becomes one stack: gather its empties (declared, ad-hoc, and homeless),
    /// collect its runs — first-parent chains, with a merge's declared legs queued as
    /// their own runs and anonymous legs absorbed — capture the shared tail once, settle
    /// leftovers at the bottom, and stamp identity from the topmost named segment.
    /// Returns the sort keys, the declared chain index (for the driver's chain→lane
    /// bookkeeping), and the stack; `None` when nothing surfaced.
    fn collect_class(
        &self,
        class_idx: usize,
        members: &[usize],
        tips: &[gix::ObjectId],
        reachable: &[HashSet<gix::ObjectId>],
        claimed_chains: &HashSet<usize>,
        class_reach: &[HashSet<gix::ObjectId>],
    ) -> Option<(usize, usize, usize, Option<usize>, Stack)> {
        // Shared = commits ≥2 members reach.
        let mut count: HashMap<gix::ObjectId, usize> = HashMap::new();
        for &m in members {
            for &c in &reachable[m] {
                *count.entry(c).or_default() += 1;
            }
        }
        let shared: HashSet<gix::ObjectId> = count
            .iter()
            .filter(|(_, n)| **n >= 2)
            .map(|(c, _)| *c)
            .collect();

        // The class's empty branches (from every member chain), deduped, target excluded.
        let mut pending: Vec<(&gix::refs::FullName, gix::ObjectId)> = Vec::new();
        for &m in members {
            if let Some(ci) = self.chain_of(reachable, m) {
                for b in &self.in_ws_stacks[ci].branches {
                    if Some(b) != self.suppress_target.as_ref()
                        && self.idx.names_empty.contains(b.as_ref())
                        && !pending.iter().any(|(n, _)| *n == b)
                        && let Some(on) = self.anchor_of(b.as_ref())
                    {
                        pending.push((b, on));
                    }
                }
            }
        }
        // AD-HOC views declare no chains, so the loop above finds nothing — yet a plain checkout
        // still has empty branches: refs resting where no commit of their own sits. They come from
        // the LAYOUT rather than from metadata, and the ones sharing the lane's own tip are filtered
        // and ordered by the persisted ad-hoc order (which of several refs at one commit names the
        // segment, and which surface above it).
        if self.ad_hoc {
            let mut at_tip: Vec<&gix::refs::FullName> = Vec::new();
            for (name, facts) in &self.layout.facts {
                // Only LOCAL branches rest as empty segments. A remote ref is a sideband on the
                // branch it tracks, never a lane of its own.
                if !facts.names_segment
                    || !facts.names_empty_segment
                    || name.category() != Some(gix::reference::Category::LocalBranch)
                    || Some(name) == self.suppress_target.as_ref()
                    || crate::ref_layout::in_gitbutler_namespace(name.as_ref())
                    || pending.iter().any(|(n, _)| *n == name)
                {
                    continue;
                }
                let Some(on) = self.anchor_of(name.as_ref()) else {
                    continue;
                };
                // Territory of this lane, or the integration bound itself: a branch resting ON the
                // bound belongs to the lane too and settles at its bottom, but the cone stops
                // before the bound so reachability alone would never find it.
                let in_territory = members.iter().any(|&m| reachable[m].contains(&on))
                    || Some(on) == self.lower_bound;
                if !in_territory {
                    continue;
                }
                if members.iter().any(|&m| tips[m] == on) {
                    at_tip.push(name);
                } else if Some(name) != self.target_local.as_ref() {
                    // BELOW the tip is the floor position, and the target's local branch never
                    // takes it by accident: standing above the base would otherwise render as
                    // "stacked on main" when the user never stacked anything. Resting AT the tip is
                    // different — there it names its own lane, which it is still entitled to do.
                    pending.push((name, on));
                }
            }
            // A CHAINLESS view starts AT its entry: everything above the entry in the persisted
            // order is out of view, and without an order only the entry itself materializes. Where
            // a declared chain speaks — a degraded workspace, whose merge is missing but whose
            // stacks are still declared — its branches are not filtered by the entry at all.
            // A PLAIN CHECKOUT is filtered by its entry — the persisted ad-hoc order says what is in
            // view. That is a view with no declared stacks at all, or one whose TIP is an ordinary
            // branch: leftover declarations do not surface above the branch you are standing on. A
            // DEGRADED workspace still declares its stacks (only the merge is missing), so its
            // branches all belong to the lane. Reachability cannot tell these apart: a lane seeded
            // on the bound reaches nothing, so asking whether a chain is reachable answers 'no' for
            // both.
            if self.in_ws_stacks.is_empty() || self.checkout_defines_lane {
                super::anchor::retain_ordered_after_entry(&mut at_tip, self.ctx, self.entry_ref);
            }
            for name in at_tip {
                if let Some(on) = self.anchor_of(name.as_ref()) {
                    pending.push((name, on));
                }
            }
        }
        // Empties of chains NO class speaks for rest wherever they rest: home each one in the
        // stack whose exclusive territory holds its anchor.
        for chain in self
            .in_ws_stacks
            .iter()
            .enumerate()
            .filter(|(ci, _)| !claimed_chains.contains(ci))
            .map(|(_, c)| c)
        {
            for b in &chain.branches {
                if Some(b) != self.suppress_target.as_ref()
                    && self.idx.names_empty.contains(b.as_ref())
                    && !pending.iter().any(|(n, _)| *n == b)
                    && let Some(on) = self.anchor_of(b.as_ref())
                    // Only an anchor INSIDE a stack adopts the branch. Resting on the integration
                    // bound, or on a commit that is itself some lane's SEED, is not being inside
                    // anything — several chains legitimately rest on one parent commit and each is
                    // its own lane, so they must not be swallowed into whichever class seeds there.
                    && Some(on) != self.lower_bound
                    && !tips.contains(&on)
                    && self.exclusive_owner(class_reach, on) == Some(class_idx)
                {
                    pending.push((b, on));
                }
            }
        }
        // Same-anchor empties stack in declaration order: chain index, then position within it.
        pending.sort_by_key(|(n, _)| {
            self.in_ws_stacks
                .iter()
                .enumerate()
                .find_map(|(li, c)| c.branches.iter().position(|b| b == *n).map(|bi| (li, bi)))
                .unwrap_or((usize::MAX, usize::MAX))
        });
        let mut emitted: HashSet<gix::refs::FullName> = HashSet::new();
        // Which branches this class DECLARES as its own — the members of the stack.
        // Does a declared chain speak for this class? If so it keeps its own names and remote refs
        // stay sidebands; a chainless view lets them name segments.
        let chain_speaks = members
            .iter()
            .any(|&m| self.chain_of(reachable, m).is_some());
        let declared: HashSet<&gix::refs::FullName> = members
            .iter()
            .filter_map(|&m| self.chain_of(reachable, m))
            .flat_map(|ci| self.in_ws_stacks[ci].branches.iter())
            .collect();
        // The bound is shared floor that EVERY lane reaches, so reaching it is no claim on it. A
        // class takes it as content only when it has one: the ENTRY rests there (the checkout IS
        // that commit), or a chain this class speaks for declares a branch there. Otherwise the
        // branch resting there is a member of ITS OWN stack, which is a lane in its own right.
        let claims_bound = self
            .entry_ref
            .is_some_and(|e| self.rests_on_bound(e.as_ref()))
            || declared.iter().any(|b| self.rests_on_bound(b.as_ref()));

        let mut segments: Vec<StackSegment> = Vec::new();
        let mut edges: Vec<(usize, usize)> = Vec::new();
        let mut lane_bottoms: Vec<usize> = Vec::new();
        let mut shared_run: Vec<gix::ObjectId> = Vec::new();
        // The class's exclusive commits form a DAG, not a line: a MERGE inside the stack forks it.
        // Collect run by run — each a first-parent chain — and when a merge on a run has further
        // parents in this class's own territory, queue each as its own run, edged from the merge's
        // segment. The fork shape is a GRAPH FACT (the merge's parent list); no declaration needed.
        let class_commits: HashSet<gix::ObjectId> = members
            .iter()
            .flat_map(|&m| reachable[m].iter().copied())
            .collect();
        let mut seg_of: HashMap<gix::ObjectId, usize> = HashMap::new();
        let mut spawned: Vec<(gix::ObjectId, usize)> = Vec::new();
        let mut taken: HashSet<gix::ObjectId> = HashSet::new();
        // (run start, the merge commit that forked it, if any) — members first, in order.
        let mut queue: Vec<(gix::ObjectId, Option<gix::ObjectId>)> =
            members.iter().rev().map(|&m| (tips[m], None)).collect();
        while let Some((start, forked_by)) = queue.pop() {
            let mut excl: Vec<gix::ObjectId> = Vec::new();
            let mut cur = Some(start);
            let mut shared_start = None;
            while let Some(id) = cur {
                if !self.in_cone(id)
                    || (Some(id) == self.lower_bound && !claims_bound)
                    || !class_commits.contains(&id)
                {
                    break;
                }
                if shared.contains(&id) {
                    shared_start = Some(id);
                    break;
                }
                if !taken.insert(id) {
                    break;
                }
                excl.push(id);
                // An in-stack merge forks the run: its non-spine parents are legs of this stack.
                let next = self.next_fp(id);
                for p in self.cg.all_parent_ids(id) {
                    // A merge's other parent is only this stack's own FORK LEG when the branch it
                    // leads to is one this stack DECLARES. The graph cannot tell an intentional
                    // in-stack fork from someone else's branch merged in — both are merges — so
                    // membership decides: a leg headed by a declared branch is a leg, anything
                    // else (a foreign branch, or a CATCHUP merge of integrated history) is just
                    // history this stack absorbed and never becomes a segment.
                    let integrated = self
                        .cg
                        .node(p)
                        .is_some_and(|n| n.flags.contains(crate::CommitFlags::Integrated));
                    let usable = Some(p) != next
                        && self.in_cone(p)
                        && !integrated
                        && class_commits.contains(&p)
                        && !shared.contains(&p)
                        && !taken.contains(&p);
                    if !usable {
                        continue;
                    }
                    match self.namer_at(p) {
                        // Headed by a branch this stack declares: its own leg, its own segments.
                        Some(n) if declared.contains(n) => queue.push((p, Some(id))),
                        // Headed by a FOREIGN branch: history this stack merged in, not a segment.
                        Some(_) => {}
                        // ANONYMOUS: content of this stack with no name of its own, so it has no
                        // segment to be — absorb it into the merge's segment. Only inside a
                        // DECLARED fork: where nothing is declared there is no fork to speak of,
                        // and a merge's other parents are just history it absorbed, which the spine
                        // walks past rather than collects.
                        None if !declared.is_empty() => {
                            let mut cur = Some(p);
                            while let Some(a) = cur {
                                let integrated = self.cg.node(a).is_some_and(|n| {
                                    n.flags.contains(crate::CommitFlags::Integrated)
                                });
                                if !self.in_cone(a)
                                    || integrated
                                    || self.namer_at(a).is_some()
                                    || shared.contains(&a)
                                    || !class_commits.contains(&a)
                                    || !taken.insert(a)
                                {
                                    break;
                                }
                                excl.push(a);
                                cur = self.next_fp(a);
                            }
                        }
                        // Nothing declared: no fork here, so the spine walks past this parent.
                        None => {}
                    }
                }
                cur = next;
            }
            // A self.detached checkout keeps ITS OWN start anonymous — only the run that begins there.
            let detached_start =
                (self.ad_hoc && self.detached && start == tips[members[0]]).then_some(start);
            let mut lane_segs = self.segment_run(
                &excl,
                &mut pending,
                &mut emitted,
                detached_start,
                chain_speaks,
            );
            // Each run carries its OWN bottom base: the convergence point it stopped at, else the
            // commit below its last one. Set here because a fork's list neighbour is a SIBLING —
            // the display must not re-thread bases by adjacency.
            if let Some(last) = lane_segs.last_mut()
                && last.base.is_none()
            {
                last.base = shared_start.or_else(|| {
                    // A lane that reached the bound has nothing below it IN VIEW, so it rests on
                    // nothing; anywhere else it rests on the commit below its last one.
                    excl.last()
                        .copied()
                        .filter(|last| Some(*last) != self.lower_bound)
                        .and_then(|id| self.next_fp(id))
                });
            }
            let base_idx = segments.len();
            for i in 1..lane_segs.len() {
                edges.push((base_idx + i - 1, base_idx + i));
            }
            if !lane_segs.is_empty() {
                lane_bottoms.push(base_idx + lane_segs.len() - 1);
                if let Some(merge) = forked_by {
                    spawned.push((merge, base_idx));
                }
            }
            // Remember which segment holds each commit, so a fork edge can find its merge.
            for (i, seg) in lane_segs.iter().enumerate() {
                for c in &seg.commits {
                    seg_of.insert(c.id, base_idx + i);
                }
            }
            segments.extend(lane_segs);
            // Capture the shared tail once, from the first convergence point.
            if shared_run.is_empty()
                && let Some(start) = shared_start
            {
                let mut cur = Some(start);
                while let Some(id) = cur {
                    if !self.in_cone(id) || !shared.contains(&id) {
                        break;
                    }
                    shared_run.push(id);
                    cur = self.next_fp(id);
                }
            }
        }
        // Fork edges: the merge's own segment is the child of each leg it forked.
        let forked = !spawned.is_empty();
        for (merge, leg_top) in spawned {
            if let Some(&from) = seg_of.get(&merge) {
                edges.push((from, leg_top));
            }
        }
        if !shared_run.is_empty() {
            let shared_top = segments.len();
            let shared_segs =
                self.segment_run(&shared_run, &mut pending, &mut emitted, None, chain_speaks);
            for i in 1..shared_segs.len() {
                edges.push((shared_top + i - 1, shared_top + i));
            }
            for &lb in &lane_bottoms {
                edges.push((lb, shared_top)); // convergence: lane bottom → shared tail top
            }
            segments.extend(shared_segs);
            if let Some(base) = shared_run.last().copied().and_then(|id| self.next_fp(id))
                && let Some(last) = segments.last_mut()
            {
                last.base = Some(base);
            }
        }
        // The bottom segment rests on the commit below the deepest collected commit (a shared
        // tail already set its own; a single-tip lane has none until here).
        if let Some(bottom) = segments.last().and_then(|s| s.commits.last().map(|c| c.id))
            && let Some(last) = segments.last_mut()
            && last.base.is_none()
        {
            last.base = self.next_fp(bottom);
        }
        // A DECLARED MEMBER OF THIS CLASS that named nothing still belongs to it. It loses naming
        // rights when a sibling shares its commit, or rests where the walk collected nothing — but
        // it is a member either way, and a declared in-workspace branch has to be visible to the
        // graph. Decided HERE, from this class's own declarations and what it just named, rather
        // than by a pass that inspects the finished projection.
        let named_here: HashSet<&gix::refs::FullNameRef> =
            segments.iter().filter_map(|s| s.ref_name()).collect();
        for b in members
            .iter()
            .filter_map(|&m| self.chain_of(reachable, m))
            .flat_map(|ci| &self.in_ws_stacks[ci].branches)
        {
            // A branch that NAMES A REAL SEGMENT of its own is not this class's to surface: the
            // class owning that commit projects it. Surfacing it here too would show one branch in
            // two lanes — what this pass exists for is the member with no segment of its own, which
            // is the only kind that can go missing.
            if self.names_own_segment(b.as_ref())
                || named_here.contains(b.as_ref())
                || emitted.contains(b)
                || pending.iter().any(|(n, _)| *n == b)
                || Some(b) == self.suppress_target.as_ref()
                || crate::ref_layout::in_gitbutler_namespace(b.as_ref())
            {
                continue;
            }
            if let Some(on) = self.anchor_of(b.as_ref()) {
                pending.push((b, on));
            }
        }

        // ── The leftover passes, in the order the driver runs them. ──

        // Leftover empties settle at the bottom (presence-gated), THIS CLASS'S OWN declared
        // branches first. Declaration index alone is a GLOBAL order, so when another chain is
        // declared ahead of this one its whole group wedges between a lane and the empties that
        // belong to it. A stack's own branches stay contiguous with it; other chains follow, in
        // declared order among themselves.
        let tail_start = segments.len();
        pending.sort_by_key(|(n, _)| {
            let own = declared.contains(n);
            let pos = self
                .in_ws_stacks
                .iter()
                .enumerate()
                .find_map(|(li, c)| c.branches.iter().position(|b| b == *n).map(|bi| (li, bi)))
                .unwrap_or((usize::MAX, usize::MAX));
            (!own, pos)
        });
        for (name, on) in std::mem::take(&mut pending) {
            if self.surfaces(Some(on)) && emitted.insert(name.clone()) {
                let mut empty = named_segment(name.clone(), None, self.ctx);
                empty.base = Some(on);
                if !segments.is_empty() {
                    edges.push((segments.len() - 1, segments.len()));
                }
                segments.push(empty);
            }
        }
        // A CHAIN rests by adjacency: each segment sits on the next one's first commit — which an
        // empty placeholder does not have, so the segment above an empty rests on nothing — while
        // the bottom keeps the base its walk stopped at. A FORKED class must NOT be re-threaded:
        // there a list neighbour is a SIBLING, and each run already carries its own base.
        let _ = tail_start;
        if !forked && segments.len() > 1 {
            for i in 0..segments.len() - 1 {
                segments[i].base = segments[i + 1].commits.first().map(|c| c.id);
            }
        }
        if segments.is_empty() {
            return None;
        }
        // Identity: the declared chain containing ANY of this stack's segment refs (the chain
        // owns the whole run, not only its top segment).
        let seg_names: Vec<&gix::refs::FullNameRef> =
            segments.iter().filter_map(|s| s.ref_name()).collect();
        // Identity follows the stack's TOP: the chain owning the topmost named segment speaks for
        // it. Segment order leads, not declaration order — a stack can span several chains (an
        // insertion puts another chain's branch below this one's tip), and then it is the tip that
        // says whose stack this is.
        // Keep the POSITION, not just the match: it is the declared index the lane order needs
        // below, and recovering it afterwards by searching for the id we just read is a round trip
        // through identity for something the lookup already knew.
        let declared_idx = seg_names.first().and_then(|n| {
            self.in_ws_stacks
                .iter()
                .position(|m| m.branches.iter().any(|b| b.as_ref() == *n))
        });
        let id = declared_idx
            .and_then(|i| self.meta_ids.get(i).copied())
            // A plain checkout is ONE stack and wears the fixed single-branch id.
            .or_else(|| {
                matches!(self.anchor, ViewAnchor::AdHoc)
                    .then(ref_metadata::StackId::single_branch_id)
            });
        edges.sort_unstable();
        edges.dedup();
        let key = members
            .iter()
            .map(|&m| self.lane_pos(Some(tips[m])))
            .min()
            .unwrap_or(usize::MAX);
        let declared = declared_idx.unwrap_or(usize::MAX);
        let parent_idx = members
            .iter()
            .map(|&m| self.parent_position(Some(tips[m])))
            .min()
            .unwrap_or(usize::MAX);
        Some((
            key,
            parent_idx,
            declared,
            declared_idx,
            Stack {
                id,
                segments,
                branch_parents: Some(edges),
            },
        ))
    }
    /// Only after every DIRECT claim is in can an unclaimed lane adopt a leftover
    /// identity, through a branch that merely RIDES one of its commits — and only an id
    /// no lane claimed for itself.
    fn adopt_identity(
        &self,
        stacks: &mut [KeyedStack],
        lane_of_chain: &mut HashMap<usize, usize>,
        worn_ids: &mut HashSet<ref_metadata::StackId>,
    ) {
        // Only now that every DIRECT claim is in can an unclaimed lane adopt a leftover identity: a
        // stack none of the declared chains names can still carry one through a branch that merely
        // RIDES on one of its commits — but only an id no lane claimed for itself.
        for (li, entry) in stacks.iter_mut().enumerate() {
            if entry.3.id.is_some() {
                continue;
            }
            let adopted = entry
                .3
                .segments
                .iter()
                .flat_map(|seg| {
                    seg.ref_name()
                        .into_iter()
                        .chain(seg.commits.iter().flat_map(|c| c.ref_iter()))
                })
                .find_map(|name| {
                    self.ws_meta?.stacks.iter().find_map(|m| {
                        (!worn_ids.contains(&m.id)
                            && m.branches.iter().any(|b| b.ref_name.as_ref() == name))
                        .then_some(m.id)
                    })
                });
            if let Some(id) = adopted {
                worn_ids.insert(id);
                if let Some(ci) = self.meta_ids.iter().position(|&mid| mid == id) {
                    lane_of_chain.insert(ci, li);
                }
                entry.3.id = Some(id);
            }
        }
    }

    /// A REPRESENTED chain's segmentless members settle at its lane's bottom — they are
    /// members either way, and a declared in-workspace branch has to be visible.
    fn settle_represented(
        &self,
        stacks: &mut [KeyedStack],
        lane_of_chain: &HashMap<usize, usize>,
        projected: &mut HashSet<gix::refs::FullName>,
    ) {
        // A chain is REPRESENTED once some lane wears its id — but the lane need not show every branch
        // it declares. Where no class spoke for the chain, the pass that surfaces a chain's segmentless
        // members never ran for it, so those branches are in no lane at all; being represented then
        // silences the empty-only pass below as well and they vanish outright. They belong under the
        // lane carrying their chain, not in one of their own, so they settle at its bottom here.
        for (ci, m) in self.in_ws_stacks.iter().enumerate() {
            if self.ad_hoc {
                continue;
            }
            let Some(&li) = lane_of_chain.get(&ci) else {
                continue;
            };
            for name in m.branches.iter() {
                if projected.contains(name)
                    || Some(name) == self.suppress_target.as_ref()
                    || crate::ref_layout::in_gitbutler_namespace(name.as_ref())
                {
                    continue;
                }
                let Some(on) = self.anchor_of(name.as_ref()) else {
                    continue;
                };
                let terr = super::presence::Territory::of(self.cg, Some(on), |id| self.in_view(id));
                if !super::presence::leftover_presence(
                    terr,
                    self.idx.names_empty.contains(name.as_ref()),
                    Some(on) == self.lower_bound,
                    false,
                )
                .surface
                {
                    continue;
                }
                let mut empty = named_segment(name.clone(), None, self.ctx);
                empty.base = Some(on);
                let stack = &mut stacks[li].3;
                if let Some(edges) = stack.branch_parents.as_mut()
                    && !stack.segments.is_empty()
                {
                    edges.push((stack.segments.len() - 1, stack.segments.len()));
                }
                stack.segments.push(empty);
                projected.insert(name.clone());
            }
        }
    }

    /// Declared chains no class represents become empty-only lanes at their rest,
    /// presence-gated; their position comes from the amended list, declared index
    /// breaking ties.
    fn empty_only_lanes(
        &self,
        stacks: &mut Vec<KeyedStack>,
        lane_of_chain: &HashMap<usize, usize>,
        projected: &HashSet<gix::refs::FullName>,
    ) {
        for (li, m) in self.in_ws_stacks.iter().enumerate() {
            // A view with no managed workspace COMMIT renders as ad-hoc: one lane for what is checked
            // out, with every branch appearing inside it. Declared chains do not each become a lane
            // there — without the merge there are no lanes to be, and its branches have already been
            // sourced from the layout above.
            if self.ad_hoc || lane_of_chain.contains_key(&li) {
                continue;
            }
            let segs: Vec<StackSegment> = m
                .branches
                .iter()
                .filter(|b| {
                    // Naming rights are not the test here. Where several branches share a commit only
                    // one of them names its segment, but the others are still applied branches that
                    // appear NOWHERE — and an applied branch has to be visible, or the next metadata
                    // write silently unapplies it. Presence decides instead: it surfaces at its rest
                    // unless some lane already displays that commit, in which case it rides there.
                    Some(*b) != self.suppress_target.as_ref()
                        && !projected.contains(*b)
                        && self.anchor_of(b.as_ref()).is_some_and(|on| {
                            // Same rule as the walk's: membership is only a question where the merge
                            // DEFINES it. A parentless merge marks nothing InWorkspace, so gating
                            // presence on the flag would deny every declared stack its lane exactly
                            // when metadata says the stacks exist.
                            let terr = super::presence::Territory::of(self.cg, Some(on), |id| {
                                self.in_view(id)
                            });
                            // rest_displayed is deliberately FALSE here: this branch belongs to a
                            // DECLARED applied stack, which is a lane in its own right. That another
                            // lane happens to show the commit it rests on does not make it a rider —
                            // several chains resting on one commit are several lanes.
                            super::presence::leftover_presence(
                                terr,
                                self.idx.names_empty.contains(b.as_ref()),
                                Some(on) == self.lower_bound,
                                false,
                            )
                            .surface
                        })
                })
                .map(|b| {
                    let mut empty = named_segment(b.clone(), None, self.ctx);
                    empty.base = self.anchor_of(b.as_ref());
                    empty
                })
                .collect();
            if segs.is_empty() {
                continue;
            }
            let last_base = segs.last().and_then(|s| s.base);
            let mut stack = Stack::from_base_and_segments_raw(segs, self.meta_ids.get(li).copied());
            if let Some(seg) = stack.segments.last_mut() {
                seg.base = last_base;
            }
            // An all-empty chain has no commits, so its lane position comes from the amended list
            // (its anchor's parent position); the declared index breaks ties among chains sharing an anchor.
            // An all-empty lane resting on a real workspace parent IS one of those parents — recover
            // its position so it orders with them; a lane that is no parent at all has none and settles
            // after them.
            let key = self.lane_pos(last_base);
            // `m` IS `self.in_ws_stacks[li]`, so its declared index is the loop index — searching for it by
            // id only rediscovers where we already are.
            stacks.push((key, self.parent_position(last_base), li, stack));
        }
    }
}

// ── Phase helpers the driver calls directly ──

/// A view with no name to give has no stack to give either; every trivial shape reduces
/// to this: the tip names one empty stack, keeping the engine total.
fn lone_empty_stack(ctx: &GraphContext, name: Option<&gix::refs::FullName>) -> Vec<Stack> {
    name.map(|name| {
        vec![Stack::from_base_and_segments_raw(
            vec![named_segment(name.clone(), None, ctx)],
            Some(ref_metadata::StackId::single_branch_id()),
        )]
    })
    .unwrap_or_default()
}

/// SEEDS — the one place the view's shape matters. A managed workspace merge contributes
/// each of its parents as a lane; any other view is a SINGLE lane seeded by the commit
/// HEAD is on: obey HEAD and walk down from it to the fork point, rather than starting at
/// its parent.
///
/// THE FRESH-CONNECTION RULE: metadata is the DESIRED state and the merge can lag it —
/// apply writes the declaration before rebuilding the workspace commit, so a declared
/// chain may have no parent edge yet (in the extreme, the merge has no parents at all).
/// Such a chain still seeds a lane, at the position of its own branch. Without this the
/// projection would be empty while metadata says two stacks exist, and every declared
/// stack would be unfindable. Only where a MERGE exists: the rule is about the merge
/// lagging the declaration, and with no merge there is nothing to lag. An ad-hoc view is
/// ONE lane by ruling, so seeding a lane per declared chain there would split the
/// checkout into several and show its branches twice.
fn seed_tips(
    cg: &CommitGraph,
    layout: &crate::ref_layout::RefLayout,
    in_ws_stacks: &[&crate::ref_layout::DeclaredStack],
    anchor: ViewAnchor,
    ad_hoc: bool,
    ws_commit: gix::ObjectId,
) -> Vec<gix::ObjectId> {
    let mut tips = if matches!(anchor, ViewAnchor::ManagedCommit) {
        cg.all_parent_ids(ws_commit)
    } else {
        vec![ws_commit]
    };
    for m in in_ws_stacks.iter().filter(|_| !ad_hoc) {
        let has_declared_parent = m.branches.iter().any(|b| {
            layout
                .positioned_on(b.as_ref())
                .is_some_and(|on| tips.contains(&on))
        });
        if has_declared_parent {
            continue;
        }
        if let Some(on) = m
            .branches
            .iter()
            .find_map(|b| layout.positioned_on(b.as_ref()))
        {
            tips.push(on);
        }
    }
    tips
}

/// Union tips that share any commit above the base — but ONLY when there IS a base.
/// Convergence is defined relative to the integration base ("seeds that meet ABOVE it");
/// with no target there is nothing to be above, every tip reaches the root, and unioning
/// would collapse unrelated stacks into one. Without a bound each tip stays its own stack.
fn converge(
    tips: &[gix::ObjectId],
    reachable: &[HashSet<gix::ObjectId>],
    lower_bound: Option<gix::ObjectId>,
) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..tips.len()).collect();
    if lower_bound.is_some() {
        for i in 0..tips.len() {
            for j in (i + 1)..tips.len() {
                if uf_find(&mut parent, i) != uf_find(&mut parent, j)
                    && reachable[i].iter().any(|c| reachable[j].contains(c))
                {
                    let (a, b) = (uf_find(&mut parent, i), uf_find(&mut parent, j));
                    parent[a] = b;
                }
            }
        }
    }
    parent
}

/// The union-find classes as ordered member lists: a class ranks by its earliest member tip.
fn class_members(parent: &mut [usize], tip_count: usize) -> Vec<Vec<usize>> {
    let mut classes: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..tip_count {
        let r = uf_find(parent, i);
        classes.entry(r).or_default().push(i);
    }
    let mut ordered: Vec<Vec<usize>> = classes.into_values().collect();
    ordered.sort_by_key(|members| members.iter().copied().min().unwrap_or(usize::MAX));
    ordered
}

/// Union-find find with path compression over a tip-index parent vector.
fn uf_find(parent: &mut [usize], x: usize) -> usize {
    let mut r = x;
    while parent[r] != r {
        r = parent[r];
    }
    let mut c = x;
    while parent[c] != r {
        let n = parent[c];
        parent[c] = r;
        c = n;
    }
    r
}

// ── The display shim — a separate concern: today's duplicated-lane view, derived ──

/// One lane per TIP of a stored stack shape (a tip is a segment nothing lists as its parent),
/// each lane the depth-first walk of that tip's sub-DAG, first edge first.
///
/// For a plain CHAIN — `(i, i+1)` adjacency, what an unforked stack stores — there is one tip
/// and the walk returns the segments unchanged: this is the identity. For a DAG it is the
/// compatibility shim that keeps today's UI shape: an in-stack FORK lists its legs under the
/// merge, and a CONVERGED multi-tip stack (whose shared tail is stored ONCE) re-materializes that
/// tail into every lane traversing it. Bases are preserved verbatim — a fork's list neighbour is a
/// SIBLING, so they must not be re-threaded by adjacency.
pub(crate) fn expand_lanes(
    segments: Vec<StackSegment>,
    edges: &[(usize, usize)],
    id: Option<ref_metadata::StackId>,
) -> Vec<Stack> {
    let n = segments.len();
    if n == 0 {
        return Vec::new();
    }
    let mut out: HashMap<usize, Vec<usize>> = HashMap::new();
    let mut is_parent = vec![false; n];
    for &(c, p) in edges {
        out.entry(c).or_default().push(p);
        if p < n {
            is_parent[p] = true;
        }
    }
    // Expand only a shape that genuinely BRANCHES: a segment with several parents (an in-stack
    // fork) or several children (a convergence point). Everything else — including chains
    // whose stored edges have gaps where leftover empties were appended — is returned in
    // its stored order as one lane, which is what those gaps mean.
    let mut incoming = vec![0usize; n];
    for &(_, p) in edges {
        if p < n {
            incoming[p] += 1;
        }
    }
    let branches = out.values().any(|ps| ps.len() > 1) || incoming.iter().any(|&c| c > 1);
    if !branches {
        return vec![Stack {
            id,
            segments,
            branch_parents: None,
        }];
    }
    let tips: Vec<usize> = (0..n).filter(|&i| !is_parent[i]).collect();
    // A cycle would leave no tip; fall back to the stored order rather than dropping the stack.
    if tips.is_empty() {
        return vec![Stack {
            id,
            segments,
            branch_parents: None,
        }];
    }
    let mut lanes = Vec::new();
    for tip in tips {
        let mut lane: Vec<StackSegment> = Vec::new();
        let mut seen = HashSet::new();
        let mut todo = vec![tip];
        while let Some(i) = todo.pop() {
            if !seen.insert(i) {
                continue;
            }
            lane.push(segments[i].clone());
            if let Some(ps) = out.get(&i) {
                todo.extend(ps.iter().rev().copied());
            }
        }
        lanes.push(Stack {
            id,
            segments: lane,
            branch_parents: None,
        });
    }
    lanes
}
