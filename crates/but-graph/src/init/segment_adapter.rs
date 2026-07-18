use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, bail, ensure};
use gix::refs::Category;

use crate::{
    Commit, CommitFlags, EntryPointCommit, Graph, Node, NodeGraph, NodeGraphEntrypoint, NodeIndex,
    NodeKind, Segment, SegmentIndex, StopCondition, node::ConstructionContext,
};

impl NodeGraph {
    /// Convert the construction graph into the legacy segmented compatibility view.
    pub(crate) fn into_segment_graph(self) -> Result<Graph> {
        let graph = self.validated()?;
        let construction_graph = std::sync::Arc::new(graph.clone());
        let NodeGraph {
            nodes,
            annotations,
            context,
        } = graph;
        let ConstructionContext {
            entrypoint,
            entrypoint_ref,
            managed_workspace_commit_id,
            traversal_tips,
            ad_hoc_branch_stack_orders,
            hard_limit_hit,
            options,
            project_meta,
            symbolic_remote_names,
        } = context;

        let children = children_by_node(&nodes);
        let mut required_first_commit_ids = traversal_tips
            .iter()
            .map(|tip| tip.id)
            .collect::<BTreeSet<_>>();
        required_first_commit_ids.extend(project_meta.target_commit_id);
        if let NodeGraphEntrypoint::Node(index) = entrypoint
            && let Some(id) = node_target_id(&nodes[index])
        {
            required_first_commit_ids.insert(id);
        }
        let forced_commit_starts = traversal_tips
            .iter()
            .filter_map(|tip| {
                (tip.is_detached
                    || matches!(tip.role, crate::init::TipRole::TargetRemote)
                        && tip.ref_name.is_none())
                .then_some(tip.id)
            })
            .collect::<BTreeSet<_>>();
        let entrypoint_node = match &entrypoint {
            NodeGraphEntrypoint::Node(index) => Some(*index),
            NodeGraphEntrypoint::Unborn(_) => None,
        };
        let stored_target_id = project_meta.target_commit_id;
        let configured_target_ref = project_meta.target_ref.clone();
        let explicit_target_local_ref_names = traversal_tips
            .iter()
            .filter_map(|tip| match &tip.role {
                crate::init::TipRole::TargetLocal { local_ref_name } => {
                    Some(local_ref_name.clone())
                }
                _ => None,
            })
            .collect::<BTreeSet<_>>();
        let attached_local_ref_targets = attached_local_ref_targets(
            &nodes,
            &annotations,
            &children,
            entrypoint_node,
            stored_target_id,
            configured_target_ref.as_ref(),
            managed_workspace_commit_id,
            &explicit_target_local_ref_names,
        );
        let starts = segment_starts(
            &nodes,
            &annotations,
            &children,
            &required_first_commit_ids,
            &forced_commit_starts,
            entrypoint_node,
            stored_target_id,
            configured_target_ref.as_ref(),
            managed_workspace_commit_id,
            &attached_local_ref_targets,
            &explicit_target_local_ref_names,
        );

        let mut graph = Graph {
            entrypoint_ref,
            traversal_tips,
            ad_hoc_branch_stack_orders,
            hard_limit_hit,
            options,
            project_meta,
            symbolic_remote_names,
            ..Graph::default()
        };
        let mut node_to_segment = vec![None; nodes.len()];

        let mut ordered_starts = starts
            .iter()
            .enumerate()
            .filter_map(|(index, starts)| starts.then_some(index))
            .collect::<Vec<_>>();
        ordered_starts.sort_by_key(|&index| {
            let priority = match &nodes[index].kind {
                NodeKind::Reference(reference)
                    if matches!(
                        reference.metadata,
                        Some(crate::SegmentMetadata::Workspace(_))
                    ) =>
                {
                    0
                }
                NodeKind::Commit { id }
                    if graph.traversal_tips.iter().any(|tip| {
                        tip.id == *id
                            && matches!(tip.role, crate::init::TipRole::TargetRemote)
                            && tip.ref_name.is_none()
                    }) =>
                {
                    1
                }
                NodeKind::Commit { .. }
                | NodeKind::Reference(_)
                | NodeKind::ShallowPoint { .. } => 2,
            };
            (priority, index)
        });

        for start in ordered_starts {
            let (segment, members) = segment_from_start(
                start,
                &nodes,
                &annotations,
                &starts,
                &children,
                entrypoint_node,
                stored_target_id,
                configured_target_ref.as_ref(),
                managed_workspace_commit_id,
                &attached_local_ref_targets,
                &explicit_target_local_ref_names,
            )?;
            let segment_index = graph.insert_segment(segment);
            for member in members {
                ensure!(
                    node_to_segment[member].replace(segment_index).is_none(),
                    "BUG: node {member} belongs to more than one segment"
                );
            }
        }

        for (&reference, &target) in &attached_local_ref_targets {
            let target_segment = node_to_segment[target].with_context(|| {
                format!("BUG: attached ref target node {target} has no segment")
            })?;
            ensure!(
                node_to_segment[reference].replace(target_segment).is_none(),
                "BUG: attached ref node {reference} unexpectedly owns a segment"
            );
        }

        for (index, node) in nodes.iter().enumerate() {
            if !matches!(node.kind, NodeKind::ShallowPoint { .. }) {
                ensure!(
                    node_to_segment[index].is_some(),
                    "BUG: node {index} was not assigned to a segment"
                );
            }
        }

        attach_non_owning_local_refs(
            &mut graph,
            &nodes,
            &node_to_segment,
            &attached_local_ref_targets,
        )?;
        connect_segments(&mut graph, &nodes, &node_to_segment)?;
        link_remote_tracking_segments(&mut graph, &nodes, &node_to_segment)?;

        match entrypoint {
            NodeGraphEntrypoint::Node(index) => {
                let segment_index = node_to_segment[index]
                    .with_context(|| format!("BUG: entrypoint node {index} has no segment"))?;
                match &nodes[index].kind {
                    NodeKind::Commit { id } => {
                        graph.entrypoint = Some((segment_index, EntryPointCommit::AtCommit(*id)));
                    }
                    NodeKind::Reference(reference) => {
                        let id = reference.ref_info.commit_id.with_context(|| {
                            format!("BUG: entrypoint reference node {index} has no target")
                        })?;
                        graph.entrypoint = Some((segment_index, EntryPointCommit::AtCommit(id)));
                    }
                    NodeKind::ShallowPoint { .. } => {
                        bail!("BUG: shallow-point node {index} cannot be the entrypoint")
                    }
                }
            }
            NodeGraphEntrypoint::Unborn(reference) => {
                let reference = *reference;
                let segment_index = graph.insert_segment(Segment {
                    ref_info: Some(reference.ref_info),
                    metadata: reference.metadata,
                    remote_tracking_ref_name: reference.remote_tracking_ref_name,
                    ..Segment::default()
                });
                graph.entrypoint = Some((segment_index, EntryPointCommit::Unborn));
            }
        }

        graph.compute_generation_numbers();
        if let Some(workspace) = graph.workspace_reconciliation_input()? {
            graph.link_anonymous_workspace_siblings(
                workspace.id,
                &workspace.stacks,
                &workspace.metadata,
            );
        }
        canonicalize_reference_storage(&mut graph)?;
        let mut graph = graph.validated()?;
        graph.construction_graph = Some(construction_graph);
        Ok(graph)
    }
}

fn children_by_node(nodes: &[Node]) -> Vec<Vec<NodeIndex>> {
    let mut children = vec![Vec::new(); nodes.len()];
    for (child, node) in nodes.iter().enumerate() {
        for &parent in &node.parents {
            children[parent].push(child);
        }
    }
    children
}

