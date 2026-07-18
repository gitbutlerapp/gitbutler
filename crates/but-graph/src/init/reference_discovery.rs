use std::collections::{BTreeMap, BTreeSet, VecDeque};

use anyhow::{Context as _, Result};
use but_core::{RefMetadata, ref_metadata::StackKind};
use gix::refs::Category;

use crate::{CommitFlags, NodeGraph, NodeIndex, NodeKind, Reference, ReferenceMetadata};

use super::{
    overlay::{OverlayMetadata, OverlayRepo},
    reference_groups::{
        GroupedReference, ReferenceGroup, ReferenceGroupChild, ReferenceGroupChildKind,
        ReferenceGroupParent, apply_reference_groups,
    },
    remotes,
};

#[derive(Debug)]
struct ReferenceOrder {
    names: Vec<gix::refs::FullName>,
    anchor: NodeIndex,
    preferred_child: Option<NodeIndex>,
    workspace_root: bool,
    place_on_ancestry: bool,
}

#[derive(Debug)]
struct WorkspaceOrder {
    workspace_name: gix::refs::FullName,
    order_index: usize,
}

#[derive(Debug, Copy, Clone)]
struct PlacementHint {
    anchor: NodeIndex,
    preferred_child: Option<NodeIndex>,
    priority: usize,
}

/// Discover all visible refs whose peeled commits were traversed, derive their adjacency from
/// metadata, and apply the resulting pure reference groups.
pub(super) fn discover_and_apply_reference_groups<T: RefMetadata>(
    graph: NodeGraph,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
) -> Result<NodeGraph> {
    let references = discover_references(&graph, repo, meta)?;
    let groups = build_reference_groups(&graph, references)?;
    apply_reference_groups(graph, groups)
}

fn discover_references<T: RefMetadata>(
    graph: &NodeGraph,
    repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
) -> Result<Vec<Reference>> {
    let commit_ids = graph
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::Commit { id } => Some(id),
            NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
        })
        .collect::<BTreeSet<_>>();
    let refs_by_id = repo.collect_ref_mapping_by_prefix(
        ["refs/heads/", "refs/remotes/"]
            .into_iter()
            .chain(graph.context.options.collect_tags.then_some("refs/tags/")),
        &[],
    )?;
    let worktree_by_branch = repo.worktree_branches(
        graph
            .context
            .entrypoint_ref
            .as_ref()
            .map(|name| name.as_ref()),
    )?;
    let configured_remote_tracking_branches = remotes::configured_remote_tracking_branches(repo)?;
    let mut refs = refs_by_id
        .into_iter()
        .filter(|(id, _)| commit_ids.contains(id))
        .flat_map(|(id, names)| names.into_iter().map(move |name| (name, (id, None))))
        .collect::<BTreeMap<_, _>>();
    for tip in &graph.context.traversal_tips {
        let Some(ref_name) = tip.ref_name.clone() else {
            continue;
        };
        anyhow::ensure!(
            commit_ids.contains(&tip.id),
            "BUG: named traversal tip {ref_name} targets untraversed commit {}",
            tip.id
        );
        refs.insert(ref_name, (tip.id, tip.metadata.clone()));
    }

    refs.into_iter()
        .map(|(ref_name, (commit_id, tip_metadata))| {
            let metadata = match tip_metadata {
                Some(metadata) => Some(metadata),
                None => metadata_for_ref(meta, ref_name.as_ref())?,
            };
            let remote_tracking_ref_name = if ref_name.category() == Some(Category::LocalBranch) {
                remotes::lookup_remote_tracking_branch_or_deduce_it(
                    repo,
                    ref_name.as_ref(),
                    &graph.context.symbolic_remote_names,
                    &configured_remote_tracking_branches,
                )?
            } else {
                None
            };
            Ok(Reference {
                ref_info: crate::RefInfo::from_ref(ref_name, commit_id, &worktree_by_branch),
                metadata,
                remote_tracking_ref_name,
            })
        })
        .collect()
}

pub(super) fn metadata_for_ref<T: RefMetadata>(
    meta: &OverlayMetadata<'_, T>,
    ref_name: &gix::refs::FullNameRef,
) -> Result<Option<ReferenceMetadata>> {
    if ref_name.category() != Some(Category::LocalBranch) {
        return Ok(None);
    }
    if let Some(branch) = meta.branch_opt(ref_name)? {
        return Ok(Some(ReferenceMetadata::Branch(branch)));
    }
    Ok(meta
        .workspace_opt(ref_name)?
        .map(ReferenceMetadata::Workspace))
}

