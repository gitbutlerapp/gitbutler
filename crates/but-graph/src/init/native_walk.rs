//! The walker: the traversal accumulating a [`CommitGraph`](crate::CommitGraph) directly — no
//! segment minting. Developed beside the legacy raw walk (S1) and held to field-exact equality
//! with its flattening across the whole corpus before that walk was deleted; per-commit ref
//! ORDER is canonicalized (sorted by name).
//!
//! Fidelity strategy: everything order- and limit-relevant is REUSED (Queue, Limit, Goals,
//! `initial_tips_*`, `find`), and naming/metadata decisions delegate to
//! [`branch_segment_from_name_and_meta`] operating on THROWAWAY `Segment` records kept in a
//! side table — the seed table — so the initial-queue sort and workspace-ownership shuffles
//! run the very same code paths without a graph.

use std::collections::HashMap;

use but_core::RefMetadata;
use gix::reference::Category;

use super::{
    InitialTips, Options, Tip, TipRole,
    overlay::{OverlayMetadata, OverlayRepo},
    types::{Goals, Instruction, Limit, Queue, QueueItem},
    walk::{RemoteQueueOutcome, WorktreeByBranch, branch_segment_from_name_and_meta, find},
};
use crate::{Commit, CommitFlags, Segment};

/// What the native traversal produces; [`CommitGraph`](crate::CommitGraph) assembly input.
pub(crate) struct NativeOutcome {
    /// Commits in collection order, flags final.
    pub commits: Vec<Commit>,
    /// `(child, parent)` pairs the traversal actually connected.
    pub connected: std::collections::HashSet<(gix::ObjectId, gix::ObjectId)>,
    pub entrypoint: Option<gix::ObjectId>,
    pub entrypoint_ref: Option<gix::refs::FullName>,
    pub hard_limit_hit: bool,
    /// The normalized traversal seeds, like the raw graph's `traversal_tips`.
    pub tips: Vec<Tip>,
}

/// Commit store: flags mutable by id, children derived from followed edges.
#[derive(Default)]
struct Store {
    commits: Vec<Commit>,
    by_id: HashMap<gix::ObjectId, usize>,
    connected: std::collections::HashSet<(gix::ObjectId, gix::ObjectId)>,
    /// Followed parents per child — "downward" propagation goes down the HISTORY (to ancestors),
    /// matching the segment graph whose edges point child-segment → parent-segment.
    parents_followed: HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
}

impl Store {
    fn flags(&self, id: gix::ObjectId) -> CommitFlags {
        self.by_id
            .get(&id)
            .map(|&ix| self.commits[ix].flags)
            .unwrap_or_default()
    }
    fn flags_or(&mut self, id: gix::ObjectId, add: CommitFlags) {
        if let Some(&ix) = self.by_id.get(&id) {
            self.commits[ix].flags |= add;
        }
    }
    fn push(&mut self, commit: Commit) {
        self.by_id.insert(commit.id, self.commits.len());
        self.commits.push(commit);
    }
    /// Record that the traversal CONNECTED `child -> parent`.
    fn connect(&mut self, child: Option<gix::ObjectId>, parent: gix::ObjectId) {
        if let Some(child) = child
            && self.connected.insert((child, parent))
        {
            self.parents_followed.entry(child).or_default().push(parent);
        }
    }
    /// `flags(x) |= add` for `start` and every stored ANCESTOR reached via followed edges, each
    /// visited once — "downward" is down the history, like the segment version walking outgoing
    /// (child→parent) edges. Returns the visited commits when `needs_visited` (for the caller's
    /// leaf-segment computation).
    fn propagate_flags_downward(
        &mut self,
        add: CommitFlags,
        start: gix::ObjectId,
        needs_visited: bool,
    ) -> Option<gix::hashtable::HashSet<gix::ObjectId>> {
        let mut visited = gix::hashtable::HashSet::default();
        let mut stack = vec![start];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            self.flags_or(id, add);
            stack.extend(
                self.parents_followed
                    .get(&id)
                    .into_iter()
                    .flatten()
                    .copied(),
            );
        }
        needs_visited.then_some(visited)
    }
}

/// Lightweight segment identity — just enough of the raw walk's segment bookkeeping to replicate
/// its behavior-relevant decisions: which queued tips inherit goals at a re-encounter (leaf
/// SEGMENTS, not leaf commits), the entrypoint owner's name and first-commit flags (the
/// integrated-cutoff gate), and the hoisted segment-name re-attachment.
/// Boundaries follow the walk exactly: every seed is a segment; merge parents start new ones
/// (`ConnectNewSegment`); a continuation splits off when its commit carries an unambiguous branch
/// name or is a merge (`try_split_non_empty_segment_at_branch`); a mid-segment re-encounter
/// splits the tail (`split_commit_into_segment`).

