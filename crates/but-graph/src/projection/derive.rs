//! The workspace, derived: [`derive_workspace`] assembles the substrate ONCE — frame facts,
//! the partition engine's stacks, the carried graph — and [`Workspace::display_stacks`]
//! derives the pruned display view PER CALL. Nothing here is written back.
//!
//! READING ORDER: the three derivations first — the substrate driver, the eager stack
//! derivation it stores, and the display twin that re-materializes the stored stacks — then
//! the display passes in the order the driver runs them (hide, enrich, view rule), the
//! name-keyed enrichment both derivations share, the prune-pass helpers, and the
//! display-completeness assert every projection exits through.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Context;
use but_core::ref_metadata::StackKind::Applied;
use gix::{ObjectId, refs::Category};
use tracing::instrument;

use crate::{
    CommitFlags, CommitGraph, Workspace,
    workspace::{
        GraphContext, Stack, StackCommit, StackCommitFlags, StackSegment, TargetRef, WorkspaceKind,
        find_segment_owner_indexes_by_refname,
    },
};

// ── The three derivations: the substrate once, its stacks, and the display twin ──

/// Reconcile the workspace SUBSTRATE from the commit graph and the build's context: the
/// frame facts, the derived [`stacks`](Workspace::stacks), and the carried graph —
/// everything the application view needs EXCEPT the display projection. Stop here to hold a
/// fully-constructed workspace without pruning; [`Workspace::display_stacks`] is the optional
/// final layer that derives the pruned [`Stack`] shape for the UI.
///
/// Every location of `HEAD` — the entrypoint — derives SOME workspace. Expensive
/// per-commit reads are spent only on commits the user can interact with.
///
/// Target commit ids and integrated traversal seeds can extend the
/// workspace to include these commits to define its lowest base.
#[instrument(name = "derive_workspace", level = "trace", skip(cg), err(Debug))]
pub(crate) fn derive_workspace(
    mut cg: CommitGraph,
    ctx: GraphContext,
) -> anyhow::Result<crate::workspace::Workspace> {
    let f = super::frame::WorkspaceFrame::derive(&cg, &ctx)
        .context("BUG: the graph must have an entrypoint seed to project a workspace")?;
    // Anchor the integrated foundation STRICTLY BELOW the lower bound (the base's ancestors)
    // so the rebase editor bounds its mutable set to the workspace's own commits. The base
    // itself stays editable — operations may reorder around it, and when the merge-base
    // coincides with a workspace commit (e.g. `main`) the operation must be able to rewrite
    // it. Seeding from the base's parents flags everything below without flagging the base.
    let below_bound: Vec<gix::ObjectId> = f
        .lower_bound
        .and_then(|base| cg.node(base).map(|n| n.parent_ids.clone()))
        .unwrap_or_default();
    cg.set_flag_on_ancestors(crate::CommitFlags::BelowBound, below_bound);
    let target_ref = f.target_ref.as_ref().map(|t| TargetRef {
        ref_name: t.name.clone(),
        tip_commit_id: t.tip,
    });

    // The substrate is complete. The frame is retained VERBATIM as the derivation
    // input — the public fact fields below are copies for callers (legacy mutation
    // included) and never feed the derivation. The derivation runs once, EAGERLY,
    // reduced to the stored-stacks fact; the display re-derives per call.
    // THE PRODUCER for EVERY view: the partition engine, via the one shared derivation.
    let derived = derive_stacks(&cg, &ctx, &f);
    let stacks = crate::workspace::reduce_to_segment_stacks(&derived);
    let ws = crate::workspace::Workspace {
        target_ref,
        metadata: f.metadata.clone(),
        stacks,
        commit_graph: cg,
        ctx,
        frame: f,
    };
    Ok(ws)
}

/// THE derivation: the partition engine's stacks plus the name-keyed enrichment operations
/// read (per-branch metadata via [`enrich_from_branch_details`]). Worktree info rides along in the
/// enrichment but is display-only (no operation reads it); pruning is a separate layer
/// ([`Workspace::display_stacks`]) and entrypoint marks are display decoration
/// ([`mark_entrypoint_segments`]). A free function of `(graph, context, frame)` so the
/// derivation can be exercised, and reused, on its own.
pub(super) fn derive_stacks(
    cg: &CommitGraph,
    ctx: &GraphContext,
    f: &super::frame::WorkspaceFrame,
) -> Vec<Stack> {
    let entry = f
        .entry_inside
        .then(|| f.entry_ref.clone())
        .flatten()
        .or_else(|| f.tip_ref_info.as_ref().map(|ri| ri.ref_name.clone()));
    // The engine is TOTAL — a layoutless graph still yields its one empty stack — so there is
    // nothing to fall back to.
    let mut stacks = super::partition::derive_partition(
        cg,
        ctx,
        f.metadata.as_ref(),
        f.kind.has_managed_ref(),
        entry.as_ref(),
        f.tip_ref_info.as_ref().map(|ri| &ri.ref_name),
        f.lower_bound,
    );
    enrich_from_branch_details(&mut stacks, ctx);
    stacks
}

