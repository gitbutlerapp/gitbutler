//! A commit-first traversal: visit commits and record their provenance, building the
//! [`CommitGraph`] directly rather than a graph of segments.
//!
//! It drives the walk with the queue/limit/goal machinery, visiting commits in a deterministic
//! order, and records as it goes:
//!
//! - commits land in a [`CommitGraph`] (flags, refs, child→parent edges) as they are visited;
//! - empty segments are created at seed tips and convergence points, each recording only which
//!   run-head commit it owns — this fixes the [`usize`] numbering;
//! - run contents are derived afterwards by following first-parent edges from each head to the
//!   next; attachments are replayed from the recorded log.
//!
//! Convergence — a path reaching an already-visited commit — registers a new run head rather than
//! splitting a segment, so commit ownership is never rewritten.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use but_core::RefMetadata;
use gix::refs::Category;

use crate::{Commit, CommitFlags, commit_graph::CommitGraph};

use super::{
    InitialTips,
    overlay::{OverlayMetadata, OverlayRepo},
    remotes,
    types::{Goals, Limit, Queue, Step},
    walk::{
        RefsById, TraverseInfo, WorktreeByBranch, branch_segment_from_name_and_meta,
        disambiguate_refs_by_branch_metadata_with_lookup, find, try_refname_to_id,
    },
};

/// Repository and ref context for a traversal, shared by the commit-first walk and the projection
/// it feeds. Carries lookups the walk needs plus the entry handling the projection applies.
pub(crate) struct Context<'a> {
    pub repo: &'a OverlayRepo<'a>,
    pub symbolic_remote_names: &'a [String],
    pub configured_remote_tracking_branches: &'a BTreeSet<gix::refs::FullName>,
    pub inserted_proxy_segments: Vec<usize>,
    pub refs_by_id: RefsById,
    pub hard_limit: bool,
    pub detach_entrypoint: bool,
    pub worktree_by_branch: WorktreeByBranch,
}

/// A connection from a segment that does not own `to`, pointing at it, recorded in walk order.
///
/// Parent links between commits are *not* recorded here — they live in [`State::commits`]
/// (see [`CommitGraph::parent_edges`]), which the projection reads directly.
pub(crate) struct Attachment {
    pub(crate) segment: usize,
    pub(crate) to: gix::ObjectId,
}

/// Traversal state: the commit store plus the per-segment naming/metadata and traversal scalars
/// the walk records as it goes. This is the input to the direct workspace projection.
pub(crate) struct State {
    /// Commit nodes (flags, refs, parent ids) and child→parent edges, built directly.
    pub(crate) commits: CommitGraph,
    /// For every visited commit: the segment owning the run it belongs to, and that run's head.
    /// The commit-first equivalent of the `seen` ownership map.
    pub(crate) run_of: gix::hashtable::HashMap<gix::ObjectId, (usize, gix::ObjectId)>,
    /// The head commit of the run each segment owns, if it owns one.
    pub(crate) head_by_owner: BTreeMap<usize, gix::ObjectId>,
    /// Segment attachments, in the order the walk established them. Commit parent links live in
    /// [`Self::commits`] instead.
    pub(crate) attachments: Vec<Attachment>,
    /// The traversal entrypoint segment, set while seeding the initial tips.
    pub(crate) entrypoint: Option<usize>,
    /// The ref the caller resolved as the entrypoint, if any.
    pub(crate) entrypoint_ref: Option<gix::refs::FullName>,
    /// The project metadata (targets) the traversal ran with.
    pub(crate) project_meta: but_core::ref_metadata::ProjectMeta,
    /// Per segment: the name the walk recorded for it (run owners and empty named segments alike).
    pub(crate) ref_info_by_segment: BTreeMap<usize, crate::RefInfo>,
    /// Per segment: the segment metadata the walk recorded.
    pub(crate) metadata_by_segment: BTreeMap<usize, crate::SegmentMetadata>,
    /// The traversal tips (roles + names) the walk ran with.
    pub(crate) traversal_tips: Vec<crate::init::Tip>,
    /// The traversal options the walk ran with.
    pub(crate) options: crate::init::Options,
    /// Next segment id to allocate. Segments are numbered 0, 1, 2, ... in creation order.
    next_segment_id: usize,
}