#[derive(Default)]
struct Segs {
    names: Vec<Option<crate::RefInfo>>,
    commits: Vec<Vec<gix::ObjectId>>,
    of: HashMap<gix::ObjectId, usize>,
}

impl Segs {
    fn add(&mut self, seg: &Segment) -> usize {
        let ix = self.names.len();
        self.names.push(seg.ref_info.clone());
        self.commits.push(Vec::new());
        ix
    }
    /// Seed-table entries created since the last call become segments; `seed_seg` maps seed → seg.
    fn sync_seeds(&mut self, seeds: &[Segment], seed_seg: &mut Vec<usize>) {
        while seed_seg.len() < seeds.len() {
            let seg = self.add(&seeds[seed_seg.len()]);
            seed_seg.push(seg);
        }
    }
    fn push_commit(&mut self, seg: usize, id: gix::ObjectId) {
        self.commits[seg].push(id);
        self.of.insert(id, seg);
    }
    /// `split_commit_into_segment`: `commits[pos..]` become their own (anonymous) segment.
    fn split_off_tail(&mut self, seg: usize, pos: usize) {
        let tail = self.commits[seg].split_off(pos);
        let new_seg = self.names.len();
        self.names.push(None);
        for id in &tail {
            self.of.insert(*id, new_seg);
        }
        self.commits.push(tail);
    }
    /// The segment a queue item collects into — seeds into their seed segment, parents into the
    /// queuing commit's CURRENT segment (splits and swaps are picked up live, which is what the
    /// walk's queue-rewriting on split achieves).
    fn landing(&self, instr: &Instruction, seed_seg: &[usize]) -> Option<usize> {
        instr
            .seed
            .map(|ix| seed_seg[ix])
            .or_else(|| instr.queued_by.map(|qb| self.of[&qb]))
    }
    /// Has this segment an edge to another segment below it (outgoing, in segment-graph terms)?
    fn has_outgoing(
        &self,
        seg: usize,
        parents_followed: &HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
    ) -> bool {
        self.commits[seg].iter().any(|c| {
            parents_followed
                .get(c)
                .is_some_and(|ps| ps.iter().any(|p| self.of.get(p).is_some_and(|&s| s != seg)))
        })
    }
}

