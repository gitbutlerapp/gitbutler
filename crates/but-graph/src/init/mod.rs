use crate::{CommitFlags, Edge};
use crate::{CommitIndex, Graph, Segment, SegmentIndex, SegmentMetadata};
use anyhow::{Context, bail};
use but_core::RefMetadata;
use gix::ObjectId;
use gix::hashtable::hash_map::Entry;
use gix::prelude::{ObjectIdExt, ReferenceExt};
use gix::refs::Category;
use petgraph::graph::EdgeReference;
use petgraph::prelude::EdgeRef;
use std::collections::VecDeque;

mod utils;
use utils::*;

mod remotes;

mod post;
mod walk;

pub(super) type PetGraph = petgraph::Graph<Segment, Edge>;

/// Options for use in [`Graph::from_head()`] and [`Graph::from_commit_traversal()`].
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// Associate tag references with commits.
    ///
    /// If `false`, tags are not collected.
    pub collect_tags: bool,
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
                    Some(ref_name),
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
    /// ### (Arbitrary) Rules
    ///
    /// These rules should help to create graphs and segmentations that feel natural and are desirable to the user.
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
    ///     - Remotes never take commits taht are already owned.
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        Options { collect_tags }: Options,
    ) -> anyhow::Result<Self> {
        // TODO: also traverse (outside)-branches that ought to be in the workspace. That way we have the desired ones
        //       automatically and just have to find a way to prune the undesired ones.
        // TODO: pickup ref-names and see if some simple logic can avoid messes, like lot's of refs pointing to a single commit.
        //       while at it: make tags work.
        let repo = tip.repo;
        let ref_name = ref_name.into();
        if ref_name
            .as_ref()
            .is_some_and(|name| name.category() == Some(Category::RemoteBranch))
        {
            // TODO: see if this is a thing - Git doesn't like to checkout remote tracking branches by name,
            //       and if we should handle it, we need to setup the initial flags accordingly.
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
        let mut workspaces = obtain_workspace_infos(ref_name.as_ref().map(|rn| rn.as_ref()), meta)?;
        let current = graph.insert_root(branch_segment_from_name_and_meta(
            ref_name.clone(),
            meta,
            Some((&refs_by_id, tip.detach())),
        )?);
        let mut seen = gix::revwalk::graph::IdMap::<SegmentIndex>::default();
        let mut flags = CommitFlags::NotInRemote;

        let target_symbolic_remote_names = {
            let remote_names = repo.remote_names();
            let mut v: Vec<_> = workspaces
                .iter()
                .filter_map(|(_, data)| {
                    let target_ref = data.target_ref.as_ref()?;
                    remotes::extract_remote_name(target_ref.as_ref(), &remote_names)
                })
                .collect();
            v.sort();
            v.dedup();
            v
        };

        if let Some(branch_ref) = ref_name {
            // Transfer workspace data to our current ref if it has some.
            workspaces.retain(|(workspace_ref, _workspace_info)| {
                if workspace_ref != &branch_ref {
                    return true;
                }

                let current = &mut graph[current];
                debug_assert!(
                    matches!(current.metadata, Some(SegmentMetadata::Workspace(_))),
                    "BUG: newly created segments have the right metadata"
                );
                flags |= CommitFlags::InWorkspace;
                false
            })
        }

        let mut next = VecDeque::<QueueItem>::new();
        next.push_back((
            tip.detach(),
            flags,
            Instruction::CollectCommit { into: current },
        ));
        for (ws_ref, workspace_info) in workspaces {
            let Some(ws_tip) = try_refname_to_id(repo, ws_ref.as_ref())? else {
                tracing::warn!(
                    "Ignoring stale workspace ref '{ws_ref}', which didn't exist in Git but still had workspace data",
                    ws_ref = ws_ref.as_bstr()
                );
                continue;
            };
            let mut ws_segment = branch_segment_from_name_and_meta(Some(ws_ref), meta, None)?;
            ws_segment.metadata = Some(SegmentMetadata::Workspace(workspace_info));
            let ws_segment = graph.insert_root(ws_segment);
            // As workspaces typically have integration branches which can help us to stop the traversal,
            // pick these up first.
            next.push_front((
                ws_tip,
                CommitFlags::InWorkspace,
                Instruction::CollectCommit { into: ws_segment },
            ));
        }

        while let Some((id, mut propagated_flags, instruction)) = next.pop_front() {
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
                    Entry::Occupied(mut existing_sidx) => {
                        let dst_sidx = *existing_sidx.get();
                        let (top_sidx, mut bottom_sidx) =
                            // If a normal branch walks into a workspace branch, put the workspace branch on top.
                            if graph[dst_sidx].workspace_metadata().is_some() &&
                                graph[src_sidx].ref_name.as_ref()
                                    .is_some_and(|rn| rn.category().is_some_and(|c| matches!(c, Category::LocalBranch))) {
                                // `dst` is basically swapping with `src`, so must swap commits and connections.
                                swap_commits_and_connections(&mut graph.inner, dst_sidx, src_sidx);
                                swap_queued_segments(&mut next, dst_sidx, src_sidx);

                                // Assure the first commit doesn't name the new owner segment.
                                {
                                    let s = &mut graph[src_sidx];
                                    if let Some(c) = s.commits.first_mut() {
                                        c.refs.retain(|rn| Some(rn) != s.ref_name.as_ref())
                                    }
                                    // Update the commit-ownership of the connecting commit, but also
                                    // of all other commits in the segment.
                                    existing_sidx.insert(src_sidx);
                                    for commit_id in s.commits.iter().skip(1).map(|c| c.id) {
                                        seen.entry(commit_id).insert(src_sidx);
                                    }
                                }
                                (dst_sidx, src_sidx)
                            } else {
                                // `src` naturally runs into destination, so nothing needs to be done
                                // except for connecting both. Commit ownership doesn't change.
                                (src_sidx, dst_sidx)
                            };
                        let top_cidx = graph[top_sidx].last_commit_index();
                        let mut bottom_cidx =
                            graph[bottom_sidx].commit_index_of(id).with_context(|| {
                                format!(
                                    "BUG: Didn't find commit {id} in segment {bottom_sidx}",
                                    bottom_sidx = dst_sidx.index(),
                                )
                            })?;

                        if bottom_cidx != 0 {
                            let new_bottom_sidx = split_commit_into_segment(
                                &mut graph,
                                &mut next,
                                &mut seen,
                                bottom_sidx,
                                bottom_cidx,
                            )?;
                            bottom_sidx = new_bottom_sidx;
                            bottom_cidx = 0;
                        }
                        graph.connect_segments(top_sidx, top_cidx, bottom_sidx, bottom_cidx);
                        let top_flags = top_cidx
                            .map(|cidx| graph[top_sidx].commits[cidx].flags)
                            .unwrap_or_default();
                        let bottom_flags = graph[bottom_sidx].commits[bottom_cidx].flags;
                        propagate_flags_downward(
                            &mut graph.inner,
                            propagated_flags | top_flags | bottom_flags,
                            bottom_sidx,
                            Some(bottom_cidx),
                        );
                        graph.validate_or_eprint_dot().unwrap();

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
                        todo!("handle previously existing segment when connecting a new one")
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

            let segment = &mut graph[segment_idx_for_id];
            let commit_idx_for_possible_fork = segment.commits.len();
            queue_parents(
                &mut next,
                &info.parent_ids,
                propagated_flags,
                segment_idx_for_id,
                commit_idx_for_possible_fork,
            );

            let refs_at_commit_before_removal = refs_by_id.remove(&id).unwrap_or_default();
            segment.commits.push(
                info.into_local_commit(
                    repo,
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

            try_queue_remote_tracking_branches(
                repo,
                &refs_at_commit_before_removal,
                &mut next,
                &mut graph,
                &target_symbolic_remote_names,
                &configured_remote_tracking_branches,
                meta,
            )?;
        }

        graph.post_processed(
            meta,
            tip.detach(),
            repo,
            &target_symbolic_remote_names,
            &configured_remote_tracking_branches,
        )
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

type QueueItem = (ObjectId, CommitFlags, Instruction);

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