/// Materialize display stacks FROM the stored segment stacks: extents are given (tip→boundary
/// walks), so no claiming runs — only per-commit materialization and name-keyed
/// enrichment. This is the display twin of the eager derivation that produced the
/// stored stacks; the shared output passes (enrichment, marks) keep the two coherent.
pub(super) fn materialize_stacks(
    segment_stacks: &[crate::workspace::SegmentStack],
    cg: &CommitGraph,
    ctx: &GraphContext,
    f: &super::frame::WorkspaceFrame,
) -> Vec<Stack> {
    use super::anchor;
    let tip_ref_name = f.tip_ref_info.as_ref().map(|ri| ri.ref_name.clone());
    let frame_entry_ref = f
        .entry_inside
        .then(|| f.entry_ref.clone())
        .flatten()
        .or_else(|| tip_ref_name.clone());
    let idx = cg.layout().and_then(|layout| {
        let (ws_commit, _materialized) =
            anchor::resolve_view_anchor(cg, layout, f.metadata.as_ref(), frame_entry_ref.as_ref())?;
        let anchor = anchor::ViewAnchor::new(
            cg.is_managed_ws_commit(ws_commit) && f.metadata.is_some(),
            f.kind.has_managed_ref(),
        );
        Some(anchor::index_layout(cg, layout, anchor))
    });
    // A detached checkout hands its name back to the commit LAST, after tags.
    let detached = cg.seeds.iter().any(|t| t.is_entrypoint && t.is_detached);
    let mut stacks: Vec<Stack> = segment_stacks
        .iter()
        .flat_map(|skel_stack| {
            let segments = skel_stack
                .segments
                .iter()
                .map(|s| {
                    let mut seg = match &s.ref_name {
                        Some(name) => anchor::named_segment(name.clone(), s.tip(), ctx),
                        None => StackSegment::default(),
                    };
                    let mut last_walked = None;
                    for &id in &s.commits {
                        let Some(node) = cg.node(id) else { break };
                        let mut commit = StackCommit::from_graph_commit(node);
                        if let Some(idx) = idx.as_ref() {
                            anchor::strip_structural_refs(
                                &mut commit,
                                seg.ref_name(),
                                &idx.names_empty,
                            );
                            if detached
                                && let Some(&name) = idx.naming_at.get(&id)
                                && let Some(i) = commit
                                    .refs
                                    .iter()
                                    .position(|ri| ri.ref_name.as_ref() == name.as_ref())
                            {
                                let ri = commit.refs.remove(i);
                                commit.refs.push(ri);
                            }
                        }
                        seg.commits.push(commit);
                        last_walked = Some(id);
                    }
                    // A commit whose raw parents were never walked ends the extent early —
                    // the traversal's limit, worn by the last collected commit.
                    if let Some(last) = last_walked
                        && cg.first_parent(last).is_none()
                        && let Some(node) = cg.node(last)
                        && !node.parent_ids.is_empty()
                        && !node.flags.contains(crate::CommitFlags::ShallowBoundary)
                        && let Some(commit) = seg.commits.last_mut()
                    {
                        commit.flags |= crate::workspace::StackCommitFlags::EarlyEnd;
                    }
                    seg.base = s.base;
                    seg
                })
                .collect();
            // THE LANE VIEW: walk the stored shape from each of its tips. A stored CHAIN has one
            // tip and yields itself unchanged (this is the identity for every unforked stack);
            // a stored DAG yields one lane per tip, so an in-stack FORK lists its legs
            // under the merge and a CONVERGED multi-tip stack re-materializes its shared tail into
            // every lane that traverses it — the duplication users see, derived, never stored.
            super::partition::expand_lanes(segments, &skel_stack.edges, skel_stack.id)
        })
        .collect();
    // IDENTITY IS PER LANE, not per stored stack. A CONVERGED stack is one shape in the graph but
    // several declared stacks to metadata — B-on-A and C-on-A that both build on A converge into one
    // multi-tip stack, yet each remains its own declared stack with its own id. Storing one id and
    // copying it to every lane made the others unfindable ('couldn't find <id> in the projection').
    // So each lane takes the id of the chain that declares its TOP branch, keeping the stored id only
    // when nothing claims it.
    if let Some(meta) = f.metadata.as_ref() {
        for stack in stacks.iter_mut() {
            let Some(top) = stack.segments.iter().find_map(|s| s.ref_name()) else {
                continue;
            };
            if let Some(id) = meta.stacks.iter().find_map(|m| {
                m.branches
                    .iter()
                    .any(|b| b.ref_name.as_ref() == top)
                    .then_some(m.id)
            }) {
                stack.id = Some(id);
            }
        }
    }
    enrich_from_branch_details(&mut stacks, ctx);
    mark_entrypoint_segments(&mut stacks, f, ctx);
    stacks
}

// ── The display passes: hide, enrich, view rule ──

impl Workspace {
    /// The DISPLAY stacks, derived per call so display cost lives at the call site: bind it
    /// where used. Operations resolve structure on the segment graph, never here; the few that do
    /// read this shape read it as display (unapply picks its checkout ref and its dissolve decision
    /// from the pruned stack list).
    ///
    /// Three KINDS of pass run over the derivation, and the order between them is load-bearing:
    ///
    /// - **HIDE** — archived (truncates a stack's tail from a named branch down), out-of-cone
    ///   empties (a per-segment filter), integrated (cuts COMMITS inside segments, then drops
    ///   the branches that emptied). Three different granularities, which is why they are not
    ///   one predicate; archived must precede integrated, which relies on it having already
    ///   skipped archived segments that still held commits.
    /// - **ENRICH** — remote reachability and commits-on-remote hide nothing, they flag and add.
    ///   After hiding, so they never describe a segment that then vanishes.
    /// - **VIEW RULE** — the single-stack base truncation asks about the whole view rather than
    ///   any segment, and reads a stack count only hiding can settle. Last, for that reason.
    #[instrument(name = "project", level = "trace", skip(self), err(Debug))]
    pub fn display_stacks(&self) -> anyhow::Result<Vec<Stack>> {
        let mut stacks =
            materialize_stacks(&self.stacks, &self.commit_graph, &self.ctx, &self.frame);
        let cg = &self.commit_graph;
        let tip_ref_name = self
            .frame
            .tip_ref_info
            .as_ref()
            .map(|ri| ri.ref_name.clone());
        // HIDE — archived first; integrated pruning below is written against it having run.
        self.prune_archived_segments(&mut stacks);
        self.prune_out_of_cone_empties(&mut stacks, cg);
        {
            // Prune inputs at commit granularity: the advanced-upstream signal, the
            // target's own top commit (the commit its ref really names, if any), and
            // the tip's name for ad-hoc keeps.
            let upstream_advanced_past_target = self.upstream_advanced_past_target();
            let target_top_anchor = self.frame.target_commit.or_else(|| {
                let name = &self.target_ref.as_ref()?.ref_name;
                let layout = cg.layout()?;
                let facts = layout.facts_of(name.as_ref())?;
                (facts.names_segment && !facts.names_empty_segment)
                    .then(|| layout.positioned_on(name.as_ref()))
                    .flatten()
            });
            self.prune_integrated_segments(
                &mut stacks,
                cg,
                upstream_advanced_past_target,
                target_top_anchor,
                tip_ref_name.as_ref().map(|r| r.as_ref()),
            );
        }
        // ENRICH — flags and additions only; nothing below removes a segment.
        self.mark_remote_reachability(&mut stacks, cg)?;
        self.add_commits_on_remote(&mut stacks, cg);
        self.add_advanced_outside(&mut stacks, cg);
        // VIEW RULE — reads the stack count hiding just settled.
        self.truncate_single_stack_to_match_base(&mut stacks);
        debug_assert_applied_stacks_projected(cg, &stacks, self);
        Ok(stacks)
    }

