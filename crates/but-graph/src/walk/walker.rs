//! The walker: the traversal that accumulates the [`CommitGraph`](crate::CommitGraph) —
//! commits in visit order with ordered parent arrays, refs attached as data (per-commit
//! ref order canonicalized by name), flags propagated until the partitions reconcile.
//! [`Queue`], [`Limit`] and [`Goals`] pace the expansion.
//!
//! Seeding decisions (naming, metadata, workspace ownership) delegate to
//! [`resolve_ref_and_meta`], which records each seed's resolved (ref, metadata) pair as a
//! [`ResolvedSeed`] in a side table, so the initial-queue sort and ownership shuffles run
//! one code path whether or not a graph exists yet.
//!
//! READING ORDER: the driver (`traverse`) first, then the stores it writes to, then the
//! seeding protocols that mint the initial queue — including the target/local parking
//! dance and the workspace-ownership shuffle — and finally the loop's phases in the
//! order the driver runs them: re-encounters, segmentation, remote discovery, parent
//! queueing, the integrated-tips exit, and the workspace-ref epilogue. The pacing rules
//! these phases obey are the parent module's "How the walk decides to stop".

use gix::hashtable::HashMap;

use but_core::RefMetadata;
use gix::reference::Category;

use super::assemble::InitialSeeds;
use super::seed::validate_explicit_seeds;
use super::utils::ResolvedSeed;
use super::{
    Options, Seed, SeedRole,
    overlay::{OverlayMetadata, OverlayRepo},
    types::{Goals, Instruction, Limit, Queue, QueueItem},
    utils::{RemoteQueueOutcome, WorktreeByBranch, find, resolve_ref_and_meta},
};
use crate::{Commit, CommitFlags};