#[allow(
    clippy::too_many_arguments,
    reason = "inline reference policy needs the complete construction context"
)]
fn attached_local_ref_targets(
    nodes: &[Node],
    annotations: &[CommitFlags],
    children: &[Vec<NodeIndex>],
    entrypoint: Option<NodeIndex>,
    stored_target_id: Option<gix::ObjectId>,
    configured_target_ref: Option<&gix::refs::FullName>,
    managed_workspace_commit_id: Option<gix::ObjectId>,
    explicit_target_local_ref_names: &BTreeSet<gix::refs::FullName>,
) -> BTreeMap<NodeIndex, NodeIndex> {
    let has_workspace_reference = nodes.iter().any(|node| {
        matches!(
            node.kind,
            NodeKind::Reference(ref reference)
                if matches!(
                    reference.metadata,
                    Some(crate::SegmentMetadata::Workspace(_))
                )
        )
    });
    nodes
        .iter()
        .enumerate()
        .filter_map(|(reference_index, node)| {
            let NodeKind::Reference(reference) = &node.kind else {
                return None;
            };
            let [target] = node.parents.as_slice() else {
                return None;
            };
            let NodeKind::Commit { id: target_id } = nodes[*target].kind else {
                return None;
            };
            if Some(target_id) != reference.ref_info.commit_id {
                return None;
            }
            let metadata_alias_target = Some(target_id) != managed_workspace_commit_id
                && !annotations[*target].contains(CommitFlags::Integrated);
            let shared_branch_ancestor = has_workspace_reference
                && metadata_alias_target
                && matches!(reference.metadata, Some(crate::SegmentMetadata::Branch(_)))
                && !children[reference_index].iter().any(|child| {
                    matches!(
                        nodes[*child].kind,
                        NodeKind::Reference(ref reference)
                            if matches!(
                                reference.metadata,
                                Some(crate::SegmentMetadata::Workspace(_))
                            )
                    )
                })
                && nearest_metadata_branch_descendants(nodes, children, reference_index) >= 2;
            let attach_unplaced_branch = has_workspace_reference
                && metadata_alias_target
                && matches!(reference.metadata, Some(crate::SegmentMetadata::Branch(_)))
                && children[reference_index].is_empty()
                && direct_reference_children(nodes, children, *target)
                    .into_iter()
                    .any(|child| child != reference_index);
            if (reference.metadata.is_some() && !attach_unplaced_branch && !shared_branch_ancestor)
                || Some(reference_index) == entrypoint
                || reference.ref_info.ref_name.category() != Some(Category::LocalBranch)
                || explicit_target_local_ref_names.contains(&reference.ref_info.ref_name)
                || configured_target_ref.is_some_and(|target_ref| {
                    reference.remote_tracking_ref_name.as_ref() == Some(target_ref)
                })
            {
                return None;
            }
            if attach_unplaced_branch || shared_branch_ancestor {
                return Some((reference_index, *target));
            }
            let owners = owning_reference_children(
                nodes,
                children,
                *target,
                entrypoint,
                stored_target_id,
                configured_target_ref,
                managed_workspace_commit_id,
                explicit_target_local_ref_names,
            );
            (owners.as_slice() != [reference_index]).then_some((reference_index, *target))
        })
        .collect()
}

/// Count the nearest metadata-branch descendants above `start`, treating each encountered branch
/// as the end of one structural path.
fn nearest_metadata_branch_descendants(
    nodes: &[Node],
    children: &[Vec<NodeIndex>],
    start: NodeIndex,
) -> usize {
    let mut pending = children[start].clone();
    let mut seen = BTreeSet::new();
    let mut branches = 0;
    while let Some(index) = pending.pop() {
        if !seen.insert(index) {
            continue;
        }
        if matches!(
            nodes[index].kind,
            NodeKind::Reference(ref reference)
                if matches!(reference.metadata, Some(crate::SegmentMetadata::Branch(_)))
        ) {
            branches += 1;
            continue;
        }
        pending.extend(children[index].iter().copied());
    }
    branches
}

#[allow(
    clippy::too_many_arguments,
    reason = "segment ownership policy needs the complete construction context"
)]
fn segment_starts(
    nodes: &[Node],
    annotations: &[CommitFlags],
    children: &[Vec<NodeIndex>],
    required_first_commit_ids: &BTreeSet<gix::ObjectId>,
    forced_commit_starts: &BTreeSet<gix::ObjectId>,
    entrypoint: Option<NodeIndex>,
    stored_target_id: Option<gix::ObjectId>,
    configured_target_ref: Option<&gix::refs::FullName>,
    managed_workspace_commit_id: Option<gix::ObjectId>,
    attached_local_ref_targets: &BTreeMap<NodeIndex, NodeIndex>,
    explicit_target_local_ref_names: &BTreeSet<gix::refs::FullName>,
) -> Vec<bool> {
    nodes
        .iter()
        .enumerate()
        .map(|(index, node)| match node.kind {
            NodeKind::Reference(_) => !attached_local_ref_targets.contains_key(&index),
            NodeKind::ShallowPoint { .. } => false,
            NodeKind::Commit { id } => {
                if forced_commit_starts.contains(&id) {
                    return true;
                }
                if Some(id) == stored_target_id {
                    return true;
                }
                if required_first_commit_ids.contains(&id)
                    && annotations[index].contains(CommitFlags::Integrated)
                    && children[index].iter().any(|reference_index| {
                        matches!(
                            &nodes[*reference_index].kind,
                            NodeKind::Reference(reference)
                                if matches!(
                                    reference.metadata,
                                    Some(crate::SegmentMetadata::Branch(_))
                                )
                        ) && children[*reference_index].iter().any(|workspace_index| {
                            matches!(
                                &nodes[*workspace_index].kind,
                                NodeKind::Reference(reference)
                                    if matches!(
                                        reference.metadata,
                                        Some(crate::SegmentMetadata::Workspace(_))
                                    )
                                        && nodes[*workspace_index]
                                            .parents
                                            .split_last()
                                            .is_some_and(|(_, overlay_parents)| {
                                                overlay_parents.contains(reference_index)
                                            })
                            )
                        })
                    })
                {
                    return true;
                }

                let owning_references = owning_reference_children(
                    nodes,
                    children,
                    index,
                    entrypoint,
                    stored_target_id,
                    configured_target_ref,
                    managed_workspace_commit_id,
                    explicit_target_local_ref_names,
                )
                .into_iter()
                .filter(|reference| !attached_local_ref_targets.contains_key(reference))
                .collect::<Vec<_>>();
                if !owning_references.is_empty() {
                    return owning_references.len() > 1;
                }

                let structural_children = children[index]
                    .iter()
                    .copied()
                    .filter(|child| !attached_local_ref_targets.contains_key(child))
                    .collect::<Vec<_>>();
                let [child] = structural_children.as_slice() else {
                    return true;
                };
                if nodes[*child].parents.len() != 1 {
                    return true;
                }
                if matches!(
                    nodes[*child].kind,
                    NodeKind::Commit { id } if Some(id) == managed_workspace_commit_id
                ) {
                    return true;
                }
                required_first_commit_ids.contains(&id)
            }
        })
        .collect()
}

fn direct_reference_children(
    nodes: &[Node],
    children: &[Vec<NodeIndex>],
    index: NodeIndex,
) -> Vec<NodeIndex> {
    children[index]
        .iter()
        .copied()
        .filter(|child| {
            let NodeKind::Reference(reference) = &nodes[*child].kind else {
                return false;
            };
            if matches!(
                reference.metadata,
                Some(crate::SegmentMetadata::Workspace(_))
            ) {
                nodes[*child].parents.last() == Some(&index)
            } else {
                nodes[*child].parents.as_slice() == [index]
            }
        })
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    reason = "segment ownership policy needs the complete construction context"
)]
fn owning_reference_children(
    nodes: &[Node],
    children: &[Vec<NodeIndex>],
    index: NodeIndex,
    entrypoint: Option<NodeIndex>,
    stored_target_id: Option<gix::ObjectId>,
    configured_target_ref: Option<&gix::refs::FullName>,
    managed_workspace_commit_id: Option<gix::ObjectId>,
    explicit_target_local_ref_names: &BTreeSet<gix::refs::FullName>,
) -> Vec<NodeIndex> {
    let direct = direct_reference_children(nodes, children, index);
    let target_id = node_target_id(&nodes[index]);
    let workspaces = direct
        .iter()
        .copied()
        .filter(|child| {
            matches!(
                &nodes[*child].kind,
                NodeKind::Reference(reference)
                    if matches!(reference.metadata, Some(crate::SegmentMetadata::Workspace(_)))
            )
        })
        .collect::<Vec<_>>();
    if target_id == managed_workspace_commit_id && !workspaces.is_empty() {
        return workspaces;
    }
    if let Some(entrypoint) = entrypoint
        .filter(|entrypoint| direct.contains(entrypoint) && !workspaces.contains(entrypoint))
    {
        return vec![entrypoint];
    }
    let target_locals = direct
        .iter()
        .copied()
        .filter(|child| {
            matches!(
                &nodes[*child].kind,
                NodeKind::Reference(reference)
                    if explicit_target_local_ref_names.contains(&reference.ref_info.ref_name)
                        || configured_target_ref.is_some_and(|target_ref| {
                            reference.remote_tracking_ref_name.as_ref() == Some(target_ref)
                        })
            )
        })
        .collect::<Vec<_>>();
    let commit_child_count = children[index]
        .iter()
        .filter(|child| matches!(nodes[**child].kind, NodeKind::Commit { .. }))
        .count();
    if target_locals.len() == 1 && (target_id == stored_target_id || commit_child_count > 0) {
        return target_locals;
    }
    let same_tip_tracking_locals = direct
        .iter()
        .copied()
        .filter(|child| {
            let NodeKind::Reference(reference) = &nodes[*child].kind else {
                return false;
            };
            reference
                .remote_tracking_ref_name
                .as_ref()
                .is_some_and(|remote_name| {
                    nodes.iter().any(|node| {
                        matches!(
                            &node.kind,
                            NodeKind::Reference(remote)
                                if &remote.ref_info.ref_name == remote_name
                                    && remote.ref_info.commit_id == target_id
                        )
                    })
                })
        })
        .collect::<Vec<_>>();
    if same_tip_tracking_locals.len() == 1 {
        return same_tip_tracking_locals;
    }
    let selected_locals = direct
        .iter()
        .copied()
        .filter(|child| {
            matches!(
                &nodes[*child].kind,
                NodeKind::Reference(reference)
                    if matches!(reference.metadata, Some(crate::SegmentMetadata::Branch(_)))
            )
        })
        .collect::<Vec<_>>();
    if selected_locals.len() == 1 {
        return selected_locals;
    }

    let tracked_locals = direct
        .iter()
        .copied()
        .filter(|child| {
            matches!(
                &nodes[*child].kind,
                NodeKind::Reference(reference)
                    if reference.ref_info.ref_name.category() == Some(Category::LocalBranch)
                        && reference.remote_tracking_ref_name.is_some()
            )
        })
        .collect::<Vec<_>>();
    if tracked_locals.len() == 1 {
        return tracked_locals;
    }

    let locals = direct
        .iter()
        .copied()
        .filter(|child| {
            matches!(
                &nodes[*child].kind,
                NodeKind::Reference(reference)
                    if !matches!(reference.metadata, Some(crate::SegmentMetadata::Workspace(_)))
                        && reference.ref_info.ref_name.category() == Some(Category::LocalBranch)
            )
        })
        .collect::<Vec<_>>();
    if locals.len() == 1 {
        return locals;
    }
    if locals.len() > 1 {
        if children[index].iter().any(|child| {
            matches!(
                nodes[*child].kind,
                NodeKind::Commit { id } if Some(id) == managed_workspace_commit_id
            )
        }) {
            return Vec::new();
        }
        return locals;
    }

    direct
        .into_iter()
        .filter(|child| !workspaces.contains(child))
        .collect()
}

