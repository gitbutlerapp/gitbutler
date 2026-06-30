//! Ad-hoc/single-branch mode: persisted GitButler-created branch ordering, applied to a finished
//! [`Graph`] by the non-managed builds — it only reads segments, refs, and metadata, so it is
//! builder-agnostic.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context as _;
use but_core::RefMetadata;
use gix::reference::Category;

use crate::{
    Direction, EntryPointCommit, Graph, SegmentIndex,
    init::{
        branch_segment_from_name_and_meta,
        overlay::{OverlayMetadata, OverlayRepo},
        types::EdgeOwned,
        walk::WorktreeByBranch,
    },
};

impl Graph {
    /// In ad-hoc/single-branch mode, use persisted GitButler-created branch
    /// ordering to split multiple same-tip refs into empty stack segments.
    pub(crate) fn ad_hoc_branch_stack_upgrades<T: RefMetadata>(
        &mut self,
        repo: &OverlayRepo<'_>,
        meta: &OverlayMetadata<'_, T>,
        worktree_by_branch: &WorktreeByBranch,
    ) -> anyhow::Result<()> {
        let Some(entrypoint_ref) = self.entrypoint_ref.clone() else {
            return Ok(());
        };
        if entrypoint_ref.category() != Some(Category::LocalBranch) {
            return Ok(());
        }
        let Some((_entrypoint_sidx, _entrypoint)) = self.entrypoint else {
            return Ok(());
        };
        let Some(branch_order) = meta.branch_stack_order(entrypoint_ref.as_ref())? else {
            return Ok(());
        };
        let mut existing_ordered_refs = Vec::new();
        for branch in branch_order {
            if branch.category() != Some(Category::LocalBranch) {
                continue;
            }
            if repo
                .try_find_reference(branch.as_ref())
                .with_context(|| {
                    format!(
                        "failed to find ordered ad-hoc branch '{}'",
                        branch.shorten()
                    )
                })?
                .is_some()
            {
                existing_ordered_refs.push(branch);
            }
        }
        if existing_ordered_refs.len() < 2 {
            return Ok(());
        }

        let Some((bottom_ref, _)) = existing_ordered_refs.split_last() else {
            return Ok(());
        };
        let Some(mut bottom_reference) = repo
            .try_find_reference(bottom_ref.as_ref())
            .with_context(|| {
                format!(
                    "failed to find bottom ordered ad-hoc branch '{}'",
                    bottom_ref.shorten()
                )
            })?
        else {
            return Ok(());
        };
        let bottom_commit_id = bottom_reference
            .peel_to_id()
            .with_context(|| {
                format!(
                    "failed to peel bottom ordered ad-hoc branch '{}'",
                    bottom_ref.shorten()
                )
            })?
            .detach();

        let mut matching_refs = Vec::new();
        for branch in &existing_ordered_refs {
            let Some(mut reference) =
                repo.try_find_reference(branch.as_ref()).with_context(|| {
                    format!(
                        "failed to find ordered ad-hoc branch '{}'",
                        branch.shorten()
                    )
                })?
            else {
                continue;
            };
            if reference
                .peel_to_id()
                .with_context(|| {
                    format!(
                        "failed to peel ordered ad-hoc branch '{}'",
                        branch.shorten()
                    )
                })?
                .detach()
                == bottom_commit_id
            {
                matching_refs.push(branch.clone());
            }
        }
        if matching_refs.len() < 2 {
            return Ok(());
        }
        self.ad_hoc_branch_stack_orders.push(matching_refs.clone());

        let bottom_segment_id =
            ad_hoc_order_bottom_segment_id(self, bottom_ref.as_ref(), bottom_commit_id);
        let Some(bottom_segment_id) = bottom_segment_id else {
            return Ok(());
        };

        rebuild_same_tip_segment_chain_by_branch_order(
            self,
            bottom_segment_id,
            matching_refs,
            entrypoint_ref.as_ref(),
            meta,
            worktree_by_branch,
        )?;
        Ok(())
    }
}

fn ad_hoc_order_bottom_segment_id(
    graph: &Graph,
    bottom_ref: &gix::refs::FullNameRef,
    bottom_commit_id: gix::ObjectId,
) -> Option<SegmentIndex> {
    graph
        .node_weights()
        .find(|segment| {
            segment
                .ref_name()
                .is_some_and(|ref_name| ref_name == bottom_ref)
        })
        .or_else(|| {
            graph
                .node_weights()
                .find(|segment| segment.tip() == Some(bottom_commit_id))
        })
        .map(|segment| segment.id)
}