    /// Prune segments whose branch is marked archived in workspace metadata — matched by name,
    /// that's all we have — top to bottom, and only when everything below them is also empty:
    /// a non-empty tail keeps the segment visible regardless of the flag. A stack whose every
    /// segment was archived is removed entirely. The flag deliberately stays out of the
    /// segment data itself so archived handling remains local to this pass.
    fn prune_archived_segments(&self, stacks: &mut Vec<Stack>) {
        // Explicit substrate path: keeps the borrow disjoint from `stacks` below.
        let Some(md) = &self.metadata else {
            return;
        };
        let archived_stack_branches = md.stacks(Applied).flat_map(|s| {
            s.branches
                .iter()
                .filter_map(|s| s.archived.then_some(s.ref_name.as_ref()))
        });
        let mut empty_stacks_to_remove = Vec::new();
        for archived_ref_name in archived_stack_branches {
            let Some((stack_idx, segment_idx)) =
                find_segment_owner_indexes_by_refname(stacks, archived_ref_name)
            else {
                continue;
            };
            let stack = &mut stacks[stack_idx];
            let all_downwards_are_empty = stack.segments[segment_idx..]
                .iter()
                .all(|s| s.commits.is_empty());
            if !all_downwards_are_empty {
                continue;
            }
            stack.segments.truncate(segment_idx);
            if stack.segments.is_empty() {
                empty_stacks_to_remove.push(stack_idx);
            }
        }

        empty_stacks_to_remove.sort();
        for stack_idx_to_remove in empty_stacks_to_remove.into_iter().rev() {
            let stack = stacks.remove(stack_idx_to_remove);
            tracing::warn!(
                "Pruned stack {stack_id:?} from workspace as all its segments were archived",
                stack_id = stack.id
            )
        }
    }

    /// The branch names an ad-hoc view keeps even when empty: the checked-out branch and
    /// every member of its persisted branch order. Empty for managed workspaces, where
    /// metadata answers instead.
    fn ad_hoc_keep_names(&self) -> BTreeSet<&gix::refs::FullNameRef> {
        if !matches!(self.kind(), WorkspaceKind::AdHoc) {
            return BTreeSet::new();
        }
        self.ctx
            .ad_hoc_branch_stack_orders
            .iter()
            .flatten()
            .map(|ref_name| ref_name.as_ref())
            .chain(
                self.frame
                    .tip_ref_info
                    .as_ref()
                    .map(|ri| ri.ref_name.as_ref()),
            )
            .collect()
    }

    /// Empty segments resting on target-only history — integrated commits the workspace
    /// tip cannot reach — are not displayed: such a branch (typically integrated while
    /// upstream advanced past the stored target) has left the workspace cone. The
    /// derivation keeps it for totality, so operations still see and un-apply it.
    /// An applied branch THE PRESENCE RULE demands (projection/presence.rs) is
    /// exempt — a true leftover empty on integrated territory or a rest on the
    /// bound must stay visible; a display-emptied shell of an integrated
    /// commit-bearing branch may hide. The prune keys on the segment's base
    /// anchor, which only the stack bottom wears — without the rule, chain
    /// position (not territory) decided which branch vanished (fuzz seed 0).
    fn prune_out_of_cone_empties(&self, stacks: &mut Vec<Stack>, cg: &crate::CommitGraph) {
        if self.target_ref.is_none() {
            return;
        }
        let applied = applied_branch_names(self.metadata.as_ref());
        // An ad-hoc view has no metadata to demand anything, but its checkout and its
        // persisted branch order are what is applied there: the checked-out branch resting
        // on the target tip is not out of the cone — it IS the cone.
        let ad_hoc_keep = self.ad_hoc_keep_names();
        let demanded = |rn: &gix::refs::FullNameRef| {
            let layout_empty = cg
                .layout()
                .and_then(|layout| layout.facts_of(rn))
                .is_some_and(|facts| facts.names_empty_segment);
            let rest = cg.layout().and_then(|layout| layout.positioned_on(rn));
            // The engine's membership question (`Derivation::in_view`), asked with the same
            // parentless-merge escape: a merge with no parents stamps nothing, so demanding
            // the flag would prune an applied branch's empties exactly in the
            // fresh-connection scenario — the silently-unapply defect class. Unified after
            // an A/B measured the escape corpus-neutral here (777 tests unmoved).
            let require_in_ws = self.frame.kind.has_managed_commit()
                && self
                    .frame
                    .tip_commit
                    .is_some_and(|ws| !cg.all_parent_ids(ws).is_empty());
            let territory = super::presence::Territory::of(cg, rest, |id| {
                cg.node(id).is_some_and(|n| {
                    !require_in_ws || n.flags.contains(crate::CommitFlags::InWorkspace)
                })
            });
            super::presence::leftover_presence(
                territory,
                layout_empty,
                rest.is_some() && rest == self.lower_bound(),
                false,
            )
            .demanded
        };
        for stack in stacks.iter_mut() {
            stack.segments.retain(|segment| {
                if !segment.commits.is_empty() {
                    return true;
                }
                if segment.ref_name().is_some_and(|rn| {
                    ad_hoc_keep.contains(rn) || (applied.contains(rn) && demanded(rn))
                }) {
                    return true;
                }
                let Some(anchor) = segment.base else {
                    return true;
                };
                let Some(node) = cg.node(anchor) else {
                    return true;
                };
                !node.flags.contains(crate::CommitFlags::Integrated)
                    || node.flags.contains(crate::CommitFlags::InWorkspace)
            });
        }
        stacks.retain(|stack| !stack.segments.is_empty());
    }

