//! Assemble the [`Workspace`]: the frame decides kind, entrypoint, target and bounds
//! (`frame`), the stacks are collected from the arena and its stored ref layout
//! (`collect`), and this module finishes the picture — name-keyed metadata/worktree
//! sidebands, the entrypoint mark, the target fields, then the pruning passes
//! (archived, integrated, sibling territory). The substrate moves onto the finished
//! workspace: the projection carries the graph it was derived from.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::Context;
use but_core::ref_metadata::{StackId, StackKind::Applied};
use gix::{ObjectId, refs::Category};
use tracing::instrument;

use crate::{
    CommitFlags, CommitGraph, Workspace,
    workspace::{
        GraphContext, Stack, StackCommit, StackCommitFlags, StackSegment, TargetCommit, TargetRef,
        WorkspaceKind, find_segment_owner_indexes_by_refname,
    },
};

/// Project the workspace from the commit graph and the build's context — the
/// application's view over the substrate.
///
/// Every location of `HEAD` — the entrypoint — projects to SOME workspace. Expensive
/// per-commit reads are spent only on commits the user can interact with.
///
/// Target commit ids and integrated traversal seeds can extend the
/// workspace to include these commits to define its lowest base.
#[instrument(name = "project_workspace", level = "trace", skip(cg), err(Debug))]
pub(crate) fn project_workspace(cg: CommitGraph, ctx: GraphContext) -> anyhow::Result<Workspace> {
    let f = super::frame::WorkspaceFrame::derive(&cg, &ctx)
        .context("BUG: the graph must have an entrypoint seed to project a workspace")?;
    let commits_below_tip = super::frame::commits_below_tip(&cg, &f);
    let tip_ref_name = f.tip_ref_info.as_ref().map(|ri| ri.ref_name.clone());
    let stacks = {
        let frame_entry_ref = f
            .entry_inside
            .then(|| f.entry_ref.clone())
            .flatten()
            .or_else(|| tip_ref_name.clone());
        let frame_entry_commit = cg.entrypoint();
        let mut stacks = super::collect::stacks_from_arena(
            &cg,
            &ctx,
            f.lower_bound,
            f.metadata.as_ref(),
            f.kind.has_managed_ref(),
            frame_entry_ref.as_ref(),
            frame_entry_commit,
        )
        .unwrap_or_else(|| {
            // Layoutless graphs — unborn refs — still name one empty stack.
            tip_ref_name
                .clone()
                .map(|name| {
                    vec![Stack::from_base_and_segments_raw(
                        vec![super::collect::named_segment(name, None, &ctx)],
                        Some(StackId::single_branch_id()),
                    )]
                })
                .unwrap_or_default()
        });
        enrich_from_branch_details(&mut stacks, &ctx);
        mark_entrypoint_segments(&mut stacks, &f, &ctx);
        stacks
    };
    let mut target_ref = f.target_ref.as_ref().map(|t| TargetRef {
        ref_name: t.name.clone(),
        tip_commit_id: t.tip,
        commits_ahead: 0,
    });

    // One traversal serves both: the ids feed the workspace, their count the target.
    let incoming_target_commit_ids = target_ref.as_ref().map(|t| {
        t.tip_commit_id
            .map(|tip| super::upstream_commits(&cg, tip, f.lower_bound))
            .unwrap_or_default()
    });
    if let Some(target) = target_ref.as_mut() {
        target.commits_ahead = incoming_target_commit_ids.as_ref().map_or(0, Vec::len);
    }

    // Capture the facts queries need once the segment view is gone, in field order.
    // The entry's commit is the ref's GIT target — a commitless entry segment
    // resolves BELOW its ref, but the entrypoint stays where HEAD pointed.
    let entrypoint = Some(crate::workspace::EntrypointInfo {
        commit_id: f
            .entry_ref
            .as_ref()
            .and_then(|r| cg.ref_target(r))
            .or_else(|| {
                // A commitless entry ref sits on no node — the walk's seed
                // still remembers where it pointed. Edited arenas keep their
                // stale walk seeds; only a live node counts.
                cg.seeds
                    .iter()
                    .find(|s| s.is_entrypoint)
                    .map(|s| s.id)
                    .filter(|id| cg.node(*id).is_some())
            })
            .or(f.entry_commit)
            // Seam-built arenas carry no walk artifacts at all; an ad-hoc
            // view's entry IS its tip.
            .or(f.tip_commit),
        ref_name: f.entry_ref.clone(),
        is_ws_tip: !f.entry_inside,
    });
    let target_ref_commit_id = target_ref.as_ref().and_then(|t| t.tip_commit_id);
    let effective_target_commit_id = target_ref_commit_id
        .or(f.target_commit)
        .or_else(|| f.integrated_tip_commits.first().copied());
    let has_multiple_worktrees = {
        let mut kinds = cg.ref_worktrees().map(|wt| &wt.kind).chain(
            ctx.branch_details
                .values()
                .filter_map(|d| d.worktree.as_ref().map(|wt| &wt.kind)),
        );
        let first = kinds.next();
        first.is_some_and(|first| kinds.any(|kind| kind != first))
    };
    let target_commit = f.target_commit.map(|commit_id| TargetCommit { commit_id });
    let mut ws = Workspace {
        kind: f.kind,
        stacks,
        lower_bound: f.lower_bound,
        target_ref,
        target_commit,
        metadata: f.metadata,
        commit_graph: crate::CommitGraph::default(),
        ctx,
        tip_ref_info: f.tip_ref_info,
        tip_commit_id: f.tip_commit,
        lower_bound_ref_name: f.lower_bound_ref_name,
        entrypoint,
        target_ref_commit_id,
        effective_target_commit_id,
        commits_below_tip,
        incoming_target_commit_ids,
        has_multiple_worktrees,
    };

    ws.prune_archived_segments();
    {
        // Prune inputs at commit granularity: the advanced-upstream signal, the
        // target's own top commit (the commit its ref really names, if any), and
        // the tip's name for ad-hoc keeps.
        let upstream_advanced_past_target = !f.integrated_tip_commits.is_empty();
        let target_top_anchor = ws
            .target_commit
            .as_ref()
            .map(|tc| tc.commit_id)
            .or_else(|| {
                let name = &ws.target_ref.as_ref()?.ref_name;
                let layout = cg.layout()?;
                let facts = layout.facts_of(name.as_ref())?;
                (facts.names_segment && !facts.names_empty_segment)
                    .then(|| layout.positioned_on(name.as_ref()))
                    .flatten()
            });
        ws.prune_integrated_segments(
            &cg,
            upstream_advanced_past_target,
            target_top_anchor,
            tip_ref_name.as_ref().map(|r| r.as_ref()),
        );
    }
    ws.mark_remote_reachability(&cg)?;
    ws.add_commits_on_remote(&cg);
    ws.truncate_single_stack_to_match_base();
    debug_assert_applied_stacks_projected(&cg, &ws);
    // The projection is done: the substrate moves onto the workspace.
    ws.commit_graph = cg;
    Ok(ws)
}

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
    if !(f.entry_inside && f.kind.has_managed_ref()) {
        return;
    }
    let ep_name = f.entry_ref.as_ref().map(|r| r.as_ref());
    let ep_commit = ctx.entry_resolved_commit.or(f.entry_commit);
    let by_name: Vec<&mut StackSegment> = stacks
        .iter_mut()
        .flat_map(|s| &mut s.segments)
        .filter(|seg| ep_name.is_some() && seg.ref_name() == ep_name)
        .collect();
    if by_name.is_empty() {
        for seg in stacks.iter_mut().flat_map(|s| &mut s.segments) {
            if ep_commit.is_some_and(|id| seg.commits.iter().any(|c| c.id == id)) {
                seg.is_entrypoint = true;
            }
        }
    } else {
        for seg in by_name {
            seg.is_entrypoint = true;
        }
    }
}