#[allow(
    clippy::too_many_arguments,
    reason = "segment ownership policy needs the complete construction context"
)]
fn segment_from_start(
    start: NodeIndex,
    nodes: &[Node],
    annotations: &[CommitFlags],
    starts: &[bool],
    children: &[Vec<NodeIndex>],
    entrypoint: Option<NodeIndex>,
    stored_target_id: Option<gix::ObjectId>,
    configured_target_ref: Option<&gix::refs::FullName>,
    managed_workspace_commit_id: Option<gix::ObjectId>,
    attached_local_ref_targets: &BTreeMap<NodeIndex, NodeIndex>,
    explicit_target_local_ref_names: &BTreeSet<gix::refs::FullName>,
) -> Result<(Segment, Vec<NodeIndex>)> {
    let mut segment = Segment::default();
    let mut members = Vec::new();
    let mut current = start;

    loop {
        members.push(current);
        match &nodes[current].kind {
            NodeKind::Commit { id } => {
                let mut flags = annotations[current];
                if nodes[current].parents.iter().any(|parent| {
                    matches!(
                        nodes[*parent].kind,
                        NodeKind::ShallowPoint { reason, .. }
                            if reason.contains(StopCondition::ShallowBoundary)
                    )
                }) {
                    flags |= CommitFlags::ShallowBoundary;
                }
                segment.commits.push(Commit {
                    id: *id,
                    parent_ids: nodes[current]
                        .parents
                        .iter()
                        .map(|parent| {
                            node_target_id(&nodes[*parent]).with_context(|| {
                                format!("BUG: parent node {parent} has no target object")
                            })
                        })
                        .collect::<Result<Vec<_>>>()?,
                    flags,
                    refs: Vec::new(),
                });
            }
            NodeKind::Reference(reference) => {
                ensure!(
                    current == start,
                    "BUG: reference node {current} did not start its segment"
                );
                segment.ref_info = Some(reference.ref_info.clone());
                segment.metadata = reference.metadata.clone();
                segment.remote_tracking_ref_name = reference.remote_tracking_ref_name.clone();
            }
            NodeKind::ShallowPoint { .. } => {
                bail!("BUG: shallow-point node {current} cannot start a segment")
            }
        }

        let Some(parent) = reference_continuation_parent(&nodes[current]) else {
            break;
        };
        if matches!(nodes[parent].kind, NodeKind::ShallowPoint { .. })
            || matches!(nodes[parent].kind, NodeKind::Reference(_))
            || starts[parent]
            || owning_reference_children(
                nodes,
                children,
                parent,
                entrypoint,
                stored_target_id,
                configured_target_ref,
                managed_workspace_commit_id,
                explicit_target_local_ref_names,
            )
            .into_iter()
            .find(|reference| !attached_local_ref_targets.contains_key(reference))
            .is_some_and(|owner| owner != current)
        {
            break;
        }
        current = parent;
    }

    Ok((segment, members))
}

fn reference_continuation_parent(node: &Node) -> Option<NodeIndex> {
    match &node.kind {
        NodeKind::Reference(reference)
            if matches!(
                reference.metadata,
                Some(crate::SegmentMetadata::Workspace(_))
            ) =>
        {
            node.parents.last().copied()
        }
        NodeKind::Commit { .. } | NodeKind::Reference(_) => {
            let [parent] = node.parents.as_slice() else {
                return None;
            };
            Some(*parent)
        }
        NodeKind::ShallowPoint { .. } => None,
    }
}

fn node_target_id(node: &Node) -> Option<gix::ObjectId> {
    match &node.kind {
        NodeKind::Commit { id } | NodeKind::ShallowPoint { id, .. } => Some(*id),
        NodeKind::Reference(reference) => reference.ref_info.commit_id,
    }
}

fn attach_non_owning_local_refs(
    graph: &mut Graph,
    nodes: &[Node],
    node_to_segment: &[Option<SegmentIndex>],
    attached_local_ref_targets: &BTreeMap<NodeIndex, NodeIndex>,
) -> Result<()> {
    for (&reference_index, &commit_index) in attached_local_ref_targets {
        let NodeKind::Reference(reference) = &nodes[reference_index].kind else {
            bail!("BUG: attached ref node {reference_index} is not a reference")
        };
        let target_id = reference
            .ref_info
            .commit_id
            .context("BUG: attached local ref has no target")?;
        let reference_segment = node_to_segment[reference_index]
            .with_context(|| format!("BUG: reference node {reference_index} has no segment"))?;
        let commit_segment = node_to_segment[commit_index]
            .with_context(|| format!("BUG: commit node {commit_index} has no segment"))?;
        ensure!(
            reference_segment == commit_segment,
            "BUG: attached local ref node {reference_index} is not mapped to its target segment"
        );
        let commit = graph[commit_segment]
            .commits
            .iter_mut()
            .find(|commit| commit.id == target_id)
            .with_context(|| {
                format!("BUG: target commit {target_id} is absent from its assigned segment")
            })?;
        if !commit
            .refs
            .iter()
            .any(|candidate| candidate.ref_name == reference.ref_info.ref_name)
        {
            commit.refs.push(reference.ref_info.clone());
        }
    }
    Ok(())
}

/// Keep each discovered reference in exactly one legacy representation.
///
/// A reference can begin a traversal as an ambiguous commit alias and become a named segment
/// after metadata changes place it in the workspace. The segment representation is authoritative
/// in that case, so remove the stale inline copy before projection inspects the graph.
fn canonicalize_reference_storage(graph: &mut Graph) -> Result<()> {
    let segment_ref_names = graph
        .inner
        .node_weights()
        .filter_map(|segment| segment.ref_info.as_ref().map(|info| info.ref_name.clone()))
        .collect::<BTreeSet<_>>();

    for segment in graph.inner.node_weights_mut() {
        for commit in &mut segment.commits {
            commit
                .refs
                .retain(|info| !segment_ref_names.contains(&info.ref_name));
        }
    }

    let mut seen = BTreeSet::new();
    for segment in graph.inner.node_weights() {
        if let Some(info) = &segment.ref_info {
            ensure!(
                seen.insert(info.ref_name.clone()),
                "BUG: reference {} occurs in more than one segment",
                info.ref_name
            );
        }
        for info in segment.commits.iter().flat_map(|commit| &commit.refs) {
            ensure!(
                seen.insert(info.ref_name.clone()),
                "BUG: reference {} occurs more than once in commit aliases",
                info.ref_name
            );
        }
    }
    Ok(())
}