    /// Remove integrated commits and empty branches at the bottom of each
    /// stack, but only those at or below the workspace's target commit.
    /// Integrated commits above the target commit are kept until the user advances
    /// the target via upstream integration.
    // TODO: the per-stack fork point is recomputed on every projection rather than
    //       stored; persisting it would avoid re-deriving the target trunk each build.
    fn prune_integrated_segments(
        &self,
        stacks: &mut Vec<Stack>,
        cg: &crate::CommitGraph,
        upstream_advanced_past_target: bool,
        target_top_anchor: Option<ObjectId>,
        ws_tip_ref: Option<&gix::refs::FullNameRef>,
    ) {
        // Integrated-commit pruning only applies to workspaces tracking an upstream
        // target ref; without one, leave the stacks untouched.
        if self.target_ref.is_none() {
            return;
        }
        // Extra integrated tips mean upstream advanced past the stored target. Bail only
        // if there's no stored target *commit* to bound pruning against.
        if self.frame.target_commit.is_none() && upstream_advanced_past_target {
            return;
        }

        // The territory anchors on the target's top commit; an EMPTY target segment
        // anchors on the stored target commit or the target ref's position.
        let target_top = target_top_anchor.or(self.frame.target_commit).or_else(|| {
            let name = &self.target_ref.as_ref()?.ref_name;
            cg.layout()?.positioned_on(name.as_ref())
        });
        let Some(target_top) = target_top else {
            return;
        };
        let prune_commits = prunable_territory(cg, target_top, upstream_advanced_past_target);
        // Explicit substrate paths from here on: these borrows live across the
        // `&mut stacks` loops below and must stay disjoint from them.
        let metadata = self.metadata.as_ref();
        let keep_empty_names = self.ad_hoc_keep_names();
        debug_assert!(
            ws_tip_ref.is_none()
                || !matches!(self.kind(), WorkspaceKind::AdHoc)
                || ws_tip_ref.is_some_and(|rn| keep_empty_names.contains(rn)),
            "the ad-hoc tip is always kept"
        );
        let keep_if_fully_integrated =
            upstream_advanced_past_target && !matches!(self.kind(), WorkspaceKind::AdHoc);
        // A below-cut segment survives as a shell when its ref is ANY applied metadata
        // branch — a chain member can be absorbed into a sibling stack's walk, so the host
        // stack's own list is not the authority. Archived branches are excluded, which
        // compensates for `prune_archived_segments` running before integrated pruning:
        // archived segments that still had commits are skipped there, then emptied here.
        let applied_branch_names = applied_branch_names(metadata);
        for stack in stacks.iter_mut() {
            // Upstream advanced: floor the stack at its fork point but keep a fully-integrated
            // tip in managed workspaces so it survives for `integrate_upstream`. Single-branch
            // mode keeps the branch shell, but prunes integrated target/base commits from it.
            prune_integrated_stack_segments(
                stack,
                &prune_commits,
                keep_if_fully_integrated,
                &applied_branch_names,
            );
        }
        let orphan_chain_names = orphan_chain_names(metadata, stacks);
        for stack in stacks.iter_mut() {
            remove_empty_branches(
                stack,
                &applied_branch_names,
                &keep_empty_names,
                &orphan_chain_names,
                matches!(self.kind(), WorkspaceKind::AdHoc),
            );
            // Pruning moved the stack's bottom, so what it is SHOWN to rest on is its own fork
            // point with the target rather than the pre-prune global merge base. Every branch has
            // its own. This writes the display value; the graph base underneath is untouched.
            stack.recompute_last_segment_fork_point(cg);
        }
        stacks.retain(|stack| !stack.segments.is_empty());
    }

    /// Trace each linked remote down the ARENA and set commit flags to indicate
    /// whether a commit in the workspace is reachable from a remote, and how.
    fn mark_remote_reachability(
        &self,
        stacks: &mut [Stack],
        cg: &crate::CommitGraph,
    ) -> anyhow::Result<()> {
        // The builder records whether it actually LINKED a remote — an ambiguous
        // remote stays deliberately unlinked, so a name lookup won't do.
        let remote_refs: Vec<_> = stacks
            .iter()
            .flat_map(|s| {
                s.segments.iter().filter_map(|s| {
                    let name = s.ref_name()?;
                    let tip = self.ctx.branch_details.get(name)?.remote_walk_tip?;
                    s.remote_tracking_ref_name.clone().map(|rn| (rn, tip))
                })
            })
            .collect();
        let remote_named_at = remote_naming_positions(cg);
        struct Flagging {
            at: ObjectId,
            remote: gix::refs::FullName,
        }
        let mut flaggings = Vec::new();
        for (remote_tracking_ref_name, tip) in remote_refs {
            let mut seen = HashSet::new();
            let mut queue = vec![tip];
            while let Some(id) = queue.pop() {
                if !seen.insert(id) {
                    continue;
                }
                let Some(node) = cg.node(id) else { continue };
                // Stop at non-remote commits, and never 'steal' commits from other
                // known remote territory.
                let prune = !node.flags.is_remote()
                    || (id != tip
                        && remote_named_at
                            .get(&id)
                            .is_some_and(|&n| n != &remote_tracking_ref_name));
                if prune {
                    if !node.flags.is_remote() {
                        flaggings.push(Flagging {
                            at: id,
                            remote: remote_tracking_ref_name.clone(),
                        });
                    }
                    continue;
                }
                queue.extend(cg.all_parent_ids(id));
            }
        }
        for Flagging { at, remote } in flaggings {
            for stack in stacks.iter_mut() {
                // The remote's reach into this stack starts where the commit appears.
                let Some((first_segment, first_commit_index)) =
                    stack.segments.iter().enumerate().find_map(|(os_idx, os)| {
                        os.commits
                            .iter()
                            .position(|c| c.id == at)
                            .map(|ci| (os_idx, ci))
                    })
                else {
                    continue;
                };
                let mut first_commit_index = Some(first_commit_index);
                for segment in &mut stack.segments[first_segment..] {
                    let remote_reachable_flags =
                        if segment.remote_tracking_ref_name.as_ref() == Some(&remote) {
                            StackCommitFlags::ReachableByMatchingRemote
                        } else {
                            StackCommitFlags::empty()
                        } | StackCommitFlags::ReachableByRemote;
                    for commit in
                        &mut segment.commits[first_commit_index.take().unwrap_or_default()..]
                    {
                        commit.flags |= remote_reachable_flags;
                    }
                }
                // keep looking - other stacks can repeat the commit!
            }
        }
        Ok(())
    }

