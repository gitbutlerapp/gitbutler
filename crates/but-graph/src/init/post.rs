use crate::init::walk::TopoWalk;
use crate::init::{EdgeOwned, PetGraph, branch_segment_from_name_and_meta};
use crate::{Commit, CommitIndex, Edge, Graph, SegmentIndex};
use bstr::{BStr, ByteSlice};
use but_core::{RefMetadata, ref_metadata};
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use std::collections::BTreeMap;

/// Processing
impl Graph {
    /// Now that the graph is complete, perform additional structural improvements with the requirement of them to be computationally cheap.
    ///
    /// * insert empty segments as defined by the workspace that affects its downstream.
    pub(super) fn post_processed(
        mut self,
        meta: &impl RefMetadata,
        tip: gix::ObjectId,
    ) -> anyhow::Result<Self> {
        // For the first id to be inserted into our entrypoint segment, set index.
        if let Some((segment, ep_commit)) = self.entrypoint.as_mut() {
            *ep_commit = self
                .inner
                .node_weight(*segment)
                .and_then(|s| s.commit_index_of(tip));
        }
        fn stack_contains_any_commit_branch(
            stack: &ref_metadata::WorkspaceStack,
            c: &Commit,
        ) -> bool {
            stack
                .branches
                .iter()
                .any(|sref| c.refs.iter().any(|cref| *cref == sref.ref_name))
        }
        // Maps any segment to any workspace that it can reach, even the workspace maps itself as it may need processing too
        let mut ws_by_segment_map =
            BTreeMap::<SegmentIndex, Vec<(SegmentIndex, std::ops::Range<CommitIndex>)>>::new();
        // All non-workspace segments that are reachable from a workspace segment.
        let mut reachable_by_ws = BTreeMap::<SegmentIndex, Vec<SegmentIndex>>::new();

        for ws_sidx in self
            .tip_segments()
            .filter(|sidx| self.inner[*sidx].workspace_metadata().is_some())
        {
            let start_idx = self[ws_sidx].commits.first().map(|_| 0);
            let mut walk = TopoWalk::start_from(ws_sidx, start_idx, Direction::Outgoing);

            while let Some((sidx, commit_range)) = walk.next(&self.inner) {
                ws_by_segment_map
                    .entry(sidx)
                    .or_default()
                    .push((ws_sidx, commit_range));
                reachable_by_ws.entry(ws_sidx).or_default().push(sidx);
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
                if is_managed_workspace_commit(commit.message.as_bstr()) {
                    tracing::warn!(
                        "Workspace commit {} had eligible references pointing to it - ignoring this for now",
                        commit.id
                    );
                    // Now we have to assign this uninteresting commit to the last created segment, if there was one.
                    if has_inserted_segment_above {
                        let commit = commit.clone();
                        self[current_above].commits.push(commit);
                    }
                    continue;
                }
                let commit_has_target_ref = commit
                    .refs
                    .iter()
                    .any(|rn| Some(rn) == ws_data.target_ref.as_ref());
                if commit_has_target_ref {
                    // wire it up as a new stack, but also find and create more stacks that may match other commit refs,
                    // and continue to the next segment when done (treat target ref commits as terminal).
                    // We link the new segment as downstream of the workspace commit.
                    todo!("multi-stack target handling");
                }

                // In ws-stack segment order, map all the indices from top to bottom
                current_above = create_multi_segment(
                    &mut self,
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
                delete_if_empty_and_reconnect(&mut self, orig_sidx);
            }
        }
        Ok(self)
    }
}

fn delete_if_empty_and_reconnect(graph: &mut Graph, sidx: SegmentIndex) {
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
    for edge in &incoming {
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

fn is_managed_workspace_commit(message: &BStr) -> bool {
    let message = gix::objs::commit::MessageRef::from_bytes(message);
    let title = message.title.trim().as_bstr();
    title == "GitButler Workspace Commit" || title == "GitButler Integration Commit"
}

/// Create a new stack from `N` refs that match a ref in `ws_stack` (in the order given there), with `N-1` segments being empty on top
/// of the last one `N`.
/// `commit_parent` is the segment to use `commit_idx` on to get its data. We also use this information to re-link
/// Return `Some(bottom_segment_index)`, or `None` no ref matched commit. There may be any amount of new segments above
/// the `bottom_segment_index`.
/// Note that the Segment at `bottom_segment_index` will own `commit`.
/// Also note that we reconnect commit-by-commit, so the outer processing has to do that.
fn create_multi_segment(
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
        let new_segment = branch_segment_from_name_and_meta(Some(ref_name.clone()), meta, None)?;
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
            let edges = collect_edges_at_commit(
                &graph.inner,
                (commit_parent, commit_idx),
                Direction::Incoming,
            );
            for edge in &edges {
                graph.inner.remove_edge(edge.id);
            }
            for edge in edges {
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

            let edges = collect_edges_at_commit(
                &graph.inner,
                (commit_parent, commit_idx),
                Direction::Outgoing,
            );
            for edge in &edges {
                graph.inner.remove_edge(edge.id);
            }
            for edge in edges {
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

fn collect_edges_at_commit(
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
