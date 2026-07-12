//! The walker: the traversal that accumulates the [`CommitGraph`](crate::CommitGraph) —
//! commits in visit order with ordered parent arrays, refs attached as data (per-commit
//! ref order canonicalized by name), flags propagated until the partitions reconcile.
//! [`Queue`], [`Limit`] and [`Goals`] pace the expansion.
//!
//! Seeding decisions (naming, metadata, workspace ownership) delegate to
//! [`branch_segment_from_name_and_meta`] over throwaway [`SeedSegment`] carriers in a
//! side table — the seed table — so the initial-queue sort and ownership shuffles run
//! one code path whether or not a graph exists yet.

use gix::hashtable::HashMap;

use but_core::RefMetadata;
use gix::reference::Category;

use super::utils::SeedSegment;
use super::{
    InitialSeeds, Options, Seed, SeedRole,
    overlay::{OverlayMetadata, OverlayRepo},
    types::{Goals, Instruction, Limit, Queue, QueueItem},
    utils::{RemoteQueueOutcome, WorktreeByBranch, branch_segment_from_name_and_meta, find},
};
use crate::{Commit, CommitFlags};

/// What the walk produces; [`CommitGraph`](crate::CommitGraph) assembly input.
pub(crate) struct WalkOutcome {
    /// Commits in collection order, flags final.
    pub commits: Vec<Commit>,
    /// Each commit's index in `commits`, as maintained by the walk.
    pub by_id: HashMap<gix::ObjectId, usize>,
    /// The parents the traversal actually connected, per child.
    pub parents_followed: HashMap<gix::ObjectId, Vec<gix::ObjectId>>,
    pub entrypoint: Option<gix::ObjectId>,
    pub entrypoint_ref: Option<gix::refs::FullName>,
    pub hard_limit_hit: bool,
    /// The normalized traversal seeds, carried onto the graph for later passes.
    pub seeds: Vec<Seed>,
}