    /// For each local segment that has a linked remote tracking branch, walk the
    /// remote side of the ARENA and collect commits that exist on the remote but
    /// not locally:
    /// - commits that are purely remote (never existed locally or pre-rebase versions), and
    /// - non-integrated commits from upper stack segments that are still on the
    ///   remote (the "branch split" case — a previously combined push left the
    ///   remote pointing at commits that now belong to branch above it).
    fn add_commits_on_remote(&self, stacks: &mut [Stack], cg: &crate::CommitGraph) {
        let remote_named_at = remote_naming_positions(cg);
        let naming = naming_positions(cg);
        for stack in stacks.iter_mut() {
            let mut above_commit_ids = HashSet::new();
            for seg_idx in 0..stack.segments.len() {
                let Some(tip) = stack.segments[seg_idx]
                    .ref_name()
                    .and_then(|name| self.ctx.branch_details.get(name))
                    .and_then(|d| d.remote_walk_tip)
                else {
                    // Still accumulate this segment's commits for lower segments.
                    above_commit_ids.extend(stack.segments[seg_idx].commits.iter().map(|c| c.id));
                    continue;
                };

                // Run-wise BFS: collect purely-remote commits, stopping runs at
                // local commits or territory another remote-naming position owns.
                let mut remote_commits = Vec::new();
                let mut seen_ids = HashSet::new();
                let mut runs = std::collections::VecDeque::from([tip]);
                while let Some(run_start) = runs.pop_front() {
                    let mut cursor = Some(run_start);
                    while let Some(id) = cursor.take() {
                        if !seen_ids.insert(id) {
                            break;
                        }
                        let Some(node) = cg.node(id) else { break };
                        if !node.flags.is_remote()
                            || (id != tip
                                && remote_named_at.get(&id).is_some_and(|&n| {
                                    stack.segments[seg_idx].remote_tracking_ref_name.as_ref()
                                        != Some(n)
                                }))
                        {
                            break;
                        }
                        let mut commit = StackCommit::from_graph_commit(node);
                        // The territory's naming ref is structure, not a rider —
                        // deduced remotes included.
                        commit.refs.retain(|ri| {
                            !naming.contains(&(commit.id, ri.ref_name.as_ref()))
                                && stack.segments[seg_idx].remote_tracking_ref_name.as_ref()
                                    != Some(&ri.ref_name)
                        });
                        remote_commits.push(commit);
                        let parents = cg.all_parent_ids(id);
                        cursor = parents.first().copied();
                        runs.extend(parents.into_iter().skip(1));
                    }
                }

                // First-parent walk: detect non-integrated commits from upper
                // stack segments that are still reachable by the remote tracking branch.
                if !above_commit_ids.is_empty() {
                    let mut seen: HashSet<_> = remote_commits.iter().map(|c| c.id).collect();
                    let mut extra = Vec::new();
                    let mut cursor = Some(tip);
                    let mut fp_seen = HashSet::new();
                    while let Some(id) = cursor.take() {
                        if !fp_seen.insert(id) {
                            break;
                        }
                        if id != tip && remote_named_at.contains_key(&id) {
                            break;
                        }
                        let Some(node) = cg.node(id) else { break };
                        if above_commit_ids.contains(&id)
                            && !node.flags.contains(CommitFlags::Integrated)
                            && seen.insert(id)
                        {
                            let mut commit = StackCommit::from_graph_commit(node);
                            commit.refs.retain(|ri| {
                                !naming.contains(&(commit.id, ri.ref_name.as_ref()))
                                    && stack.segments[seg_idx].remote_tracking_ref_name.as_ref()
                                        != Some(&ri.ref_name)
                            });
                            extra.push(commit);
                        }
                        cursor = cg.all_parent_ids(id).first().copied();
                    }
                    remote_commits.extend(extra);
                }

                stack.segments[seg_idx].commits_on_remote = remote_commits;

                // Accumulate this segment's commits for lower segments.
                above_commit_ids.extend(stack.segments[seg_idx].commits.iter().map(|c| c.id));
            }
        }
    }

