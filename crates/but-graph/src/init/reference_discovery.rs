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
    ad_hoc_branch_stack_orders: &[Vec<gix::refs::FullName>],
) -> Result<NodeGraph> {
    let references = discover_references(&graph, repo, meta)?;
    let groups = build_reference_groups(&graph, references, ad_hoc_branch_stack_orders)?;
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
        .filter_map(|node| node.kind.addressable_commit_id())
        .collect::<BTreeSet<_>>();
    let refs_by_id =
        repo.collect_ref_mapping_by_prefix(["refs/heads/", "refs/remotes/"].into_iter(), &[])?;
    let worktree_by_branch = repo.worktree_branches(
        graph
            .context
            .entrypoint_ref
            .as_ref()
            .map(|name| name.as_ref()),
    )?;
    let effective_upstreams = remotes::effective_remote_tracking_branches(repo)?;
    let refs = refs_by_id
        .into_iter()
        .filter(|(id, _)| commit_ids.contains(id))
        .flat_map(|(id, names)| names.into_iter().map(move |name| (name, (id, None))))
        .collect::<BTreeMap<_, _>>();

    refs.into_iter()
        .map(|(ref_name, (commit_id, tip_metadata))| {
            let metadata = match tip_metadata {
                Some(metadata) => Some(metadata),
                None => metadata_for_ref(meta, ref_name.as_ref())?,
            };
            let remote_tracking_ref_name = if ref_name.category() == Some(Category::LocalBranch) {
                effective_upstreams.get(&ref_name).cloned()
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
    ad_hoc_branch_stack_orders: &[Vec<gix::refs::FullName>],
) -> Result<Vec<ReferenceGroup>> {
    let commit_by_id = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| node.kind.addressable_commit_id().map(|id| (id, index)))
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
    let (orders, workspace_orders) = reference_orders(
        graph,
        &references,
        &reference_target,
        &commit_by_id,
        ad_hoc_branch_stack_orders,
    );
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
    ad_hoc_branch_stack_orders: &[Vec<gix::refs::FullName>],
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
            || (!graph.annotations[anchor].contains(CommitFlags::TargetSide)
                && reaches(graph, workspace_node, anchor))
            || (graph.annotations[anchor].contains(CommitFlags::TargetSide)
                && matches!(
                    graph.nodes[anchor].kind,
                    NodeKind::Commit { id }
                        if Some(id) == graph.context.project_meta.target_commit_id
                ))
            || (graph.annotations[anchor].contains(CommitFlags::TargetSide)
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
            && (!graph.annotations[anchor].contains(CommitFlags::TargetSide)
                || direct_workspace_parent);
    }

    for names in ad_hoc_branch_stack_orders {
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
                if graph.annotations[upper_anchor].contains(CommitFlags::TargetSide)
                    || graph.annotations[lower_anchor].contains(CommitFlags::TargetSide)
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

    let authoritative_target_locals = references
        .iter()
        .filter(|reference| {
            reference.remote_tracking_ref_name.as_ref()
                == graph.context.project_meta.target_ref.as_ref()
        })
        .map(|reference| &reference.ref_info.ref_name)
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
            if graph.annotations[*anchor].contains(CommitFlags::TargetSide) {
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