fn build_reference_groups(
    graph: &NodeGraph,
    references: Vec<Reference>,
) -> Result<Vec<ReferenceGroup>> {
    let commit_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match node.kind {
            NodeKind::Commit { id } => Some((id, index)),
            NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    let reference_target = references
        .iter()
        .filter_map(|reference| {
            Some((
                reference.ref_info.ref_name.clone(),
                *commit_by_id.get(&reference.ref_info.commit_id?)?,
            ))
        })
        .collect::<BTreeMap<_, _>>();
    let (orders, workspace_orders) =
        reference_orders(graph, &references, &reference_target, &commit_by_id);
    let mut references_by_commit = BTreeMap::<NodeIndex, Vec<Reference>>::new();
    for reference in references {
        let id = reference
            .ref_info
            .commit_id
            .context("BUG: discovered reference has no target")?;
        let parent = *commit_by_id
            .get(&id)
            .context("BUG: discovered reference target was not traversed")?;
        references_by_commit
            .entry(parent)
            .or_default()
            .push(reference);
    }

    references_by_commit
        .into_iter()
        .map(|(parent, references)| {
            group_at_commit(
                graph,
                parent,
                references,
                &orders,
                &workspace_orders,
                &reference_target,
            )
        })
        .collect()
}

fn reference_orders(
    graph: &NodeGraph,
    references: &[Reference],
    reference_target: &BTreeMap<gix::refs::FullName, NodeIndex>,
    commit_by_id: &BTreeMap<gix::ObjectId, NodeIndex>,
) -> (Vec<ReferenceOrder>, Vec<WorkspaceOrder>) {
    let mut orders = Vec::new();
    let mut workspace_orders = Vec::new();

    for reference in references {
        let Some(ReferenceMetadata::Workspace(workspace)) = &reference.metadata else {
            continue;
        };
        let workspace_name = reference.ref_info.ref_name.clone();
        let workspace_node = reference
            .ref_info
            .commit_id
            .and_then(|id| commit_by_id.get(&id).copied());
        for stack in workspace.stacks(StackKind::Applied) {
            let names = stack
                .branches
                .iter()
                .take_while(|branch| !branch.archived)
                .map(|branch| branch.ref_name.clone())
                .filter(|name| name != &workspace_name)
                .filter(|name| reference_target.contains_key(name))
                .collect::<Vec<_>>();
            let Some(anchor) = names
                .first()
                .and_then(|name| reference_target.get(name).copied())
            else {
                continue;
            };
            let order_index = orders.len();
            orders.push(ReferenceOrder {
                names,
                anchor,
                preferred_child: workspace_node,
                workspace_root: false,
                place_on_ancestry: false,
            });
            workspace_orders.push(WorkspaceOrder {
                workspace_name: workspace_name.clone(),
                order_index,
            });
        }
    }

    stitch_nested_workspace_orders(graph, &mut orders, &mut workspace_orders);

    for workspace_order in &workspace_orders {
        let order = &orders[workspace_order.order_index];
        let Some(&workspace_node) = reference_target.get(&workspace_order.workspace_name) else {
            continue;
        };
        let anchor = order.anchor;
        let direct_workspace_parent = graph.nodes[workspace_node].parents.contains(&anchor);
        let workspace_root = anchor == workspace_node
            || direct_workspace_parent
            || (!graph.annotations[anchor].contains(CommitFlags::Integrated)
                && reaches(graph, workspace_node, anchor))
            || (graph.annotations[anchor].contains(CommitFlags::Integrated)
                && matches!(
                    graph.nodes[anchor].kind,
                    NodeKind::Commit { id }
                        if Some(id) == graph.context.project_meta.target_commit_id
                ))
            || (graph.annotations[anchor].contains(CommitFlags::Integrated)
                && workspace_orders
                    .iter()
                    .filter(|candidate| candidate.workspace_name == workspace_order.workspace_name)
                    .map(|candidate| orders[candidate.order_index].anchor)
                    .any(|other_anchor| {
                        other_anchor != anchor
                            && graph.nodes[other_anchor].parents.contains(&anchor)
                    }));
        let order = &mut orders[workspace_order.order_index];
        order.workspace_root = workspace_root;
        order.place_on_ancestry = workspace_root
            && (!graph.annotations[anchor].contains(CommitFlags::Integrated)
                || direct_workspace_parent);
    }

    for names in &graph.context.ad_hoc_branch_stack_orders {
        let names = names
            .iter()
            .filter(|name| reference_target.contains_key(*name))
            .cloned()
            .collect::<Vec<_>>();
        let Some(anchor) = names
            .first()
            .and_then(|name| reference_target.get(name).copied())
        else {
            continue;
        };
        orders.push(ReferenceOrder {
            names,
            anchor,
            preferred_child: None,
            workspace_root: false,
            place_on_ancestry: true,
        });
    }
    (orders, workspace_orders)
}

/// Join metadata orders that describe consecutive pieces of the same ancestry path.
///
/// Workspace metadata may retain these pieces as separate stacks while the graph proves that one
/// anchor is strictly above the other. Treating them as independent orders makes their references
/// compete for the same commit-child slot; one effective order preserves every
/// reference between the upper and lower commits.
fn stitch_nested_workspace_orders(
    graph: &NodeGraph,
    orders: &mut [ReferenceOrder],
    workspace_orders: &mut Vec<WorkspaceOrder>,
) {
    loop {
        let mut relations = Vec::new();
        for left in 0..workspace_orders.len() {
            for right in left + 1..workspace_orders.len() {
                if workspace_orders[left].workspace_name != workspace_orders[right].workspace_name {
                    continue;
                }
                let left_order = workspace_orders[left].order_index;
                let right_order = workspace_orders[right].order_index;
                let left_anchor = orders[left_order].anchor;
                let right_anchor = orders[right_order].anchor;
                let direction = if left_anchor != right_anchor
                    && reaches(graph, left_anchor, right_anchor)
                {
                    Some((left, right, left_order, right_order))
                } else if left_anchor != right_anchor && reaches(graph, right_anchor, left_anchor) {
                    Some((right, left, right_order, left_order))
                } else {
                    None
                };
                let Some((upper, lower, upper_order, lower_order)) = direction else {
                    continue;
                };
                let upper_anchor = orders[upper_order].anchor;
                let lower_anchor = orders[lower_order].anchor;
                let same_workspace_node = orders[upper_order]
                    .preferred_child
                    .filter(|workspace| Some(*workspace) == orders[lower_order].preferred_child);
                if graph.annotations[upper_anchor].contains(CommitFlags::Integrated)
                    || graph.annotations[lower_anchor].contains(CommitFlags::Integrated)
                    || orders[upper_order]
                        .names
                        .iter()
                        .any(|name| orders[lower_order].names.contains(name))
                    || same_workspace_node.is_none()
                    || same_workspace_node.is_some_and(|workspace| {
                        graph.nodes[workspace].parents.contains(&lower_anchor)
                    })
                    || !has_unique_ancestry_path(graph, upper_anchor, lower_anchor)
                {
                    continue;
                }
                relations.push((upper, lower, upper_order, lower_order));
            }
        }

        relations.retain(|(upper, lower, upper_order, lower_order)| {
            let upper_anchor = orders[*upper_order].anchor;
            let lower_anchor = orders[*lower_order].anchor;
            !workspace_orders
                .iter()
                .enumerate()
                .any(|(middle, middle_workspace_order)| {
                    if middle == *upper
                        || middle == *lower
                        || middle_workspace_order.workspace_name
                            != workspace_orders[*upper].workspace_name
                    {
                        return false;
                    }
                    let middle_anchor = orders[middle_workspace_order.order_index].anchor;
                    middle_anchor != upper_anchor
                        && middle_anchor != lower_anchor
                        && reaches(graph, upper_anchor, middle_anchor)
                        && reaches(graph, middle_anchor, lower_anchor)
                })
        });
        let mut successor_count = BTreeMap::<usize, usize>::new();
        let mut predecessor_count = BTreeMap::<usize, usize>::new();
        for (upper, lower, _, _) in &relations {
            *successor_count.entry(*upper).or_default() += 1;
            *predecessor_count.entry(*lower).or_default() += 1;
        }
        let nested_pair = relations.into_iter().find(|(upper, lower, _, _)| {
            successor_count.get(upper) == Some(&1) && predecessor_count.get(lower) == Some(&1)
        });
        let Some((upper, lower, upper_order, lower_order)) = nested_pair else {
            break;
        };

        let mut names = Vec::new();
        let mut seen = BTreeSet::new();
        for name in orders[upper_order]
            .names
            .iter()
            .chain(&orders[lower_order].names)
        {
            if seen.insert(name.clone()) {
                names.push(name.clone());
            }
        }
        let left = upper.min(lower);
        let right = upper.max(lower);
        let keep_order = workspace_orders[left]
            .order_index
            .min(workspace_orders[right].order_index);
        let drop_order = if keep_order == workspace_orders[left].order_index {
            workspace_orders[right].order_index
        } else {
            workspace_orders[left].order_index
        };
        orders[keep_order].names = names;
        orders[keep_order].anchor = orders[upper_order].anchor;
        orders[keep_order].preferred_child = orders[upper_order]
            .preferred_child
            .or(orders[lower_order].preferred_child);
        orders[keep_order].workspace_root = false;
        orders[keep_order].place_on_ancestry = false;
        orders[drop_order].names.clear();

        workspace_orders[left].order_index = keep_order;
        workspace_orders.remove(right);
    }
}

fn has_unique_ancestry_path(graph: &NodeGraph, upper: NodeIndex, lower: NodeIndex) -> bool {
    let mut reachable = vec![false; graph.nodes.len()];
    let mut pending = vec![upper];
    while let Some(current) = pending.pop() {
        if std::mem::replace(&mut reachable[current], true) {
            continue;
        }
        pending.extend(graph.nodes[current].parents.iter().copied());
    }
    if !reachable[lower] {
        return false;
    }

    let mut unprocessed_children = vec![0usize; graph.nodes.len()];
    for (child, node) in graph.nodes.iter().enumerate() {
        if reachable[child] {
            for parent in &node.parents {
                unprocessed_children[*parent] += 1;
            }
        }
    }

    let mut path_counts = vec![0u8; graph.nodes.len()];
    path_counts[upper] = 1;
    let mut pending = VecDeque::from([upper]);
    while let Some(current) = pending.pop_front() {
        for parent in &graph.nodes[current].parents {
            path_counts[*parent] = path_counts[*parent]
                .saturating_add(path_counts[current])
                .min(2);
            unprocessed_children[*parent] -= 1;
            if unprocessed_children[*parent] == 0 {
                pending.push_back(*parent);
            }
        }
    }
    path_counts[lower] == 1
}

fn group_at_commit(
    graph: &NodeGraph,
    parent: NodeIndex,
    mut references: Vec<Reference>,
    orders: &[ReferenceOrder],
    workspace_orders: &[WorkspaceOrder],
    reference_target: &BTreeMap<gix::refs::FullName, NodeIndex>,
) -> Result<ReferenceGroup> {
    references.sort_by(|left, right| left.ref_info.ref_name.cmp(&right.ref_info.ref_name));
    let index_by_name = references
        .iter()
        .enumerate()
        .map(|(index, reference)| (reference.ref_info.ref_name.clone(), index))
        .collect::<BTreeMap<_, _>>();
    let mut parents = vec![vec![ReferenceGroupParent::Commit]; references.len()];
    let mut ordered = BTreeSet::new();
    let mut hints = BTreeMap::<usize, PlacementHint>::new();

    for (priority, order) in orders.iter().enumerate() {
        let mut seen = BTreeSet::new();
        let matching = order
            .names
            .iter()
            .filter_map(|name| index_by_name.get(name).copied())
            .filter(|index| !ordered.contains(index) && seen.insert(*index))
            .collect::<Vec<_>>();
        let Some(&root) = matching.first() else {
            continue;
        };
        for pair in matching.windows(2) {
            parents[pair[0]] = vec![ReferenceGroupParent::Reference(pair[1])];
        }
        ordered.extend(matching.iter().copied());
        if order.place_on_ancestry {
            hints.insert(
                root,
                PlacementHint {
                    anchor: order.anchor,
                    preferred_child: order.preferred_child,
                    priority,
                },
            );
        }
    }

    for workspace in references
        .iter()
        .enumerate()
        .filter_map(|(index, reference)| {
            matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_)))
                .then_some((index, &reference.ref_info.ref_name))
        })
    {
        let (workspace_index, workspace_name) = workspace;
        let stack_roots = workspace_orders
            .iter()
            .filter(|order| &order.workspace_name == workspace_name)
            .filter(|workspace_order| orders[workspace_order.order_index].workspace_root)
            .filter_map(|workspace_order| {
                let order = &orders[workspace_order.order_index];
                order
                    .names
                    .iter()
                    .find_map(|name| {
                        reference_target
                            .get(name)
                            .copied()
                            .map(|anchor| (name, anchor))
                    })
                    .map(|(name, anchor)| (name.clone(), anchor))
            })
            .filter(|(root, _)| root != workspace_name)
            .collect::<Vec<_>>();
        let mut stack_roots = select_workspace_stack_roots(graph, &stack_roots);
        if let ([root], Some(entrypoint_name)) = (
            stack_roots.as_slice(),
            graph.context.entrypoint_ref.as_ref(),
        ) && entrypoint_name != workspace_name
            && entrypoint_name != root
            && entrypoint_name.category() == Some(Category::LocalBranch)
            && let Some(&root_index) = index_by_name.get(root)
            && let Some(&entrypoint_index) = index_by_name.get(entrypoint_name)
            && parents[entrypoint_index] == [ReferenceGroupParent::Commit]
            && references[entrypoint_index]
                .remote_tracking_ref_name
                .as_ref()
                != graph.context.project_meta.target_ref.as_ref()
            && !reference_parent_reaches(&parents, &index_by_name, root_index, entrypoint_index)
        {
            parents[entrypoint_index] = vec![ReferenceGroupParent::Reference(root_index)];
            stack_roots[0] = entrypoint_name.clone();
        }
        parents[workspace_index] = stack_roots
            .into_iter()
            .map(ReferenceGroupParent::ReferenceByName)
            .chain(std::iter::once(ReferenceGroupParent::Commit))
            .collect();
    }

    let authoritative_target_locals = graph
        .context
        .traversal_tips
        .iter()
        .filter_map(|tip| match &tip.role {
            super::TipRole::TargetLocal { local_ref_name } => Some(local_ref_name),
            super::TipRole::Reachable
            | super::TipRole::Workspace
            | super::TipRole::WorkspaceStackBranch { .. }
            | super::TipRole::TargetRemote => None,
        })
        .collect::<BTreeSet<_>>();
    let mut pairing_order = (0..references.len()).collect::<Vec<_>>();
    pairing_order.sort_by_key(|local| {
        (
            !authoritative_target_locals.contains(&references[*local].ref_info.ref_name),
            *local,
        )
    });

    let mut incoming = reference_incoming(&parents, &index_by_name);
    let mut paired_remote_locals = BTreeMap::new();
    for local in pairing_order {
        let Some(remote_name) = references[local].remote_tracking_ref_name.as_ref() else {
            continue;
        };
        let Some(&remote) = index_by_name.get(remote_name) else {
            continue;
        };
        if reference_target.get(remote_name) != Some(&parent)
            || incoming[remote] != 0
            || parents[remote] != [ReferenceGroupParent::Commit]
        {
            continue;
        }
        parents[remote] = vec![ReferenceGroupParent::Reference(local)];
        paired_remote_locals.insert(remote, local);
        incoming[local] += 1;
        if let Some(hint) = hints.remove(&local) {
            hints.insert(remote, hint);
        }
    }

    incoming = reference_incoming(&parents, &index_by_name);
    let child_slots = commit_child_slots(graph, parent);
    let mut claimed_slots = BTreeSet::new();
    let mut roots = incoming
        .iter()
        .enumerate()
        .filter_map(|(index, incoming)| (*incoming == 0).then_some(index))
        .collect::<Vec<_>>();
    roots.sort_by_key(|index| {
        hints
            .get(index)
            .map(|hint| (0, hint.priority, *index))
            .unwrap_or((1, usize::MAX, *index))
    });
    let unhinted_ordinary_roots = roots
        .iter()
        .filter(|index| {
            !hints.contains_key(index)
                && !matches!(
                    references[**index].metadata.as_ref(),
                    Some(ReferenceMetadata::Workspace(_))
                )
                && references[**index].ref_info.ref_name.category() != Some(Category::Tag)
        })
        .count();

    let mut children = Vec::new();
    let mut outside = Vec::new();
    for root in roots {
        let placement_parent = paired_remote_locals.get(&root).copied().unwrap_or(root);
        let is_tag = references[root].ref_info.ref_name.category() == Some(Category::Tag);
        let slot = (!is_tag)
            .then(|| {
                placement_slot(
                    graph,
                    parent,
                    hints.get(&root).copied(),
                    &child_slots,
                    &claimed_slots,
                    hints.contains_key(&root)
                        || matches!(
                            references[root].metadata.as_ref(),
                            Some(ReferenceMetadata::Workspace(_))
                        )
                        || unhinted_ordinary_roots == 1,
                )
            })
            .flatten();
        if let Some((index, parent_order)) = slot {
            claimed_slots.insert((index, parent_order));
            children.push(ReferenceGroupChild {
                child: ReferenceGroupChildKind::Commit {
                    index,
                    parent_order,
                },
                parents: vec![placement_parent],
            });
            if placement_parent != root {
                outside.push(root);
            }
        } else {
            outside.push(root);
        }
    }
    if !outside.is_empty() {
        children.push(ReferenceGroupChild {
            child: ReferenceGroupChildKind::Outside,
            parents: outside,
        });
    }

    Ok(ReferenceGroup {
        parent,
        references: references
            .into_iter()
            .zip(parents)
            .map(|(reference, parents)| GroupedReference { reference, parents })
            .collect(),
        children,
    })
}