/// The native counterpart of the raw `traverse_tips_with_overlay`: same queue, same limits,
/// same dequeue order — commits stored directly.
#[allow(clippy::too_many_arguments)]
pub(crate) fn traverse<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    tips: Vec<Tip>,
    meta: &OverlayMetadata<'_, T>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: Options,
    ref_name_override: Option<gix::refs::FullName>,
) -> anyhow::Result<NativeOutcome> {
    let entrypoint_tip = super::validate_explicit_tips(repo, &tips, ref_name_override.as_ref())?;
    let tip = entrypoint_tip.id;
    let detach_entrypoint = entrypoint_tip.is_detached;
    let ref_name = if detach_entrypoint {
        None
    } else {
        ref_name_override.or_else(|| entrypoint_tip.ref_name.clone())
    };

    let Options {
        collect_tags,
        extra_target_commit_id,
        commits_limit_hint: limit,
        commits_limit_recharge_location: mut max_commits_recharge_location,
        hard_limit,
    } = options;
    let max_limit = Limit::new(limit);
    if ref_name
        .as_ref()
        .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
    {
        anyhow::bail!("Cannot currently handle remotes as start position");
    }
    let commit_graph = repo.commit_graph_if_enabled()?;
    let shallow_commits = repo.shallow_commits()?;
    let mut buf = Vec::new();

    let configured_remote_tracking_branches =
        super::remotes::configured_remote_tracking_branches(repo)?;
    let initial_tips =
        super::initial_tips_from_tips(repo, tips, &project_meta, extra_target_commit_id);
    let mut refs_by_id = repo.collect_ref_mapping_by_prefix(
        ["refs/heads/", "refs/remotes/"]
            .into_iter()
            .chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
        &initial_tips
            .workspace_ref_names
            .iter()
            .map(|ref_name| ref_name.as_ref())
            .collect::<Vec<_>>(),
    )?;
    let worktree_by_branch = repo.worktree_branches(ref_name.as_ref().map(|r| r.as_ref()))?;

    let mut goals = Goals::default();
    let tip_flags = CommitFlags::NotInRemote
        | goals
            .flag_for(tip)
            .expect("we have more than one bitflag for this");

    let mut store = Store::default();
    let mut seen = gix::hashtable::HashSet::default();
    let mut next: Queue = Queue::new_with_limit(hard_limit);
    let mut seeds: Vec<Segment> = Vec::new();
    let mut segs = Segs::default();
    let mut seed_seg: Vec<usize> = Vec::new();
    // The seed whose segment `graph.entrypoint` points at — its (evolving) first commit gates
    // `prune_integrated_tips`.
    let mut ep_seed: Option<usize> = None;

    let target_limit = max_limit
        .with_indirect_goal(tip, &mut goals)
        .without_allowance();

    queue_initial_tips_native(
        &mut next,
        &mut seeds,
        &mut ep_seed,
        &initial_tips,
        tip,
        tip_flags,
        max_limit,
        target_limit,
        &mut goals,
        commit_graph.as_ref(),
        repo,
        meta,
        &refs_by_id,
        &worktree_by_branch,
        &mut buf,
    )?;
    prioritize_and_assure_ws_ownership_native(
        &mut next,
        &mut seeds,
        &mut ep_seed,
        (initial_tips.workspace_tips.clone(), repo, meta),
        &worktree_by_branch,
    )?;

    max_commits_recharge_location.sort();
    let mut points_of_interest_to_traverse_first = next.iter().count();
    while let Some((info, mut propagated_flags, instr, mut limit)) = next.pop_front() {
        points_of_interest_to_traverse_first =
            points_of_interest_to_traverse_first.saturating_sub(1);

        let id = info.id;
        segs.sync_seeds(&seeds, &mut seed_seg);
        if max_commits_recharge_location.binary_search(&id).is_ok() {
            limit.set_but_keep_goal(max_limit);
        }
        // Pick up flags propagated onto the queuing commit since this item was queued.
        let src_flags = instr
            .queued_by
            .map(|qid| store.flags(qid))
            .unwrap_or_default();
        propagated_flags |= src_flags;
        let is_shallow_boundary = shallow_commits
            .as_ref()
            .is_some_and(|boundary| boundary.binary_search(&id).is_ok());
        if is_shallow_boundary {
            propagated_flags |= CommitFlags::ShallowBoundary;
        }

        if seen.contains(&id) {
            // Re-encounter: the native `possibly_split_occupied_segment` — record the edge,
            // replay the segment-ownership choreography (workspace swap, mid-segment split),
            // merge flags, propagate downward, adjust queued tips.
            store.connect(instr.queued_by, id);
            let bottom_seg = segs.of[&id];
            let pos = segs.commits[bottom_seg]
                .iter()
                .position(|&c| c == id)
                .expect("owning segment lists its commits");
            if pos != 0 {
                segs.split_off_tail(bottom_seg, pos);
            }
            let existing_flags = store.flags(id);
            let new_flags = propagated_flags | existing_flags;
            let needs_leafs = !limit.goal_reached();
            let visited = if new_flags != existing_flags
                || (needs_leafs
                    && next.iter().any(|(_, _, _, tip_limit)| {
                        !tip_limit.goal_flags().contains(limit.goal_flags())
                    })) {
                store.propagate_flags_downward(new_flags, id, needs_leafs)
            } else {
                None
            };
            if let Some(visited) = visited {
                // Tips scheduled to land in one of the propagation's leaf segments — visited,
                // no outgoing edges — inherit the goal (`propagate_goals_of_reachable_tips`).
                let leaf_segs: Vec<usize> = visited
                    .iter()
                    .map(|c| segs.of[c])
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .filter(|&s| !segs.has_outgoing(s, &store.parents_followed))
                    .collect();
                let goal_flags = limit.goal_flags();
                for (_, _, item_instr, item_limit) in next.iter_mut() {
                    if segs
                        .landing(item_instr, &seed_seg)
                        .is_some_and(|s| leaf_segs.contains(&s))
                    {
                        *item_limit = item_limit.additional_goal(goal_flags);
                    }
                }
            }
            // Tips that saw this commit extend their limit if that would extend it.
            let bottom_commit_goals = Limit::new(None)
                .additional_goal(store.flags(id))
                .goal_flags();
            for queued_tip_limit in next.iter_mut().filter_map(|(_, _, _, l)| {
                l.goal_flags().intersects(bottom_commit_goals).then_some(l)
            }) {
                queued_tip_limit.adjust_limit_if_bigger(limit);
            }
            // NB: the walk `continue`s straight past prune and sort on this path.
            continue;
        }
        seen.insert(id);
        store.connect(instr.queued_by, id);

        // Which segment does this commit join? Seeds own their segment; a merge parent starts a
        // new one (`ConnectNewSegment`); a continuation splits off when the commit carries an
        // unambiguous branch name or is itself a merge (`try_split_non_empty_segment_at_branch`)
        // — decided BEFORE the refs are consumed, like the walk.
        let seg_for_id = if let Some(ix) = instr.seed {
            seed_seg[ix]
        } else if instr.new_segment {
            let seg = branch_segment_from_name_and_meta(
                None,
                meta,
                Some((&refs_by_id, id)),
                &worktree_by_branch,
            )?;
            segs.add(&seg)
        } else {
            let src_seg = segs.of[&instr
                .queued_by
                .expect("non-seed items have a queuing commit")];
            let maybe_name = super::walk::disambiguate_refs_by_branch_metadata_with_lookup(
                (&refs_by_id, id),
                meta,
            );
            if !segs.commits[src_seg].is_empty()
                && (maybe_name.is_some() || info.parent_ids.len() >= 2)
            {
                let seg =
                    branch_segment_from_name_and_meta(maybe_name, meta, None, &worktree_by_branch)?;
                segs.add(&seg)
            } else {
                src_seg
            }
        };
        segs.push_commit(seg_for_id, id);

        let refs_at_commit_before_removal = refs_by_id.remove(&id).unwrap_or_default();
        let RemoteQueueOutcome {
            items_to_queue_later: remote_items_to_queue_later,
            maybe_make_id_a_goal_so_remote_can_find_local,
            limit_to_let_local_find_remote,
        } = try_queue_remote_tracking_branches_native(
            repo,
            &refs_at_commit_before_removal,
            &mut seeds,
            &mut ep_seed,
            &next,
            &initial_tips.symbolic_remote_names,
            &configured_remote_tracking_branches,
            &initial_tips.target_refs,
            meta,
            id,
            limit,
            &mut goals,
            &worktree_by_branch,
            commit_graph.as_ref(),
            repo.for_find_only(),
            &mut buf,
        )?;

        let propagated_flags = propagated_flags | maybe_make_id_a_goal_so_remote_can_find_local;
        queue_parents_native(
            &mut next,
            &info.parent_ids,
            propagated_flags,
            id,
            limit.additional_goal(limit_to_let_local_find_remote),
            is_shallow_boundary,
            commit_graph.as_ref(),
            repo.for_find_only(),
            &mut buf,
        )?;

        // Store the commit: flags inherit the queuing commit's CURRENT flags (`src_flags` above)
        // — the segment version inherits from the collecting segment's last commit, which IS the
        // queuing commit. Refs are the refs at the commit MINUS the owning segment's name (the
        // walk hoists that onto the segment; it is re-attached to the segment's FINAL first
        // commit after the traversal, following ownership swaps and splits), canonically SORTED.
        let mut refs: Vec<crate::RefInfo> = refs_at_commit_before_removal
            .into_iter()
            .filter(|rn| {
                segs.names[seg_for_id]
                    .as_ref()
                    .is_none_or(|ri| ri.ref_name != *rn)
            })
            .map(|rn| crate::RefInfo::from_ref(rn, id, &worktree_by_branch))
            .collect();
        refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        store.push(Commit {
            id,
            parent_ids: info.parent_ids.iter().copied().collect(),
            flags: propagated_flags,
            refs,
        });

        for item in remote_items_to_queue_later {
            if next.push_back_exhausted(item) {
                break;
            }
        }
        let ep_first_flags = ep_first_commit_flags(&store, &segs, &seed_seg, ep_seed);
        prune_integrated_tips_native(&mut next, ep_first_flags);
        if points_of_interest_to_traverse_first == 0 {
            next.sort();
        }
    }

    // Like the flatten: the ref of the segment OWNING the entrypoint commit — None after
    // ownership moved to an anonymous stand-in or an ambiguous split.
    let entrypoint_ref = segs
        .of
        .get(&tip)
        .and_then(|&s| segs.names[s].as_ref().map(|ri| ri.ref_name.clone()));
    // Re-attach every segment's hoisted name to its FINAL first commit. This is the
    // ADVANCED-REF presentation contract, not bookkeeping: when a named seed's ref points at a
    // commit the walk never reached (branch advanced outside the workspace, ref deleted, or an
    // overlay preview), the name still surfaces at the walk-visible position — census-verified:
    // every corpus divergence between `first` and the ref's true commit is exactly the
    // true-commit-not-in-graph case (~1.4k across the suite, mostly apply/unapply previews).
    for (seg, name) in segs.names.iter().enumerate() {
        let (Some(ri), Some(first)) = (name, segs.commits[seg].first()) else {
            continue;
        };
        let cix = store.by_id[first];
        let refs = &mut store.commits[cix].refs;
        if !refs.iter().any(|r| r.ref_name == ri.ref_name) {
            refs.push(ri.clone());
            refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        }
    }
    Ok(NativeOutcome {
        commits: store.commits,
        connected: store.connected,
        entrypoint: Some(tip),
        entrypoint_ref,
        hard_limit_hit: next.hard_limit_hit(),
        tips: initial_tips.tips.clone(),
    })
}