/// Commit store: flags mutable by id, children derived from followed edges.
#[derive(Default)]
struct Store {
    commits: Vec<Commit>,
    by_id: HashMap<gix::ObjectId, usize>,
    /// Followed parents per child — edges point down the history.
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
    /// Record that the traversal CONNECTED `child -> parent`. Parents per child are few, so a
    /// linear dedup beats hashing `(child, parent)` pairs.
    fn connect(&mut self, child: Option<gix::ObjectId>, parent: gix::ObjectId) {
        if let Some(child) = child {
            let parents = self.parents_followed.entry(child).or_default();
            if !parents.contains(&parent) {
                parents.push(parent);
            }
        }
    }
    /// `flags(x) |= add` for `start` and every stored ANCESTOR reached via followed edges, each
    /// visited once. Returns the visited commits when `needs_visited` (for the caller's
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

/// The walk's own segment bookkeeping — which segment each collected commit belongs to. Three
/// decisions read it: which queued tips inherit goals at a re-encounter (leaf SEGMENTS, not
/// leaf commits), the entrypoint owner's name and first-commit flags (the integrated-cutoff
/// gate), and the workspace-ref attachment. Boundaries: every seed is a segment; merge parents
/// start new ones; a continuation splits off at an unambiguous branch name or a merge; a
/// mid-segment re-encounter splits the tail.

#[derive(Default)]
struct Segs {
    names: Vec<Option<crate::RefInfo>>,
    commits: Vec<Vec<gix::ObjectId>>,
    of: HashMap<gix::ObjectId, usize>,
}

impl Segs {
    fn add(&mut self, seg: &SeedSegment) -> usize {
        let ix = self.names.len();
        self.names.push(seg.ref_info.clone());
        self.commits.push(Vec::new());
        ix
    }
    /// Seed-table entries created since the last call become segments; `seed_seg` maps seed → seg.
    fn sync_seeds(&mut self, seed_segments: &[SeedSegment], seed_seg: &mut Vec<usize>) {
        while seed_seg.len() < seed_segments.len() {
            let seg = self.add(&seed_segments[seed_seg.len()]);
            seed_seg.push(seg);
        }
    }
    fn push_commit(&mut self, seg: usize, id: gix::ObjectId) {
        self.commits[seg].push(id);
        self.of.insert(id, seg);
    }
    /// `commits[pos..]` become their own (anonymous) segment.
    fn split_off_tail(&mut self, seg: usize, pos: usize) {
        let tail = self.commits[seg].split_off(pos);
        let new_seg = self.names.len();
        self.names.push(None);
        for id in &tail {
            self.of.insert(*id, new_seg);
        }
        self.commits.push(tail);
    }
    /// The segment a queue item collects into — seed_segments into their seed segment, parents into the
    /// queuing commit's CURRENT segment (splits and swaps are picked up live, which is what the
    /// walk's queue-rewriting on split achieves).
    fn landing(&self, instr: &Instruction, seed_seg: &[usize]) -> Option<usize> {
        instr
            .seed
            .map(|ix| seed_seg[ix])
            .or_else(|| instr.queued_by.map(|qb| self.of[&qb]))
    }
    /// Does any commit of this segment have a followed edge into a different segment below it?
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

/// THE commit traversal: a queue-driven walk from the seeds under the workspace limits,
/// storing commits directly.
#[allow(clippy::too_many_arguments)]
#[tracing::instrument(name = "walker::traverse", level = "trace", skip_all, err(Debug))]
pub(crate) fn traverse<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    seeds: Vec<Seed>,
    meta: &OverlayMetadata<'_, T>,
    project_meta: but_core::ref_metadata::ProjectMeta,
    options: Options,
    ref_name_override: Option<gix::refs::FullName>,
) -> anyhow::Result<WalkOutcome> {
    let entrypoint_seed = super::validate_explicit_seeds(repo, &seeds, ref_name_override.as_ref())?;
    let entrypoint_id = entrypoint_seed.id;
    let detach_entrypoint = entrypoint_seed.is_detached;
    let ref_name = if detach_entrypoint {
        None
    } else {
        ref_name_override.or_else(|| entrypoint_seed.ref_name.clone())
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
    let initial_seeds =
        super::assemble_initial_seeds(repo, seeds, &project_meta, extra_target_commit_id);
    let mut refs_by_id = repo.collect_ref_mapping_by_prefix(
        ["refs/heads/", "refs/remotes/"]
            .into_iter()
            .chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
        &initial_seeds
            .workspace_ref_names
            .iter()
            .map(|ref_name| ref_name.as_ref())
            .collect::<Vec<_>>(),
    )?;
    let worktree_by_branch = repo.worktree_branches(ref_name.as_ref().map(|r| r.as_ref()))?;

    let mut goals = Goals::default();
    let tip_flags = CommitFlags::NotInRemote
        | goals
            .flag_for(entrypoint_id)
            .expect("we have more than one bitflag for this");

    let mut store = Store::default();
    let mut seen = gix::hashtable::HashSet::default();
    let mut next: Queue = Queue::new_with_limit(hard_limit);
    let mut seed_segments: Vec<SeedSegment> = Vec::new();
    let mut segs = Segs::default();
    let mut seed_seg: Vec<usize> = Vec::new();
    // The seed whose segment `graph.entrypoint` points at — its (evolving) first commit gates
    // `prune_integrated_tips`.
    let mut ep_seed: Option<usize> = None;

    let target_limit = max_limit
        .with_indirect_goal(entrypoint_id, &mut goals)
        .without_allowance();

    queue_initial_seeds(
        &mut next,
        &mut seed_segments,
        &mut ep_seed,
        &initial_seeds,
        entrypoint_id,
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
    prioritize_and_ensure_ws_ownership(
        &mut next,
        &mut seed_segments,
        &mut ep_seed,
        (initial_seeds.workspace_seeds.clone(), repo, meta),
        &worktree_by_branch,
    )?;

    max_commits_recharge_location.sort();
    let mut points_of_interest_to_traverse_first = next.inner.len();
    while let Some((info, mut propagated_flags, instr, mut limit)) = next.pop_front() {
        points_of_interest_to_traverse_first =
            points_of_interest_to_traverse_first.saturating_sub(1);

        let id = info.id;
        segs.sync_seeds(&seed_segments, &mut seed_seg);
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
            re_encounter(
                &mut store,
                &mut segs,
                &mut next,
                &seed_seg,
                id,
                instr.queued_by,
                propagated_flags,
                limit,
            );
            // NB: the walk `continue`s straight past prune and sort on this path.
            continue;
        }
        seen.insert(id);
        store.connect(instr.queued_by, id);

        let seg_for_id = segment_for_commit(
            &mut segs,
            &instr,
            &seed_seg,
            &info,
            id,
            meta,
            &refs_by_id,
            &worktree_by_branch,
        )?;
        segs.push_commit(seg_for_id, id);

        let refs_at_commit_before_removal = refs_by_id.remove(&id).unwrap_or_default();
        let RemoteQueueOutcome {
            items_to_queue_later: remote_items_to_queue_later,
            maybe_make_id_a_goal_so_remote_can_find_local,
            limit_to_let_local_find_remote,
        } = try_queue_remote_tracking_branches(
            repo,
            &refs_at_commit_before_removal,
            &mut seed_segments,
            &mut ep_seed,
            &next,
            &initial_seeds.symbolic_remote_names,
            &configured_remote_tracking_branches,
            &initial_seeds.target_refs,
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
        queue_parents(
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

        // Store the commit: flags inherit the queuing commit's CURRENT flags (`src_flags`
        // above); refs are ALL the refs at the commit, canonically sorted by name.
        let mut refs: Vec<crate::RefInfo> = refs_at_commit_before_removal
            .into_iter()
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
        let ep_first_flags = entrypoint_first_commit_flags(&store, &segs, &seed_seg, ep_seed);
        prune_integrated_tips(&mut next, ep_first_flags);
        if points_of_interest_to_traverse_first == 0 {
            next.sort();
        }
    }

    // The ref of the segment OWNING the entrypoint commit — `None` once ownership moved to an
    // anonymous stand-in or an ambiguous split. Only the walk can know this: ownership evolves
    // DURING traversal, so the build cannot re-derive it afterwards. It is the walk's only
    // naming output besides the ws-ref attachment below; all other segment naming lives in
    // the build.
    let entrypoint_ref = segs
        .of
        .get(&entrypoint_id)
        .and_then(|&s| segs.names[s].as_ref().map(|ri| ri.ref_name.clone()));
    attach_workspace_refs(&mut store, &segs, &initial_seeds.workspace_ref_names);
    Ok(WalkOutcome {
        commits: store.commits,
        by_id: store.by_id,
        parents_followed: store.parents_followed,
        entrypoint: Some(entrypoint_id),
        entrypoint_ref,
        hard_limit_hit: next.hard_limit_hit(),
        seeds: initial_seeds.seeds,
    })
}

/// Queue the initial seeds, with segments replaced by throwaway seed records (created by
/// the SAME `branch_segment_from_name_and_meta`).
#[allow(clippy::too_many_arguments)]
fn queue_initial_seeds<T: RefMetadata>(
    next: &mut Queue,
    seed_segments: &mut Vec<SeedSegment>,
    ep_seed: &mut Option<usize>,
    initial_seeds: &InitialSeeds,
    entrypoint: gix::ObjectId,
    entrypoint_flags: CommitFlags,
    max_limit: Limit,
    target_limit: Limit,
    goals: &mut Goals,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::utils::RefsById,
    worktree_by_branch: &WorktreeByBranch,
    buf: &mut Vec<u8>,
) -> anyhow::Result<()> {
    let mut pairing = PendingTargets {
        target_limit,
        commit_graph,
        repo,
        local_goals: Default::default(),
        parked: Default::default(),
    };

    for seed in &initial_seeds.seeds {
        match &seed.role {
            SeedRole::WorkspaceStackBranch { .. } if next.iter().any(|t| t.0.id == seed.id) => {
                next.add_goal_to(seed.id, goals.flag_for(entrypoint).unwrap_or_default());
                continue;
            }
            SeedRole::TargetRemote
                if seed
                    .is_auxiliary_integrated_seed(&initial_seeds.auxiliary_integrated_seed_ids)
                    && next.iter().any(|(info, _, _, _)| info.id == seed.id) =>
            {
                continue;
            }
            _ => {}
        }

        let segment = mint_seed_segment(seed, meta, refs_by_id, worktree_by_branch)?;
        let seg = seed_segments.len();
        seed_segments.push(segment);

        if let SeedRole::TargetRemote = &seed.role {
            let pending = PendingSeed {
                id: seed.id,
                seed: seg,
                queue_front: super::queue_should_frontload_seed(
                    seed,
                    initial_seeds.frontload_workspace_related_seeds,
                    &initial_seeds.auxiliary_integrated_seed_ids,
                ),
            };
            let target_ref = seed
                .ref_name
                .as_ref()
                .filter(|ref_name| {
                    initial_seeds
                        .target_local_links
                        .local_by_target
                        .contains_key(*ref_name)
                })
                .cloned();
            pairing.park_or_queue(next, target_ref, pending, buf)?;
            continue;
        }

        let (flags, limit) = match &seed.role {
            SeedRole::Reachable if seed.is_entrypoint => {
                *ep_seed = Some(seg);
                (entrypoint_flags, max_limit)
            }
            SeedRole::Reachable => {
                super::reachable_seed_flags_and_limit(seed.id, entrypoint, max_limit, goals)
            }
            SeedRole::TargetRemote => unreachable!("handled above"),
            SeedRole::Workspace => {
                if seed.is_entrypoint && ep_seed.is_none() {
                    *ep_seed = Some(seg);
                }
                let extra_flags = if seed.is_entrypoint {
                    entrypoint_flags
                } else {
                    CommitFlags::empty()
                };
                let limit = if seed.is_entrypoint {
                    max_limit
                } else {
                    max_limit.with_indirect_goal(entrypoint, goals)
                };
                (
                    CommitFlags::InWorkspace | CommitFlags::NotInRemote | extra_flags,
                    limit,
                )
            }
            SeedRole::TargetLocal { local_ref_name } => {
                let goal = goals.flag_for(seed.id).unwrap_or_default();
                if let Some(target_ref) = initial_seeds
                    .target_local_links
                    .target_by_local
                    .get(local_ref_name)
                {
                    pairing.local_goals.insert(target_ref.clone(), goal);
                }
                next.add_goal_to(entrypoint, goal);
                (CommitFlags::NotInRemote | goal, target_limit)
            }
            SeedRole::WorkspaceStackBranch { .. } => (
                CommitFlags::NotInRemote,
                max_limit.with_indirect_goal(entrypoint, goals),
            ),
        };
        let item = seed_item(
            commit_graph,
            repo.for_find_only(),
            seed.id,
            seg,
            flags,
            limit,
            buf,
        )?;
        let paired_target_ref = match &seed.role {
            SeedRole::TargetLocal { local_ref_name } => initial_seeds
                .target_local_links
                .target_by_local
                .get(local_ref_name)
                .cloned(),
            _ => None,
        };
        // A parked target pointing at the LOCAL'S OWN commit queues before the local...
        if let Some(target_ref) = &paired_target_ref
            && pairing
                .parked
                .get(target_ref)
                .is_some_and(|pending| pending.id == seed.id)
        {
            pairing.release(next, target_ref, buf)?;
        }
        if super::queue_should_frontload_seed(
            seed,
            initial_seeds.frontload_workspace_related_seeds,
            &initial_seeds.auxiliary_integrated_seed_ids,
        ) {
            _ = next.push_front_exhausted(item);
        } else {
            _ = next.push_back_exhausted(item);
        }
        // ...any other parked target follows it.
        if let Some(target_ref) = &paired_target_ref {
            pairing.release(next, target_ref, buf)?;
        }
    }
    Ok(())
}

/// Mint the queue item for seed `seed`'s tip `id`: looked up fresh, queued by nobody,
/// never starting a new segment.
fn seed_item(
    cache: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    id: gix::ObjectId,
    seed: usize,
    flags: CommitFlags,
    limit: Limit,
    buf: &mut Vec<u8>,
) -> anyhow::Result<QueueItem> {
    Ok((
        find(cache, objects, id, buf)?,
        flags,
        Instruction {
            queued_by: None,
            seed: Some(seed),
            new_segment: false,
        },
        limit,
    ))
}

/// A target seed parked until it can queue: where it points, its seed-table parent number, and
/// which queue end it wants.
struct PendingSeed {
    id: gix::ObjectId,
    seed: usize,
    queue_front: bool,
}

/// The target/local pairing ledger of the initial-seed queueing: a target seed must
/// queue with its LOCAL partner's goal flag in its limit (so the target's walk can find
/// the local), so a target arriving before its local PARKS here and is released when
/// the local passes by — before the local's own queue item when both point at the same
/// commit, right after it otherwise.
struct PendingTargets<'a, 'repo> {
    target_limit: Limit,
    commit_graph: Option<&'a gix::commitgraph::Graph>,
    repo: &'a OverlayRepo<'repo>,
    /// Per target ref: its local partner's goal flag, recorded when the local queues.
    local_goals: std::collections::BTreeMap<gix::refs::FullName, CommitFlags>,
    /// Targets parked until their local partner passes by.
    parked: std::collections::BTreeMap<gix::refs::FullName, PendingSeed>,
}

impl PendingTargets<'_, '_> {
    /// Queue `pending` as an integrated tip whose limit carries `local_goal`.
    fn queue(
        &self,
        next: &mut Queue,
        pending: PendingSeed,
        local_goal: CommitFlags,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let item = seed_item(
            self.commit_graph,
            self.repo.for_find_only(),
            pending.id,
            pending.seed,
            CommitFlags::Integrated,
            self.target_limit.additional_goal(local_goal),
            buf,
        )?;
        if pending.queue_front {
            _ = next.push_front_exhausted(item);
        } else {
            _ = next.push_back_exhausted(item);
        }
        Ok(())
    }

    /// A target seed arrives: queue it right away when it has no local partner (no goal
    /// to wait for) or when the partner's goal is already on the ledger — park it
    /// otherwise.
    fn park_or_queue(
        &mut self,
        next: &mut Queue,
        target_ref: Option<gix::refs::FullName>,
        pending: PendingSeed,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let Some(target_ref) = target_ref else {
            return self.queue(next, pending, CommitFlags::empty(), buf);
        };
        match self.local_goals.get(&target_ref).copied() {
            Some(local_goal) => self.queue(next, pending, local_goal, buf),
            None => {
                self.parked.insert(target_ref, pending);
                Ok(())
            }
        }
    }

    /// Release the target parked for `target_ref`, if any, with the ledger's goal flag.
    fn release(
        &mut self,
        next: &mut Queue,
        target_ref: &gix::refs::FullName,
        buf: &mut Vec<u8>,
    ) -> anyhow::Result<()> {
        let Some(pending) = self.parked.remove(target_ref) else {
            return Ok(());
        };
        let local_goal = self
            .local_goals
            .get(target_ref)
            .copied()
            .unwrap_or(CommitFlags::empty());
        self.queue(next, pending, local_goal, buf)
    }
}

/// Mint the seed's table record via the shared namer; a workspace stack branch whose
/// desired name is REMOTE and resolved to nothing still records that remote name.
fn mint_seed_segment<T: RefMetadata>(
    seed: &Seed,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::utils::RefsById,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<SeedSegment> {
    let mut segment = branch_segment_from_name_and_meta(
        seed.ref_name
            .clone()
            .map(|ref_name| (ref_name, seed.metadata.clone())),
        meta,
        Some((refs_by_id, seed.id)),
        worktree_by_branch,
    )?;
    if let SeedRole::WorkspaceStackBranch { desired_ref_name } = &seed.role {
        let is_remote = desired_ref_name
            .category()
            .is_some_and(|c| c == Category::RemoteBranch);
        if segment.ref_info.is_none() && is_remote {
            segment.ref_info = Some(crate::RefInfo::from_ref(
                desired_ref_name.clone(),
                seed.id,
                worktree_by_branch,
            ));
        }
    }
    Ok(segment)
}
/// Swap the first two queued items on `ws_tip` when the first one's workspace-seed-ness
/// equals `swap_when_first_is_ws`; no-op with fewer than two items on the tip.
fn swap_first_two_on_tip_if(
    next: &mut Queue,
    seed_segments: &[SeedSegment],
    ws_tip: gix::ObjectId,
    swap_when_first_is_ws: bool,
) {
    let mut with_ws_tip = next
        .iter()
        .enumerate()
        .filter_map(|(idx, (info, _, instr, _))| (info.id == ws_tip).then_some((idx, instr.seed)));
    let (Some(first), Some(second)) = (with_ws_tip.next(), with_ws_tip.next()) else {
        return;
    };
    drop(with_ws_tip);
    let first_is_ws = first
        .1
        .is_some_and(|ix| seed_segments[ix].workspace_metadata().is_some());
    if first_is_ws == swap_when_first_is_ws {
        next.inner.swap(first.0, second.0);
    }
}

/// Prioritize the initial seeds and assure workspace-commit ownership: the initial-queue
/// sort and swap logic, run against the seed table.
fn prioritize_and_ensure_ws_ownership<T: RefMetadata>(
    next: &mut Queue,
    seed_segments: &mut Vec<SeedSegment>,
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
        let seed_segments = &*seed_segments;
        next.inner
            .make_contiguous()
            .sort_by_key(|(_info, _flags, instr, _limit)| {
                instr
                    .seed
                    .and_then(|ix| seed_segments[ix].ref_name())
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

    for ws_tip in ws_tips {
        if crate::workspace::commit::is_managed_workspace_by_message(
            repo.find_commit(ws_tip)?.message_raw()?,
        ) {
            // A non-workspace seed arriving first would own the managed commit — the
            // workspace seed must go second-to-first.
            swap_first_two_on_tip_if(next, seed_segments, ws_tip, false);
        } else if next
            .iter()
            .filter(|(info, ..)| info.id == ws_tip)
            .take(2)
            .count()
            >= 2
        {
            // Unmanaged commit: the WORKSPACE seed must NOT own it — swap it back.
            swap_first_two_on_tip_if(next, seed_segments, ws_tip, true);
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
            let seed = seed_segments.len();
            seed_segments.push(anon);
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
/// Re-encounter of an already-collected commit: record the edge, run the
/// segment-ownership choreography (mid-segment split), merge flags, propagate them
/// downward, and adjust queued tips' goals and limits.
#[allow(clippy::too_many_arguments)]
fn re_encounter(
    store: &mut Store,
    segs: &mut Segs,
    next: &mut Queue,
    seed_seg: &[usize],
    id: gix::ObjectId,
    queued_by: Option<gix::ObjectId>,
    propagated_flags: CommitFlags,
    limit: Limit,
) {
    store.connect(queued_by, id);
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
            && next
                .iter()
                .any(|(_, _, _, tip_limit)| !tip_limit.goal_flags().contains(limit.goal_flags())))
    {
        store.propagate_flags_downward(new_flags, id, needs_leafs)
    } else {
        None
    };
    if let Some(visited) = visited {
        // Tips scheduled to land in one of the propagation's leaf segments — visited,
        // no outgoing edges — inherit the goal.
        let leaf_segs: std::collections::BTreeSet<usize> = visited
            .iter()
            .map(|c| segs.of[c])
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .filter(|&s| !segs.has_outgoing(s, &store.parents_followed))
            .collect();
        let goal_flags = limit.goal_flags();
        for (_, _, item_instr, item_limit) in next.iter_mut() {
            if segs
                .landing(item_instr, seed_seg)
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
    for queued_tip_limit in next
        .iter_mut()
        .filter_map(|(_, _, _, l)| l.goal_flags().intersects(bottom_commit_goals).then_some(l))
    {
        queued_tip_limit.adjust_limit_if_bigger(limit);
    }
}

/// Which segment does this commit join? Seeds own their segment; a merge parent starts a
/// new one; a continuation splits off when the commit carries an unambiguous branch name
/// or is itself a merge — decided BEFORE the refs are consumed.
#[allow(clippy::too_many_arguments)]
fn segment_for_commit<T: RefMetadata>(
    segs: &mut Segs,
    instr: &Instruction,
    seed_seg: &[usize],
    info: &super::utils::TraverseInfo,
    id: gix::ObjectId,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::utils::RefsById,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<usize> {
    Ok(if let Some(ix) = instr.seed {
        seed_seg[ix]
    } else if instr.new_segment {
        let seg = branch_segment_from_name_and_meta(
            None,
            meta,
            Some((refs_by_id, id)),
            worktree_by_branch,
        )?;
        segs.add(&seg)
    } else {
        let src_seg = segs.of[&instr
            .queued_by
            .expect("non-seed items have a queuing commit")];
        let maybe_name =
            super::utils::disambiguate_refs_by_branch_metadata_with_lookup((refs_by_id, id), meta);
        if !segs.commits[src_seg].is_empty() && (maybe_name.is_some() || info.parent_ids.len() >= 2)
        {
            let seg =
                branch_segment_from_name_and_meta(maybe_name, meta, None, worktree_by_branch)?;
            segs.add(&seg)
        } else {
            src_seg
        }
    })
}

/// Attach each workspace ref to its seed segment's first commit — the ws ref never enters
/// ref collection, so it must come back as data here. This is the ADVANCED-REF presentation
/// contract, not bookkeeping: when the ref points at a commit the walk never reached
/// (advanced outside the workspace, overlay preview), the name still surfaces at the
/// walk-visible position.
fn attach_workspace_refs(store: &mut Store, segs: &Segs, ws_ref_names: &[gix::refs::FullName]) {
    for (seg, name) in segs.names.iter().enumerate() {
        let (Some(ri), Some(first)) = (name, segs.commits[seg].first()) else {
            continue;
        };
        if !ws_ref_names.contains(&ri.ref_name) {
            continue;
        }
        let cix = store.by_id[first];
        let refs = &mut store.commits[cix].refs;
        if !refs.iter().any(|r| r.ref_name == ri.ref_name) {
            refs.push(ri.clone());
            refs.sort_by(|a, b| a.ref_name.cmp(&b.ref_name));
        }
    }
}

/// Queue every parent of `commit` (goals, limits, flag propagation); the payload carries the
/// queuing commit.
#[allow(clippy::too_many_arguments)]
fn queue_parents(
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

/// The flags of the first commit currently in the entrypoint seed's segment (its contents
/// evolve through swaps and splits), `None` while empty or not yet materialized.
fn entrypoint_first_commit_flags(
    store: &Store,
    segs: &Segs,
    seed_seg: &[usize],
    ep_seed: Option<usize>,
) -> Option<CommitFlags> {
    let first = *segs.commits[*seed_seg.get(ep_seed?)?].first()?;
    Some(store.flags(first)).filter(|f| !f.is_empty())
}

/// Drop queued tips that are already integrated; the entrypoint-integrated check gets
/// the entrypoint segment's first-commit flags from [`entrypoint_first_commit_flags`].
fn prune_integrated_tips(next: &mut Queue, ep_first_flags: Option<CommitFlags>) {
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

/// Queue the remote-tracking branches of newly seen local branches (lookup, goals, limits);
/// the proxy segment becomes a seed record (its name is used for the double-queue check).
#[allow(clippy::too_many_arguments)]
fn try_queue_remote_tracking_branches<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    seed_segments: &mut Vec<SeedSegment>,
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
            super::utils::try_refname_to_id(repo, remote_tracking_branch.as_ref())?
        else {
            continue;
        };
        if next.iter().any(|t| {
            t.0.id == remote_tip
                && t.2
                    .seed
                    .and_then(|ix| seed_segments[ix].ref_name())
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
        let seed = seed_segments.len();
        seed_segments.push(seg);
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