impl State {
    /// A fresh state with an empty commit store and no recorded segments.
    pub(crate) fn new() -> Self {
        State {
            commits: CommitGraph::default(),
            run_of: Default::default(),
            head_by_owner: Default::default(),
            attachments: Vec::new(),
            entrypoint: None,
            entrypoint_ref: None,
            project_meta: Default::default(),
            ref_info_by_segment: Default::default(),
            metadata_by_segment: Default::default(),
            traversal_tips: Default::default(),
            options: Default::default(),
            next_segment_id: 0,
        }
    }

    /// Allocate a fresh segment id (sequential: 0, 1, 2, ... in creation order) and record its
    /// name/metadata.
    fn new_segment(
        &mut self,
        ref_info: Option<crate::RefInfo>,
        metadata: Option<crate::SegmentMetadata>,
    ) -> usize {
        let id = self.next_segment_id;
        self.next_segment_id += 1;
        if let Some(ri) = ref_info {
            self.ref_info_by_segment.insert(id, ri);
        }
        if let Some(md) = metadata {
            self.metadata_by_segment.insert(id, md);
        }
        id
    }

    /// Record a new segment from its name/metadata, returning its id.
    pub(crate) fn insert_recording(
        &mut self,
        (ref_info, metadata): (Option<crate::RefInfo>, Option<crate::SegmentMetadata>),
    ) -> usize {
        self.new_segment(ref_info, metadata)
    }

    /// Like [`Self::insert_recording`], but also sets the entrypoint to this segment if unset.
    pub(crate) fn insert_recording_set_entrypoint(
        &mut self,
        (ref_info, metadata): (Option<crate::RefInfo>, Option<crate::SegmentMetadata>),
    ) -> usize {
        let id = self.new_segment(ref_info, metadata);
        if self.entrypoint.is_none() {
            self.entrypoint = Some(id);
        }
        id
    }

    /// The ref name recorded for segment `id`, if any (mirrors `Segment::ref_name`).
    pub(crate) fn seg_ref_name(&self, id: usize) -> Option<&gix::refs::FullNameRef> {
        self.ref_info_by_segment
            .get(&id)
            .map(|ri| ri.ref_name.as_ref())
    }

    /// Whether segment `id` was recorded with a name.
    fn seg_has_ref_info(&self, id: usize) -> bool {
        self.ref_info_by_segment.contains_key(&id)
    }

    /// The workspace metadata recorded for segment `id`, if it governs a workspace
    /// (mirrors `Segment::workspace_metadata`).
    pub(crate) fn seg_workspace_metadata(
        &self,
        id: usize,
    ) -> Option<&but_core::ref_metadata::Workspace> {
        self.metadata_by_segment.get(&id).and_then(|md| match md {
            crate::SegmentMetadata::Workspace(md) => Some(md),
            _ => None,
        })
    }

    fn is_run_head(&self, id: gix::ObjectId) -> bool {
        self.run_of.get(&id).is_some_and(|(_, head)| *head == id)
    }

    /// Make `owner` own the run headed by `head`.
    fn own(&mut self, owner: usize, head: gix::ObjectId) {
        self.head_by_owner.insert(owner, head);
        self.run_of.insert(head, (owner, head));
    }

    /// Cut the run containing `at` so that `at` and everything below it (until the next head)
    /// belongs to `new_owner` with `at` as its head. The old owner keeps the upper part.
    fn cut_tail(&mut self, at: gix::ObjectId, new_owner: usize) {
        let old = *self.run_of.get(&at).expect("commit was visited");
        debug_assert_ne!(old.1, at, "BUG: cutting at an existing head is a no-op");
        let mut cur = at;
        loop {
            self.run_of.insert(cur, (new_owner, at));
            cur = match self.commits.first_parent_id(cur) {
                Some(p) if self.run_of.get(&p) == Some(&old) => p,
                _ => break,
            };
        }
        self.head_by_owner.insert(new_owner, at);
    }

