use crate::{CommitFlags, Edge};
use crate::{CommitIndex, Graph, Segment, SegmentIndex, SegmentMetadata};
use anyhow::bail;
use but_core::RefMetadata;
use gix::hashtable::hash_map::Entry;
use gix::prelude::{ObjectIdExt, ReferenceExt};
use gix::refs::Category;
use petgraph::graph::EdgeReference;
use petgraph::prelude::EdgeRef;
use std::collections::VecDeque;
use tracing::instrument;

mod utils;
use utils::*;

mod remotes;

mod post;
pub(crate) mod walk;

pub(super) type PetGraph = petgraph::Graph<Segment, Edge>;

/// Options for use in [`Graph::from_head()`] and [`Graph::from_commit_traversal()`].
#[derive(Default, Debug, Clone)]
pub struct Options {
    /// Associate tag references with commits.
    ///
    /// If `false`, tags are not collected.
    pub collect_tags: bool,
    /// The (soft) maximum number of commits we should traverse.
    /// Workspaces with a target branch automatically have unlimited traversals as they rely on the target
    /// branch to eventually stop the traversal.
    ///
    /// If `None`, there is no limit, which typically means that when lacking a workspace, the traversal
    /// will end only when no commit is left to traverse.
    /// `Some(0)` means nothing but the first commit is going to be returned, but it should be avoided.
    ///
    /// Note that this doesn't affect the traversal of integrated commits, which is always stopped once there
    /// is nothing interesting left to traverse.
    ///
    /// Also note: This is a hint and not an exact measure, and it's always possible to receive a more commits
    /// for various reasons, for instance the need to let remote branches find their local branch independently
    /// of the limit.
    pub commits_limit_hint: Option<usize>,
    /// A list of the last commits of partial segments previously returned that reset the amount of available
    /// commits to traverse back to `commit_limit_hint`.
    /// Imagine it like a gas station that can be chosen to direct where the commit-budge should be spent.
    pub commits_limit_recharge_location: Vec<gix::ObjectId>,
    /// As opposed to the limit-hint, if not `None` we will stop after pretty much this many commits have been seen.
    ///
    /// This is a last line of defense against runaway traversals and for not it's recommended to set it to a high
    /// but manageable value. Note that depending on the commit-graph, we may need more commits to find the local branch
    /// for a remote branch, leaving remote branches unconnected.
    ///
    /// Due to multiple paths being taken, more commits may be queued (which is what's counted here) than actually
    /// end up in the graph, so usually one will see many less.
    pub hard_limit: Option<usize>,
}

/// Builder
impl Options {
    /// Set the maximum amount of commits that each lane in a tip may traverse, but that's less important
    /// than building consistent, connected graphs.
    pub fn with_limit_hint(mut self, limit: usize) -> Self {
        self.commits_limit_hint = Some(limit);
        self
    }

    /// Set a hard limit for the amount of commits to traverse. Even though it may be off by a couple, it's not dependent
    /// on any additional logic.
    pub fn with_hard_limit(mut self, limit: usize) -> Self {
        self.hard_limit = Some(limit);
        self
    }

    /// Keep track of commits at which the traversal limit should be reset to the [`limit`](Self::with_limit_hint()).
    pub fn with_limit_extension_at(
        mut self,
        commits: impl IntoIterator<Item = gix::ObjectId>,
    ) -> Self {
        self.commits_limit_recharge_location.extend(commits);
        self
    }
}