/// Rebuild a same-tip ref bundle into the persisted ad-hoc branch order.
///
/// The refs all point at the same commit, so only the bottom ordered branch
/// owns that commit. Branches above it become explicit empty segments, allowing
/// single-branch mode to represent "top is empty above bottom" without managed
/// workspace metadata.
fn rebuild_same_tip_segment_chain_by_branch_order<T: RefMetadata>(
    graph: &mut Graph,
    bottom_segment_id: SegmentIndex,
    matching_refs: Vec<gix::refs::FullName>,
    entrypoint_ref: &gix::refs::FullNameRef,
    meta: &OverlayMetadata<'_, T>,
    worktree_by_branch: &WorktreeByBranch,
) -> anyhow::Result<()> {
    let Some((bottom_ref, empty_refs)) = matching_refs.split_last() else {
        return Ok(());
    };
    if empty_refs.is_empty() {
        return Ok(());
    }

    let mut empty_segment_by_ref = BTreeMap::new();
    for segment in graph.node_weights() {
        if segment.commits.is_empty()
            && let Some(ref_name) = segment.ref_name()
            && empty_refs
                .iter()
                .any(|empty_ref| empty_ref.as_ref() == ref_name)
        {
            empty_segment_by_ref.insert(ref_name.to_owned(), segment.id);
        }
    }

    let bottom_commit_id = graph[bottom_segment_id]
        .commits
        .first()
        .map(|commit| commit.id);
    let mut bottom_segment = branch_segment_from_name_and_meta(
        Some((bottom_ref.clone(), None)),
        meta,
        None,
        worktree_by_branch,
    )?;
    if let Some(ref_info) = bottom_segment.ref_info.as_mut() {
        ref_info.commit_id = bottom_commit_id;
    }
    graph[bottom_segment_id].ref_info = bottom_segment.ref_info;
    graph[bottom_segment_id].metadata = bottom_segment.metadata;
    if let Some(commit) = graph[bottom_segment_id].commits.first_mut() {
        commit.refs.retain(|ri| {
            !matching_refs
                .iter()
                .any(|ref_name| ref_name == &ri.ref_name)
        });
    }

    let mut ordered_segment_ids = Vec::new();
    let mut entrypoint_segment_id = None;
    for ref_name in empty_refs {
        let segment_id = if let Some(segment_id) = empty_segment_by_ref.get(ref_name.as_ref()) {
            *segment_id
        } else {
            let new_segment = branch_segment_from_name_and_meta(
                Some((ref_name.clone(), None)),
                meta,
                None,
                worktree_by_branch,
            )?;
            graph.insert_segment(new_segment)
        };
        if ref_name.as_ref() == entrypoint_ref {
            entrypoint_segment_id = Some(segment_id);
        }
        ordered_segment_ids.push(segment_id);
    }
    ordered_segment_ids.push(bottom_segment_id);
    if bottom_ref.as_ref() == entrypoint_ref {
        entrypoint_segment_id = Some(bottom_segment_id);
    }

    let involved_segments = ordered_segment_ids.iter().copied().collect::<BTreeSet<_>>();
    let top_segment_id = ordered_segment_ids
        .first()
        .copied()
        .context("BUG: empty refs should create a top segment")?;
    // Re-point outside incoming edges at the (empty) top segment in place, preserving their
    // position in the source's connections and with it the source's parent order.
    let incoming_edges: Vec<_> = involved_segments
        .iter()
        .flat_map(|segment_id| {
            graph
                .inner
                .edges_directed(*segment_id, Direction::Incoming)
                .map(EdgeOwned::from)
                .collect::<Vec<_>>()
        })
        .filter(|edge| !involved_segments.contains(&edge.source))
        .collect();
    for edge in incoming_edges {
        let weight = graph
            .inner
            .edge_weight_mut(edge.source, &edge.weight)
            .expect("still present as we just saw it");
        weight.target = top_segment_id;
        weight.dst = None;
        weight.dst_id = None;
    }
    // Drop the involved segments' own outgoing connections — except the bottom segment's
    // connections leading outside — then re-chain the segments in the desired order.
    let outgoing_edges_to_remove: Vec<_> = involved_segments
        .iter()
        .flat_map(|segment_id| {
            graph
                .inner
                .edges_directed(*segment_id, Direction::Outgoing)
                .filter(|edge| {
                    *segment_id != bottom_segment_id || involved_segments.contains(&edge.target())
                })
                .map(EdgeOwned::from)
                .collect::<Vec<_>>()
        })
        .collect();
    for edge in outgoing_edges_to_remove {
        graph.inner.remove_edge(edge.source, &edge.weight);
    }

    for pair in ordered_segment_ids.windows(2) {
        let [above, below] = pair else {
            continue;
        };
        let below_commit_index = (*below == bottom_segment_id).then_some(0);
        graph.connect_segments(*above, None, *below, below_commit_index);
    }

    if graph
        .entrypoint
        .as_ref()
        .is_some_and(|(entrypoint_segment_id, _)| involved_segments.contains(entrypoint_segment_id))
    {
        let entrypoint_commit = bottom_commit_id
            .map(EntryPointCommit::AtCommit)
            .unwrap_or(EntryPointCommit::Unborn);
        graph.entrypoint = Some((
            entrypoint_segment_id
                .context("BUG: split branch order should retain the entrypoint ref")?,
            entrypoint_commit,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Commit, CommitFlags, RefInfo, Segment};

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid test object id")
    }

    fn ref_name(name: &str) -> gix::refs::FullName {
        name.try_into().expect("valid full ref name")
    }

    fn segment(name: &str, commit_id: gix::ObjectId) -> Segment {
        Segment {
            ref_info: Some(RefInfo {
                ref_name: ref_name(name),
                commit_id: Some(commit_id),
                worktree: None,
            }),
            commits: vec![Commit {
                id: commit_id,
                parent_ids: Vec::new(),
                flags: CommitFlags::default(),
                refs: Vec::new(),
            }],
            ..Default::default()
        }
    }

    #[test]
    fn ad_hoc_bottom_segment_selection_prefers_segment_named_by_bottom_ref() {
        let commit_id = oid("1111111111111111111111111111111111111111");
        let bottom_ref = ref_name("refs/heads/bottom");
        let mut graph = Graph::default();
        let unrelated = graph.insert_segment(segment("refs/heads/unrelated", commit_id));
        let bottom = graph.insert_segment(segment("refs/heads/bottom", commit_id));

        assert_eq!(
            ad_hoc_order_bottom_segment_id(&graph, bottom_ref.as_ref(), commit_id),
            Some(bottom),
            "same-tip ordered branch rewrite should not pick an unrelated segment first"
        );
        assert_ne!(
            unrelated, bottom,
            "test setup must create two distinct same-tip segments"
        );
    }
}