fn connect_segments(
    graph: &mut Graph,
    nodes: &[Node],
    node_to_segment: &[Option<SegmentIndex>],
) -> Result<()> {
    let mut workspace_overlay_parents = BTreeMap::<NodeIndex, Vec<(gix::ObjectId, usize)>>::new();
    let mut workspace_reference_parent_orders = BTreeMap::<NodeIndex, Vec<usize>>::new();
    for (reference_index, node) in nodes.iter().enumerate() {
        let NodeKind::Reference(reference) = &node.kind else {
            continue;
        };
        if !matches!(
            reference.metadata,
            Some(crate::SegmentMetadata::Workspace(_))
        ) {
            continue;
        }
        let Some((own_target, overlay_parents)) = node.parents.split_last() else {
            continue;
        };
        if overlay_parents.is_empty() {
            continue;
        }
        let mut claimed_parent_slots = BTreeSet::new();
        let mut next_logical_order = nodes[*own_target].parents.len();
        let overlay_parents = overlay_parents
            .iter()
            .filter_map(|parent| {
                let id = node_target_id(&nodes[*parent])?;
                let parent_order = nodes[*own_target]
                    .parents
                    .iter()
                    .enumerate()
                    .find_map(|(order, candidate)| {
                        (!claimed_parent_slots.contains(&order)
                            && node_target_id(&nodes[*candidate]) == Some(id))
                        .then_some(order)
                    })
                    .unwrap_or_else(|| {
                        let order = next_logical_order;
                        next_logical_order += 1;
                        order
                    });
                claimed_parent_slots.insert(parent_order);
                Some((id, parent_order))
            })
            .collect::<Vec<_>>();
        workspace_reference_parent_orders.insert(
            reference_index,
            overlay_parents.iter().map(|(_, order)| *order).collect(),
        );
        workspace_overlay_parents
            .entry(*own_target)
            .or_default()
            .extend(overlay_parents);
    }
    for (source, node) in nodes.iter().enumerate() {
        if matches!(node.kind, NodeKind::ShallowPoint { .. }) {
            continue;
        }
        let source_segment = node_to_segment[source]
            .with_context(|| format!("BUG: source node {source} has no segment"))?;
        let mut covered_parent_ids = workspace_overlay_parents
            .get(&source)
            .map(|parents| parents.iter().map(|(id, _)| *id).collect::<Vec<_>>())
            .unwrap_or_default();
        let is_workspace_reference = matches!(
            &node.kind,
            NodeKind::Reference(reference)
                if matches!(reference.metadata, Some(crate::SegmentMetadata::Workspace(_)))
        );
        for (parent_order, parent) in node.parents.iter().copied().enumerate() {
            if is_workspace_reference
                && parent_order + 1 == node.parents.len()
                && let Some(own_target_id) = node_target_id(&nodes[parent])
                && node.parents[..parent_order].iter().any(|overlay_parent| {
                    node_target_id(&nodes[*overlay_parent]) == Some(own_target_id)
                })
            {
                continue;
            }
            if !covered_parent_ids.is_empty()
                && !matches!(nodes[parent].kind, NodeKind::ShallowPoint { .. })
                && let Some(parent_id) = node_target_id(&nodes[parent])
                && let Some(position) = covered_parent_ids.iter().position(|id| *id == parent_id)
            {
                covered_parent_ids.remove(position);
                continue;
            }
            if matches!(nodes[parent].kind, NodeKind::ShallowPoint { .. }) {
                continue;
            }
            let destination_segment = node_to_segment[parent]
                .with_context(|| format!("BUG: parent node {parent} has no segment"))?;
            if source_segment == destination_segment {
                continue;
            }
            let parent_order = if is_workspace_reference {
                workspace_reference_parent_orders
                    .get(&source)
                    .and_then(|orders| orders.get(parent_order))
                    .copied()
                    .unwrap_or(parent_order)
            } else {
                parent_order
            };
            let parent_order = u32::try_from(parent_order)
                .context("BUG: a node has more parent slots than Edge can represent")?;
            let source_commit = graph[source_segment].last_commit_index();
            let destination_commit = graph[destination_segment].commits.first().map(|_| 0);
            graph.connect_segments_with_ids(
                source_segment,
                source_commit,
                None,
                destination_segment,
                destination_commit,
                None,
                parent_order,
            );
        }
    }
    Ok(())
}