/// Native `queue_parents`: identical queue semantics; the payload carries the queuing commit.
#[allow(clippy::too_many_arguments)]
fn queue_parents_native(
    next: &mut Queue,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    current: gix::ObjectId,
    mut limit: Limit,
    is_shallow_boundary: bool,
    commit_graph: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    buf: &mut Vec<u8>,
) -> anyhow::Result<bool> {
    if is_shallow_boundary {
        return Ok(false);
    }
    if next.is_exhausted() {
        return Ok(next.hard_limit_hit());
    }
    if limit.is_exhausted_or_decrement(flags, next) {
        return Ok(false);
    }
    let mut queue_is_exhausted = false;
    if parent_ids.len() > 1 {
        let instr = Instruction {
            queued_by: Some(current),
            seed: None,
            new_segment: true,
        };
        let limit_per_parent = limit.per_parent(parent_ids.len());
        for pid in parent_ids.iter() {
            let info = find(commit_graph, objects, *pid, buf)?;
            queue_is_exhausted =
                next.push_back_even_if_exhausted((info, flags, instr, limit_per_parent));
        }
    } else if !parent_ids.is_empty() {
        let instr = Instruction {
            queued_by: Some(current),
            seed: None,
            new_segment: false,
        };
        let info = find(commit_graph, objects, parent_ids[0], buf)?;
        queue_is_exhausted |= next.push_back_exhausted((info, flags, instr, limit));
    }
    Ok(queue_is_exhausted)
}