// ── The driver ──

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
    /// Whether the commit budget cut an extent short, leaving history unwalked below it.
    pub limit_hint_hit: bool,
    /// The normalized traversal seeds, carried onto the graph for later passes.
    pub seeds: Vec<Seed>,
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
    let entrypoint_seed = validate_explicit_seeds(repo, &seeds, ref_name_override.as_ref())?;
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
        worktree_tips,
        worktrees: _,
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
    let initial_seeds = super::assemble::assemble_initial_seeds(
        repo,
        seeds,
        &project_meta,
        extra_target_commit_id,
        worktree_tips,
    );
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
    // Per SEEKER (seed descent), the goal bits any of its items already found: a goal's
    // purpose is the seed's connection, so one item's find retires its siblings' hunts.
    let mut reached_by_seeker: Vec<CommitFlags> = Vec::new();
    let mut resolved_seeds: Vec<ResolvedSeed> = Vec::new();
    // Per-seed first collected commit — what consumers 2+3 of the old seg model read.
    let mut seed_first: Vec<Option<gix::ObjectId>> = Vec::new();
    // The seed holding the entrypoint role: the entrypoint seed when one is queued, else the first
    // stand-in or remote seed to take it. Its (evolving) first commit gates `prune_integrated_tips`.
    let mut ep_seed: Option<usize> = None;

    let target_limit = max_limit
        .with_indirect_goal(entrypoint_id, &mut goals)
        .without_allowance();

    let skipped_auxiliary_seeds = queue_initial_seeds(
        &mut next,
        &mut resolved_seeds,
        &mut ep_seed,
        &initial_seeds,
        entrypoint_id,
        tip_flags,
        max_limit,
        target_limit,
        // A zero budget means "tips only" and outranks even the explicit anchor.
        limit != Some(0),
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
        &mut resolved_seeds,
        &mut ep_seed,
        (initial_seeds.workspace_seeds.clone(), repo, meta),
        &worktree_by_branch,
    )?;

    max_commits_recharge_location.sort();
    let mut points_of_interest_to_traverse_first = next.inner.len();
    while let Some(QueueItem {
        info,
        flags: mut propagated_flags,
        instr,
        mut limit,
    }) = next.pop_front()
    {
        points_of_interest_to_traverse_first =
            points_of_interest_to_traverse_first.saturating_sub(1);

        let id = info.id;
        seed_first.resize(resolved_seeds.len(), None);
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
                &mut next,
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

        // Per-seed first collected commit — a seed collects its tip exactly once, and
        // a stolen tip (already collected by an earlier seed's walk) leaves None: the
        // seed item re-encounters and never collects.
        if let Some(ix) = instr.seed
            && seed_first[ix].is_none()
        {
            seed_first[ix] = Some(id);
        }

        let refs_at_commit_before_removal = refs_by_id.remove(&id).unwrap_or_default();
        let RemoteQueueOutcome {
            items_to_queue_later: remote_items_to_queue_later,
            maybe_make_id_a_goal_so_remote_can_find_local,
            limit_to_let_local_find_remote,
        } = try_queue_remote_tracking_branches(
            repo,
            &refs_at_commit_before_removal,
            &mut resolved_seeds,
            &mut ep_seed,
            &next,
            RemoteResolution {
                symbolic_remote_names: &initial_seeds.symbolic_remote_names,
                configured_tracking: &configured_remote_tracking_branches,
                target_refs: &initial_seeds.target_refs,
            },
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
            instr.seeker,
            &mut reached_by_seeker,
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
        let ep_first_flags = entrypoint_first_commit_flags(&store, &seed_first, ep_seed)
            // A shared tip claimed by another seed's segment leaves the entrypoint's segment
            // empty; the commit's own stored flags still answer "is the entrypoint integrated".
            .or_else(|| Some(store.flags(entrypoint_id)).filter(|f| !f.is_empty()));
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
    // NAMING LEAVES THE WALK (ratified 2026-08-05): the entry is named by what the
    // caller asked to look at — an input fact — or, detached, by the metadata ladder
    // over the refs standing at the entrypoint commit. Never by which seed's walk
    // collected the commit first: both arms are deterministic from repository state.
    // (Census: 73 walk-level ownership divergences rendered 5 product-visible diffs —
    // four false `DETACHED`s for a checked-out workspace ref, and one error message
    // that can now name the ref missing its base.)
    let entrypoint_ref = ref_name.or_else(|| entry_name_from_facts(&store, entrypoint_id, meta));
    attach_workspace_refs(
        &mut store,
        &resolved_seeds,
        &seed_first,
        &initial_seeds.workspace_ref_names,
    );
    Ok(WalkOutcome {
        commits: store.commits,
        by_id: store.by_id,
        parents_followed: store.parents_followed,
        entrypoint: Some(entrypoint_id),
        entrypoint_ref,
        hard_limit_hit: next.hard_limit_hit(),
        limit_hint_hit: next.limit_hint_hit(),
        seeds: initial_seeds
            .seeds
            .into_iter()
            .enumerate()
            .filter(|(idx, _)| !skipped_auxiliary_seeds.contains(idx))
            .map(|(_, seed)| seed)
            .collect(),
    })
}

// ── The driver's store: the collected commits ──

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

// ── Seeding: minting the initial queue, and its protocols ──

/// The detached-entry naming arm: the metadata ladder over the refs standing at the
/// entrypoint commit — the same disambiguation the build applies, on facts alone.
fn entry_name_from_facts<T: RefMetadata>(
    store: &Store,
    entrypoint_id: gix::ObjectId,
    meta: &OverlayMetadata<'_, T>,
) -> Option<gix::refs::FullName> {
    let mut refs_at_entry: super::utils::RefsById = Default::default();
    let cix = *store.by_id.get(&entrypoint_id)?;
    refs_at_entry.insert(
        entrypoint_id,
        store.commits[cix]
            .refs
            .iter()
            .map(|ri| ri.ref_name.clone())
            .collect(),
    );
    super::utils::disambiguate_refs_by_branch_metadata_with_lookup(
        (&refs_at_entry, entrypoint_id),
        meta,
    )
    .map(|(name, _)| name)
}

/// Queue the initial seeds, each resolved to its (ref, metadata) record by the same
/// `resolve_ref_and_meta` that resolves mid-walk tips.
#[allow(clippy::too_many_arguments)]
fn queue_initial_seeds<T: RefMetadata>(
    next: &mut Queue,
    resolved_seeds: &mut Vec<ResolvedSeed>,
    ep_seed: &mut Option<usize>,
    initial_seeds: &InitialSeeds,
    entrypoint: gix::ObjectId,
    entrypoint_flags: CommitFlags,
    max_limit: Limit,
    target_limit: Limit,
    anchor_extends: bool,
    goals: &mut Goals,
    commit_graph: Option<&gix::commitgraph::Graph>,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::utils::RefsById,
    worktree_by_branch: &WorktreeByBranch,
    buf: &mut Vec<u8>,
) -> anyhow::Result<Vec<usize>> {
    let mut pairing = PendingTargets {
        target_limit,
        commit_graph,
        repo,
        local_goals: Default::default(),
        parked: Default::default(),
    };

    let mut skipped_auxiliary = Vec::new();
    for (seed_idx, seed) in initial_seeds.seeds.iter().enumerate() {
        match &seed.role {
            // A local proven behind its target is a recorded fact, not a walk: no queue
            // item, no goals, no segment. Readers resolve it from the seed table.
            SeedRole::TargetLocal {
                behind_target: true,
                ..
            } => continue,
            SeedRole::WorkspaceStackBranch { .. } if next.iter().any(|t| t.info.id == seed.id) => {
                next.add_goal_to(seed.id, goals.flag_for(entrypoint).unwrap_or_default());
                continue;
            }
            SeedRole::TargetRemote
                if seed
                    .is_auxiliary_integrated_seed(&initial_seeds.auxiliary_integrated_seed_ids)
                    && next.iter().any(|t| t.info.id == seed.id) =>
            {
                // Auxiliary context another seed already owns contributes nothing — not
                // even a record: a stored target sitting on the entrypoint's own tip says
                // nothing about the entrypoint's work, and the post-hoc flags read the
                // seed table.
                skipped_auxiliary.push(seed_idx);
                continue;
            }
            _ => {}
        }

        let segment = resolve_seed(seed, meta, refs_by_id, worktree_by_branch)?;
        let seg = resolved_seeds.len();
        resolved_seeds.push(segment);

        if let SeedRole::TargetRemote = &seed.role {
            // The caller's EXPLICIT extra target is a DELIBERATE request to extend the
            // view down to it, so the entrypoint seeks it as a goal. Previously the
            // target-local pairing goals masked the integrated prune long enough for
            // this connection to happen by accident. The metadata-recorded target
            // commit gets no goal: ambient context must not extend the walk.
            if anchor_extends
                && Some(seed.id) == initial_seeds.explicit_extra_target
                && next.iter().any(|t| t.info.id == entrypoint)
            {
                next.add_goal_to(entrypoint, goals.flag_for(seed.id).unwrap_or_default());
            }
            let pending = PendingSeed {
                id: seed.id,
                seed: seg,
                queue_front: super::assemble::queue_should_frontload_seed(
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
            SeedRole::Reachable => super::assemble::reachable_seed_flags_and_limit(
                seed.id, entrypoint, max_limit, goals,
            ),
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
            SeedRole::TargetLocal { local_ref_name, .. } => {
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
            SeedRole::TargetLocal { local_ref_name, .. } => initial_seeds
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
        if super::assemble::queue_should_frontload_seed(
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
    Ok(skipped_auxiliary)
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
    Ok(QueueItem {
        info: find(cache, objects, id, buf)?,
        flags,
        instr: Instruction {
            queued_by: None,
            seed: Some(seed),
            seeker: Some(seed),
        },
        limit,
    })
}

/// A target seed parked until it can queue: where it points, its seed-table index, and
/// which queue end it wants.
struct PendingSeed {
    id: gix::ObjectId,
    seed: usize,
    queue_front: bool,
}

/// The target/local pairing registry of the initial-seed queueing: a target seed must
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
    /// to wait for) or when the partner's goal is already recorded — park it
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

    /// Release the target parked for `target_ref`, if any, with the recorded goal flag.
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

/// Resolve the seed's (ref, metadata) record via the shared namer; a workspace stack
/// branch whose desired name is REMOTE and resolved to nothing still records that
/// remote name.
fn resolve_seed<T: RefMetadata>(
    seed: &Seed,
    meta: &OverlayMetadata<'_, T>,
    refs_by_id: &super::utils::RefsById,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<ResolvedSeed> {
    let mut segment = resolve_ref_and_meta(
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
    resolved_seeds: &[ResolvedSeed],
    ws_tip: gix::ObjectId,
    swap_when_first_is_ws: bool,
) {
    let mut with_ws_tip = next
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| (item.info.id == ws_tip).then_some((idx, item.instr.seed)));
    let (Some(first), Some(second)) = (with_ws_tip.next(), with_ws_tip.next()) else {
        return;
    };
    drop(with_ws_tip);
    let first_is_ws = first
        .1
        .is_some_and(|ix| resolved_seeds[ix].workspace_metadata().is_some());
    if first_is_ws == swap_when_first_is_ws {
        next.inner.swap(first.0, second.0);
    }
}

/// Prioritize the initial seeds and assure workspace-commit ownership: the initial-queue
/// sort and swap logic, run against the seed table.
fn prioritize_and_ensure_ws_ownership<T: RefMetadata>(
    next: &mut Queue,
    resolved_seeds: &mut Vec<ResolvedSeed>,
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
        let resolved_seeds = &*resolved_seeds;
        next.inner.make_contiguous().sort_by_key(|item| {
            item.instr
                .seed
                .and_then(|ix| resolved_seeds[ix].ref_name())
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
            swap_first_two_on_tip_if(next, resolved_seeds, ws_tip, false);
        } else if next
            .iter()
            .filter(|item| item.info.id == ws_tip)
            .take(2)
            .count()
            >= 2
        {
            // Unmanaged commit: the WORKSPACE seed must NOT own it — swap it back.
            swap_first_two_on_tip_if(next, resolved_seeds, ws_tip, true);
        } else {
            // Single tip on an unmanaged workspace commit with workspace metadata: an anonymous
            // stand-in owns the commit first (a duplicate front item; the real item then takes
            // the re-encounter path).
            let QueueItem {
                info, flags, limit, ..
            } = next
                .iter()
                .find(|t| t.info.id == ws_tip)
                .cloned()
                .expect("each ws-tip has one entry on queue");
            let anon = resolve_ref_and_meta(None, meta, None, worktree_by_branch)?;
            let seed = resolved_seeds.len();
            resolved_seeds.push(anon);
            if ep_seed.is_none() {
                *ep_seed = Some(seed);
            }
            _ = next.push_front_exhausted(QueueItem {
                info,
                flags,
                instr: Instruction {
                    queued_by: None,
                    seed: Some(seed),
                    seeker: Some(seed),
                },
                limit,
            });
        }
    }
    Ok(())
}
// ── The loop's phases, in the order the driver runs them ──

/// Re-encounter of an already-collected commit: record the edge, run the
/// segment-ownership choreography (mid-segment split), merge flags, propagate them
/// downward, and adjust queued tips' goals and limits.
#[allow(clippy::too_many_arguments)]
fn re_encounter(
    store: &mut Store,
    next: &mut Queue,
    id: gix::ObjectId,
    queued_by: Option<gix::ObjectId>,
    propagated_flags: CommitFlags,
    limit: Limit,
) {
    store.connect(queued_by, id);
    let existing_flags = store.flags(id);
    let new_flags = propagated_flags | existing_flags;
    let needs_leafs = !limit.goal_reached();
    let visited = if new_flags != existing_flags
        || (needs_leafs
            && next
                .iter()
                .any(|item| !item.limit.goal_flags().contains(limit.goal_flags())))
    {
        // The visited cone always comes back: the budget handoff below needs it even
        // when the arriving tip carries no goal — the goal-inheritance loop no-ops then.
        store.propagate_flags_downward(new_flags, id, true)
    } else {
        None
    };
    if let Some(visited) = visited {
        // Tips whose queuing commit sits at the FRONTIER of the propagated cone —
        // in the cone, its followed parents not collected yet — inherit the goal.
        // (Commit-wise since rung 2 of walker-segs-brief.md: shadow-proven equal to
        // the old leaf-segment rule across the battery, the op suites, and 3,000
        // fuzzed repositories. Deliberately narrower than the budget handoff below.)
        let goal_flags = limit.goal_flags();
        for item in next.iter_mut() {
            let lands_on_frontier = item.instr.queued_by.is_some_and(|qb| {
                visited.contains(&qb) && !store.parents_followed.contains_key(&qb)
            });
            if lands_on_frontier {
                item.limit = item.limit.additional_goal(goal_flags);
            }
        }
        // BUDGET HANDOFF, never goals: everything in `visited` is now proven reachable
        // from this tip, so continuations hanging off that cone may walk on this tip's
        // budget when it is the larger. Without it, a lane first descended by the
        // target's zero-allowance items (target seeds frontload) pages the below-floor
        // tail at 0 instead of the entrypoint's budget.
        for item in next.iter_mut() {
            if item.instr.queued_by.is_some_and(|qb| visited.contains(&qb)) {
                item.limit.adjust_limit_if_bigger(limit);
            }
        }
    }
    // Tips that saw this commit extend their limit if that would extend it.
    let bottom_commit_goals = super::types::goal_bits(store.flags(id));
    for queued_tip_limit in next.iter_mut().filter_map(|item| {
        item.limit
            .goal_flags()
            .intersects(bottom_commit_goals)
            .then_some(&mut item.limit)
    }) {
        queued_tip_limit.adjust_limit_if_bigger(limit);
    }
}

/// How to resolve and exclude remote-tracking branches during the walk: the symbolic remote
/// names to try, the configured remote-tracking set, and the workspace target refs (never
/// re-queued as their own remotes). Bundled so `target_refs` can't be transposed with the
/// commit's `refs` — both were bare `&[FullName]`.
#[derive(Clone, Copy)]
struct RemoteResolution<'a> {
    symbolic_remote_names: &'a [String],
    configured_tracking: &'a std::collections::BTreeSet<gix::refs::FullName>,
    target_refs: &'a [gix::refs::FullName],
}

/// Queue the remote-tracking branches of newly seen local branches (lookup, goals, limits);
/// each remote tip gets a seed record (its name is used for the double-queue check).
#[allow(clippy::too_many_arguments)]
fn try_queue_remote_tracking_branches<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    resolved_seeds: &mut Vec<ResolvedSeed>,
    ep_seed: &mut Option<usize>,
    next: &Queue,
    remotes: RemoteResolution<'_>,
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
                remotes.symbolic_remote_names,
                remotes.configured_tracking,
            )?
        else {
            continue;
        };
        if remotes.target_refs.contains(&remote_tracking_branch) {
            continue;
        }
        let Some(remote_tip) =
            super::utils::try_refname_to_id(repo, remote_tracking_branch.as_ref())?
        else {
            continue;
        };
        if next.iter().any(|t| {
            t.info.id == remote_tip
                && t.instr
                    .seed
                    .and_then(|ix| resolved_seeds[ix].ref_name())
                    .is_some_and(|sn| sn == remote_tracking_branch.as_ref())
        }) {
            continue;
        }
        let seg = resolve_ref_and_meta(
            Some((remote_tracking_branch.clone(), None)),
            meta,
            None,
            worktree_by_branch,
        )?;
        let seed = resolved_seeds.len();
        resolved_seeds.push(seg);
        if ep_seed.is_none() {
            *ep_seed = Some(seed);
        }

        let remote_limit = limit.with_indirect_goal(id, goals);
        let self_flags = goals.flag_for(remote_tip).unwrap_or_default();
        limit_flags |= self_flags;
        goal_flags |= remote_limit.goal_flags();
        let remote_tip_info = find(commit_graph, objects, remote_tip, buf)?;
        queue.push(QueueItem {
            info: remote_tip_info,
            flags: self_flags,
            instr: Instruction {
                queued_by: None,
                seed: Some(seed),
                seeker: Some(seed),
            },
            limit: remote_limit,
        });
    }
    Ok(RemoteQueueOutcome {
        items_to_queue_later: queue,
        maybe_make_id_a_goal_so_remote_can_find_local: goal_flags,
        limit_to_let_local_find_remote: limit_flags,
    })
}

/// Queue every parent of `commit` (goals, limits, flag propagation); each queued item carries
/// the queuing commit.
#[allow(clippy::too_many_arguments)]
fn queue_parents(
    next: &mut Queue,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    current: gix::ObjectId,
    seeker: Option<usize>,
    reached_by_seeker: &mut Vec<CommitFlags>,
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
    if let Some(sk) = seeker {
        if reached_by_seeker.len() <= sk {
            reached_by_seeker.resize(sk + 1, CommitFlags::empty());
        }
        // A sibling of this seeker already found these goals: found for this item too.
        limit.retire_goals(reached_by_seeker[sk]);
    }
    let goals_before = limit.goal_flags();
    let exhausted = limit.is_exhausted_or_decrement(flags, next);
    // Record resolutions BEFORE acting on exhaustion: an item can find its goal and run
    // out of budget in the same step, and the siblings must still learn of the find.
    let resolved = goals_before.difference(limit.goal_flags());
    if let Some(sk) = seeker
        && !resolved.is_empty()
    {
        reached_by_seeker[sk] |= resolved;
    }
    if exhausted {
        // THE CUT: the budget ran out with history still below. A parentless commit ends for
        // its own reason, so only an unwalked parent makes this a truncation.
        if !parent_ids.is_empty() {
            next.record_limit_hint_cut();
        }
        return Ok(false);
    }
    let mut queue_is_exhausted = false;
    if parent_ids.len() > 1 {
        let instr = Instruction {
            queued_by: Some(current),
            seed: None,
            seeker,
        };
        let limit_per_parent = limit.per_parent(parent_ids.len());
        for pid in parent_ids.iter() {
            let info = find(commit_graph, objects, *pid, buf)?;
            queue_is_exhausted = next.push_back_even_if_exhausted(QueueItem {
                info,
                flags,
                instr,
                limit: limit_per_parent,
            });
        }
    } else if !parent_ids.is_empty() {
        let instr = Instruction {
            queued_by: Some(current),
            seed: None,
            seeker,
        };
        let info = find(commit_graph, objects, parent_ids[0], buf)?;
        queue_is_exhausted |= next.push_back_exhausted(QueueItem {
            info,
            flags,
            instr,
            limit,
        });
    }
    Ok(queue_is_exhausted)
}

/// The flags of the first commit currently in the entrypoint seed's segment (its contents
/// evolve through swaps and splits), `None` while empty or not yet materialized.
fn entrypoint_first_commit_flags(
    store: &Store,
    seed_first: &[Option<gix::ObjectId>],
    ep_seed: Option<usize>,
) -> Option<CommitFlags> {
    let first = (*seed_first.get(ep_seed?)?)?;
    Some(store.flags(first)).filter(|f| !f.is_empty())
}

/// Drop queued tips that are already integrated; the entrypoint-integrated check gets
/// the entrypoint segment's first-commit flags from [`entrypoint_first_commit_flags`].
fn prune_integrated_tips(next: &mut Queue, ep_first_flags: Option<CommitFlags>) {
    if next.is_exhausted() {
        return;
    }
    let all_integrated_and_done = next
        .iter()
        .all(|item| item.flags.contains(CommitFlags::Integrated) && item.limit.goal_reached());
    if !all_integrated_and_done {
        return;
    }
    if ep_first_flags.is_some_and(|flags| flags.contains(CommitFlags::Integrated)) {
        return;
    }
    next.exhaust();
}

/// Attach each workspace ref to its seed segment's first commit — the ws ref never enters
/// ref collection, so it must come back as data here. This is the ADVANCED-REF presentation
/// contract, not bookkeeping: when the ref points at a commit the walk never reached
/// (advanced outside the workspace, overlay preview), the name still surfaces at the
/// walk-visible position.
fn attach_workspace_refs(
    store: &mut Store,
    resolved_seeds: &[ResolvedSeed],
    seed_first: &[Option<gix::ObjectId>],
    ws_ref_names: &[gix::refs::FullName],
) {
    for (seed, first) in resolved_seeds.iter().zip(seed_first) {
        let (Some(ri), Some(first)) = (seed.ref_info.as_ref(), first) else {
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