fn select_workspace_stack_roots(
    graph: &NodeGraph,
    candidates: &[(gix::refs::FullName, NodeIndex)],
) -> Vec<gix::refs::FullName> {
    let roots_per_anchor = candidates.iter().fold(
        BTreeMap::<NodeIndex, BTreeSet<&gix::refs::FullName>>::new(),
        |mut roots, (name, anchor)| {
            roots.entry(*anchor).or_default().insert(name);
            roots
        },
    );
    let mut represented_ambiguous_anchors = BTreeSet::new();
    let mut seen_roots = BTreeSet::new();
    candidates
        .iter()
        .filter(|(_, anchor)| {
            if graph.annotations[*anchor].contains(CommitFlags::Integrated) {
                return true;
            }
            let dominated = candidates.iter().any(|(_, other_anchor)| {
                other_anchor != anchor && reaches(graph, *other_anchor, *anchor)
            });
            !dominated
                || (roots_per_anchor[anchor].len() > 1
                    && represented_ambiguous_anchors.insert(*anchor))
        })
        .filter(|(root, _)| seen_roots.insert(root.clone()))
        .map(|(root, _)| root.clone())
        .collect()
}

fn reference_parent_reaches(
    parents: &[Vec<ReferenceGroupParent>],
    index_by_name: &BTreeMap<gix::refs::FullName, usize>,
    start: usize,
    wanted: usize,
) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if index == wanted {
            return true;
        }
        if !seen.insert(index) {
            continue;
        }
        pending.extend(parents[index].iter().filter_map(|parent| match parent {
            ReferenceGroupParent::Reference(index) => Some(*index),
            ReferenceGroupParent::ReferenceByName(name) => index_by_name.get(name).copied(),
            ReferenceGroupParent::Commit => None,
        }));
    }
    false
}