/// The entrypoint segment's `non_empty_flags_of_first_commit()`: the first commit currently in
/// the entrypoint seed's segment (its contents evolve through swaps and splits), `None` while
/// empty or not yet materialized.
fn ep_first_commit_flags(
    store: &Store,
    segs: &Segs,
    seed_seg: &[usize],
    ep_seed: Option<usize>,
) -> Option<CommitFlags> {
    let first = *segs.commits[*seed_seg.get(ep_seed?)?].first()?;
    Some(store.flags(first)).filter(|f| !f.is_empty())
}

/// Native `prune_integrated_tips`: queue logic identical; the entrypoint-integrated check gets
/// the entrypoint segment's first-commit flags from [`ep_first_commit_flags`].
fn prune_integrated_tips_native(next: &mut Queue, ep_first_flags: Option<CommitFlags>) {
    if next.is_exhausted() {
        return;
    }
    let all_integrated_and_done = next.iter().all(|(_id, flags, _instr, tip_limit)| {
        flags.contains(CommitFlags::Integrated) && tip_limit.goal_reached()
    });
    if !all_integrated_and_done {
        return;
    }
    if ep_first_flags.is_some_and(|flags| flags.contains(CommitFlags::Integrated)) {
        return;
    }
    next.exhaust();
}

/// Native seeding: a faithful port of `queue_initial_tips`, with segments replaced by
/// throwaway seed records (created by the SAME `branch_segment_from_name_and_meta`).
#[allow(clippy::too_many_arguments)]
fn queue_initial_tips_native<T: RefMetadata>(
    next: &mut Queue,
    seeds: &mut Vec<Segment>,
    ep_seed: &mut Option<usize>,
    initial_tips: &InitialTips,
    entrypoint: gix::ObjectId,
    entrypoint_flags: CommitFlags,
    max_limit: Limit,
    target_limit: Limit,
    goals: &mut Goals,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::walk::RefsById,
    worktree_by_branch: &WorktreeByBranch,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    use std::collections::BTreeMap;
    struct PendingNative {
        id: gix::ObjectId,
        seed: usize,
        queue_front: bool,
    }
    let mut target_local_seeds =
        BTreeMap::<gix::refs::FullName, (Option<usize>, CommitFlags)>::new();
    let mut pending_integrated_tips = BTreeMap::<gix::refs::FullName, PendingNative>::new();

    #[allow(clippy::too_many_arguments)]
    fn queue_pending(
        next: &mut Queue,
        seeds: &[Segment],
        pending: PendingNative,
        local_goal: CommitFlags,
        target_limit: Limit,
        commit_graph: Option<&gix::commitgraph::Graph>,
        repo: &OverlayRepo<'_>,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let _ = seeds;
        let tip_info = find(commit_graph, repo.for_find_only(), pending.id, buf)?;
        let item: QueueItem = (
            tip_info,
            CommitFlags::Integrated,
            Instruction {
                queued_by: None,
                seed: Some(pending.seed),
                new_segment: false,
            },
            target_limit.additional_goal(local_goal),
        );
        if pending.queue_front {
            _ = next.push_front_exhausted(item);
        } else {
            _ = next.push_back_exhausted(item);
        }
        Ok(())
    }

    for tip in &initial_tips.tips {
        match &tip.role {
            TipRole::WorkspaceStackBranch { .. } if next.iter().any(|t| t.0.id == tip.id) => {
                next.add_goal_to(tip.id, goals.flag_for(entrypoint).unwrap_or_default());
                continue;
            }
            TipRole::TargetRemote
                if tip.is_auxiliary_integrated_tip(&initial_tips.auxiliary_integrated_tip_ids)
                    && next.iter().any(|(info, _, _, _)| info.id == tip.id) =>
            {
                continue;
            }
            _ => {}
        }

        let mut segment = branch_segment_from_name_and_meta(
            tip.ref_name
                .clone()
                .map(|ref_name| (ref_name, tip.metadata.clone())),
            meta,
            Some((refs_by_id, tip.id)),
            worktree_by_branch,
        )?;
        if let TipRole::WorkspaceStackBranch { desired_ref_name } = &tip.role {
            let is_remote = desired_ref_name
                .category()
                .is_some_and(|c| c == Category::RemoteBranch);
            if segment.ref_info.is_none() && is_remote {
                segment.ref_info = Some(crate::RefInfo::from_ref(
                    desired_ref_name.clone(),
                    tip.id,
                    worktree_by_branch,
                ));
                segment.metadata = meta
                    .branch_opt(desired_ref_name.as_ref())?
                    .map(crate::SegmentMetadata::Branch);
            }
        }
        let seed = seeds.len();
        seeds.push(segment);

        if let TipRole::TargetRemote = &tip.role {
            let pending = PendingNative {
                id: tip.id,
                seed,
                queue_front: super::queue_should_frontload_tip(
                    tip,
                    initial_tips.frontload_workspace_related_tips,
                    &initial_tips.auxiliary_integrated_tip_ids,
                ),
            };
            if let Some(target_ref) = tip
                .ref_name
                .as_ref()
                .filter(|ref_name| {
                    initial_tips
                        .target_local_links
                        .local_by_target
                        .contains_key(*ref_name)
                })
                .cloned()
            {
                let Some((_local_seed, local_goal)) = target_local_seeds.get(&target_ref).copied()
                else {
                    pending_integrated_tips.insert(target_ref, pending);
                    continue;
                };
                queue_pending(
                    next,
                    seeds,
                    pending,
                    local_goal,
                    target_limit,
                    commit_graph,
                    repo,
                    buf,
                )?;
            } else {
                queue_pending(
                    next,
                    seeds,
                    pending,
                    CommitFlags::empty(),
                    target_limit,
                    commit_graph,
                    repo,
                    buf,
                )?;
            }
            continue;
        }

        let (flags, limit) = match &tip.role {
            TipRole::Reachable if tip.is_entrypoint => {
                *ep_seed = Some(seed);
                (entrypoint_flags, max_limit)
            }
            TipRole::Reachable => {
                super::reachable_tip_flags_and_limit(tip.id, entrypoint, max_limit, goals)
            }
            TipRole::TargetRemote => unreachable!("handled above"),
            TipRole::Workspace => {
                if tip.is_entrypoint && ep_seed.is_none() {
                    *ep_seed = Some(seed);
                }
                let extra_flags = if tip.is_entrypoint {
                    entrypoint_flags
                } else {
                    CommitFlags::empty()
                };
                let limit = if tip.is_entrypoint {
                    max_limit
                } else {
                    max_limit.with_indirect_goal(entrypoint, goals)
                };
                (
                    CommitFlags::InWorkspace | CommitFlags::NotInRemote | extra_flags,
                    limit,
                )
            }
            TipRole::TargetLocal { local_ref_name } => {
                let has_remote_link = seeds[seed]
                    .ref_name()
                    .is_some_and(|ref_name| ref_name == local_ref_name.as_ref());
                let goal = goals.flag_for(tip.id).unwrap_or_default();
                if let Some(target_ref) = initial_tips
                    .target_local_links
                    .target_by_local
                    .get(local_ref_name)
                {
                    target_local_seeds
                        .insert(target_ref.clone(), (has_remote_link.then_some(seed), goal));
                }
                next.add_goal_to(entrypoint, goal);
                (CommitFlags::NotInRemote | goal, target_limit)
            }
            TipRole::WorkspaceStackBranch { .. } => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
        };
        let tip_info = find(commit_graph, repo.for_find_only(), tip.id, buf)?;
        let item: QueueItem = (
            tip_info,
            flags,
            Instruction {
                queued_by: None,
                seed: Some(seed),
                new_segment: false,
            },
            limit,
        );
        let paired_target_ref = match &tip.role {
            TipRole::TargetLocal { local_ref_name } => initial_tips
                .target_local_links
                .target_by_local
                .get(local_ref_name)
                .cloned(),
            _ => None,
        };
        let pending_before_current = paired_target_ref.as_ref().and_then(|target_ref| {
            pending_integrated_tips
                .get(target_ref)
                .is_some_and(|pending| pending.id == tip.id)
                .then(|| pending_integrated_tips.remove(target_ref))
                .flatten()
        });
        if let Some(pending) = pending_before_current {
            let (_seed, local_goal) = paired_target_ref
                .as_ref()
                .and_then(|target_ref| target_local_seeds.get(target_ref))
                .copied()
                .unwrap_or((None, CommitFlags::empty()));
            queue_pending(
                next,
                seeds,
                pending,
                local_goal,
                target_limit,
                commit_graph,
                repo,
                buf,
            )?;
        }
        if super::queue_should_frontload_tip(
            tip,
            initial_tips.frontload_workspace_related_tips,
            &initial_tips.auxiliary_integrated_tip_ids,
        ) {
            _ = next.push_front_exhausted(item);
        } else {
            _ = next.push_back_exhausted(item);
        }

        if let Some(target_ref) = paired_target_ref
            && let Some(pending) = pending_integrated_tips.remove(&target_ref)
        {
            let (_seed, local_goal) = target_local_seeds
                .get(&target_ref)
                .copied()
                .unwrap_or((None, CommitFlags::empty()));
            queue_pending(
                next,
                seeds,
                pending,
                local_goal,
                target_limit,
                commit_graph,
                repo,
                buf,
            )?;
        }
    }
    Ok(())
}