/// Test-build tripwire for the most dangerous projection failure: an APPLIED metadata stack
/// silently disappearing from `stacks`. Operations rebuild the workspace merge and metadata
/// heads from the projection, so a vanished stack does not just render wrong — it gets
/// PERSISTED by the next write. Scoped to managed workspaces; a stack counts only when at
/// least one non-archived branch of it actually names a segment in this graph.
#[cfg(debug_assertions)]
fn debug_assert_applied_stacks_projected(cg: &CommitGraph, ws: &Workspace) {
    use but_core::ref_metadata::StackKind::Applied;
    if !matches!(ws.kind, WorkspaceKind::Managed { .. }) {
        return;
    }
    let Some(meta) = ws.metadata.as_ref() else {
        return;
    };
    let projected: std::collections::HashSet<_> = ws
        .stacks
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
        .tip_commit_id
        .into_iter()
        .chain(ws.commits_below_tip.iter().copied())
        .collect();
    while let Some(id) = queue.pop() {
        if !reachable.insert(id) {
            continue;
        }
        queue.extend(cg.all_parent_ids(id));
    }
    let positions = cg.layout();
    for stack in meta.stacks(Applied) {
        let represented_branches: Vec<_> = stack
            .branches
            .iter()
            .filter(|b| !b.archived)
            .filter(|b| {
                let Some(facts) = positions.and_then(|layout| layout.facts_of(b.ref_name.as_ref()))
                else {
                    return false;
                };
                if !facts.names_segment {
                    return false;
                }
                let Some(on) =
                    positions.and_then(|layout| layout.positioned_on(b.ref_name.as_ref()))
                else {
                    return false;
                };
                reachable.contains(&on)
                        // A stack anchored on INTEGRATED territory away from the workspace
                        // lower bound is deliberately not overzealously materialized; one
                        // resting ON the lower bound (or holding live commits, or empty)
                        // must keep its stack.
                        && (facts.names_empty_segment
                            || cg.node(on).is_none_or(|n| {
                                !n.flags.contains(crate::CommitFlags::Integrated)
                                    || Some(on) == ws.lower_bound
                            }))
            })
            .map(|b| b.ref_name.clone())
            .collect();
        if represented_branches.is_empty() {
            continue;
        }
        debug_assert!(
            represented_branches
                .iter()
                .any(|b| projected.contains(b.as_ref())),
            "applied metadata stack {:?} (branches {:?}) vanished from the projection —                  operations writing from this projection would drop the stack on disk",
            stack.id,
            represented_branches
                .iter()
                .map(|b| b.as_bstr().to_string())
                .collect::<Vec<_>>(),
        );
    }
}