fn reference_incoming(
    parents: &[Vec<ReferenceGroupParent>],
    index_by_name: &BTreeMap<gix::refs::FullName, usize>,
) -> Vec<usize> {
    let mut incoming = vec![0; parents.len()];
    for parent in parents.iter().flatten() {
        match parent {
            ReferenceGroupParent::Reference(index) => incoming[*index] += 1,
            ReferenceGroupParent::ReferenceByName(name) => {
                if let Some(index) = index_by_name.get(name) {
                    incoming[*index] += 1;
                }
            }
            ReferenceGroupParent::Commit => {}
        }
    }
    incoming
}

fn commit_child_slots(graph: &NodeGraph, parent: NodeIndex) -> Vec<(NodeIndex, usize)> {
    graph
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| matches!(node.kind, NodeKind::Commit { .. }))
        .flat_map(|(child, node)| {
            node.parents
                .iter()
                .enumerate()
                .filter_map(move |(order, candidate)| {
                    (*candidate == parent).then_some((child, order))
                })
        })
        .collect()
}

fn placement_slot(
    graph: &NodeGraph,
    parent: NodeIndex,
    hint: Option<PlacementHint>,
    child_slots: &[(NodeIndex, usize)],
    claimed: &BTreeSet<(NodeIndex, usize)>,
    allow_fallback: bool,
) -> Option<(NodeIndex, usize)> {
    if let Some(hint) = hint {
        if hint.anchor != parent {
            let on_path = child_slots
                .iter()
                .copied()
                .filter(|(child, _)| reaches(graph, hint.anchor, *child))
                .collect::<Vec<_>>();
            let [slot] = on_path.as_slice() else {
                return None;
            };
            return (!claimed.contains(slot)).then_some(*slot);
        }
        if let Some(preferred_child) = hint.preferred_child.filter(|child| *child != parent) {
            if let Some(slot) = child_slots
                .iter()
                .find(|(child, _)| *child == preferred_child)
            {
                return (!claimed.contains(slot)).then_some(*slot);
            }
            let on_path = child_slots
                .iter()
                .copied()
                .filter(|(child, _)| reaches(graph, preferred_child, *child))
                .collect::<Vec<_>>();
            let [slot] = on_path.as_slice() else {
                return None;
            };
            return (!claimed.contains(slot)).then_some(*slot);
        }
    }
    if !allow_fallback {
        return None;
    }
    let [slot] = child_slots else {
        return None;
    };
    (!claimed.contains(slot)).then_some(*slot)
}