    /// Publish the declared branches that ADVANCED outside the workspace: the user committed
    /// to a branch directly, so its ref points at commits the workspace merge does not
    /// contain. Each is attached to the segment its first-parent spine rejoins — the lane it
    /// left behind — as a `(ref name, commits outside)` pair.
    ///
    /// The segment is NOT renamed: a segment is named by the ref that points at it, and this
    /// ref points elsewhere. The pair is the honest version of that fact.
    fn add_advanced_outside(&self, stacks: &mut [Stack], cg: &crate::CommitGraph) {
        let Some(layout) = cg.layout() else {
            return;
        };
        for stack in stacks.iter_mut() {
            // The lane's declared chain is the one declaring its top branch — the same rule
            // that stamps lane identity; the id is a boundary stamp, not a lookup key.
            let Some(declared) = stack
                .segments
                .iter()
                .find_map(|s| s.ref_name())
                .and_then(|top| {
                    layout
                        .stacks
                        .iter()
                        .find(|d| d.branches.iter().any(|b| b.as_ref() == top))
                })
            else {
                continue;
            };
            for branch in &declared.branches {
                let Some(tip) = cg.commit_by_ref(branch.as_ref()) else {
                    continue;
                };
                if cg
                    .node(tip)
                    .is_none_or(|n| n.flags.contains(CommitFlags::InWorkspace))
                {
                    // In the workspace (or below it — `InWorkspace` propagates to
                    // ancestors), so nothing ran away.
                    continue;
                }
                // Walk the outside run down to where it rejoins the workspace. A run that
                // never rejoins (the walk was cut, or the branch was rebased elsewhere)
                // has no lane here to report on.
                let mut commits_outside = Vec::new();
                let mut rejoin = None;
                let mut cursor = Some(tip);
                while let Some(id) = cursor.take() {
                    let Some(node) = cg.node(id) else { break };
                    if node.flags.contains(CommitFlags::InWorkspace) {
                        rejoin = Some(id);
                        break;
                    }
                    commits_outside.push(StackCommit::from_graph_commit(node));
                    cursor = cg.first_parent(id);
                }
                let Some(rejoin) = rejoin else { continue };
                let Some(segment) = stack
                    .segments
                    .iter_mut()
                    .find(|s| s.commits.iter().any(|c| c.id == rejoin))
                else {
                    continue;
                };
                segment
                    .advanced_outside
                    .push(crate::workspace::AdvancedOutside {
                        ref_name: branch.clone(),
                        commits_outside,
                    });
            }
        }
    }

    /// If there is a single stack and the base happens to be itself (which happens if the stack is directly integrated/inline with the target),
    /// then empty all commits and segment-related metadata.
    ///
    /// The "single stack" it keys on is the POST-HIDE count — hiding can empty a stack out of
    /// existence — so this belongs after the hide passes, not among them.
    fn truncate_single_stack_to_match_base(&self, stacks: &mut [Stack]) {
        if stacks.len() != 1 {
            return;
        }
        let Some(stack) = stacks.first_mut() else {
            return;
        };
        let stack_is_base =
            stack
                .segments
                .first()
                .zip(self.lower_bound())
                .is_some_and(|(segment, base)|
                // The stack IS the base when its first commit is the bound commit itself.
                segment.commits.first().is_some_and(|c| c.id == base));
        if !stack_is_base {
            return;
        }

        stack.segments.drain(1..);
        let first_segment = stack.segments.first_mut().expect("non-empty");
        first_segment.commits.clear();
    }
}

// ── Name-keyed enrichment and the entrypoint mark, shared by both derivations ──

/// Name-keyed enrichment from the builder's capture: metadata sidebands and worktree
/// info ride on names, not structure.
fn enrich_from_branch_details(stacks: &mut [Stack], ctx: &GraphContext) {
    for seg in stacks.iter_mut().flat_map(|s| &mut s.segments) {
        let Some(details) = seg.ref_name().and_then(|name| ctx.branch_details.get(name)) else {
            continue;
        };
        seg.metadata = details.metadata.clone();
        if let Some(ri) = seg.ref_info.as_mut() {
            ri.worktree = details.worktree.clone();
        }
    }
}

/// The entrypoint mark mirrors the frame's entrypoint: its named segment, else the
/// first segment holding the entrypoint commit. Only managed workspaces mark — ad-hoc
/// views ARE their entrypoint.
fn mark_entrypoint_segments(
    stacks: &mut [Stack],
    f: &super::frame::WorkspaceFrame,
    ctx: &GraphContext,
) {
    use super::frame::EntryMark;
    let mark = f.entry_mark(ctx, |name| {
        stacks
            .iter()
            .flat_map(|s| &s.segments)
            .any(|seg| seg.ref_name() == Some(name))
    });
    match mark {
        EntryMark::None => {}
        EntryMark::ByName(name) => {
            for seg in stacks
                .iter_mut()
                .flat_map(|s| &mut s.segments)
                .filter(|seg| seg.ref_name() == Some(name))
            {
                seg.is_entrypoint = true;
            }
        }
        EntryMark::ByCommit(id) => {
            for seg in stacks.iter_mut().flat_map(|s| &mut s.segments) {
                if seg.commits.iter().any(|c| c.id == id) {
                    seg.is_entrypoint = true;
                }
            }
        }
    }
}

// ── Prune-pass helpers ──

/// Prune the integrated tail whose commits are in `prune_commits`; commits in other
/// territory are kept (e.g. above the target, or reaching it only via a merge's 2nd parent).
/// Cutting happens at whole-GROUP boundaries (the prune map's tokens): a group holding any
/// live commit survives in full.
///
/// With `keep_if_fully_integrated`, a stack whose every commit would be pruned is left
/// untouched, keeping a fully-integrated branch's tip visible for `integrate_upstream`.
fn prune_integrated_stack_segments(
    stack: &mut Stack,
    prune_commits: &HashMap<ObjectId, usize>,
    keep_if_fully_integrated: bool,
    metadata_branch_names: &std::collections::BTreeSet<&gix::refs::FullNameRef>,
) {
    // Walk stack segments bottom-up, then prune-groups bottom-up within each stack
    // segment. Stop at the first group that is either not fully integrated or not
    // in the prunable territory.
    let mut cut: Option<(usize, usize)> = None;
    // Whether any commit would survive the cut (not integrated, or not prunable).
    let mut has_surviving_commit = false;
    'outer: for seg_idx in (0..stack.segments.len()).rev() {
        let seg = &stack.segments[seg_idx];
        if seg.commits.is_empty() {
            continue;
        }
        // Contiguous runs of the same prune-group, bottom-up.
        let mut end = seg.commits.len();
        while end > 0 {
            let group = prune_commits.get(&seg.commits[end - 1].id).copied();
            let mut start = end - 1;
            while start > 0 && prune_commits.get(&seg.commits[start - 1].id).copied() == group {
                start -= 1;
            }
            let commits = &seg.commits[start..end];
            if group.is_some() && commits_are_integrated(commits) {
                cut = Some((seg_idx, start));
            } else {
                has_surviving_commit = true;
                break 'outer;
            }
            end = start;
        }
    }

    let Some((cut_seg_idx, cut_offset)) = cut else {
        return;
    };

    // The whole stack is integrated trunk. While upstream is ahead, keep it (e.g. a
    // fully-integrated branch behind its remote) rather than pruning it out of existence.
    if keep_if_fully_integrated && !has_surviving_commit {
        return;
    }

    // Emptied entirely, the segment rests on the commit its ref points to — the first commit
    // pruning took — rather than on whatever lay below it in the graph.
    let pointed_to = (cut_offset == 0)
        .then(|| stack.segments[cut_seg_idx].commits.first().map(|c| c.id))
        .flatten();
    stack.segments[cut_seg_idx].commits.truncate(cut_offset);
    if let Some(pointed_to) = pointed_to {
        stack.segments[cut_seg_idx].fork_point = Some(pointed_to);
    }

    // Remove all stack segments below the cut. If the cut emptied the topmost
    // stack segment, keep it so `remove_empty_branches` can decide whether its
    // branch ref should be preserved, e.g. a metadata-tracked branch at the fork point.
    let keep = if stack.segments[cut_seg_idx].commits.is_empty() && cut_seg_idx > 0 {
        cut_seg_idx
    } else {
        cut_seg_idx + 1
    };
    // Below the cut, metadata-tracked branches survive as empty shells — their commits are
    // integrated trunk, but the branches themselves are applied workspace state whose fate
    // belongs to `remove_empty_branches` (dropping them here would silently unapply them
    // on the next metadata write). Everything else — trunk naming refs like the target's
    // local — goes with its territory.
    let shells: Vec<StackSegment> = stack
        .segments
        .drain(keep..)
        .filter(|seg| {
            seg.ref_name()
                .is_some_and(|rn| metadata_branch_names.contains(rn))
        })
        .map(|mut seg| {
            seg.commits.clear();
            seg
        })
        .collect();
    stack.segments.extend(shells);
}