    /// Transfer ownership of `from_owner`'s whole run to `to_owner`, leaving `from_owner` empty.
    /// The run head stays the same.
    fn transfer_run(&mut self, from_owner: usize, to_owner: usize) {
        let Some(head) = self.head_by_owner.remove(&from_owner) else {
            return;
        };
        let old = (from_owner, head);
        let mut cur = head;
        loop {
            self.run_of.insert(cur, (to_owner, head));
            cur = match self.commits.first_parent_id(cur) {
                Some(p) if self.run_of.get(&p) == Some(&old) => p,
                _ => break,
            };
        }
        self.head_by_owner.insert(to_owner, head);
    }
}

/// The commit-first traversal: visit commits, record provenance, and return the record-only
/// segments alongside the traversal [`State`], which the segmentless workspace projection consumes.
#[expect(clippy::too_many_arguments)]
pub(crate) fn traverse<T: RefMetadata>(
    mut state: State,
    mut next: Queue,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    ctx: &mut Context<'_>,
    initial_tips: &InitialTips,
    configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    commit_graph: Option<&gix::commitgraph::Graph>,
    shallow_commits: Option<&gix::shallow::Commits>,
    max_commits_recharge_location: &[gix::ObjectId],
    max_limit: Limit,
    goals: &mut Goals,
    buf: &mut Vec<u8>,
) -> anyhow::Result<State> {
    let mut points_of_interest_to_traverse_first = next.iter().count();
    while let Some((info, mut propagated_flags, step, mut limit)) = next.pop_front() {
        points_of_interest_to_traverse_first =
            points_of_interest_to_traverse_first.saturating_sub(1);

        let id = info.id;
        if max_commits_recharge_location.binary_search(&id).is_ok() {
            limit.set_but_keep_goal(max_limit);
        }
        // The flags of the commit we extend from: the `child` commit of this step.
        let src_flags = match &step {
            Step::SeedTip { .. } => CommitFlags::default(),
            Step::Continue { child } | Step::NewRunBelow { child, .. } => state
                .commits
                .flags_of(*child)
                .context("BUG: the child commit of a queued step was visited")?,
        };
        propagated_flags |= src_flags;
        let is_shallow_boundary =
            shallow_commits.is_some_and(|boundary| boundary.binary_search(&id).is_ok());
        if is_shallow_boundary {
            propagated_flags |= CommitFlags::ShallowBoundary;
        }

        if state.run_of.contains_key(&id) {
            on_occupied(&mut state, &mut next, id, propagated_flags, step, limit)?;
            continue;
        }

        // Vacant: decide which segment's run this commit belongs to, creating empty segments at
        // the seed and convergence points that fix the segment indexing.
        match step {
            Step::SeedTip { into } => {
                state.own(into, id);
            }
            Step::Continue { child } => {
                let (child_run, child_head) = *state
                    .run_of
                    .get(&child)
                    .context("BUG: child of continuation was visited")?;
                // The equivalent of `try_split_non_empty_segment_at_branch`: an unambiguous
                // branch or a merge commit begins its own segment.
                let maybe_segment_name =
                    disambiguate_refs_by_branch_metadata_with_lookup((&ctx.refs_by_id, id), meta);
                if let Some((ref_name, metadata)) = maybe_segment_name {
                    let segment = state.insert_recording(branch_segment_from_name_and_meta(
                        Some((ref_name, metadata)),
                        meta,
                        None,
                        &ctx.worktree_by_branch,
                    )?);
                    state.own(segment, id);
                } else if info.parent_ids.len() >= 2 {
                    let segment = state.insert_recording(branch_segment_from_name_and_meta(
                        None,
                        meta,
                        None,
                        &ctx.worktree_by_branch,
                    )?);
                    state.own(segment, id);
                } else {
                    state.run_of.insert(id, (child_run, child_head));
                }
            }
            Step::NewRunBelow { .. } => {
                // Each parent of a merge starts a new segment, named from the refs at the commit
                // if they are unambiguous.
                let segment = state.insert_recording(branch_segment_from_name_and_meta(
                    None,
                    meta,
                    Some((&ctx.refs_by_id, id)),
                    &ctx.worktree_by_branch,
                )?);
                state.own(segment, id);
            }
        }

        let refs_at_commit_before_removal = ctx.refs_by_id.remove(&id).unwrap_or_default();
        let RemoteQueueOutcome {
            items_to_queue_later: remote_items_to_queue_later,
            maybe_make_id_a_goal_so_remote_can_find_local,
            limit_to_let_local_find_remote,
        } = try_queue_remote_tracking_branches(
            repo,
            &refs_at_commit_before_removal,
            &mut state,
            &initial_tips.symbolic_remote_names,
            configured_remote_tracking_branches,
            &initial_tips.target_refs,
            meta,
            id,
            limit,
            goals,
            &next,
            ctx,
            commit_graph,
            repo.for_find_only(),
            buf,
        )?;

        let propagated_flags = propagated_flags | maybe_make_id_a_goal_so_remote_can_find_local;
        queue_parents(
            &mut next,
            &info.parent_ids,
            propagated_flags,
            id,
            limit.additional_goal(limit_to_let_local_find_remote),
            is_shallow_boundary,
            commit_graph,
            repo.for_find_only(),
            buf,
        )?;

        // Store the commit with all refs on it; the projection strips the owning segment's name
        // from the displayed commit.
        state.commits.add_commit(Commit {
            id,
            flags: propagated_flags,
            refs: refs_at_commit_before_removal
                .iter()
                .map(|rn| crate::RefInfo::from_ref(rn.clone(), Some(id), &ctx.worktree_by_branch))
                .collect(),
            parent_ids: info.parent_ids.iter().cloned().collect(),
        });
        match step {
            Step::SeedTip { .. } => {}
            Step::Continue { child } => {
                state.commits.add_parent_edge(child, id, 0);
            }
            Step::NewRunBelow {
                child,
                parent_order,
            } => {
                state.commits.add_parent_edge(child, id, parent_order);
            }
        }

        for item in remote_items_to_queue_later {
            if next.push_back_exhausted(item) {
                // We may end up with unconnected remote tracking ref segments, that's fine.
                break;
            }
        }

        prune_integrated_tips(&state, &mut next)?;
        if points_of_interest_to_traverse_first == 0 {
            next.sort();
        }
    }

    ctx.hard_limit = next.hard_limit_hit();

    Ok(state)
}

