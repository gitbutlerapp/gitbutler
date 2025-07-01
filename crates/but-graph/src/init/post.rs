use crate::init::types::{EdgeOwned, TopoWalk};
use crate::init::{PetGraph, branch_segment_from_name_and_meta, remotes};
use crate::{Commit, CommitFlags, CommitIndex, Edge, Graph, SegmentIndex, is_workspace_ref_name};
use but_core::{RefMetadata, ref_metadata};
use gix::reference::Category;
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::collections::{BTreeMap, BTreeSet};

/// Processing
impl Graph {
    /// Now that the graph is complete, perform additional structural improvements with
    /// the requirement of them to be computationally cheap.
    pub(super) fn post_processed(
        mut self,
        meta: &impl RefMetadata,
        tip: gix::ObjectId,
        repo: &gix::Repository,
        symbolic_remote_names: &[String],
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<Self> {
        // For the first id to be inserted into our entrypoint segment, set index.
        if let Some((segment, ep_commit)) = self.entrypoint.as_mut() {
            *ep_commit = self
                .inner
                .node_weight(*segment)
                .and_then(|s| s.commit_index_of(tip));
        }

        self.workspace_upgrades(meta)?;
        self.non_workspace_adjustments(
            repo,
            symbolic_remote_names,
            configured_remote_tracking_branches,
        )?;

        Ok(self)
    }

    /// Perform operations on segments that can reach a workspace segment when searching upwards.
    ///
    /// * insert empty segments as defined by the workspace that affects its downstream.
    /// * put workspace connection into the order defined in the workspace metadata.
    fn workspace_upgrades(&mut self, meta: &impl RefMetadata) -> anyhow::Result<()> {
        let tips_with_ws_data: Vec<_> = self
            .tip_segments()
            .filter(|sidx| self.inner[*sidx].workspace_metadata().is_some())
            .collect();

        // Maps any segment to any workspace that it can reach, even the workspace maps itself as it may need processing too
        let mut ws_by_segment_map =
            BTreeMap::<SegmentIndex, Vec<(SegmentIndex, std::ops::Range<CommitIndex>)>>::new();
        for ws_sidx in tips_with_ws_data.iter().cloned() {
            let start_idx = self[ws_sidx].commits.first().map(|_| 0);
            let mut walk = TopoWalk::start_from(ws_sidx, start_idx, Direction::Outgoing);

            while let Some((sidx, commit_range)) = walk.next(&self.inner) {
                ws_by_segment_map
                    .entry(sidx)
                    .or_default()
                    .push((ws_sidx, commit_range));
            }
        }

        for (orig_sidx, mut ws_indices) in ws_by_segment_map {
            if ws_indices.len() > 1 {
                tracing::warn!(
                    "Segment named {ref_name:?} ({idx}) is contained in multiple workspaces - ignored for post-processing",
                    ref_name = self[orig_sidx].ref_name.as_ref(),
                    idx = orig_sidx.index()
                );
                continue;
            }
            let Some((_ws_sidx, ws_data, commit_range)) =
                ws_indices.pop().map(|(ws_sidx, commit_range)| {
                    let ws = &self[ws_sidx];
                    let md = ws
                        .workspace_metadata()
                        .expect("we know this is a workspace");
                    (ws_sidx, md.clone(), commit_range)
                })
            else {
                continue;
            };

            // Find all commit-refs which are mentioned in ws_data.stacks, for simplicity in any stack that matches (for now).
            // Stacks shouldn't be so different that they don't match reality anymore and each mutation has to re-set them to
            // match reality.
            let mut current_above = orig_sidx;
            let mut truncate_commits_from = None;
            for commit_idx in commit_range {
                let commit = &self[orig_sidx].commits[commit_idx];
                let has_inserted_segment_above = current_above != orig_sidx;
                let Some(ws_stack) = ws_data
                    .stacks
                    .iter()
                    .find(|stack| stack_contains_any_commit_branch(stack, commit))
                else {
                    // Now we have to assign this uninteresting commit to the last created segment, if there was one.
                    if has_inserted_segment_above {
                        let commit = commit.clone();
                        self[current_above].commits.push(commit);
                    }
                    continue;
                };
                if commit
                    .refs
                    .iter()
                    .any(|rn| is_workspace_ref_name(rn.as_ref()))
                {
                    tracing::warn!(
                        "Commit {} had eligible workspace references pointing to it - ignoring this for now",
                        commit.id
                    );
                    // Now we have to assign this uninteresting commit to the last created segment, if there was one.
                    if has_inserted_segment_above {
                        let commit = commit.clone();
                        self[current_above].commits.push(commit);
                    }
                    continue;
                }

                // In ws-stack segment order, map all the indices from top to bottom
                current_above = create_connected_multi_segment(
                    self,
                    current_above,
                    orig_sidx,
                    commit_idx,
                    ws_stack,
                    meta,
                )?
                .unwrap_or(current_above);
                truncate_commits_from.get_or_insert(commit_idx);
            }
            if let Some(truncate_from) = truncate_commits_from {
                let segment = &mut self[orig_sidx];
                // Keep only the commits that weren't reassigned to other segments.
                segment.commits.truncate(truncate_from);
                delete_anon_if_empty_and_reconnect(self, orig_sidx);
            }
        }

        // Redo workspace outgoing connections according to desired stack order.
        for ws_tip in tips_with_ws_data {
            let ws_data = &self[ws_tip]
                .workspace_metadata()
                .expect("tips are chosen because they have metadata");
            let mut edges_pointing_to_named_segment = self
                .edges_directed_in_order_of_creation(ws_tip, Direction::Outgoing)
                .into_iter()
                .map(|e| {
                    let rn = self[e.target()].ref_name.clone();
                    (e.id(), e.target(), rn)
                })
                .collect::<Vec<_>>();

            let edges_original_order: Vec<_> = edges_pointing_to_named_segment
                .iter()
                .map(|(_e, sidx, _rn)| *sidx)
                .collect();
            edges_pointing_to_named_segment.sort_by_key(|(_e, sidx, rn)| {
                let res = ws_data.stacks.iter().position(|s| {
                    s.branches
                        .first()
                        .is_some_and(|b| Some(&b.ref_name) == rn.as_ref())
                });
                // This makes it so that edges that weren't mentioned in workspace metadata
                // retain their relative order, with first-come-first-serve semantics.
                // The expected case is that each segment is defined.
                res.or_else(|| {
                    edges_original_order
                        .iter()
                        .position(|sidx_for_order| sidx_for_order == sidx)
                })
            });

            for (eid, target_sidx, _) in edges_pointing_to_named_segment {
                let weight = self
                    .inner
                    .remove_edge(eid)
                    .expect("we found the edge before");
                // Reconnect according to the new order.
                self.inner.add_edge(ws_tip, target_sidx, weight);
            }
        }
        Ok(())
    }

    /// Name ambiguous segments if they are reachable by remote tracking branch and
    /// if the first commit has (unambiguously) the matching local tracking branch.
    fn non_workspace_adjustments(
        &mut self,
        repo: &gix::Repository,
        symbolic_remote_names: &[String],
        configured_remote_tracking_branches: &BTreeSet<gix::refs::FullName>,
    ) -> anyhow::Result<()> {
        // Map (segment-to-be-named, [candidate-remote]), so we don't set a name if there is more
        // than one remote.
        let mut remotes_by_segment_map =
            BTreeMap::<SegmentIndex, Vec<(gix::refs::FullName, gix::refs::FullName)>>::new();

        for (remote_sidx, remote_ref_name) in self.inner.node_indices().filter_map(|sidx| {
            self[sidx]
                .ref_name
                .as_ref()
                .filter(|rn| (rn.category() == Some(Category::RemoteBranch)))
                .map(|rn| (sidx, rn))
        }) {
            let start_idx = self[remote_sidx].commits.first().map(|_| 0);
            let mut walk = TopoWalk::start_from(remote_sidx, start_idx, Direction::Outgoing)
                .skip_tip_segment();

            while let Some((sidx, commit_range)) = walk.next(&self.inner) {
                let segment = &self[sidx];
                if segment.ref_name.is_some() {
                    // Assume simple linear histories - otherwise this could abort too early, and
                    // we'd need a complex traversal - not now.
                    break;
                }

                if segment.commits.is_empty() {
                    // skip over empty anonymous buckets, even though these shouldn't exist, ever.
                    tracing::warn!(
                        "Skipped segment {sidx} which was anonymous and empty",
                        sidx = sidx.index()
                    );
                    continue;
                } else if segment.commits[commit_range]
                    .iter()
                    .all(|c| c.flags.contains(CommitFlags::NotInRemote))
                {
                    // a candidate for naming, and we'd either expect all or none of the commits
                    // to be in or outside a remote.
                    let first_commit = segment.commits.first().expect("we know there is commits");
                    if let Some(local_tracking_branch) = first_commit.refs.iter().find_map(|rn| {
                        remotes::lookup_remote_tracking_branch_or_deduce_it(
                            repo,
                            rn.as_ref(),
                            symbolic_remote_names,
                            configured_remote_tracking_branches,
                        )
                        .ok()
                        .flatten()
                        .and_then(|rrn| {
                            (rrn.as_ref() == remote_ref_name.as_ref()).then_some(rn.clone())
                        })
                    }) {
                        remotes_by_segment_map
                            .entry(sidx)
                            .or_default()
                            .push((local_tracking_branch, remote_ref_name.clone()));
                    }
                    break;
                }
                // Assume that the segment is fully remote.
                continue;
            }
        }

        for (anon_sidx, mut disambiguated_name) in remotes_by_segment_map
            .into_iter()
            .filter(|(_, candidates)| candidates.len() == 1)
        {
            let s = &mut self[anon_sidx];
            let (local, remote) = disambiguated_name.pop().expect("one item as checked above");
            s.ref_name = Some(local);
            s.remote_tracking_ref_name = Some(remote);
            let rn = s.ref_name.as_ref().unwrap();
            s.commits.first_mut().unwrap().refs.retain(|crn| crn != rn);
        }

        // TODO: we should probably try to set this right when we traverse the segment
        //       to save remote-ref lookup.
        for segment in self.inner.node_weights_mut() {
            if segment.remote_tracking_ref_name.is_some() {
                continue;
            };
            let Some(ref_name) = segment.ref_name.as_ref() else {
                continue;
            };
            segment.remote_tracking_ref_name = remotes::lookup_remote_tracking_branch_or_deduce_it(
                repo,
                ref_name.as_ref(),
                symbolic_remote_names,
                configured_remote_tracking_branches,
            )?;
        }
        Ok(())
    }
}

fn delete_anon_if_empty_and_reconnect(graph: &mut Graph, sidx: SegmentIndex) {
    let segment = &graph[sidx];
    let may_delete = segment.commits.is_empty() && segment.ref_name.is_none();
    if !may_delete {
        return;
    }

    let mut outgoing = graph.inner.edges_directed(sidx, Direction::Outgoing);
    let Some(first_outgoing) = outgoing.next() else {
        return;
    };

    if outgoing.next().is_some() {
        return;
    }
    // Reconnect
    let new_target = first_outgoing.target();
    let incoming: Vec<_> = graph
        .inner
        .edges_directed(sidx, Direction::Incoming)
        .map(EdgeOwned::from)
        .collect();
    for edge in incoming.iter().rev() {
        graph.inner.add_edge(edge.source, new_target, edge.weight);
    }
    graph.inner.remove_node(sidx);

    if let Some(ep_sidx) = graph
        .entrypoint
        .as_mut()
        .map(|t| &mut t.0)
        .filter(|ep_sidx| **ep_sidx == sidx)
    {
        *ep_sidx = new_target;
    }
}

/// Create a new stack from `N` refs that match a ref in `ws_stack` (in the order given there), with `N-1` segments being empty on top
/// of the last one `N`.
/// `commit_parent` is the segment to use `commit_idx` on to get its data. We also use this information to re-link
/// Return `Some(bottom_segment_index)`, or `None` no ref matched commit. There may be any amount of new segments above
/// the `bottom_segment_index`.
/// Note that the Segment at `bottom_segment_index` will own `commit`.
/// Also note that we reconnect commit-by-commit, so the outer processing has to do that.
fn create_connected_multi_segment(
    graph: &mut Graph,
    mut above_idx: SegmentIndex,
    commit_parent: SegmentIndex,
    commit_idx: CommitIndex,
    ws_stack: &ref_metadata::WorkspaceStack,
    meta: &impl RefMetadata,
) -> anyhow::Result<Option<SegmentIndex>> {
    let commit = &graph[commit_parent].commits[commit_idx];
    let matching_refs_in_commit_indices: Vec<_> = ws_stack
        .branches
        .iter()
        .filter_map(|s| commit.refs.iter().position(|crn| &s.ref_name == crn))
        .collect();
    if matching_refs_in_commit_indices.is_empty() {
        return Ok(None);
    }

    let (commit, refs) = {
        let mut current = 0;
        let mut c = commit.clone();
        let refs = commit.refs.clone();
        c.refs.retain(|_rn| {
            let keep = !matching_refs_in_commit_indices.contains(&current);
            current += 1;
            keep
        });
        (c, refs)
    };
    let iter_len = matching_refs_in_commit_indices.len();
    for (is_first, is_last, ref_idx) in
        matching_refs_in_commit_indices
            .into_iter()
            .enumerate()
            .map(|(idx, ref_idx)| {
                let (mut first, mut last) = (false, false);
                if idx == 0 {
                    first = true;
                }
                if idx + 1 == iter_len {
                    last = true;
                }
                (first, last, ref_idx)
            })
    {
        let ref_name = &refs[ref_idx];
        let new_segment =
            branch_segment_from_name_and_meta(Some((ref_name.clone(), None)), meta, None)?;
        let above_commit_idx = {
            let s = &graph[above_idx];
            let cidx = s.commit_index_of(commit.id);
            if cidx.is_some() {
                // We will take the current commit, so must commit to the one above.
                // This works just once, for the actually passed parent commit.
                cidx.and_then(|cidx| cidx.checked_sub(1))
            } else {
                // Otherwise, assure the connection is valid by using the last commit.
                s.last_commit_index()
            }
        };
        let new_segment = graph.connect_new_segment(
            above_idx,
            above_commit_idx,
            new_segment,
            is_last.then_some(0),
            is_last.then_some(commit.id),
        );
        above_idx = new_segment;
        if is_first {
            // connect incoming edges (and disconnect from source)
            // Connect to the commit if we have one.
            let edges = collect_edges_at_commit_reverse_order(
                &graph.inner,
                (commit_parent, commit_idx),
                Direction::Incoming,
            );
            for edge in &edges {
                graph.inner.remove_edge(edge.id);
            }
            for edge in edges.into_iter().rev() {
                let (target, target_cidx) = if commit_idx == 0 {
                    // the current target of the edge will be empty after we steal its commit.
                    // Thus, we want to keep pointing to it to naturally reach the commit later.
                    (edge.target, None)
                } else {
                    // The new segment is the shortest way to the commit we loose.
                    (new_segment, is_last.then_some(0))
                };
                graph.inner.add_edge(
                    edge.source,
                    target,
                    Edge {
                        src: edge.weight.src,
                        src_id: edge.weight.src_id,
                        dst: target_cidx,
                        dst_id: target_cidx.map(|_| commit.id),
                    },
                );
            }
        }
        if is_last {
            // connect outgoing edges (and disconnect them)
            let commit_id = commit.id;
            graph[new_segment].commits.push(commit);

            let edges = collect_edges_at_commit_reverse_order(
                &graph.inner,
                (commit_parent, commit_idx),
                Direction::Outgoing,
            );
            for edge in &edges {
                graph.inner.remove_edge(edge.id);
            }
            for edge in edges.into_iter().rev() {
                graph.inner.add_edge(
                    new_segment,
                    edge.target,
                    Edge {
                        src: Some(0),
                        src_id: Some(commit_id),
                        dst: edge.weight.dst,
                        dst_id: edge.weight.dst_id,
                    },
                );
            }
            break;
        }
    }
    Ok(Some(above_idx))
}

fn collect_edges_at_commit_reverse_order(
    graph: &PetGraph,
    (segment, commit): (SegmentIndex, CommitIndex),
    direction: Direction,
) -> Vec<EdgeOwned> {
    graph
        .edges_directed(segment, direction)
        .filter(|&e| match direction {
            Direction::Incoming => e.weight().dst == Some(commit),
            Direction::Outgoing => e.weight().src == Some(commit),
        })
        .map(Into::into)
        .collect()
}

fn stack_contains_any_commit_branch(stack: &ref_metadata::WorkspaceStack, c: &Commit) -> bool {
    stack
        .branches
        .iter()
        .any(|sref| c.refs.iter().any(|cref| *cref == sref.ref_name))
}