/// The prunable territory keyed by COMMIT, each with an opaque group token so pruning
/// still cuts at whole-group boundaries: groups break at positioned naming refs, the
/// arena's segmentation marks.
///
/// With upstream advanced past the stored target, only the target's FIRST-PARENT trunk
/// is prunable — commits reaching the target via a merge's second parent are off it, so
/// a branch's own work is preserved. Otherwise the territory is the target's top commit
/// plus all its ancestors.
fn prunable_territory(
    cg: &crate::CommitGraph,
    target_top: ObjectId,
    upstream_advanced_past_target: bool,
) -> HashMap<ObjectId, usize> {
    let naming_at: HashSet<ObjectId> = naming_positions(cg).into_iter().map(|(id, _)| id).collect();
    let mut prune_commits = HashMap::<ObjectId, usize>::new();
    let mut groups = HashMap::<ObjectId, usize>::new();
    let group_of = |anchor: ObjectId, groups: &mut HashMap<ObjectId, usize>| {
        let next = groups.len();
        *groups.entry(anchor).or_insert(next)
    };
    if upstream_advanced_past_target {
        let mut anchor = target_top;
        let mut cursor = Some(target_top);
        while let Some(id) = cursor.take() {
            if naming_at.contains(&id) {
                anchor = id;
            }
            if prune_commits
                .insert(id, group_of(anchor, &mut groups))
                .is_some()
            {
                break;
            }
            cursor = cg.all_parent_ids(id).first().copied();
        }
    } else {
        let mut queue = vec![(target_top, target_top)];
        while let Some((id, mut anchor)) = queue.pop() {
            if naming_at.contains(&id) {
                anchor = id;
            }
            if prune_commits
                .insert(id, group_of(anchor, &mut groups))
                .is_some()
            {
                continue;
            }
            queue.extend(cg.all_parent_ids(id).into_iter().map(|p| (p, anchor)));
        }
    }
    prune_commits
}

fn commits_are_integrated(commits: &[StackCommit]) -> bool {
    commits
        .iter()
        .all(|commit| commit.flags.contains(StackCommitFlags::Integrated))
}