/// The commit-first equivalent of `possibly_split_occupied_segment`: a path reached the
/// already-visited commit `id`. Establish the connection, register a run boundary at `id` if it is
/// mid-run, merge flags and propagate them, and adjust queued goals/limits.
fn on_occupied(
    state: &mut State,
    next: &mut Queue,
    id: gix::ObjectId,
    propagated_flags: CommitFlags,
    step: Step,
    limit: Limit,
) -> anyhow::Result<()> {
    let (dst_owner, _) = *state
        .run_of
        .get(&id)
        .context("BUG: occupied means visited")?;

    // If a normal branch walks into a workspace branch's run, transfer the run to the branch so
    // the workspace segment doesn't own the commits and attaches to them instead.
    let mut attach_from_workspace = None;
    if let Step::SeedTip { into } = step
        && into != dst_owner
        && state.seg_workspace_metadata(dst_owner).is_some()
        && state
            .seg_ref_name(into)
            .and_then(|rn| rn.category())
            .is_some_and(|c| matches!(c, Category::LocalBranch))
    {
        state.transfer_run(dst_owner, into);
        attach_from_workspace = Some(dst_owner);
    }

    // Register a run boundary at `id` if it is mid-run. An unconnected empty named segment arriving
    // at a remote run takes the run over (stand-in); otherwise a new anonymous segment owns it.
    if !state.is_run_head(id) {
        let standin = match step {
            Step::SeedTip { into } => (attach_from_workspace.is_none()
                && !state.head_by_owner.contains_key(&into)
                && state.seg_has_ref_info(into)
                && state
                    .commits
                    .flags_of(state.run_of[&id].1)
                    .is_some_and(|f| f.is_remote()))
            .then_some(into),
            Step::Continue { .. } | Step::NewRunBelow { .. } => None,
        };
        match standin {
            Some(into) => state.cut_tail(id, into),
            None => {
                let segment = state.insert_recording((None, None));
                state.cut_tail(id, segment);
            }
        }
    }

    // Establish the connection that brought us here, unless the stand-in made it a self-connection.
    match step {
        Step::SeedTip { into } => {
            let attacher = attach_from_workspace.unwrap_or(into);
            if state.run_of[&id].0 != attacher {
                state.attachments.push(Attachment {
                    segment: attacher,
                    to: id,
                });
            }
        }
        Step::Continue { child } => {
            state.commits.add_parent_edge(child, id, 0);
        }
        Step::NewRunBelow {
            child,
            parent_order,
        } => {
            state.commits.add_parent_edge(child, id, parent_order);
        }
    }

    // Merge flags and propagate downward if anything changed, collecting the leaf commits of the
    // propagated region to extend the goals of tips that continue from them.
    let top_flags = match step {
        Step::SeedTip { .. } => CommitFlags::default(),
        Step::Continue { child } | Step::NewRunBelow { child, .. } => state
            .commits
            .flags_of(child)
            .context("BUG: the child commit of a queued step was visited")?,
    };
    let bottom_flags = state
        .commits
        .flags_of(id)
        .context("BUG: occupied means visited")?;
    let new_flags = propagated_flags | top_flags | bottom_flags;
    let needs_leafs = !limit.goal_reached();
    let leafs = if new_flags != bottom_flags
        || (needs_leafs
            && next
                .iter()
                .any(|(_, _, _, tip_limit)| !tip_limit.goal_flags().contains(limit.goal_flags())))
    {
        propagate_flags_downward(&mut state.commits, new_flags, id, needs_leafs)
    } else {
        None
    };

    if let Some(leafs) = leafs {
        let goal_flags = limit.goal_flags();
        for (_, _, step, queued_limit) in next.iter_mut() {
            let continues_from = match step {
                Step::Continue { child } | Step::NewRunBelow { child, .. } => Some(*child),
                Step::SeedTip { .. } => None,
            };
            if continues_from.is_some_and(|c| leafs.contains(&c)) {
                *queued_limit = queued_limit.additional_goal(goal_flags);
            }
        }
    }

    // Find the tips that saw this commit, and adjust their limit if that would extend it.
    let bottom_commit_goals = Limit::new(None)
        .additional_goal(
            state
                .commits
                .flags_of(state.run_of[&id].1)
                .expect("head of an owned run is visited"),
        )
        .goal_flags();
    for queued_tip_limit in next
        .iter_mut()
        .filter_map(|(_, _, _, l)| l.goal_flags().intersects(bottom_commit_goals).then_some(l))
    {
        queued_tip_limit.adjust_limit_if_bigger(limit);
    }
    Ok(())
}