/// Native port of `prioritize_initial_tips_and_assure_ws_commit_ownership`, running the same
/// sort and swap logic against the seed table.
fn prioritize_and_assure_ws_ownership_native<T: RefMetadata>(
    next: &mut Queue,
    seeds: &mut Vec<Segment>,
    ep_seed: &mut Option<usize>,
    (ws_tips, repo, meta): (
        Vec<gix::ObjectId>,
        &OverlayRepo<'_>,
        &OverlayMetadata<'_, T>,
    ),
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<()> {
    #[derive(Ord, PartialOrd, PartialEq, Eq)]
    enum Kind {
        Local,
        Workspace,
        NonLocal,
    }
    {
        let seeds = &*seeds;
        next.inner
            .make_contiguous()
            .sort_by_key(|(_info, _flags, instr, _limit)| {
                instr
                    .seed
                    .and_then(|ix| seeds[ix].ref_name())
                    .map(|rn| match rn.category() {
                        Some(Category::LocalBranch) => {
                            if but_core::is_workspace_ref_name(rn) {
                                Kind::Workspace
                            } else {
                                Kind::Local
                            }
                        }
                        _ => Kind::NonLocal,
                    })
            });
    }

    'next_ws_tip: for ws_tip in ws_tips {
        if crate::workspace::commit::is_managed_workspace_by_message(
            repo.find_commit(ws_tip)?.message_raw()?,
        ) {
            if next.iter().filter(|(info, ..)| info.id == ws_tip).count() <= 1 {
                continue 'next_ws_tip;
            }
            let mut with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (info, _, instr, _))| {
                        (info.id == ws_tip).then_some((idx, instr.seed))
                    });
            let (first, second) = (
                with_ws_tip.next().expect("at least two"),
                with_ws_tip.next().expect("at least two"),
            );
            let first_is_ws = first
                .1
                .is_some_and(|ix| seeds[ix].workspace_metadata().is_some());
            if first_is_ws {
                continue 'next_ws_tip;
            }
            drop(with_ws_tip);
            next.inner.swap(first.0, second.0);
        } else if next.iter().filter(|(info, ..)| info.id == ws_tip).count() >= 2 {
            let mut with_ws_tip =
                next.iter()
                    .enumerate()
                    .filter_map(|(idx, (info, _, instr, _))| {
                        (info.id == ws_tip).then_some((idx, instr.seed))
                    });
            let (first, second) = (
                with_ws_tip.next().expect("at least two"),
                with_ws_tip.next().expect("at least two"),
            );
            let first_is_ws = first
                .1
                .is_some_and(|ix| seeds[ix].workspace_metadata().is_some());
            if !first_is_ws {
                continue 'next_ws_tip;
            }
            drop(with_ws_tip);
            next.inner.swap(first.0, second.0);
        } else {
            // Single tip on an unmanaged workspace commit with workspace metadata: an anonymous
            // stand-in owns the commit first (a duplicate front item; the real item then takes
            // the re-encounter path).
            let (info, flags, _instr, limit) = next
                .iter()
                .find(|t| t.0.id == ws_tip)
                .cloned()
                .expect("each ws-tip has one entry on queue");
            let anon = branch_segment_from_name_and_meta(None, meta, None, worktree_by_branch)?;
            let seed = seeds.len();
            seeds.push(anon);
            if ep_seed.is_none() {
                *ep_seed = Some(seed);
            }
            _ = next.push_front_exhausted((
                info,
                flags,
                Instruction {
                    queued_by: None,
                    seed: Some(seed),
                    new_segment: false,
                },
                limit,
            ));
        }
    }
    Ok(())
}