fn reaches(graph: &NodeGraph, start: NodeIndex, wanted: NodeIndex) -> bool {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    while let Some(index) = pending.pop() {
        if index == wanted {
            return true;
        }
        if seen.insert(index) {
            pending.extend(graph.nodes[index].parents.iter().copied());
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;
    use but_core::{
        RefMetadata,
        ref_metadata::{
            ProjectMeta, StackId, Workspace, WorkspaceCommitRelation, WorkspaceStack,
            WorkspaceStackBranch,
        },
    };
    use but_testsupport::InMemoryRefMetadata;
    use gix::refs::Target;

    use super::*;
    use crate::{Node, NodeGraphEntrypoint, init};

    fn scenario(name: &str) -> Result<gix::Repository> {
        let root = but_testsupport::gix_testtools::scripted_fixture_read_only("scenarios.sh")
            .map_err(anyhow::Error::from_boxed)?;
        Ok(gix::open_opts(root.join(name), gix::open::Options::isolated())?.with_object_memory())
    }

    fn name(name: &str) -> gix::refs::FullName {
        name.try_into().expect("valid test ref name")
    }

    fn id(repo: &gix::Repository, spec: &str) -> Result<gix::ObjectId> {
        Ok(repo.rev_parse_single(spec)?.object()?.peel_to_commit()?.id)
    }

    fn overlay_ref(name: &str, id: gix::ObjectId) -> gix::refs::Reference {
        gix::refs::Reference {
            name: self::name(name),
            target: Target::Object(id),
            peeled: Some(id),
        }
    }

    fn options() -> init::Options {
        init::Options {
            collect_tags: true,
            ..Default::default()
        }
    }

    /// Test-only proof of the intended construction pipeline:
    /// tips -> commits -> discovered reference groups -> applied node graph.
    fn construct<T: RefMetadata>(
        repo: &gix::Repository,
        meta: &T,
        tips: Vec<init::Tip>,
        project_meta: ProjectMeta,
        options: init::Options,
        overlay: init::Overlay,
        entrypoint_ref_override: Option<gix::refs::FullName>,
    ) -> Result<NodeGraph> {
        let (repo, meta, _) = overlay.into_parts(repo, meta);
        let graph = super::super::node_traversal::traverse_tips(
            &repo,
            tips,
            &meta,
            project_meta,
            options,
            entrypoint_ref_override,
        )?;
        discover_and_apply_reference_groups(graph, &repo, &meta)
    }

    fn stack(id: usize, branches: &[&str]) -> WorkspaceStack {
        WorkspaceStack {
            id: StackId::from_number_for_testing(id as u128),
            branches: branches
                .iter()
                .map(|branch| WorkspaceStackBranch {
                    ref_name: name(&format!("refs/heads/{branch}")),
                    archived: false,
                })
                .collect(),
            workspacecommit_relation: WorkspaceCommitRelation::Merged,
        }
    }

    fn metadata(
        stacks: Vec<WorkspaceStack>,
        project_meta: ProjectMeta,
    ) -> (InMemoryRefMetadata, Workspace) {
        let workspace = Workspace::new(Default::default(), stacks, project_meta);
        let mut meta = InMemoryRefMetadata::default();
        meta.workspaces
            .push((name("refs/heads/gitbutler/workspace"), workspace.clone()));
        (meta, workspace)
    }

    fn reference_node<'a>(graph: &'a NodeGraph, ref_name: &str) -> (NodeIndex, &'a Node) {
        graph
            .nodes
            .iter()
            .enumerate()
            .find(|(_, node)| {
                matches!(
                    &node.kind,
                    NodeKind::Reference(reference)
                        if reference.ref_info.ref_name == name(ref_name)
                )
            })
            .unwrap_or_else(|| panic!("missing reference node {ref_name}"))
    }

    fn commit_node(graph: &NodeGraph, id: gix::ObjectId) -> &Node {
        graph
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Commit { id: actual } if actual == id))
            .expect("commit node")
    }

    fn parent_ref_name(graph: &NodeGraph, node: &Node) -> Option<String> {
        let [parent] = node.parents.as_slice() else {
            return None;
        };
        match &graph.nodes[*parent].kind {
            NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.to_string()),
            NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
        }
    }

    #[test]
    fn preferred_descendant_selects_the_unique_immediate_child_on_its_path() {
        let oid = |value: u8| {
            gix::ObjectId::from_hex(format!("{value:040x}").as_bytes())
                .expect("valid test object id")
        };
        let graph = NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id: oid(1) },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(2) },
                    parents: vec![0],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(3) },
                    parents: vec![1],
                },
            ],
            annotations: vec![CommitFlags::empty(); 3],
            context: crate::node::ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(2),
                entrypoint_ref: None,
                managed_workspace_commit_id: Some(oid(3)),
                traversal_tips: Vec::new(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: ProjectMeta::default(),
                symbolic_remote_names: Vec::new(),
            },
        };
        let child_slots = commit_child_slots(&graph, 0);

        assert_eq!(child_slots, [(1, 0)]);
        assert_eq!(
            placement_slot(
                &graph,
                0,
                Some(PlacementHint {
                    anchor: 0,
                    preferred_child: Some(2),
                    priority: 0,
                }),
                &child_slots,
                &BTreeSet::new(),
                true,
            ),
            Some((1, 0)),
            "the stack root is inserted below the first child on the path to the workspace commit"
        );
    }

    #[test]
    fn unique_ancestry_path_handles_deep_history_without_recursion() {
        const DEPTH: usize = 25_000;
        let id = gix::ObjectId::from_hex(format!("{:040x}", 1).as_bytes())
            .expect("valid test object id");
        let nodes = (0..DEPTH)
            .map(|index| Node {
                kind: NodeKind::Commit { id },
                parents: index.checked_sub(1).into_iter().collect(),
            })
            .collect::<Vec<_>>();
        let mut graph = NodeGraph {
            annotations: vec![CommitFlags::empty(); nodes.len()],
            nodes,
            context: crate::node::ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(DEPTH - 1),
                entrypoint_ref: None,
                managed_workspace_commit_id: None,
                traversal_tips: Vec::new(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: ProjectMeta::default(),
                symbolic_remote_names: Vec::new(),
            },
        };

        assert!(has_unique_ancestry_path(&graph, DEPTH - 1, 0));
        graph.nodes[DEPTH - 1].parents.push(DEPTH - 2);
        assert!(
            !has_unique_ancestry_path(&graph, DEPTH - 1, 0),
            "path counts are capped but duplicate ancestry remains ambiguous"
        );
    }

    #[test]
    fn nested_workspace_orders_form_one_effective_ancestry_order() {
        let oid = |value: u8| {
            gix::ObjectId::from_hex(format!("{value:040x}").as_bytes())
                .expect("valid test object id")
        };
        let graph = NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id: oid(1) },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(2) },
                    parents: vec![0],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(3) },
                    parents: vec![1],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(4) },
                    parents: vec![0],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(5) },
                    parents: vec![2, 3],
                },
            ],
            annotations: vec![CommitFlags::empty(); 5],
            context: crate::node::ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(4),
                entrypoint_ref: None,
                managed_workspace_commit_id: Some(oid(5)),
                traversal_tips: Vec::new(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: ProjectMeta::default(),
                symbolic_remote_names: Vec::new(),
            },
        };
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let mut orders = vec![
            ReferenceOrder {
                names: ["above-bottom", "bottom"]
                    .map(|branch| name(&format!("refs/heads/{branch}")))
                    .into(),
                anchor: 1,
                preferred_child: Some(4),
                workspace_root: false,
                place_on_ancestry: false,
            },
            ReferenceOrder {
                names: ["above-A-commit", "above-A", "A", "below-A-commit"]
                    .map(|branch| name(&format!("refs/heads/{branch}")))
                    .into(),
                anchor: 2,
                preferred_child: Some(4),
                workspace_root: false,
                place_on_ancestry: false,
            },
            ReferenceOrder {
                names: vec![name("refs/heads/B")],
                anchor: 3,
                preferred_child: Some(4),
                workspace_root: false,
                place_on_ancestry: false,
            },
        ];
        let mut workspace_orders = vec![
            WorkspaceOrder {
                workspace_name: workspace_name.clone(),
                order_index: 0,
            },
            WorkspaceOrder {
                workspace_name: workspace_name.clone(),
                order_index: 1,
            },
            WorkspaceOrder {
                workspace_name,
                order_index: 2,
            },
        ];
        stitch_nested_workspace_orders(&graph, &mut orders, &mut workspace_orders);

        assert_eq!(
            orders[0]
                .names
                .iter()
                .map(|name| name.shorten().to_string())
                .collect::<Vec<_>>(),
            [
                "above-A-commit",
                "above-A",
                "A",
                "below-A-commit",
                "above-bottom",
                "bottom",
            ]
        );
        assert_eq!(orders[0].anchor, 2, "the upper commit anchors the chain");
        assert!(
            orders[1].names.is_empty(),
            "the old order no longer competes"
        );
        assert_eq!(
            workspace_orders
                .iter()
                .map(|order| order.order_index)
                .collect::<Vec<_>>(),
            [0, 2],
            "the parallel B stack remains independent"
        );
    }

    #[test]
    fn ambiguous_lower_anchor_keeps_one_workspace_stack_root() {
        let oid = |value: u8| {
            gix::ObjectId::from_hex(format!("{value:040x}").as_bytes())
                .expect("valid test object id")
        };
        let graph = NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id: oid(1) },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: oid(2) },
                    parents: vec![0],
                },
            ],
            annotations: vec![CommitFlags::empty(); 2],
            context: crate::node::ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(1),
                entrypoint_ref: None,
                managed_workspace_commit_id: None,
                traversal_tips: Vec::new(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: ProjectMeta::default(),
                symbolic_remote_names: Vec::new(),
            },
        };
        let upper = name("refs/heads/upper");
        let lower = name("refs/heads/lower");
        let alternative_lower = name("refs/heads/alternative-lower");

        assert_eq!(
            select_workspace_stack_roots(
                &graph,
                &[
                    (upper.clone(), 1),
                    (lower.clone(), 0),
                    (alternative_lower, 0),
                ],
            ),
            [upper.clone(), lower.clone()],
            "one lower root survives when ancestry cannot absorb both alternatives"
        );
        assert_eq!(
            select_workspace_stack_roots(&graph, &[(upper.clone(), 1), (lower, 0)]),
            [upper],
            "an unambiguous lower continuation remains part of the upper stack"
        );
    }

    #[test]
    fn workspace_fan_out_is_derived_from_applied_stack_order() -> Result<()> {
        let repo = scenario("detached")?;
        let tip = id(&repo, "HEAD")?;
        let (meta, workspace) = metadata(
            vec![stack(1, &["A"]), stack(2, &["B"])],
            ProjectMeta::default(),
        );
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let tips = vec![
            init::Tip::entrypoint(tip, Some(workspace_name.clone()))
                .with_role(init::TipRole::Workspace)
                .with_metadata(ReferenceMetadata::Workspace(workspace)),
            init::Tip::new(tip).with_role(init::TipRole::WorkspaceStackBranch {
                desired_ref_name: name("refs/heads/A"),
            }),
            init::Tip::new(tip).with_role(init::TipRole::WorkspaceStackBranch {
                desired_ref_name: name("refs/heads/B"),
            }),
        ];
        let overlay = init::Overlay::default().with_references([
            overlay_ref("refs/heads/gitbutler/workspace", tip),
            overlay_ref("refs/heads/A", tip),
            overlay_ref("refs/heads/B", tip),
        ]);
        let graph = construct(
            &repo,
            &meta,
            tips,
            ProjectMeta::default(),
            options(),
            overlay,
            Some(workspace_name),
        )?;

        let (_, workspace) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        let parent_names = workspace
            .parents
            .iter()
            .filter_map(|parent| match &graph.nodes[*parent].kind {
                NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.to_string()),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(parent_names, ["refs/heads/A", "refs/heads/B"]);
        assert!(
            matches!(
                graph.nodes[*workspace.parents.last().expect("own-target parent")].kind,
                NodeKind::Commit { id } if id == tip
            ),
            "workspace overlays retain one direct path to their own ref target"
        );
        Ok(())
    }

    #[test]
    fn duplicate_workspace_branch_names_do_not_form_a_reference_cycle() -> Result<()> {
        let repo = scenario("detached")?;
        let tip = id(&repo, "HEAD")?;
        let (meta, workspace) = metadata(vec![stack(1, &["B", "B"])], ProjectMeta::default());
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(tip, Some(workspace_name.clone()))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
                init::Tip::new(tip).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: name("refs/heads/B"),
                }),
            ],
            ProjectMeta::default(),
            options(),
            init::Overlay::default().with_references([
                overlay_ref("refs/heads/gitbutler/workspace", tip),
                overlay_ref("refs/heads/B", tip),
            ]),
            Some(workspace_name),
        )?;

        let (branch_index, branch) = reference_node(&graph, "refs/heads/B");
        assert!(
            !branch.parents.contains(&branch_index),
            "duplicate metadata entries must not make a reference its own parent"
        );
        let (_, workspace) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        assert_eq!(
            workspace
                .parents
                .iter()
                .filter(|parent| **parent == branch_index)
                .count(),
            1,
            "duplicate metadata entries produce one structural workspace root"
        );
        Ok(())
    }

    #[test]
    fn workspace_overlay_ignores_missing_archived_and_unapplied_roots() -> Result<()> {
        let repo = scenario("detached")?;
        let tip = id(&repo, "HEAD")?;
        let mut archived = stack(2, &["archived"]);
        archived.branches[0].archived = true;
        let mut unapplied = stack(4, &["unapplied"]);
        unapplied.workspacecommit_relation = WorkspaceCommitRelation::Outside;
        let (meta, workspace) = metadata(
            vec![
                stack(1, &["A"]),
                archived,
                stack(3, &["missing"]),
                unapplied,
            ],
            ProjectMeta::default(),
        );
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(tip, Some(workspace_name.clone()))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
            ],
            ProjectMeta::default(),
            options(),
            init::Overlay::default().with_references([
                overlay_ref("refs/heads/gitbutler/workspace", tip),
                overlay_ref("refs/heads/A", tip),
                overlay_ref("refs/heads/archived", tip),
                overlay_ref("refs/heads/unapplied", tip),
            ]),
            Some(workspace_name),
        )?;

        let (_, workspace) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        let overlay_names = workspace
            .parents
            .iter()
            .filter_map(|parent| match &graph.nodes[*parent].kind {
                NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.to_string()),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(overlay_names, ["refs/heads/A"]);
        assert!(matches!(
            graph.nodes[*workspace.parents.last().expect("own target")].kind,
            NodeKind::Commit { id } if id == tip
        ));
        Ok(())
    }

    #[test]
    fn cross_target_workspace_root_does_not_require_a_same_tip_reference() -> Result<()> {
        let repo = scenario("detached")?;
        let workspace_id = id(&repo, "HEAD")?;
        let stack_id = id(&repo, "HEAD~1")?;
        let (meta, workspace) = metadata(vec![stack(1, &["A"])], ProjectMeta::default());
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let peer_name = name("refs/heads/peer");
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(workspace_id, Some(workspace_name.clone()))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
                init::Tip::new(stack_id).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: name("refs/heads/A"),
                }),
            ],
            ProjectMeta::default(),
            options(),
            init::Overlay::default().with_references([
                overlay_ref("refs/heads/gitbutler/workspace", workspace_id),
                overlay_ref("refs/heads/peer", workspace_id),
                overlay_ref("refs/heads/A", stack_id),
            ]),
            Some(peer_name),
        )?;

        let (_, workspace) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        assert!(workspace.parents.iter().any(|parent| matches!(
            &graph.nodes[*parent].kind,
            NodeKind::Reference(reference)
                if reference.ref_info.ref_name == name("refs/heads/A")
        )));
        Ok(())
    }

    #[test]
    fn hard_limit_keeps_workspace_target_when_stack_tip_is_not_materialized() -> Result<()> {
        let repo = scenario("triple-merge")?;
        let workspace_id = id(&repo, "C")?;
        let rejected_stack_id = id(&repo, "A")?;
        let (meta, workspace) = metadata(vec![stack(1, &["A"])], ProjectMeta::default());
        let workspace_name = name("refs/heads/gitbutler/workspace");
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(workspace_id, Some(workspace_name.clone()))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
                init::Tip::new(rejected_stack_id).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: name("refs/heads/A"),
                }),
            ],
            ProjectMeta::default(),
            options().with_hard_limit(1),
            init::Overlay::default().with_references([
                overlay_ref("refs/heads/gitbutler/workspace", workspace_id),
                overlay_ref("refs/heads/A", rejected_stack_id),
            ]),
            Some(workspace_name),
        )?;

        assert!(graph.context.hard_limit_hit);
        let (_, workspace) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        assert_eq!(workspace.parents.len(), 1);
        assert!(matches!(
            graph.nodes[workspace.parents[0]].kind,
            NodeKind::Commit { id } if id == workspace_id
        ));
        Ok(())
    }

    #[test]
    fn dependent_branches_on_a_shared_base_use_their_metadata_paths() -> Result<()> {
        let repo = scenario("ws/dependent-branch-on-base")?;
        let project_meta = ProjectMeta {
            target_ref: Some(name("refs/remotes/origin/main")),
            target_commit_id: None,
            push_remote: None,
        };
        let (meta, workspace) = metadata(
            vec![
                stack(1, &["A", "below-A", "below-below-A"]),
                stack(2, &["B", "below-B", "below-below-B"]),
                stack(
                    3,
                    &[
                        "C",
                        "C2-1",
                        "C2-2",
                        "C2-3",
                        "C1-3",
                        "C1-2",
                        "C1-1",
                        "below-C",
                        "below-below-C",
                    ],
                ),
            ],
            project_meta.clone(),
        );
        let workspace_id = id(&repo, "gitbutler/workspace")?;
        let target_id = id(&repo, "origin/main")?;
        let tips = vec![
            init::Tip::entrypoint(workspace_id, Some(name("refs/heads/gitbutler/workspace")))
                .with_role(init::TipRole::Workspace)
                .with_metadata(ReferenceMetadata::Workspace(workspace)),
            init::Tip::integrated(target_id, Some(name("refs/remotes/origin/main"))),
            init::Tip::new(id(&repo, "A")?).with_role(init::TipRole::WorkspaceStackBranch {
                desired_ref_name: name("refs/heads/A"),
            }),
            init::Tip::new(id(&repo, "B")?).with_role(init::TipRole::WorkspaceStackBranch {
                desired_ref_name: name("refs/heads/B"),
            }),
            init::Tip::new(id(&repo, "C")?).with_role(init::TipRole::WorkspaceStackBranch {
                desired_ref_name: name("refs/heads/C"),
            }),
        ];
        let graph = construct(
            &repo,
            &meta,
            tips,
            project_meta,
            options(),
            init::Overlay::default(),
            Some(name("refs/heads/gitbutler/workspace")),
        )?;

        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/heads/B").1),
            Some("refs/heads/below-B".into())
        );
        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/heads/below-B").1),
            Some("refs/heads/below-below-B".into())
        );
        assert_eq!(
            parent_ref_name(&graph, commit_node(&graph, id(&repo, "A")?)),
            Some("refs/heads/below-A".into()),
            "the dependent A chain is placed on A's unique path from the shared base"
        );
        assert_eq!(
            parent_ref_name(&graph, commit_node(&graph, id(&repo, "C~1")?)),
            Some("refs/heads/below-C".into()),
            "the dependent C chain is placed on C's unique path from the shared base"
        );
        let (_, workspace_reference) = reference_node(&graph, "refs/heads/gitbutler/workspace");
        let overlay_order = workspace_reference
            .parents
            .iter()
            .filter_map(|parent| match &graph.nodes[*parent].kind {
                NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.to_string()),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            overlay_order,
            ["refs/heads/A", "refs/heads/B", "refs/heads/C"],
            "workspace overlay roots preserve applied stack metadata order"
        );
        assert!(
            matches!(
                graph.nodes[*workspace_reference
                    .parents
                    .last()
                    .expect("workspace target parent")]
                .kind,
                NodeKind::Commit { id } if id == workspace_id
            ),
            "cross-target overlays retain the workspace ref target"
        );
        let workspace_parents = commit_node(&graph, workspace_id)
            .parents
            .iter()
            .filter_map(|parent| match &graph.nodes[*parent].kind {
                NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.to_string()),
                NodeKind::Commit { .. } | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            workspace_parents,
            ["refs/heads/A", "refs/heads/B", "refs/heads/C"]
                .into_iter()
                .map(str::to_owned)
                .collect()
        );
        Ok(())
    }

    #[test]
    fn ad_hoc_three_branch_order_becomes_one_same_tip_chain() -> Result<()> {
        let repo = scenario("detached")?;
        let tip = id(&repo, "HEAD")?;
        let order = [
            name("refs/heads/top"),
            name("refs/heads/middle"),
            name("refs/heads/bottom"),
        ];
        let overlay = init::Overlay::default()
            .with_references(order.iter().map(|name| gix::refs::Reference {
                name: name.clone(),
                target: Target::Object(tip),
                peeled: Some(tip),
            }))
            .with_branch_stack_order_override(order.clone());
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![init::Tip::entrypoint(tip, Some(order[0].clone()))],
            ProjectMeta::default(),
            options(),
            overlay,
            Some(order[0].clone()),
        )?;

        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/heads/top").1),
            Some("refs/heads/middle".into())
        );
        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/heads/middle").1),
            Some("refs/heads/bottom".into())
        );
        assert_eq!(
            graph.entrypoint(),
            &crate::NodeGraphEntrypoint::Node(reference_node(&graph, "refs/heads/top").0)
        );
        Ok(())
    }

    #[test]
    fn explicit_tag_is_preserved_when_tag_collection_is_disabled() -> Result<()> {
        let repo = scenario("detached")?;
        let tip = id(&repo, "refs/tags/release/v1")?;
        let tag = name("refs/tags/release/v1");
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![init::Tip::entrypoint(tip, Some(tag.clone()))],
            ProjectMeta::default(),
            init::Options::default(),
            init::Overlay::default(),
            Some(tag.clone()),
        )?;

        let (tag_index, tag_node) = reference_node(&graph, "refs/tags/release/v1");
        let NodeKind::Reference(reference) = &tag_node.kind else {
            unreachable!("looked up a reference")
        };
        assert_eq!(reference.ref_info.commit_id, Some(tip));
        assert_eq!(
            graph.entrypoint(),
            &crate::NodeGraphEntrypoint::Node(tag_index),
            "the named entrypoint moves from its commit to the explicitly seeded tag"
        );
        Ok(())
    }

    #[test]
    fn custom_namespace_tip_keeps_its_target_and_attached_metadata() -> Result<()> {
        let repo = scenario("four-diamond")?;
        let head = id(&repo, "merged")?;
        let custom_tip = id(&repo, "C~1")?;
        let custom = name("refs/custom/review-tip");
        let metadata = ReferenceMetadata::Branch(Default::default());
        let overlay = init::Overlay::default()
            .with_references([overlay_ref("refs/custom/review-tip", custom_tip)]);
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![
                init::Tip::entrypoint(head, Some(name("refs/heads/merged"))),
                init::Tip::reachable(custom_tip, Some(custom.clone()))
                    .with_metadata(metadata.clone()),
            ],
            ProjectMeta::default(),
            init::Options::default(),
            overlay,
            Some(name("refs/heads/merged")),
        )?;

        let (_, node) = reference_node(&graph, "refs/custom/review-tip");
        let NodeKind::Reference(reference) = &node.kind else {
            unreachable!("looked up a reference")
        };
        assert_eq!(reference.ref_info.commit_id, Some(custom_tip));
        assert_eq!(reference.metadata, Some(metadata.clone()));
        Ok(())
    }

    #[test]
    fn same_tip_target_local_and_remote_share_one_commit_owner() -> Result<()> {
        let mut repo = scenario("ws/duplicate-workspace-connection-no-target")?;
        let workspace_id = id(&repo, "gitbutler/workspace")?;
        let target_id = id(&repo, "origin/main")?;
        let tips = vec![
            init::Tip::entrypoint(workspace_id, Some(name("refs/heads/gitbutler/workspace"))),
            init::Tip::integrated(target_id, Some(name("refs/remotes/origin/main"))),
            init::Tip::new(target_id).with_role(init::TipRole::TargetLocal {
                local_ref_name: name("refs/heads/main"),
            }),
        ];
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            tips.clone(),
            ProjectMeta::default(),
            options(),
            init::Overlay::default(),
            Some(name("refs/heads/gitbutler/workspace")),
        )?;

        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/remotes/origin/main").1),
            Some("refs/heads/main".into())
        );
        repo.config_snapshot_mut()
            .set_raw_value("branch.aaa-main.remote", "origin")?;
        repo.config_snapshot_mut()
            .set_raw_value("branch.aaa-main.merge", "refs/heads/main")?;
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            tips,
            ProjectMeta::default(),
            options(),
            init::Overlay::default()
                .with_references([overlay_ref("refs/heads/aaa-main", target_id)]),
            Some(name("refs/heads/gitbutler/workspace")),
        )?;
        let (_, competing_local) = reference_node(&graph, "refs/heads/aaa-main");
        let NodeKind::Reference(competing_local) = &competing_local.kind else {
            unreachable!("looked up a reference")
        };
        assert_eq!(
            competing_local.remote_tracking_ref_name,
            Some(name("refs/remotes/origin/main")),
            "the earlier-sorting local must be a real pairing competitor"
        );
        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/remotes/origin/main").1),
            Some("refs/heads/main".into()),
            "the explicit target-local role wins over alphabetical local order"
        );
        Ok(())
    }

    #[test]
    fn detached_entrypoint_keeps_tags_outside() -> Result<()> {
        let repo = scenario("detached")?;
        let head = id(&repo, "HEAD")?;
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![init::Tip::detached_entrypoint(head)],
            ProjectMeta::default(),
            options(),
            init::Overlay::default(),
            None,
        )?;

        assert!(matches!(
            graph.nodes[*match graph.entrypoint() {
                crate::NodeGraphEntrypoint::Node(index) => index,
                crate::NodeGraphEntrypoint::Unborn(_) => panic!("detached HEAD is born"),
            }]
            .kind,
            NodeKind::Commit { .. }
        ));
        for tag in ["refs/tags/release/v1", "refs/tags/annotated"] {
            let (_, node) = reference_node(&graph, tag);
            assert_eq!(graph.child_counts()[reference_node(&graph, tag).0], 0);
            assert_eq!(node.parents.len(), 1);
        }
        Ok(())
    }

    #[test]
    fn overlay_add_and_drop_are_applied_during_discovery() -> Result<()> {
        let repo = scenario("four-diamond")?;
        let head = id(&repo, "merged")?;
        let replacement_tip = id(&repo, "C~1")?;
        let overlay = init::Overlay::default()
            .with_references([overlay_ref("refs/heads/new-reference", replacement_tip)])
            .with_dropped_references([name("refs/heads/C")]);
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![init::Tip::entrypoint(head, Some(name("refs/heads/merged")))],
            ProjectMeta::default(),
            options(),
            overlay,
            Some(name("refs/heads/merged")),
        )?;

        reference_node(&graph, "refs/heads/new-reference");
        assert!(graph.nodes.iter().all(|node| !matches!(
            &node.kind,
            NodeKind::Reference(reference)
                if reference.ref_info.ref_name == name("refs/heads/C")
        )));
        Ok(())
    }

    #[test]
    fn ambiguous_unhinted_refs_at_an_interior_commit_all_stay_outside() -> Result<()> {
        let repo = scenario("four-diamond")?;
        let head = id(&repo, "merged")?;
        let interior = id(&repo, "C~1")?;
        let overlay = init::Overlay::default().with_references([
            overlay_ref("refs/heads/alpha", interior),
            overlay_ref("refs/heads/beta", interior),
        ]);
        let graph = construct(
            &repo,
            &InMemoryRefMetadata::default(),
            vec![init::Tip::entrypoint(head, Some(name("refs/heads/merged")))],
            ProjectMeta::default(),
            options(),
            overlay,
            Some(name("refs/heads/merged")),
        )?;

        let children = graph.child_counts();
        let (alpha, _) = reference_node(&graph, "refs/heads/alpha");
        let (beta, _) = reference_node(&graph, "refs/heads/beta");
        assert_eq!((children[alpha], children[beta]), (0, 0));
        let interior_index = graph
            .nodes
            .iter()
            .position(|node| matches!(node.kind, NodeKind::Commit { id } if id == interior))
            .expect("interior commit");
        assert!(
            graph.nodes.iter().any(|node| {
                matches!(node.kind, NodeKind::Commit { .. })
                    && node.parents.contains(&interior_index)
            }),
            "the original commit edge remains inline instead of choosing a ref alphabetically"
        );
        Ok(())
    }

    #[test]
    fn remote_pairing_can_share_a_local_ref_used_by_a_metadata_chain() -> Result<()> {
        let mut repo = scenario("ws/duplicate-workspace-connection-no-target")?;
        let workspace_id = id(&repo, "gitbutler/workspace")?;
        let a_id = id(&repo, "A")?;
        let base_id = id(&repo, "main")?;
        repo.config_snapshot_mut()
            .set_raw_value("branch.A.remote", "origin")?;
        repo.config_snapshot_mut()
            .set_raw_value("branch.A.merge", "refs/heads/A")?;
        let (meta, workspace) = metadata(vec![stack(1, &["A", "main"])], ProjectMeta::default());
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(workspace_id, Some(name("refs/heads/gitbutler/workspace")))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
                init::Tip::new(base_id).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: name("refs/heads/A"),
                }),
                init::Tip::integrated(base_id, Some(name("refs/remotes/origin/main"))),
                init::Tip::new(base_id).with_role(init::TipRole::TargetLocal {
                    local_ref_name: name("refs/heads/main"),
                }),
            ],
            ProjectMeta::default(),
            options(),
            init::Overlay::default().with_references([overlay_ref("refs/remotes/origin/A", a_id)]),
            Some(name("refs/heads/gitbutler/workspace")),
        )?;

        let (local_index, local) = reference_node(&graph, "refs/heads/main");
        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/heads/A").1),
            Some("refs/heads/main".into()),
            "the local top branch stays directly above the local bottom branch"
        );
        assert_eq!(
            parent_ref_name(&graph, reference_node(&graph, "refs/remotes/origin/main").1),
            Some("refs/heads/main".into())
        );
        assert_eq!(graph.child_counts()[local_index], 2);
        for remote in ["refs/remotes/origin/A", "refs/remotes/origin/main"] {
            let (remote, _) = reference_node(&graph, remote);
            assert_eq!(
                graph.child_counts()[remote],
                0,
                "paired remotes remain outside the workspace ancestry"
            );
        }
        assert_eq!(
            local.parents.len(),
            1,
            "the shared local ref remains acyclic"
        );
        Ok(())
    }

    #[test]
    fn duplicate_merge_parent_slots_are_replaced_one_at_a_time() -> Result<()> {
        let repo = scenario("ws/duplicate-workspace-connection-no-target")?;
        let workspace_id = id(&repo, "gitbutler/workspace")?;
        let base_id = id(&repo, "main")?;
        let (meta, workspace) = metadata(vec![stack(1, &["A"])], ProjectMeta::default());
        let graph = construct(
            &repo,
            &meta,
            vec![
                init::Tip::entrypoint(workspace_id, Some(name("refs/heads/gitbutler/workspace")))
                    .with_role(init::TipRole::Workspace)
                    .with_metadata(ReferenceMetadata::Workspace(workspace)),
                init::Tip::new(base_id).with_role(init::TipRole::WorkspaceStackBranch {
                    desired_ref_name: name("refs/heads/A"),
                }),
            ],
            ProjectMeta::default(),
            options(),
            init::Overlay::default(),
            Some(name("refs/heads/gitbutler/workspace")),
        )?;

        let workspace_commit = graph
            .nodes
            .iter()
            .find(|node| matches!(node.kind, NodeKind::Commit { id } if id == workspace_id))
            .context("workspace commit node")?;
        assert_eq!(workspace_commit.parents.len(), 2);
        let a_ref = reference_node(&graph, "refs/heads/A").0;
        assert_eq!(
            workspace_commit
                .parents
                .iter()
                .filter(|parent| **parent == a_ref)
                .count(),
            1,
            "one metadata stack claims one duplicate parent slot"
        );
        ensure!(
            workspace_commit.parents.iter().any(|parent| matches!(
                graph.nodes[*parent].kind,
                NodeKind::Commit { id } if id == base_id
            )),
            "the other duplicate parent slot remains a direct commit edge"
        );
        Ok(())
    }
}