#[cfg(not(debug_assertions))]
fn debug_assert_applied_stacks_projected(_cg: &CommitGraph, _ws: &Workspace) {}

impl Workspace {
    /// Prune segments whose branch is marked archived in workspace metadata — matched by name,
    /// that's all we have — top to bottom, and only when everything below them is also empty:
    /// a non-empty tail keeps the segment visible regardless of the flag. A stack whose every
    /// segment was archived is removed entirely. The flag deliberately stays out of the
    /// segment data itself so archived handling remains local to this pass.
    fn prune_archived_segments(&mut self) {
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
                find_segment_owner_indexes_by_refname(&self.stacks, archived_ref_name)
            else {
                continue;
            };
            let stack = &mut self.stacks[stack_idx];
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
            let stack = self.stacks.remove(stack_idx_to_remove);
            tracing::warn!(
                "Pruned stack {stack_id:?} from workspace as all its segments were archived",
                stack_id = stack.id
            )
        }
    }

    /// Remove integrated commits and empty branches at the bottom of each
    /// stack, but only those at or below the workspace's target commit.
    /// Integrated commits above the target commit are kept until the user advances
    /// the target via upstream integration.
    // TODO: the per-stack fork point is recomputed on every projection rather than
    //       stored; persisting it would avoid re-deriving the target trunk each build.
    fn prune_integrated_segments(
        &mut self,
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
        if self.target_commit.is_none() && upstream_advanced_past_target {
            return;
        }

        // The territory anchors on the target's top commit; an EMPTY target segment
        // anchors on the stored target commit or the target ref's position.
        let target_top = target_top_anchor
            .or_else(|| self.target_commit.as_ref().map(|tc| tc.commit_id))
            .or_else(|| {
                let name = &self.target_ref.as_ref()?.ref_name;
                cg.layout()?.positioned_on(name.as_ref())
            });
        let Some(target_top) = target_top else {
            return;
        };
        let prune_commits = prunable_territory(cg, target_top, upstream_advanced_past_target);
        let metadata = self.metadata.as_ref();
        let keep_empty_names = if matches!(self.kind, WorkspaceKind::AdHoc) {
            self.ctx
                .ad_hoc_branch_stack_orders
                .iter()
                .flatten()
                .map(|ref_name| ref_name.as_ref())
                .chain(ws_tip_ref)
                .collect::<BTreeSet<_>>()
        } else {
            BTreeSet::new()
        };
        let keep_if_fully_integrated =
            upstream_advanced_past_target && !matches!(self.kind, WorkspaceKind::AdHoc);
        // The target's LOCAL tracking branch is exempt from integrated pruning when METADATA
        // applies it as a stack in a MANAGED workspace: caught up with the target, ALL its
        // commits are integrated by definition, so pruning would empty the stack by construction
        // and slide its base to the workspace lower bound (`ref_target=M2, base=M1` — an
        // incoherent segment). An applied main keeps its commits: its stack IS the base
        // indicator. The single-branch view of a checked-out main keeps pruning its integrated
        // base as before — membership, and thus the exemption, is metadata-explicit.
        let target_local = self
            .kind
            .has_managed_ref()
            .then(|| {
                self.target_ref
                    .as_ref()
                    .and_then(|tr| {
                        self.ctx
                            .remote_tracking
                            .iter()
                            .find_map(|(local, r)| (r == &tr.ref_name).then_some(local))
                    })
                    .map(|local| local.as_ref())
            })
            .flatten();
        for stack in &mut self.stacks {
            let is_target_local_stack =
                stack.id.is_some() && stack.ref_name().is_some_and(|rn| Some(rn) == target_local);
            // Upstream advanced: floor the stack at its fork point but keep a fully-integrated
            // tip in managed workspaces so it survives for `integrate_upstream`. Single-branch
            // mode keeps the branch shell, but prunes integrated target/base commits from it.
            if !is_target_local_stack {
                prune_integrated_stack_segments(stack, &prune_commits, keep_if_fully_integrated);
            }
        }
        let orphan_chain_names = orphan_chain_names(metadata, &self.stacks);
        for stack in &mut self.stacks {
            remove_empty_branches(
                stack,
                metadata,
                &keep_empty_names,
                &orphan_chain_names,
                matches!(self.kind, WorkspaceKind::AdHoc),
            );
            // Pruning moved the stack's bottom; refresh its base to its own fork point with the
            // target rather than leaving the pre-prune global merge base. Every branch has its
            // own base.
            stack.recompute_last_segment_base_from(cg);
        }
        self.stacks.retain(|stack| !stack.segments.is_empty());
        hide_bases_on_sibling_territory(&mut self.stacks);
    }
    /// Trace each linked remote down the ARENA and set commit flags to indicate
    /// whether a commit in the workspace is reachable from a remote, and how.
    fn mark_remote_reachability(&mut self, cg: &crate::CommitGraph) -> anyhow::Result<()> {
        // The builder records whether it actually LINKED a remote — an ambiguous
        // remote stays deliberately unlinked, so a name lookup won't do.
        let remote_refs: Vec<_> = self
            .stacks
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
            for stack in &mut self.stacks {
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
    fn add_commits_on_remote(&mut self, cg: &crate::CommitGraph) {
        let remote_named_at = remote_naming_positions(cg);
        let naming = naming_positions(cg);
        for stack in &mut self.stacks {
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

    /// If there is a single stack and the base happens to be itself (which happens if the stack is directly integrated/inline with the target),
    /// then empty all commits and segment-related metadata.
    fn truncate_single_stack_to_match_base(&mut self) {
        if self.stacks.len() != 1 {
            return;
        }
        let Some(stack) = self.stacks.first_mut() else {
            return;
        };
        let stack_is_base =
            stack
                .segments
                .first()
                .zip(self.lower_bound)
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

    stack.segments[cut_seg_idx].commits.truncate(cut_offset);

    // Remove all stack segments below the cut. If the cut emptied the topmost
    // stack segment, keep it so `remove_empty_branches` can decide whether its
    // branch ref should be preserved, e.g. a metadata-tracked branch at the fork point.
    let keep = if stack.segments[cut_seg_idx].commits.is_empty() && cut_seg_idx > 0 {
        cut_seg_idx
    } else {
        cut_seg_idx + 1
    };
    stack.segments.truncate(keep);
}

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

fn commits_are_integrated(commits: &[StackCommit]) -> bool {
    commits
        .iter()
        .all(|commit| commit.flags.contains(StackCommitFlags::Integrated))
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

/// Branch names of applied chains with no commit-bearing segment in ANY stack. Their
/// empty segments are the chain's last projected trace and must survive removal —
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

/// A recomputed base is shown only when it lies OUTSIDE the workspace's own stacks; a base
/// resting on a sibling stack's territory is hidden.
fn hide_bases_on_sibling_territory(stacks: &mut [Stack]) {
    let owned: HashSet<ObjectId> = stacks
        .iter()
        .flat_map(|s| s.segments.iter().flat_map(|seg| &seg.commits))
        .map(|c| c.id)
        .collect();
    for stack in stacks {
        let own: HashSet<ObjectId> = stack
            .segments
            .iter()
            .flat_map(|seg| &seg.commits)
            .map(|c| c.id)
            .collect();
        if let Some(seg) = stack.segments.last_mut()
            && seg
                .base
                .is_some_and(|b| owned.contains(&b) && !own.contains(&b))
        {
            seg.base = None;
        }
    }
}

/// Remove empty segments unless they are mentioned in workspace metadata
/// (e.g. a branch the user just added at the fork point with no commits yet).
/// `orphan_chain_names` are branches of applied chains with no commit-bearing
/// segment in ANY stack — their empty segments are the chain's last projected
/// trace and always survive.
fn remove_empty_branches(
    stack: &mut Stack,
    metadata: Option<&but_core::ref_metadata::Workspace>,
    keep_empty_names: &BTreeSet<&gix::refs::FullNameRef>,
    orphan_chain_names: &HashSet<&gix::refs::FullNameRef>,
    keep_tip: bool,
) {
    let own_metadata_stack = stack.id.and_then(|stack_id| {
        metadata.and_then(|meta| meta.stacks(Applied).find(|ms| ms.id == stack_id))
    });
    let mut idx = 0;
    stack.segments.retain(|seg| {
        let is_tip = idx == 0;
        idx += 1;
        (keep_tip && is_tip)
            || !seg.commits.is_empty()
            || seg
                .ref_name()
                .is_some_and(|rn| keep_empty_names.contains(rn) || orphan_chain_names.contains(rn))
            || own_metadata_stack.is_some_and(|ms| {
                seg.ref_info
                    .as_ref()
                    // NOTE: `!b.archived` compensates for `prune_archived_segments`
                    // running *before* integrated-commit pruning — archived segments
                    // that still had commits are skipped there, then emptied here.
                    // Once metadata is kept trimmed and up-to-date we can drop this.
                    .is_some_and(|ri| {
                        ms.branches
                            .iter()
                            .any(|b| b.ref_name == ri.ref_name && !b.archived)
                    })
            })
    });
}