/// Native port of `try_queue_remote_tracking_branches`: same lookup, goals, and limits; the
/// proxy segment becomes a seed record (its name is used for the double-queue check).
#[allow(clippy::too_many_arguments)]
fn try_queue_remote_tracking_branches_native<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    seeds: &mut Vec<Segment>,
    ep_seed: &mut Option<usize>,
    next: &Queue,
    target_symbolic_remote_names: &[String],
    configured_remote_tracking_branches: &std::collections::BTreeSet<gix::refs::FullName>,
    target_refs: &[gix::refs::FullName],
    meta: &OverlayMetadata<'_, T>,
    id: gix::ObjectId,
    limit: Limit,
    goals: &mut Goals,
    worktree_by_branch: &WorktreeByBranch,
    commit_graph: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    buf: &mut Vec<u8>,
) -> anyhow::Result<RemoteQueueOutcome> {
    let mut goal_flags = CommitFlags::empty();
    let mut limit_flags = CommitFlags::empty();
    let mut queue = Vec::new();
    for rn in refs {
        let Some(remote_tracking_branch) =
            super::remotes::lookup_remote_tracking_branch_or_deduce_it(
                repo,
                rn.as_ref(),
                target_symbolic_remote_names,
                configured_remote_tracking_branches,
            )?
        else {
            continue;
        };
        if target_refs.contains(&remote_tracking_branch) {
            continue;
        }
        let Some(remote_tip) =
            super::walk::try_refname_to_id(repo, remote_tracking_branch.as_ref())?
        else {
            continue;
        };
        if next.iter().any(|t| {
            t.0.id == remote_tip
                && t.2
                    .seed
                    .and_then(|ix| seeds[ix].ref_name())
                    .is_some_and(|sn| sn == remote_tracking_branch.as_ref())
        }) {
            continue;
        }
        let seg = branch_segment_from_name_and_meta(
            Some((remote_tracking_branch.clone(), None)),
            meta,
            None,
            worktree_by_branch,
        )?;
        let seed = seeds.len();
        seeds.push(seg);
        if ep_seed.is_none() {
            *ep_seed = Some(seed);
        }

        let remote_limit = limit.with_indirect_goal(id, goals);
        let self_flags = goals.flag_for(remote_tip).unwrap_or_default();
        limit_flags |= self_flags;
        goal_flags |= remote_limit.goal_flags();
        let remote_tip_info = find(commit_graph, objects, remote_tip, buf)?;
        queue.push((
            remote_tip_info,
            self_flags,
            Instruction {
                queued_by: None,
                seed: Some(seed),
                new_segment: false,
            },
            remote_limit,
        ));
    }
    Ok(RemoteQueueOutcome {
        items_to_queue_later: queue,
        maybe_make_id_a_goal_so_remote_can_find_local: goal_flags,
        limit_to_let_local_find_remote: limit_flags,
    })
}