/// Every applied non-archived metadata branch name — the display keep-authority shared
/// by the prune passes. Excludes gitbutler/*: unconsolidated legacy metadata can list the
/// workspace ref itself as a branch, and that namespace never rests as a segment.
fn applied_branch_names(
    metadata: Option<&but_core::ref_metadata::Workspace>,
) -> BTreeSet<&gix::refs::FullNameRef> {
    metadata
        .map(|meta| {
            meta.stacks(Applied)
                .flat_map(|ms| {
                    ms.branches
                        .iter()
                        .filter(|b| !b.archived)
                        .map(|b| b.ref_name.as_ref())
                        .filter(|rn| !crate::ref_layout::in_gitbutler_namespace(rn))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Branch names of applied chains with no commit-bearing segment in ANY stack. Their
/// empty segments are the chain's last projected placeholder and must survive removal —
/// operations rebuild metadata from the projection, so a vanished chain gets un-applied
/// on disk (e.g. an applied branch merged upstream, spliced as an empty segment into
/// the stack that walks its commit).
fn orphan_chain_names<'m>(
    metadata: Option<&'m but_core::ref_metadata::Workspace>,
    stacks: &[Stack],
) -> HashSet<&'m gix::refs::FullNameRef> {
    metadata
        .map(|meta| {
            meta.stacks(Applied)
                .filter(|ms| {
                    !stacks.iter().any(|stack| {
                        stack
                            .segments
                            .iter()
                            .filter(|seg| !seg.commits.is_empty())
                            .filter_map(|seg| seg.ref_name())
                            .any(|name| {
                                ms.branches
                                    .iter()
                                    .any(|b| b.ref_name.as_ref() == name && !b.archived)
                            })
                    })
                })
                .flat_map(|ms| {
                    ms.branches
                        .iter()
                        .filter(|b| !b.archived)
                        .map(|b| b.ref_name.as_ref())
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Remove empty segments unless they are mentioned in workspace metadata
/// (e.g. a branch the user just added at the fork point with no commits yet).
/// `orphan_chain_names` are branches of applied chains with no commit-bearing
/// segment in ANY stack — their empty segments are the chain's last projected
/// placeholder and always survive.
fn remove_empty_branches(
    stack: &mut Stack,
    applied_branch_names: &BTreeSet<&gix::refs::FullNameRef>,
    keep_empty_names: &BTreeSet<&gix::refs::FullNameRef>,
    orphan_chain_names: &HashSet<&gix::refs::FullNameRef>,
    keep_tip: bool,
) {
    let bottom_base = stack.segments.last().and_then(|seg| seg.resting_on());
    let mut idx = 0;
    stack.segments.retain(|seg| {
        let is_tip = idx == 0;
        idx += 1;
        (keep_tip && is_tip)
            || !seg.commits.is_empty()
            // Any applied (non-archived) metadata branch keeps its empty segment,
            // WHEREVER it surfaced — a chain member absorbed into a sibling stack's walk
            // is still applied state, and dropping it would unapply it on the next
            // metadata write.
            || seg.ref_name().is_some_and(|rn| {
                applied_branch_names.contains(rn)
                    || keep_empty_names.contains(rn)
                    || orphan_chain_names.contains(rn)
            })
    });
    // Removing an empty bottom doesn't change what the stack rests on: carry the old bottom's
    // resting point when the new bottom has none — an all-empty stack has no commits for
    // `recompute_last_segment_fork_point` to derive one from.
    if let Some(seg) = stack.segments.last_mut()
        && seg.resting_on().is_none()
    {
        seg.fork_point = bottom_base;
    }
}

// ── Remote naming facts the enrich passes read ──

/// Remote-category refs that NAME territory, per commit: a commit reachable
/// from two remotes has exactly one deliberate owner.
fn remote_naming_positions(cg: &crate::CommitGraph) -> HashMap<ObjectId, &gix::refs::FullName> {
    cg.layout()
        .map(|l| {
            l.segment_naming_placements()
                .filter(|(name, _)| name.category() == Some(Category::RemoteBranch))
                .map(|(name, on)| (on, name))
                .collect()
        })
        .unwrap_or_default()
}

/// Every (commit, name) pair where the name is a positioned SEGMENT-naming ref of
/// any category — such refs are structure, never riders.
fn naming_positions(cg: &crate::CommitGraph) -> HashSet<(ObjectId, &gix::refs::FullNameRef)> {
    cg.layout()
        .map(|l| {
            l.segment_naming_placements()
                .map(|(name, on)| (on, name.as_ref()))
                .collect()
        })
        .unwrap_or_default()
}

// ── The exit assert ──

/// Test-build tripwire: an APPLIED metadata stack silently disappearing from `stacks`.
/// The persistence danger is gone (the write-back removal: no write reads the view back) and
/// the merge resolves named tips from metadata + the graph. Operations no longer read this
/// projection for structure either — they resolve on the derived `view` (a raw-graph
/// resolution was proven impossible: tip-ness is irreducibly derived). So this is now a
/// DISPLAY-COMPLETENESS guard — applied branches must stay visible to the UI — that doubles
/// as a forward-guard against the projection→metadata anti-pattern returning. The
/// placeholder/keep-list machinery it guards is genuine display machinery, not operation
/// scaffolding: relaxing it would regress the UI, not operations. Scoped to managed
/// workspaces; a stack counts only when at least one non-archived branch of it actually
/// names a segment in this graph.
#[cfg(debug_assertions)]
fn debug_assert_applied_stacks_projected(cg: &CommitGraph, stacks: &[Stack], ws: &Workspace) {
    if !matches!(ws.kind(), WorkspaceKind::Managed { .. }) {
        return;
    }
    let Some(meta) = ws.metadata.as_ref() else {
        return;
    };
    let projected: std::collections::HashSet<_> = stacks
        .iter()
        .flat_map(|s| s.segments.iter())
        .filter_map(|s| s.ref_name().map(|r| r.to_owned()))
        .collect();
    // Only branches REACHABLE from the workspace tip count: metadata is the DESIRED state
    // and legitimately runs ahead of the graph mid-operation (apply writes metadata before
    // the workspace merge is rebuilt). The failure class is a branch that IS in the
    // workspace's reach and still lost its stack. Reachability is seeded from the tip and
    // everything below it — the below-walk stops at the bound, the parent walk continues.
    let mut reachable = std::collections::HashSet::new();
    let mut queue: Vec<_> = ws
        .tip_commit_id()
        .into_iter()
        .chain(super::frame::commits_below_tip(cg, &ws.frame))
        .collect();
    while let Some(id) = queue.pop() {
        if !reachable.insert(id) {
            continue;
        }
        queue.extend(cg.all_parent_ids(id));
    }
    let layout = cg.layout();
    for stack in meta.stacks(but_core::ref_metadata::StackKind::Applied) {
        for branch in stack.branches.iter().filter(|branch| !branch.archived) {
            let name = branch.ref_name.as_ref();
            let facts = layout.and_then(|layout| layout.facts_of(name));
            let rest = layout.and_then(|layout| layout.positioned_on(name));
            // THE PRESENCE RULE decides (projection/presence.rs) — workspace
            // membership by tip REACHABILITY here, as this assert always had it.
            let territory = super::presence::Territory::of(cg, rest, |on| reachable.contains(&on));
            // Only a branch that NAMES a segment can be demanded of the view (a passive
            // rider is representable only through its carrier).
            let represented = facts.is_some_and(|facts| facts.names_segment)
                && super::presence::leftover_presence(
                    territory,
                    facts.is_some_and(|facts| facts.names_empty_segment),
                    rest.is_some() && rest == ws.lower_bound(),
                    false,
                )
                .demanded;
            debug_assert!(
                !represented || projected.contains(name),
                "applied branch {:?} of stack {:?} ({territory:?}) vanished from the projection — \
                 operations writing from this projection would drop it on disk",
                name.as_bstr(),
                stack.id,
            );
        }
    }
}

#[cfg(not(debug_assertions))]
fn debug_assert_applied_stacks_projected(_cg: &CommitGraph, _stacks: &[Stack], _ws: &Workspace) {}