fn link_remote_tracking_segments(
    graph: &mut Graph,
    nodes: &[Node],
    node_to_segment: &[Option<SegmentIndex>],
) -> Result<()> {
    let references = nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match &node.kind {
            NodeKind::Reference(reference) => Some((
                index,
                reference.ref_info.ref_name.clone(),
                reference.remote_tracking_ref_name.clone(),
            )),
            NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
        })
        .collect::<Vec<_>>();
    let mut segments_by_ref_name = BTreeMap::new();
    for (index, ref_name, _) in &references {
        let segment = node_to_segment[*index]
            .with_context(|| format!("BUG: reference node {index} has no segment"))?;
        ensure!(
            segments_by_ref_name
                .insert(ref_name.clone(), segment)
                .is_none(),
            "BUG: reference {ref_name} appears in more than one node"
        );
    }

    let mut claimed_remote_segments = BTreeSet::new();
    for (index, _, remote_ref_name) in references {
        let Some(remote_ref_name) = remote_ref_name else {
            continue;
        };
        let local_segment = node_to_segment[index]
            .with_context(|| format!("BUG: reference node {index} has no segment"))?;
        let Some(&remote_segment) = segments_by_ref_name.get(&remote_ref_name) else {
            continue;
        };
        if !claimed_remote_segments.insert(remote_segment) {
            continue;
        }
        graph[local_segment].remote_tracking_branch_segment_id = Some(remote_segment);
        graph[remote_segment].sibling_segment_id = Some(local_segment);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use but_core::ref_metadata::ProjectMeta;
    use petgraph::{Direction, visit::EdgeRef};

    use super::*;
    use crate::{RefInfo, Reference, SegmentMetadata, Worktree, WorktreeKind, init};

    fn oid(value: u8) -> gix::ObjectId {
        let hex = format!("{value:040x}");
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid test object id")
    }

    fn commit(id: gix::ObjectId, parents: Vec<NodeIndex>) -> Node {
        Node {
            kind: NodeKind::Commit { id },
            parents,
        }
    }

    fn reference(name: &str, id: gix::ObjectId, parents: Vec<NodeIndex>) -> Node {
        Node {
            kind: NodeKind::Reference(Box::new(Reference {
                ref_info: RefInfo {
                    ref_name: name.try_into().expect("valid full ref name"),
                    commit_id: Some(id),
                    worktree: None,
                },
                metadata: None,
                remote_tracking_ref_name: None,
            })),
            parents,
        }
    }

    fn graph(
        nodes: Vec<Node>,
        entrypoint: NodeGraphEntrypoint,
        traversal_tips: Vec<init::Tip>,
    ) -> NodeGraph {
        let entrypoint_ref = match &entrypoint {
            NodeGraphEntrypoint::Node(index) => match &nodes[*index].kind {
                NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.clone()),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            },
            NodeGraphEntrypoint::Unborn(reference) => Some(reference.ref_info.ref_name.clone()),
        };
        NodeGraph {
            annotations: vec![CommitFlags::default(); nodes.len()],
            nodes,
            context: ConstructionContext {
                entrypoint,
                entrypoint_ref,
                managed_workspace_commit_id: None,
                traversal_tips,
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: ProjectMeta::default(),
                symbolic_remote_names: Vec::new(),
            },
        }
    }

    #[test]
    fn named_tip_owns_its_commit_and_target_starts_the_next_segment() -> Result<()> {
        let root = oid(1);
        let below_target = oid(2);
        let target = oid(3);
        let tip = oid(4);
        let main = "refs/heads/main";
        let mut node_graph = graph(
            vec![
                commit(root, vec![]),
                commit(below_target, vec![0]),
                commit(target, vec![1]),
                commit(tip, vec![2]),
                reference(main, tip, vec![3]),
            ],
            NodeGraphEntrypoint::Node(4),
            vec![init::Tip::entrypoint(
                tip,
                Some(main.try_into().expect("valid full ref name")),
            )],
        );
        node_graph.context.options.collect_tags = true;
        node_graph.context.project_meta.target_commit_id = Some(target);
        node_graph.context.symbolic_remote_names = vec!["origin".into()];
        node_graph.context.ad_hoc_branch_stack_orders =
            vec![vec![main.try_into().expect("valid full ref name")]];

        let graph = node_graph.into_segment_graph()?;

        assert_eq!(graph.num_segments(), 2, "the target splits linear history");
        let main_segment = graph
            .segment_by_ref_name(main.try_into().expect("valid full ref name"))
            .expect("main segment");
        assert_eq!(
            main_segment
                .commits
                .iter()
                .map(|commit| commit.id)
                .collect::<Vec<_>>(),
            [tip],
            "the named segment owns its tip"
        );
        assert_eq!(main_segment.generation, 0, "the tip is a root segment");
        let target_segment = graph.segment_by_commit_id(target)?;
        assert_eq!(
            target_segment
                .commits
                .iter()
                .map(|commit| commit.id)
                .collect::<Vec<_>>(),
            [target, below_target, root],
            "the target is first in the lower segment"
        );
        assert_eq!(target_segment.generation, 1, "the target is below the tip");
        assert!(
            graph.options.collect_tags,
            "construction options are retained"
        );
        assert_eq!(
            graph.project_meta.target_commit_id,
            Some(target),
            "project target context is retained"
        );
        assert_eq!(graph.symbolic_remote_names, ["origin"]);
        assert_eq!(graph.ad_hoc_branch_stack_orders.len(), 1);
        assert_eq!(
            graph.entrypoint_ref.as_ref().map(ToString::to_string),
            Some(main.to_owned()),
            "the authoritative entrypoint ref is retained"
        );
        Ok(())
    }

    #[test]
    fn authoritative_entrypoint_owns_before_metadata_and_tracking_refs() -> Result<()> {
        let id = oid(1);
        let entrypoint_name = "refs/heads/checked-out";
        let metadata_name = "refs/heads/metadata";
        let tracking_name = "refs/heads/main";
        let remote_name = "refs/remotes/origin/main";
        let mut metadata = reference(metadata_name, id, vec![0]);
        let NodeKind::Reference(metadata_reference) = &mut metadata.kind else {
            unreachable!("constructed a reference")
        };
        metadata_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut tracking = reference(tracking_name, id, vec![0]);
        let NodeKind::Reference(tracking_reference) = &mut tracking.kind else {
            unreachable!("constructed a reference")
        };
        tracking_reference.remote_tracking_ref_name = Some(
            remote_name
                .try_into()
                .expect("valid remote-tracking ref name"),
        );
        let mut node_graph = graph(
            vec![
                commit(id, vec![]),
                reference(entrypoint_name, id, vec![0]),
                metadata,
                tracking,
                reference(remote_name, id, vec![0]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::entrypoint(
                id,
                Some(entrypoint_name.try_into().expect("valid full ref name")),
            )],
        );
        node_graph.context.project_meta.target_ref =
            Some(remote_name.try_into().expect("valid target ref name"));
        let graph = node_graph.into_segment_graph()?;

        let entrypoint = graph
            .segment_by_ref_name(entrypoint_name.try_into().expect("valid full ref name"))
            .expect("entrypoint segment");
        assert_eq!(
            entrypoint.commits.first().map(|commit| commit.id),
            Some(id),
            "the authoritative entrypoint owns the shared commit"
        );
        for name in [metadata_name, tracking_name] {
            assert!(
                graph
                    .segment_by_ref_name(name.try_into().expect("valid full ref name"))
                    .is_some_and(|segment| segment.commits.is_empty()),
                "non-owning metadata and target-tracking refs remain explicit at {name}"
            );
        }
        Ok(())
    }

    #[test]
    fn unplaced_metadata_branch_can_remain_an_ambiguous_commit_alias() -> Result<()> {
        let id = oid(1);
        let managed_id = oid(2);
        let metadata_name = "refs/heads/E";
        let mut metadata = reference(metadata_name, id, vec![0]);
        let NodeKind::Reference(metadata_reference) = &mut metadata.kind else {
            unreachable!("constructed a reference")
        };
        metadata_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut workspace = reference("refs/heads/gitbutler/workspace", managed_id, vec![3]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(id, vec![]),
                reference("refs/heads/D", id, vec![0]),
                metadata,
                commit(managed_id, vec![0]),
                workspace,
            ],
            NodeGraphEntrypoint::Node(4),
            vec![init::Tip::entrypoint(
                managed_id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        );
        node_graph.context.managed_workspace_commit_id = Some(managed_id);
        let graph = node_graph.into_segment_graph()?;

        let segment = graph.segment_by_commit_id(id)?;
        assert!(
            segment.ref_info.is_none(),
            "ambiguous unplaced refs do not become an arbitrary segment owner"
        );
        assert!(
            segment.commits[0]
                .refs
                .iter()
                .any(|reference| reference.ref_name.to_string() == metadata_name),
            "the unplaced metadata branch remains discoverable as a commit alias"
        );
        Ok(())
    }

    #[test]
    fn ordinary_commit_child_does_not_make_a_leaf_metadata_branch_an_alias() -> Result<()> {
        let base_id = oid(1);
        let child_id = oid(2);
        let managed_id = oid(3);
        let branch_name = "refs/heads/E";
        let mut branch = reference(branch_name, base_id, vec![0]);
        let NodeKind::Reference(branch_reference) = &mut branch.kind else {
            unreachable!("constructed a reference")
        };
        branch_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut workspace = reference("refs/heads/gitbutler/workspace", managed_id, vec![3]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(base_id, vec![]),
                branch,
                commit(child_id, vec![0]),
                commit(managed_id, vec![2]),
                workspace,
            ],
            NodeGraphEntrypoint::Node(4),
            vec![init::Tip::entrypoint(
                managed_id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        );
        node_graph.context.managed_workspace_commit_id = Some(managed_id);

        let graph = node_graph.into_segment_graph()?;

        let branch = graph
            .segment_by_ref_name(branch_name.try_into().expect("valid full ref name"))
            .expect("leaf metadata branch keeps a segment");
        assert_eq!(
            branch.commits.first().map(|commit| commit.id),
            Some(base_id),
            "an ordinary commit child is not a competing same-tip reference owner"
        );
        Ok(())
    }

    #[test]
    fn shared_metadata_branch_ancestor_is_one_commit_alias() -> Result<()> {
        let base = oid(1);
        let tip = oid(2);
        let mut e = reference("refs/heads/E", base, vec![0]);
        let mut b = reference("refs/heads/B", tip, vec![2]);
        let mut c = reference("refs/heads/C", tip, vec![2]);
        for node in [&mut e, &mut b, &mut c] {
            let NodeKind::Reference(reference) = &mut node.kind else {
                unreachable!("constructed a reference")
            };
            reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        }
        let mut workspace = reference("refs/heads/gitbutler/workspace", tip, vec![3, 4, 2]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(base, vec![]),
                e,
                commit(tip, vec![1]),
                b,
                c,
                workspace,
            ],
            NodeGraphEntrypoint::Node(5),
            vec![init::Tip::entrypoint(
                tip,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        );
        node_graph.context.managed_workspace_commit_id = Some(tip);
        let graph = node_graph.into_segment_graph()?;

        assert!(
            graph
                .segment_by_ref_name("refs/heads/E".try_into().expect("valid full ref name"))
                .is_none(),
            "a shared branch ancestor does not own a segment used by both paths"
        );
        assert_eq!(
            graph
                .segments()
                .flat_map(|segment| &graph[segment].commits)
                .flat_map(|commit| &commit.refs)
                .filter(|reference| reference.ref_name.to_string() == "refs/heads/E")
                .count(),
            1,
            "the shared ancestor remains available as one commit alias"
        );
        Ok(())
    }

    #[test]
    fn promoted_commit_alias_is_removed_when_the_ref_owns_a_segment() -> Result<()> {
        let id = oid(1);
        let ref_info = RefInfo {
            ref_name: "refs/heads/E".try_into().expect("valid full ref name"),
            commit_id: Some(id),
            worktree: None,
        };
        let mut graph = Graph::default();
        graph.insert_segment(Segment {
            ref_info: Some(ref_info.clone()),
            ..Segment::default()
        });
        let alias_segment = graph.insert_segment(Segment {
            commits: vec![Commit {
                id,
                parent_ids: Vec::new(),
                flags: CommitFlags::default(),
                refs: vec![ref_info],
            }],
            ..Segment::default()
        });

        canonicalize_reference_storage(&mut graph)?;

        assert!(
            graph[alias_segment].commits[0].refs.is_empty(),
            "the promoted segment is the only representation of E"
        );
        Ok(())
    }

    #[test]
    fn detached_entrypoint_stays_anonymous_when_a_ref_points_to_it() -> Result<()> {
        let root = oid(1);
        let tip = oid(2);
        let graph = graph(
            vec![
                commit(root, vec![]),
                commit(tip, vec![0]),
                reference("refs/heads/other", tip, vec![1]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::detached_entrypoint(tip)],
        )
        .into_segment_graph()?;

        let entrypoint = graph.entrypoint()?;
        assert!(
            entrypoint.segment.ref_info.is_none(),
            "a detached entrypoint must remain anonymous"
        );
        assert_eq!(
            entrypoint.segment.commits.first().map(|commit| commit.id),
            Some(tip),
            "the detached tip starts its segment"
        );
        assert!(
            graph
                .segment_by_ref_name("refs/heads/other".try_into().expect("valid full ref name"))
                .is_some_and(|segment| segment.commits.is_empty()),
            "other refs become virtual instead of naming the detached segment"
        );
        Ok(())
    }

    #[test]
    fn unnamed_non_detached_entrypoint_can_take_its_unique_discovered_ref() -> Result<()> {
        let root = oid(1);
        let tip = oid(2);
        let other = "refs/heads/other";
        let graph = graph(
            vec![
                commit(root, vec![]),
                commit(tip, vec![0]),
                reference(other, tip, vec![1]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::entrypoint(tip, None)],
        )
        .into_segment_graph()?;

        let entrypoint = graph.entrypoint()?;
        assert_eq!(
            entrypoint.segment.ref_name(),
            Some(other.try_into().expect("valid full ref name")),
            "an unnamed non-detached entrypoint accepts its unique discovered ref"
        );
        assert_eq!(
            entrypoint.segment.commits.first().map(|commit| commit.id),
            Some(tip),
            "the discovered ref owns the entrypoint commit"
        );
        Ok(())
    }

    #[test]
    fn fork_and_duplicate_merge_parents_keep_segment_and_parent_order() -> Result<()> {
        let root = oid(1);
        let left = oid(2);
        let right = oid(3);
        let fork = graph(
            vec![
                commit(root, vec![]),
                commit(left, vec![0]),
                commit(right, vec![0]),
                reference("refs/heads/left", left, vec![1]),
                reference("refs/heads/right", right, vec![2]),
            ],
            NodeGraphEntrypoint::Node(3),
            vec![
                init::Tip::entrypoint(
                    left,
                    Some("refs/heads/left".try_into().expect("valid full ref name")),
                ),
                init::Tip::reachable(
                    right,
                    Some("refs/heads/right".try_into().expect("valid full ref name")),
                ),
            ],
        )
        .into_segment_graph()?;
        assert_eq!(
            fork.num_segments(),
            3,
            "both fork arms and their base are split"
        );
        assert_eq!(
            fork.segment_by_commit_id(root)?
                .commits
                .first()
                .map(|commit| commit.id),
            Some(root),
            "the shared fork base starts its own segment"
        );

        let merge = oid(4);
        let merged = graph(
            vec![
                commit(root, vec![]),
                commit(merge, vec![0, 0]),
                reference("refs/heads/main", merge, vec![1]),
            ],
            NodeGraphEntrypoint::Node(2),
            vec![init::Tip::entrypoint(
                merge,
                Some("refs/heads/main".try_into().expect("valid full ref name")),
            )],
        )
        .into_segment_graph()?;
        let merge_segment = merged.segment_by_commit_id(merge)?;
        assert_eq!(
            merge_segment.commits[0].parent_ids,
            [root, root],
            "duplicate Git parent slots are retained"
        );
        let parent_orders = merged
            .inner
            .edges_directed(merge_segment.id, Direction::Outgoing)
            .map(|edge| edge.weight().parent_order().expect("edge reaches a commit"))
            .collect::<Vec<_>>();
        assert_eq!(parent_orders, [0, 1], "edge order follows Git parent slots");

        let via_empty_ref = graph(
            vec![
                commit(root, vec![]),
                reference("refs/heads/first-parent", root, vec![0]),
                reference("refs/heads/also-at-first-parent", root, vec![0]),
                commit(right, vec![]),
                commit(merge, vec![1, 3]),
                reference("refs/heads/main", merge, vec![4]),
            ],
            NodeGraphEntrypoint::Node(5),
            vec![init::Tip::entrypoint(
                merge,
                Some("refs/heads/main".try_into().expect("valid full ref name")),
            )],
        )
        .into_segment_graph()?;
        let merge_segment = via_empty_ref.segment_by_commit_id(merge)?;
        let parent_orders = via_empty_ref
            .inner
            .edges_directed(merge_segment.id, Direction::Outgoing)
            .map(|edge| {
                edge.weight()
                    .parent_order()
                    .expect("the edge starts at the merge commit")
            })
            .collect::<Vec<_>>();
        assert_eq!(
            parent_orders,
            [0, 1],
            "an empty reference parent does not erase merge parent order"
        );
        Ok(())
    }

    #[test]
    fn unique_reference_owns_a_shared_fork_base() -> Result<()> {
        let base = oid(1);
        let left = oid(2);
        let right = oid(3);
        let base_name = "refs/heads/base";
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(left, vec![0]),
                commit(right, vec![0]),
                reference(base_name, base, vec![0]),
                reference("refs/heads/left", left, vec![1]),
                reference("refs/heads/right", right, vec![2]),
            ],
            NodeGraphEntrypoint::Node(4),
            vec![
                init::Tip::entrypoint(
                    left,
                    Some("refs/heads/left".try_into().expect("valid full ref name")),
                ),
                init::Tip::reachable(
                    right,
                    Some("refs/heads/right".try_into().expect("valid full ref name")),
                ),
            ],
        )
        .into_segment_graph()?;

        let base_segment = graph
            .segment_by_ref_name(base_name.try_into().expect("valid full ref name"))
            .expect("base segment");
        assert_eq!(
            base_segment.commits.first().map(|commit| commit.id),
            Some(base),
            "the unique reference owns the shared fork base"
        );
        Ok(())
    }

    #[test]
    fn ambiguous_references_leave_a_shared_fork_base_anonymous() -> Result<()> {
        let base = oid(1);
        let left = oid(2);
        let right = oid(3);
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(left, vec![0]),
                commit(right, vec![0]),
                reference("refs/heads/base-a", base, vec![0]),
                reference("refs/heads/base-b", base, vec![0]),
                reference("refs/heads/left", left, vec![1]),
                reference("refs/heads/right", right, vec![2]),
            ],
            NodeGraphEntrypoint::Node(5),
            vec![
                init::Tip::entrypoint(
                    left,
                    Some("refs/heads/left".try_into().expect("valid full ref name")),
                ),
                init::Tip::reachable(
                    right,
                    Some("refs/heads/right".try_into().expect("valid full ref name")),
                ),
            ],
        )
        .into_segment_graph()?;

        let base_segment = graph.segment_by_commit_id(base)?;
        assert!(
            base_segment.ref_info.is_none(),
            "multiple direct references leave the fork base anonymous"
        );
        for name in ["refs/heads/base-a", "refs/heads/base-b"] {
            assert!(
                graph
                    .segment_by_ref_name(name.try_into().expect("valid full ref name"))
                    .is_none(),
                "attached references do not also survive as empty compatibility segments"
            );
        }
        assert_eq!(
            base_segment.commits[0]
                .refs
                .iter()
                .map(|reference| reference.ref_name.to_string())
                .collect::<Vec<_>>(),
            ["refs/heads/base-a", "refs/heads/base-b"],
            "ambiguous references are represented exactly once on the commit"
        );
        Ok(())
    }

    #[test]
    fn reference_fan_out_and_remote_links_survive_segmentation() -> Result<()> {
        let id = oid(1);
        let mut workspace = reference("refs/heads/gitbutler/workspace", id, vec![1, 2, 0]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));

        let fan_out = graph(
            vec![
                commit(id, vec![]),
                reference("refs/heads/a", id, vec![0]),
                reference("refs/heads/b", id, vec![0]),
                workspace,
            ],
            NodeGraphEntrypoint::Node(3),
            vec![init::Tip::entrypoint(
                id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        )
        .into_segment_graph()?;
        let workspace_segment = fan_out
            .segment_by_ref_name(
                "refs/heads/gitbutler/workspace"
                    .try_into()
                    .expect("valid full ref name"),
            )
            .expect("workspace segment");
        assert!(
            workspace_segment.commits.is_empty(),
            "workspace ref stays virtual"
        );
        assert_eq!(
            fan_out
                .inner
                .edges_directed(workspace_segment.id, Direction::Outgoing)
                .count(),
            2,
            "same-target overlays make the structural workspace edge redundant"
        );
        let fan_out_order = fan_out
            .inner
            .edges_directed(workspace_segment.id, Direction::Outgoing)
            .map(|edge| fan_out[edge.target()].ref_name().map(ToString::to_string))
            .collect::<Vec<_>>();
        assert_eq!(
            fan_out_order,
            [None, None],
            "attached named paths resolve directly to their shared commit segment"
        );
        let shared_commit = fan_out.segment_by_commit_id(id)?;
        assert!(shared_commit.ref_info.is_none());
        assert_eq!(
            shared_commit.commits[0]
                .refs
                .iter()
                .map(|reference| reference.ref_name.to_string())
                .collect::<Vec<_>>(),
            ["refs/heads/a", "refs/heads/b"],
            "ambiguous non-owning locals remain attached to the anonymous commit"
        );

        let local_name = "refs/heads/main";
        let remote_name = "refs/remotes/origin/main";
        let mut local = reference(local_name, id, vec![0]);
        let NodeKind::Reference(local_reference) = &mut local.kind else {
            unreachable!("constructed a reference")
        };
        local_reference.remote_tracking_ref_name =
            Some(remote_name.try_into().expect("valid full ref name"));
        let second_local_name = "refs/heads/also-main";
        let mut second_local = reference(second_local_name, id, vec![1]);
        let NodeKind::Reference(second_local_reference) = &mut second_local.kind else {
            unreachable!("constructed a reference")
        };
        second_local_reference.remote_tracking_ref_name =
            Some(remote_name.try_into().expect("valid full ref name"));
        let linked = graph(
            vec![
                commit(id, vec![]),
                local,
                second_local,
                reference(remote_name, id, vec![2]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::entrypoint(
                id,
                Some(local_name.try_into().expect("valid full ref name")),
            )],
        )
        .into_segment_graph()?;
        let local_segment = linked
            .segment_by_ref_name(local_name.try_into().expect("valid full ref name"))
            .expect("local segment");
        let remote_segment = linked
            .segment_by_ref_name(remote_name.try_into().expect("valid full ref name"))
            .expect("remote segment");
        let second_local_segment = linked
            .segment_by_ref_name(second_local_name.try_into().expect("valid full ref name"))
            .expect("second local segment");
        assert_eq!(
            local_segment.commits.first().map(|commit| commit.id),
            Some(id),
            "the lower same-tip ref owns the commit"
        );
        assert!(
            remote_segment.commits.is_empty(),
            "the upper same-tip ref remains virtual"
        );
        assert_eq!(
            local_segment.remote_tracking_branch_segment_id,
            Some(remote_segment.id),
            "local points to its remote"
        );
        assert_eq!(
            remote_segment.sibling_segment_id,
            Some(local_segment.id),
            "remote points back to its local"
        );
        assert_eq!(
            second_local_segment
                .remote_tracking_ref_name
                .as_ref()
                .map(ToString::to_string),
            Some(remote_name.to_owned()),
            "later locals retain their configured remote name"
        );
        assert_eq!(
            second_local_segment.remote_tracking_branch_segment_id, None,
            "a singular remote segment is claimed by only one local"
        );

        Ok(())
    }

    #[test]
    fn a_unique_local_owns_a_target_shared_with_the_workspace_ref() -> Result<()> {
        let id = oid(1);
        let mut workspace = reference("refs/heads/gitbutler/workspace", id, vec![0]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let graph = graph(
            vec![
                commit(id, vec![]),
                reference("refs/heads/a", id, vec![0]),
                workspace,
            ],
            NodeGraphEntrypoint::Node(2),
            vec![init::Tip::entrypoint(
                id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        )
        .into_segment_graph()?;

        let local = graph
            .segment_by_ref_name("refs/heads/a".try_into().expect("valid full ref name"))
            .expect("local segment");
        assert_eq!(local.commits.first().map(|commit| commit.id), Some(id));
        assert!(
            local.commits[0].refs.is_empty(),
            "the owning ref is not attached twice"
        );
        assert!(
            graph
                .segment_by_ref_name(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                )
                .is_some_and(|segment| segment.commits.is_empty()),
            "a workspace ref without a managed commit stays virtual"
        );
        Ok(())
    }

    #[test]
    fn workspace_owns_an_explicit_managed_commit_despite_a_metadata_local() -> Result<()> {
        let base_id = oid(1);
        let managed_id = oid(2);
        let mut workspace = reference("refs/heads/gitbutler/workspace", managed_id, vec![1]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut local = reference("refs/heads/a", managed_id, vec![1]);
        let NodeKind::Reference(local_reference) = &mut local.kind else {
            unreachable!("constructed a reference")
        };
        local_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(base_id, vec![]),
                commit(managed_id, vec![0]),
                local,
                workspace,
            ],
            NodeGraphEntrypoint::Node(3),
            vec![init::Tip::entrypoint(
                managed_id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        );
        node_graph.context.managed_workspace_commit_id = Some(managed_id);
        let graph = node_graph.into_segment_graph()?;

        let workspace = graph
            .segment_by_ref_name(
                "refs/heads/gitbutler/workspace"
                    .try_into()
                    .expect("valid full ref name"),
            )
            .expect("workspace segment");
        assert_eq!(
            workspace
                .commits
                .iter()
                .map(|commit| commit.id)
                .collect::<Vec<_>>(),
            [managed_id],
            "the workspace segment owns only its managed commit"
        );
        assert!(
            graph
                .segment_by_ref_name("refs/heads/a".try_into().expect("valid full ref name"))
                .is_some_and(|segment| segment.commits.is_empty()),
            "metadata locals remain empty when the commit is explicitly managed"
        );
        let outgoing = graph
            .inner
            .edges_directed(workspace.id, Direction::Outgoing)
            .map(|edge| graph[edge.target()].commits.first().map(|commit| commit.id))
            .collect::<Vec<_>>();
        assert_eq!(
            outgoing,
            [Some(base_id)],
            "ancestry below the managed commit starts in an outgoing segment"
        );
        Ok(())
    }

    #[test]
    fn metadata_branch_at_an_integrated_workspace_base_remains_empty() -> Result<()> {
        let base_id = oid(1);
        let target_id = oid(2);
        let managed_id = oid(3);
        let branch_name = "refs/heads/new-branch";
        let workspace_name = "refs/heads/gitbutler/workspace";
        let mut branch = reference(branch_name, base_id, vec![0]);
        let NodeKind::Reference(branch_reference) = &mut branch.kind else {
            unreachable!("constructed a reference")
        };
        branch_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut workspace = reference(workspace_name, managed_id, vec![3, 2]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(base_id, vec![]),
                commit(target_id, vec![0]),
                commit(managed_id, vec![0]),
                branch,
                workspace,
            ],
            NodeGraphEntrypoint::Node(4),
            vec![
                init::Tip::entrypoint(
                    managed_id,
                    Some(workspace_name.try_into().expect("valid full ref name")),
                ),
                init::Tip::new(base_id).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: branch_name.try_into().expect("valid full ref name"),
                }),
            ],
        );
        node_graph.context.managed_workspace_commit_id = Some(managed_id);
        node_graph.context.project_meta.target_commit_id = Some(target_id);
        node_graph.annotations[0] |= CommitFlags::Integrated;

        let graph = node_graph.into_segment_graph()?;

        assert!(
            graph
                .segment_by_ref_name(branch_name.try_into().expect("valid full ref name"))
                .is_some_and(|segment| segment.commits.is_empty()),
            "a metadata selector at the stored target must stay explicit and empty"
        );
        let target = graph.segment_by_commit_id(base_id)?;
        assert!(
            target.ref_info.is_none(),
            "the integrated base commit remains the anonymous workspace lower bound"
        );
        let workspace = graph
            .segment_by_ref_name(workspace_name.try_into().expect("valid full ref name"))
            .expect("workspace segment");
        assert!(
            graph
                .inner
                .edges_directed(workspace.id, Direction::Outgoing)
                .any(|edge| edge.target()
                    == graph
                        .segment_by_ref_name(branch_name.try_into().expect("valid full ref name"))
                        .expect("branch segment")
                        .id),
            "the empty branch stays connected as an independent workspace root"
        );
        Ok(())
    }

    #[test]
    fn metadata_free_sole_owner_below_a_managed_workspace_keeps_its_segment() -> Result<()> {
        let base_id = oid(1);
        let stack_id = oid(2);
        let managed_id = oid(3);
        let mut stack = reference("refs/heads/A", stack_id, vec![1]);
        let NodeKind::Reference(stack_reference) = &mut stack.kind else {
            unreachable!("constructed a reference")
        };
        stack_reference.metadata = Some(SegmentMetadata::Branch(Default::default()));
        let mut workspace = reference("refs/heads/gitbutler/workspace", managed_id, vec![4, 2]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let mut node_graph = graph(
            vec![
                commit(base_id, vec![]),
                commit(stack_id, vec![0]),
                commit(managed_id, vec![4]),
                reference("refs/heads/E", base_id, vec![0]),
                stack,
                workspace,
            ],
            NodeGraphEntrypoint::Node(5),
            vec![init::Tip::entrypoint(
                managed_id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        );
        node_graph.context.managed_workspace_commit_id = Some(managed_id);
        let graph = node_graph.into_segment_graph()?;

        let base = graph
            .segment_by_ref_name("refs/heads/E".try_into().expect("valid full ref name"))
            .expect("the sole owning local keeps a segment");
        assert_eq!(
            base.commits.first().map(|commit| commit.id),
            Some(base_id),
            "the metadata-free local owns its target commit"
        );
        assert!(
            base.commits[0].refs.is_empty(),
            "an owning local is not duplicated as an inline alias"
        );
        Ok(())
    }

    #[test]
    fn workspace_without_a_managed_commit_stays_empty_when_only_a_remote_shares_its_tip()
    -> Result<()> {
        let id = oid(1);
        let mut workspace = reference("refs/heads/gitbutler/workspace", id, vec![0]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let graph = graph(
            vec![
                commit(id, vec![]),
                workspace,
                reference("refs/remotes/origin/main", id, vec![0]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::entrypoint(
                id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        )
        .into_segment_graph()?;

        let workspace = graph
            .segment_by_ref_name(
                "refs/heads/gitbutler/workspace"
                    .try_into()
                    .expect("valid full ref name"),
            )
            .expect("workspace segment");
        assert!(
            workspace.commits.is_empty(),
            "a workspace ref without a managed commit stays virtual"
        );
        let remote = graph
            .segment_by_ref_name(
                "refs/remotes/origin/main"
                    .try_into()
                    .expect("valid full ref name"),
            )
            .expect("remote segment");
        assert_eq!(
            remote.commits.first().map(|commit| commit.id),
            Some(id),
            "the non-workspace reference owns the shared tip"
        );
        assert_eq!(
            graph.tip_skip_empty(workspace.id).map(|commit| commit.id),
            Some(id),
            "the virtual workspace segment resolves through its outgoing edge"
        );
        Ok(())
    }

    #[test]
    fn incomplete_workspace_overlay_replaces_only_exact_git_parents() -> Result<()> {
        let base = oid(1);
        let left = oid(2);
        let right = oid(3);
        let workspace_id = oid(4);
        let mut workspace = reference("refs/heads/gitbutler/workspace", workspace_id, vec![4, 3]);
        let NodeKind::Reference(workspace_reference) = &mut workspace.kind else {
            unreachable!("constructed a reference")
        };
        workspace_reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        let graph = graph(
            vec![
                commit(base, vec![]),
                commit(left, vec![0]),
                commit(right, vec![0]),
                commit(workspace_id, vec![1, 2]),
                reference("refs/heads/right", right, vec![2]),
                workspace,
            ],
            NodeGraphEntrypoint::Node(5),
            vec![init::Tip::entrypoint(
                workspace_id,
                Some(
                    "refs/heads/gitbutler/workspace"
                        .try_into()
                        .expect("valid full ref name"),
                ),
            )],
        )
        .into_segment_graph()?;

        let workspace = graph
            .segment_by_ref_name(
                "refs/heads/gitbutler/workspace"
                    .try_into()
                    .expect("valid full ref name"),
            )
            .expect("workspace segment");
        assert!(
            workspace.commits.is_empty(),
            "a workspace reference without a managed commit stays virtual"
        );
        let overlay_outgoing = graph
            .inner
            .edges_directed(workspace.id, Direction::Outgoing)
            .filter_map(|edge| {
                graph[edge.target()]
                    .ref_name()
                    .map(|name| (name.to_string(), edge.weight().parent_order))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            overlay_outgoing,
            [("refs/heads/right".into(), 1)],
            "the overlay keeps the exact second-parent slot"
        );
        let workspace_tip = graph.segment_by_commit_id(workspace_id)?;
        let ancestry_outgoing = graph
            .inner
            .edges_directed(workspace_tip.id, Direction::Outgoing)
            .map(|edge| {
                (
                    graph[edge.target()].ref_name().map(ToString::to_string),
                    edge.weight()
                        .parent_order()
                        .expect("edge starts at the workspace tip"),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            ancestry_outgoing,
            [(None, 0)],
            "the unclaimed first parent remains outgoing from the workspace-tip owner"
        );
        Ok(())
    }

    #[test]
    fn shallow_and_limit_sentinels_become_legacy_stop_conditions() -> Result<()> {
        let omitted = oid(1);
        let tip = oid(2);
        let shallow = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id: omitted,
                        reason: StopCondition::ShallowBoundary,
                    },
                    parents: vec![],
                },
                commit(tip, vec![0]),
                reference("refs/heads/main", tip, vec![1]),
            ],
            NodeGraphEntrypoint::Node(2),
            vec![init::Tip::entrypoint(
                tip,
                Some("refs/heads/main".try_into().expect("valid full ref name")),
            )],
        )
        .into_segment_graph()?;
        let shallow_segment = shallow.segment_by_commit_id(tip)?;
        assert_eq!(shallow_segment.commits[0].parent_ids, [omitted]);
        assert_eq!(
            shallow.stop_condition(shallow_segment.id),
            Some(StopCondition::ShallowBoundary),
            "shallow boundaries remain distinguishable from limits"
        );

        let limited_tip = oid(3);
        let mut limited = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id: omitted,
                        reason: StopCondition::Limit,
                    },
                    parents: vec![],
                },
                commit(limited_tip, vec![0]),
            ],
            NodeGraphEntrypoint::Node(1),
            vec![init::Tip::entrypoint(limited_tip, None)],
        );
        limited.context.hard_limit_hit = true;
        let limited = limited.into_segment_graph()?;
        let limited_segment = limited.segment_by_commit_id(limited_tip)?;
        assert_eq!(
            limited.stop_condition(limited_segment.id),
            Some(StopCondition::Limit),
            "an omitted non-shallow parent is a traversal limit"
        );
        assert!(limited.hard_limit_hit(), "hard-limit context is retained");

        Ok(())
    }

    #[test]
    fn segmented_mutations_invalidate_construction_provenance() -> Result<()> {
        let make_graph = || {
            let id = oid(1);
            graph(
                vec![commit(id, vec![])],
                NodeGraphEntrypoint::Node(0),
                vec![init::Tip::entrypoint(id, None)],
            )
            .into_segment_graph()
        };

        let mut compatibility = make_graph()?;
        assert!(compatibility.construction_graph().is_some());
        let segment = compatibility.segments().next().expect("one segment");
        compatibility[segment].generation += 1;
        assert!(compatibility.construction_graph().is_none());

        let mut compatibility = make_graph()?;
        compatibility.insert_segment(Segment::default());
        assert!(compatibility.construction_graph().is_none());

        let mut compatibility = make_graph()?;
        compatibility.add_node(Segment::default());
        assert!(compatibility.construction_graph().is_none());
        Ok(())
    }

    #[test]
    fn unborn_entrypoint_is_a_valid_empty_named_segment() -> Result<()> {
        let main = "refs/heads/main";
        let remote = "refs/remotes/origin/main";
        let graph = graph(
            Vec::new(),
            NodeGraphEntrypoint::Unborn(Box::new(Reference {
                ref_info: RefInfo {
                    ref_name: main.try_into().expect("valid full ref name"),
                    commit_id: None,
                    worktree: Some(Worktree {
                        kind: WorktreeKind::Main,
                        owned_by_repo: true,
                    }),
                },
                metadata: Some(SegmentMetadata::Branch(Default::default())),
                remote_tracking_ref_name: Some(remote.try_into().expect("valid full ref name")),
            })),
            Vec::new(),
        )
        .into_segment_graph()?;

        let entrypoint = graph.entrypoint()?;
        assert_eq!(
            entrypoint.segment.ref_name(),
            Some(main.try_into().expect("valid full ref name")),
            "the unborn ref remains the entrypoint"
        );
        assert!(
            entrypoint.commit().is_none(),
            "unborn entrypoints have no commit"
        );
        assert!(
            entrypoint
                .segment
                .ref_info
                .as_ref()
                .is_some_and(|ref_info| {
                    ref_info
                        .worktree
                        .as_ref()
                        .is_some_and(|worktree| worktree.owned_by_repo)
                }),
            "unborn worktree information is retained"
        );
        assert!(
            matches!(
                entrypoint.segment.metadata,
                Some(SegmentMetadata::Branch(_))
            ),
            "unborn branch metadata is retained"
        );
        assert_eq!(
            entrypoint
                .segment
                .remote_tracking_ref_name
                .as_ref()
                .map(ToString::to_string),
            Some(remote.to_owned()),
            "unborn remote configuration is retained"
        );
        assert_eq!(
            graph.num_segments(),
            1,
            "no stand-in segment is synthesized"
        );
        Ok(())
    }
}