/// Propagate `flags_to_add` from `start` toward the base over the established parent edges,
/// returning the leaf commits (those without parent edges yet) when `needs_leafs` is set.
fn propagate_flags_downward(
    commits: &mut CommitGraph,
    flags_to_add: CommitFlags,
    start: gix::ObjectId,
    needs_leafs: bool,
) -> Option<BTreeSet<gix::ObjectId>> {
    let mut leafs = needs_leafs.then(BTreeSet::new);
    let mut visited = gix::hashtable::HashSet::default();
    let mut stack = vec![start];
    visited.insert(start);
    while let Some(id) = stack.pop() {
        commits.add_flags(id, flags_to_add);
        let nx = commits.node(id).expect("visited commits have nodes");
        let mut edge_count = 0;
        let parents: Vec<_> = commits
            .parents(nx)
            .map(|(p, _)| commits.inner[p].id)
            .collect();
        for parent in parents {
            edge_count += 1;
            if visited.insert(parent) {
                stack.push(parent);
            }
        }
        if edge_count == 0
            && let Some(leafs) = leafs.as_mut()
        {
            leafs.insert(id);
        }
    }
    leafs.filter(|l| !l.is_empty())
}

/// Stop traversal once only integrated tips with reached goals are left, mirroring
/// `walk::prune_integrated_tips` with entrypoint flags read from the commit store.
fn prune_integrated_tips(state: &State, next: &mut Queue) -> anyhow::Result<()> {
    if next.is_exhausted() {
        return Ok(());
    }
    let all_integrated_and_done = next.iter().all(|(_id, flags, _step, tip_limit)| {
        flags.contains(CommitFlags::Integrated) && tip_limit.goal_reached()
    });
    if !all_integrated_and_done {
        return Ok(());
    }
    let ep_sidx = state
        .entrypoint
        .context("BUG: entrypoint is set after initial tips are queued")?;
    if state
        .head_by_owner
        .get(&ep_sidx)
        .and_then(|head| state.commits.flags_of(*head))
        .is_some_and(|flags| flags.contains(CommitFlags::Integrated))
    {
        return Ok(());
    }

    next.exhaust();
    Ok(())
}