/// Lifecycle
impl Graph {
    /// Read the `HEAD` of `repo` and represent whatever is visible as a graph.
    ///
    /// See [`Self::from_commit_traversal()`] for details.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        options: Options,
    ) -> anyhow::Result<Self> {
        let head = repo.head()?;
        let (tip, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                let mut graph = Graph::default();
                graph.insert_root(branch_segment_from_name_and_meta(
                    Some((ref_name, None)),
                    meta,
                    None,
                )?);
                return Ok(graph);
            }
            gix::head::Kind::Detached { target, peeled } => {
                (peeled.unwrap_or(target).attach(repo), None)
            }
            gix::head::Kind::Symbolic(existing_reference) => {
                let mut existing_reference = existing_reference.attach(repo);
                let tip = existing_reference.peel_to_id_in_place()?;
                (tip, Some(existing_reference.inner.name))
            }
        };
        Self::from_commit_traversal(tip, maybe_name, meta, options)
    }
    /// Produce a minimal-effort representation of the commit-graph reachable from the commit at `tip` such the returned instance
    /// can represent everything that's observed, without loosing information.
    /// `ref_name` is assumed to point to `tip` if given.
    ///
    /// `meta` is used to learn more about the encountered references.
    ///
    /// ### Features
    ///
    /// * discover a Workspace on the fly based on `meta`-data.
    /// * support the notion of a branch to integrate with, the *target*
    ///     - *target* branches consist of a local and remote tracking branch, and one can be ahead of the other.
    ///     - workspaces are relative to the local tracking branch of the target.
    /// * remote tracking branches are seen in relation to their branches.
    /// * the graph of segments assigns each reachable commit to exactly one segment
    /// * one can use [`petgraph::algo`] and [`petgraph::visit`]
    ///     - It maintains information about the intended connections, so modifications afterwards will show
    ///       in debugging output if edges are now in violation of this constraint.
    ///
    /// ### Rules
    ///
    /// These rules should help to create graphs and segmentations that feel natural and are desirable to the user,
    /// while avoiding traversing the entire commit-graph all the time.
    /// Change the rules as you see fit to accomplish this.
    ///
    /// * a commit can be governed by multiple workspaces
    /// * as workspaces and entrypoints "grow" together, we don't know anything about workspaces until the every end,
    ///   or when two streams touch. This means we can't make decisions based on [flags](CommitFlags) until the traversal
    ///   is finished.
    /// * an entrypoint always causes the start of a segment.
    /// * Segments are always named if their first commit has a single local branch pointing to it.
    /// * Anonymous segments are created if there are more than one local branches pointing to it.
    /// * Anonymous segments are created if another segment connects to a commit that it contains that is not the first one.
    ///    - This means, all connections go from the last commit in a segment to the first commit in another segment.
    /// * Segments stored in the *workspace metadata* are used/relevant only if they are backed by an existing branch.
    /// * Remote tracking branches are picked up during traversal for any ref that we reached through traversal.
    ///     - This implies that remotes aren't relevant for segments added during post-processing, which would typically
    ///       be empty anyway.
    ///     - Remotes never take commits that are already owned.
    /// * The traversal is cut short when there is only tips which are integrated, even though named segments that are
    ///   supposed to be in the workspace will be fully traversed (implying they will stop at the first anon segment
    ///   as will happen at merge commits).
    /// * The traversal is always as long as it needs to be to fully reconcile possibly disjoint branches, despite
    ///   this sometimes costing some time when the remote is far ahead in a huge repository.
    #[instrument(skip(meta, ref_name), err(Debug))]
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        Options {
            collect_tags,
            commits_limit_hint: limit,
            commits_limit_recharge_location: mut max_commits_recharge_location,
            hard_limit,
        }: Options,
    ) -> anyhow::Result<Self> {
        let repo = tip.repo;
        let max_limit = Limit::new(limit);
        // TODO: also traverse (outside)-branches that ought to be in the workspace. That way we have the desired ones
        //       automatically and just have to find a way to prune the undesired ones.
        let ref_name = ref_name.into();
        if ref_name
            .as_ref()
            .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
        {
            // TODO: see if this is a thing - Git doesn't like to checkout remote tracking branches by name,
            //       and if we should handle it, we need to setup the initial flags accordingly.
            //       Also we have to assure not to double-traverse the ref, once as tip and once by discovery.
            bail!("Cannot currently handle remotes as start position");
        }
        let commit_graph = repo.commit_graph_if_enabled()?;
        let mut buf = Vec::new();
        let mut graph = Graph::default();

        let configured_remote_tracking_branches =
            remotes::configured_remote_tracking_branches(repo)?;
        let mut refs_by_id = collect_ref_mapping_by_prefix(
            repo,
            std::iter::once("refs/heads/").chain(if collect_tags {
                Some("refs/tags/")
            } else {
                None
            }),
        )?;
        let (workspaces, target_refs) =
            obtain_workspace_infos(repo, ref_name.as_ref().map(|rn| rn.as_ref()), meta)?;
        let mut seen = gix::revwalk::graph::IdMap::<SegmentIndex>::default();
        let mut goals = Goals::default();
        // The tip transports itself.
        let tip_flags = CommitFlags::NotInRemote
            | goals
                .flag_for(tip.detach())
                .expect("we more than one bitflags for this");

        let target_symbolic_remote_names = {
            let remote_names = repo.remote_names();
            let mut v: Vec<_> = workspaces
                .iter()
                .filter_map(|(_, _, data)| {
                    let target_ref = data.target_ref.as_ref()?;
                    remotes::extract_remote_name(target_ref.as_ref(), &remote_names)
                })
                .collect();
            v.sort();
            v.dedup();
            v
        };

        let mut next = Queue::new_with_limit(hard_limit);
        if !workspaces
            .iter()
            .any(|(_, wsrn, _)| Some(wsrn) == ref_name.as_ref())
        {
            let current = graph.insert_root(branch_segment_from_name_and_meta(
                ref_name.clone().map(|rn| (rn, None)),
                meta,
                Some((&refs_by_id, tip.detach())),
            )?);
            if next.push_back_exhausted((
                tip.detach(),
                tip_flags,
                Instruction::CollectCommit { into: current },
                max_limit,
            )) {
                return Ok(graph.with_hard_limit());
            }
        }
        for (ws_tip, ws_ref, workspace_info) in workspaces {
            let target = workspace_info.target_ref.as_ref().and_then(|trn| {
                try_refname_to_id(repo, trn.as_ref())
                    .map_err(|err| {
                        tracing::warn!(
                            "Ignoring non-existing target branch {trn}: {err}",
                            trn = trn.as_bstr()
                        );
                        err
                    })
                    .ok()
                    .flatten()
                    .map(|tid| (trn.clone(), tid))
            });

            let (ws_extra_flags, ws_limit) = if Some(&ws_ref) == ref_name.as_ref() {
                (tip_flags, max_limit)
            } else {
                (
                    CommitFlags::empty(),
                    max_limit.with_indirect_goal(tip.detach(), &mut goals),
                )
            };
            let mut ws_segment =
                branch_segment_from_name_and_meta(Some((ws_ref, None)), meta, None)?;
            // The limits for the target ref and the worktree ref are synced so they can always find each other,
            // while being able to stop when the entrypoint is included.
            ws_segment.metadata = Some(SegmentMetadata::Workspace(workspace_info));
            let ws_segment = graph.insert_root(ws_segment);
            // As workspaces typically have integration branches which can help us to stop the traversal,
            // pick these up first.
            if next.push_front_exhausted((
                ws_tip,
                CommitFlags::InWorkspace |
                    // We only allow workspaces that are not remote, and that are not target refs.
                    // Theoretically they can still cross-reference each other, but then we'd simply ignore
                    // their status for now.
                    CommitFlags::NotInRemote | ws_extra_flags,
                Instruction::CollectCommit { into: ws_segment },
                ws_limit,
            )) {
                return Ok(graph.with_hard_limit());
            }
            if let Some((target_ref, target_ref_id)) = target {
                let target_segment = graph.insert_root(branch_segment_from_name_and_meta(
                    Some((target_ref, None)),
                    meta,
                    None,
                )?);
                if next.push_front_exhausted((
                    target_ref_id,
                    CommitFlags::Integrated,
                    Instruction::CollectCommit {
                        into: target_segment,
                    },
                    // Once the goal was found, be done immediately,
                    // we are not interested in these.
                    max_limit
                        .with_indirect_goal(tip.detach(), &mut goals)
                        .without_allowance(),
                )) {
                    return Ok(graph.with_hard_limit());
                }
            }
        }

        max_commits_recharge_location.sort();
        while let Some((id, mut propagated_flags, instruction, mut limit)) = next.pop_front() {
            if max_commits_recharge_location.binary_search(&id).is_ok() {
                limit.set_but_keep_goal(max_limit);
            }
            let info = find(commit_graph.as_ref(), repo, id, &mut buf)?;
            let src_flags = graph[instruction.segment_idx()]
                .commits
                .last()
                .map(|c| c.flags)
                .unwrap_or_default();

            // These flags might be outdated as they have been queued, meanwhile we may have propagated flags.
            // So be sure this gets picked up.
            propagated_flags |= src_flags;
            let segment_idx_for_id = match instruction {
                Instruction::CollectCommit { into: src_sidx } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        possibly_split_occupied_segment(
                            &mut graph,
                            &mut seen,
                            &mut next,
                            id,
                            propagated_flags,
                            src_sidx,
                        )?;
                        continue;
                    }
                    Entry::Vacant(e) => {
                        let src_sidx = try_split_non_empty_segment_at_branch(
                            &mut graph,
                            src_sidx,
                            &info,
                            &refs_by_id,
                            meta,
                        )?
                        .unwrap_or(src_sidx);
                        e.insert(src_sidx);
                        src_sidx
                    }
                },
                Instruction::ConnectNewSegment {
                    parent_above,
                    at_commit,
                } => match seen.entry(id) {
                    Entry::Occupied(_) => {
                        possibly_split_occupied_segment(
                            &mut graph,
                            &mut seen,
                            &mut next,
                            id,
                            propagated_flags,
                            parent_above,
                        )?;
                        continue;
                    }
                    Entry::Vacant(e) => {
                        let segment_below =
                            branch_segment_from_name_and_meta(None, meta, Some((&refs_by_id, id)))?;
                        let segment_below = graph.connect_new_segment(
                            parent_above,
                            at_commit,
                            segment_below,
                            0,
                            id,
                        );
                        e.insert(segment_below);
                        segment_below
                    }
                },
            };

            let refs_at_commit_before_removal = refs_by_id.remove(&id).unwrap_or_default();
            let (remote_items, maybe_goal_for_id) = try_queue_remote_tracking_branches(
                repo,
                &refs_at_commit_before_removal,
                &mut graph,
                &target_symbolic_remote_names,
                &configured_remote_tracking_branches,
                &target_refs,
                meta,
                id,
                limit,
                &mut goals,
            )?;

            let segment = &mut graph[segment_idx_for_id];
            let commit_idx_for_possible_fork = segment.commits.len();
            let propagated_flags = propagated_flags | maybe_goal_for_id;
            let hard_limit_hit = queue_parents(
                &mut next,
                &info.parent_ids,
                propagated_flags,
                segment_idx_for_id,
                commit_idx_for_possible_fork,
                limit,
            );
            if hard_limit_hit {
                return Ok(graph.with_hard_limit());
            }

            segment.commits.push(
                info.into_commit(
                    segment
                        .commits
                        // Flags are additive, and meanwhile something may have dumped flags on us
                        // so there is more compared to when the 'flags' value was put onto the queue.
                        .last()
                        .map_or(propagated_flags, |last| last.flags | propagated_flags),
                    refs_at_commit_before_removal
                        .clone()
                        .into_iter()
                        .filter(|rn| segment.ref_name.as_ref() != Some(rn))
                        .collect(),
                )?,
            );

            for item in remote_items {
                if next.push_back_exhausted(item) {
                    return Ok(graph.with_hard_limit());
                }
            }

            prune_integrated_tips(&mut graph, &mut next);
        }

        graph.post_processed(
            meta,
            tip.detach(),
            repo,
            &target_symbolic_remote_names,
            &configured_remote_tracking_branches,
        )
    }

    fn with_hard_limit(mut self) -> Self {
        self.hard_limit_hit = true;
        self
    }
}