/// Queue the parents of `id`, mirroring `walk::queue_parents` with commit-first steps.
#[expect(clippy::too_many_arguments)]
fn queue_parents(
    next: &mut Queue,
    parent_ids: &[gix::ObjectId],
    flags: CommitFlags,
    id: gix::ObjectId,
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
        let limit_per_parent = limit.per_parent(parent_ids.len());
        for (parent_order, pid) in parent_ids.iter().enumerate() {
            let step = Step::NewRunBelow {
                child: id,
                parent_order: parent_order
                    .try_into()
                    .context("commit parent position does not fit into u32")?,
            };
            let info = find(commit_graph, objects, *pid, buf)?;
            queue_is_exhausted =
                next.push_back_even_if_exhausted((info, flags, step, limit_per_parent));
        }
    } else if !parent_ids.is_empty() {
        let info = find(commit_graph, objects, parent_ids[0], buf)?;
        queue_is_exhausted |=
            next.push_back_exhausted((info, flags, Step::Continue { child: id }, limit));
    }

    Ok(queue_is_exhausted)
}

struct RemoteQueueOutcome {
    items_to_queue_later: Vec<(TraverseInfo, CommitFlags, Step, Limit)>,
    maybe_make_id_a_goal_so_remote_can_find_local: CommitFlags,
    limit_to_let_local_find_remote: CommitFlags,
}

/// Mirror of `walk::try_queue_remote_tracking_branches` with commit-first steps.
#[expect(clippy::too_many_arguments)]
fn try_queue_remote_tracking_branches<T: RefMetadata>(
    repo: &OverlayRepo<'_>,
    refs: &[gix::refs::FullName],
    state: &mut State,
    target_symbolic_remote_names: &[String],
    configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    target_refs: &[gix::refs::FullName],
    meta: &OverlayMetadata<'_, T>,
    id: gix::ObjectId,
    limit: Limit,
    goals: &mut Goals,
    next: &Queue,
    ctx: &Context<'_>,
    commit_graph: Option<&gix::commitgraph::Graph>,
    objects: &impl gix::objs::Find,
    buf: &mut Vec<u8>,
) -> anyhow::Result<RemoteQueueOutcome> {
    let mut goal_flags = CommitFlags::empty();
    let mut limit_flags = CommitFlags::empty();
    let mut queue = Vec::new();
    for rn in refs {
        let Some(remote_tracking_branch) = remotes::lookup_remote_tracking_branch_or_deduce_it(
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
        let Some(remote_tip) = try_refname_to_id(repo, remote_tracking_branch.as_ref())? else {
            continue;
        };

        // It can happen a remote is in the workspace and was already queued as workspace tip.
        // Don't double-queue.
        if next.iter().any(|t| {
            t.0.id == remote_tip
                && match t.2 {
                    Step::SeedTip { into } => state
                        .seg_ref_name(into)
                        .is_some_and(|rn| rn == remote_tracking_branch.as_ref()),
                    Step::Continue { .. } | Step::NewRunBelow { .. } => false,
                }
        }) {
            continue;
        };
        let remote_segment =
            state.insert_recording_set_entrypoint(branch_segment_from_name_and_meta(
                Some((remote_tracking_branch.clone(), None)),
                meta,
                None,
                &ctx.worktree_by_branch,
            )?);

        let remote_limit = limit.with_indirect_goal(id, goals);
        let self_flags = goals.flag_for(remote_tip).unwrap_or_default();
        limit_flags |= self_flags;
        goal_flags |= remote_limit.goal_flags();
        let remote_tip_info = find(commit_graph, objects, remote_tip, buf)?;
        queue.push((
            remote_tip_info,
            self_flags,
            Step::SeedTip {
                into: remote_segment,
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