/// A queue to keep track of tips, which additionally counts how much was queued over time.
struct Queue {
    inner: VecDeque<QueueItem>,
    /// The current number of queued items.
    count: usize,
    /// The maximum number of queuing operations, each representing one commit.
    max: Option<usize>,
}

/// A set of commits to keep track of in bitflags.
#[derive(Default)]
struct Goals(Vec<gix::ObjectId>);

impl Goals {
    /// Return the bitflag for `goal`, or `None` if we can't track any more goals.
    fn flag_for(&mut self, goal: gix::ObjectId) -> Option<CommitFlags> {
        let existing_flags = CommitFlags::all().iter().count();
        let max_goals = size_of::<CommitFlags>() * 8 - existing_flags;

        let goals = &mut self.0;
        let goal_index = match goals.iter().position(|existing| existing == &goal) {
            None => {
                let idx = goals.len();
                goals.push(goal);
                idx
            }
            Some(idx) => idx,
        };
        if goal_index >= max_goals {
            tracing::warn!("Goals limit reached, cannot track {goal}");
            None
        } else {
            Some(CommitFlags::from_bits_retain(
                1 << (existing_flags + goal_index),
            ))
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum Instruction {
    /// Contains the segment into which to place this commit.
    CollectCommit { into: SegmentIndex },
    /// This is the first commit in a new segment which is below `parent_above` and which should be placed
    /// at the last commit (at the time) via `at_commit`.
    ConnectNewSegment {
        parent_above: SegmentIndex,
        at_commit: CommitIndex,
    },
}

impl Instruction {
    /// Returns any segment index we may be referring to.
    fn segment_idx(&self) -> SegmentIndex {
        match self {
            Instruction::CollectCommit { into } => *into,
            Instruction::ConnectNewSegment { parent_above, .. } => *parent_above,
        }
    }

    fn with_replaced_sidx(self, sidx: SegmentIndex) -> Self {
        match self {
            Instruction::CollectCommit { into: _ } => Instruction::CollectCommit { into: sidx },
            Instruction::ConnectNewSegment {
                parent_above: _,
                at_commit,
            } => Instruction::ConnectNewSegment {
                parent_above: sidx,
                at_commit,
            },
        }
    }
}

type QueueItem = (gix::ObjectId, CommitFlags, Instruction, Limit);

#[derive(Debug)]
pub(crate) struct EdgeOwned {
    source: SegmentIndex,
    target: SegmentIndex,
    weight: Edge,
    id: petgraph::graph::EdgeIndex,
}

impl From<EdgeReference<'_, Edge>> for EdgeOwned {
    fn from(e: EdgeReference<'_, Edge>) -> Self {
        EdgeOwned {
            source: e.source(),
            target: e.target(),
            weight: *e.weight(),
            id: e.id(),
        }
    }
}
