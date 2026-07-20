use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context as _, Result, ensure};

use crate::{CommitFlags, Node, NodeGraph, NodeIndex, NodeKind, Reference};

type ReferenceIndex = usize;

#[derive(Debug, Clone)]
pub(super) struct ReferenceGroup {
    pub parent: NodeIndex,
    pub references: Vec<GroupedReference>,
    pub children: Vec<ReferenceGroupChild>,
}

#[derive(Debug, Clone)]
pub(super) struct GroupedReference {
    pub reference: Reference,
    pub parents: Vec<ReferenceGroupParent>,
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum ReferenceGroupParent {
    Commit,
    Reference(ReferenceIndex),
    ReferenceByName(gix::refs::FullName),
}

#[derive(Debug, Clone)]
pub(super) struct ReferenceGroupChild {
    pub child: ReferenceGroupChildKind,
    pub parents: Vec<ReferenceIndex>,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub(super) enum ReferenceGroupChildKind {
    Commit {
        index: NodeIndex,
        parent_order: usize,
    },
    Outside,
}

/// Append the references described by `groups` and place them between existing commit nodes.
///
/// Groups are validated before the input graph is changed. `Outside` is only a construction
/// marker: references attached to it become graph roots and need no persistent context entry.
pub(super) fn apply_reference_groups(
    graph: NodeGraph,
    groups: Vec<ReferenceGroup>,
) -> Result<NodeGraph> {
    let mut graph = graph.validated()?;
    validate_groups(&graph, &groups)?;

    let mut replacements = BTreeMap::<NodeIndex, BTreeMap<usize, NodeIndex>>::new();
    let mut next_index = graph.nodes.len();
    let mut reference_nodes = graph
        .nodes
        .iter()
        .enumerate()
        .filter_map(|(index, node)| match &node.kind {
            NodeKind::Reference(reference) => Some((reference.ref_info.ref_name.clone(), index)),
            NodeKind::Commit { .. } | NodeKind::Boundary { .. } | NodeKind::None => None,
        })
        .collect::<BTreeMap<_, _>>();
    let first_references = groups
        .iter()
        .map(|group| {
            let first = next_index;
            for grouped in &group.references {
                reference_nodes.insert(grouped.reference.ref_info.ref_name.clone(), next_index);
                next_index += 1;
            }
            first
        })
        .collect::<Vec<_>>();

    for (group, first_reference) in groups.into_iter().zip(first_references) {
        for grouped in group.references {
            let parents = grouped
                .parents
                .into_iter()
                .map(|parent| match parent {
                    ReferenceGroupParent::Commit => group.parent,
                    ReferenceGroupParent::Reference(index) => first_reference + index,
                    ReferenceGroupParent::ReferenceByName(name) => reference_nodes[&name],
                })
                .collect();
            graph.nodes.push(Node {
                kind: NodeKind::Reference(Box::new(grouped.reference)),
                parents,
            });
            graph.annotations.push(CommitFlags::empty());
        }

        for child in group.children {
            let ReferenceGroupChildKind::Commit {
                index,
                parent_order,
            } = child.child
            else {
                continue;
            };
            let parent = child.parents[0];
            replacements
                .entry(index)
                .or_default()
                .insert(parent_order, first_reference + parent);
        }
    }

    for (child, replacements) in replacements {
        let old_parents = std::mem::take(&mut graph.nodes[child].parents);
        graph.nodes[child].parents = old_parents
            .into_iter()
            .enumerate()
            .map(|(order, parent)| replacements.get(&order).copied().unwrap_or(parent))
            .collect();
    }

    graph.validated()
}

fn validate_groups(graph: &NodeGraph, groups: &[ReferenceGroup]) -> Result<()> {
    let mut group_parents = BTreeSet::new();
    let mut reference_names = graph
        .nodes
        .iter()
        .filter_map(|node| match &node.kind {
            NodeKind::Reference(reference) => Some(reference.ref_info.ref_name.clone()),
            NodeKind::Commit { .. } | NodeKind::Boundary { .. } | NodeKind::None => None,
        })
        .collect::<BTreeSet<_>>();
    let mut claimed_slots = BTreeSet::new();
    for group in groups {
        for grouped in &group.references {
            ensure!(
                reference_names.insert(grouped.reference.ref_info.ref_name.clone()),
                "BUG: reference {} appears in more than one node",
                grouped.reference.ref_info.ref_name
            );
        }
    }

    for group in groups {
        ensure!(
            group_parents.insert(group.parent),
            "BUG: commit node {} has more than one reference group",
            group.parent
        );
        let parent = graph.nodes.get(group.parent).with_context(|| {
            format!(
                "BUG: reference-group parent {} is out of bounds",
                group.parent
            )
        })?;
        let Some(parent_id) = parent.kind.addressable_commit_id() else {
            anyhow::bail!(
                "BUG: reference-group parent {} is not an addressable commit",
                group.parent
            );
        };
        ensure!(
            !group.references.is_empty(),
            "BUG: reference group at commit {parent_id} is empty"
        );

        let mut incoming = vec![0usize; group.references.len()];
        let local_reference_index = group
            .references
            .iter()
            .enumerate()
            .map(|(index, grouped)| (grouped.reference.ref_info.ref_name.clone(), index))
            .collect::<BTreeMap<_, _>>();
        for (index, grouped) in group.references.iter().enumerate() {
            ensure!(
                grouped.reference.ref_info.commit_id == Some(parent_id),
                "BUG: grouped reference {} targets {:?}, not parent commit {parent_id}",
                grouped.reference.ref_info.ref_name,
                grouped.reference.ref_info.commit_id
            );
            ensure!(
                !grouped.parents.is_empty(),
                "BUG: grouped reference {index} has no parents"
            );
            ensure!(
                grouped
                    .parents
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    .len()
                    == grouped.parents.len(),
                "BUG: grouped reference {index} has duplicate parents"
            );
            let is_workspace = matches!(
                grouped.reference.metadata,
                Some(crate::ReferenceMetadata::Workspace(_))
            );
            if grouped.parents.len() > 1 {
                ensure!(
                    is_workspace,
                    "BUG: ordinary grouped reference {index} has more than one parent"
                );
            }
            if is_workspace {
                ensure!(
                    grouped
                        .parents
                        .iter()
                        .filter(|parent| matches!(parent, ReferenceGroupParent::Commit))
                        .count()
                        == 1,
                    "BUG: workspace grouped reference {index} must retain exactly one own-target commit parent"
                );
                ensure!(
                    matches!(grouped.parents.last(), Some(ReferenceGroupParent::Commit)),
                    "BUG: workspace grouped reference {index} must keep its own-target commit parent last"
                );
            }
            for parent in &grouped.parents {
                match parent {
                    ReferenceGroupParent::Commit => {}
                    ReferenceGroupParent::Reference(parent) => {
                        ensure!(
                            *parent < group.references.len(),
                            "BUG: grouped reference {index} has out-of-bounds reference parent {parent}"
                        );
                        incoming[*parent] += 1;
                    }
                    ReferenceGroupParent::ReferenceByName(name) => {
                        ensure!(
                            is_workspace,
                            "BUG: ordinary grouped reference {index} has cross-group reference parent {name}"
                        );
                        ensure!(
                            reference_names.contains(name),
                            "BUG: grouped reference {index} has unknown cross-group reference parent {name}"
                        );
                        if let Some(parent) = local_reference_index.get(name) {
                            incoming[*parent] += 1;
                        }
                    }
                }
            }
        }

        let mut outside = BTreeSet::new();
        for child in &group.children {
            ensure!(
                !child.parents.is_empty(),
                "BUG: reference-group child has no reference parents"
            );
            ensure!(
                child.parents.iter().copied().collect::<BTreeSet<_>>().len() == child.parents.len(),
                "BUG: reference-group child has duplicate reference parents"
            );
            for &parent in &child.parents {
                ensure!(
                    parent < group.references.len(),
                    "BUG: reference-group child has out-of-bounds reference parent {parent}"
                );
                incoming[parent] += 1;
            }

            match child.child {
                ReferenceGroupChildKind::Outside => {
                    for &parent in &child.parents {
                        ensure!(
                            outside.insert(parent),
                            "BUG: grouped reference {parent} is marked Outside more than once"
                        );
                    }
                }
                ReferenceGroupChildKind::Commit {
                    index,
                    parent_order,
                } => {
                    ensure!(
                        child.parents.len() == 1,
                        "BUG: child commit {index} parent slot {parent_order} has {} reference parents instead of one",
                        child.parents.len()
                    );
                    let commit = graph.nodes.get(index).with_context(|| {
                        format!("BUG: reference-group child commit {index} is out of bounds")
                    })?;
                    ensure!(
                        matches!(commit.kind, NodeKind::Commit { .. }),
                        "BUG: reference-group child {index} is not a commit"
                    );
                    ensure!(
                        commit.parents.get(parent_order) == Some(&group.parent),
                        "BUG: child commit {index} parent slot {parent_order} does not point to group parent {}",
                        group.parent
                    );
                    ensure!(
                        claimed_slots.insert((index, parent_order)),
                        "BUG: child commit {index} parent slot {parent_order} is claimed more than once"
                    );
                }
            }
        }

        for (index, incoming) in incoming.into_iter().enumerate() {
            ensure!(
                incoming > 0,
                "BUG: grouped reference {index} has no child placement"
            );
            if outside.contains(&index) {
                ensure!(
                    incoming == 1,
                    "BUG: grouped reference {index} is both inline and Outside"
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeGraphEntrypoint, RefInfo, ReferenceMetadata, node::ConstructionContext};

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

    fn reference(name: &str, id: gix::ObjectId) -> Reference {
        Reference {
            ref_info: RefInfo {
                ref_name: name.try_into().expect("valid full ref name"),
                commit_id: Some(id),
                worktree: None,
            },
            metadata: None,
            remote_tracking_ref_name: None,
        }
    }

    fn grouped(reference: Reference, parents: Vec<ReferenceGroupParent>) -> GroupedReference {
        GroupedReference { reference, parents }
    }

    fn child(child: ReferenceGroupChildKind, parents: Vec<ReferenceIndex>) -> ReferenceGroupChild {
        ReferenceGroupChild { child, parents }
    }

    fn graph(nodes: Vec<Node>, entrypoint: NodeIndex, entrypoint_ref: Option<&str>) -> NodeGraph {
        let NodeKind::Commit { .. } = nodes[entrypoint].kind else {
            panic!("test entrypoint must be a commit")
        };
        NodeGraph {
            annotations: vec![CommitFlags::empty(); nodes.len()],
            nodes,
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(entrypoint),
                entrypoint_ref: entrypoint_ref
                    .map(|name| name.try_into().expect("valid full ref name")),
                managed_workspace_commit_id: None,
                project_meta: Default::default(),
            },
        }
    }

    #[test]
    fn applies_same_tip_reference_chain_and_keeps_annotations_parallel() -> Result<()> {
        let id = oid(1);
        let mut graph = graph(vec![commit(id, vec![])], 0, None);
        graph.annotations[0] = CommitFlags::TargetSide;
        let graph = apply_reference_groups(
            graph,
            vec![ReferenceGroup {
                parent: 0,
                references: vec![
                    grouped(
                        reference("refs/heads/bottom", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                    grouped(
                        reference("refs/heads/top", id),
                        vec![ReferenceGroupParent::Reference(0)],
                    ),
                ],
                children: vec![child(ReferenceGroupChildKind::Outside, vec![1])],
            }],
        )?;

        assert_eq!(graph.nodes[1].parents, [0], "bottom points to the commit");
        assert_eq!(graph.nodes[2].parents, [1], "top points to bottom");
        assert_eq!(
            graph.annotations,
            [
                CommitFlags::TargetSide,
                CommitFlags::empty(),
                CommitFlags::empty()
            ]
        );
        Ok(())
    }

    #[test]
    fn applies_workspace_reference_fan_out() -> Result<()> {
        let id = oid(1);
        let mut workspace = reference("refs/heads/gitbutler/workspace", id);
        workspace.metadata = Some(ReferenceMetadata::Workspace(Default::default()));
        let graph = apply_reference_groups(
            graph(vec![commit(id, vec![])], 0, None),
            vec![ReferenceGroup {
                parent: 0,
                references: vec![
                    grouped(
                        reference("refs/heads/A", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                    grouped(
                        reference("refs/heads/B", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                    grouped(
                        workspace,
                        vec![
                            ReferenceGroupParent::ReferenceByName(
                                "refs/heads/A".try_into().expect("valid full ref name"),
                            ),
                            ReferenceGroupParent::ReferenceByName(
                                "refs/heads/B".try_into().expect("valid full ref name"),
                            ),
                            ReferenceGroupParent::Commit,
                        ],
                    ),
                ],
                children: vec![child(ReferenceGroupChildKind::Outside, vec![2])],
            }],
        )?;

        assert_eq!(graph.nodes[3].parents, [1, 2, 0]);
        assert_eq!(graph.child_counts(), [3, 1, 1, 0]);
        Ok(())
    }

    #[test]
    fn applies_cross_target_workspace_roots_in_declared_order() -> Result<()> {
        let base = oid(1);
        let workspace_id = oid(2);
        let mut workspace = reference("refs/heads/gitbutler/workspace", workspace_id);
        workspace.metadata = Some(ReferenceMetadata::Workspace(Default::default()));
        let graph = apply_reference_groups(
            graph(
                vec![commit(base, vec![]), commit(workspace_id, vec![0])],
                1,
                Some("refs/heads/gitbutler/workspace"),
            ),
            vec![
                ReferenceGroup {
                    parent: 0,
                    references: vec![
                        grouped(
                            reference("refs/heads/A", base),
                            vec![ReferenceGroupParent::Commit],
                        ),
                        grouped(
                            reference("refs/heads/B", base),
                            vec![ReferenceGroupParent::Commit],
                        ),
                    ],
                    children: vec![child(ReferenceGroupChildKind::Outside, vec![0, 1])],
                },
                ReferenceGroup {
                    parent: 1,
                    references: vec![grouped(
                        workspace,
                        vec![
                            ReferenceGroupParent::ReferenceByName(
                                "refs/heads/B".try_into().expect("valid full ref name"),
                            ),
                            ReferenceGroupParent::ReferenceByName(
                                "refs/heads/A".try_into().expect("valid full ref name"),
                            ),
                            ReferenceGroupParent::Commit,
                        ],
                    )],
                    children: vec![child(ReferenceGroupChildKind::Outside, vec![0])],
                },
            ],
        )?;

        assert_eq!(graph.nodes[4].parents, [3, 2, 1]);
        assert_eq!(graph.entrypoint(), &NodeGraphEntrypoint::Node(1));
        Ok(())
    }

    #[test]
    fn rejects_named_overlay_parents_on_ordinary_refs_and_missing_workspace_targets() {
        let id = oid(1);
        let name = "refs/heads/main".try_into().expect("valid full ref name");
        let ordinary = ReferenceGroup {
            parent: 0,
            references: vec![grouped(
                reference("refs/heads/main", id),
                vec![ReferenceGroupParent::ReferenceByName(name)],
            )],
            children: vec![child(ReferenceGroupChildKind::Outside, vec![0])],
        };
        assert!(
            apply_reference_groups(graph(vec![commit(id, vec![])], 0, None), vec![ordinary])
                .unwrap_err()
                .to_string()
                .contains("ordinary grouped reference 0 has cross-group reference parent")
        );

        let mut workspace = reference("refs/heads/gitbutler/workspace", id);
        workspace.metadata = Some(ReferenceMetadata::Workspace(Default::default()));
        let missing_target = ReferenceGroup {
            parent: 0,
            references: vec![
                grouped(
                    reference("refs/heads/A", id),
                    vec![ReferenceGroupParent::Commit],
                ),
                grouped(
                    workspace,
                    vec![ReferenceGroupParent::ReferenceByName(
                        "refs/heads/A".try_into().expect("valid full ref name"),
                    )],
                ),
            ],
            children: vec![child(ReferenceGroupChildKind::Outside, vec![0, 1])],
        };
        assert!(
            apply_reference_groups(
                graph(vec![commit(id, vec![])], 0, None),
                vec![missing_target]
            )
            .unwrap_err()
            .to_string()
            .contains("must retain exactly one own-target commit parent")
        );
    }

    #[test]
    fn replaces_the_exact_duplicate_parent_slot() -> Result<()> {
        let id = oid(1);
        let child_id = oid(2);
        let graph = apply_reference_groups(
            graph(
                vec![commit(id, vec![]), commit(child_id, vec![0, 0])],
                1,
                None,
            ),
            vec![ReferenceGroup {
                parent: 0,
                references: vec![grouped(
                    reference("refs/heads/inline", id),
                    vec![ReferenceGroupParent::Commit],
                )],
                children: vec![child(
                    ReferenceGroupChildKind::Commit {
                        index: 1,
                        parent_order: 1,
                    },
                    vec![0],
                )],
            }],
        )?;

        assert_eq!(graph.nodes[1].parents, [0, 2]);
        Ok(())
    }

    #[test]
    fn keeps_outside_refs_as_roots_while_replacing_inline_edges() -> Result<()> {
        let id = oid(1);
        let child_id = oid(2);
        let graph = apply_reference_groups(
            graph(vec![commit(id, vec![]), commit(child_id, vec![0])], 1, None),
            vec![ReferenceGroup {
                parent: 0,
                references: vec![
                    grouped(
                        reference("refs/heads/inline", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                    grouped(
                        reference("refs/tags/outside-1", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                    grouped(
                        reference("refs/tags/outside-2", id),
                        vec![ReferenceGroupParent::Commit],
                    ),
                ],
                children: vec![
                    child(
                        ReferenceGroupChildKind::Commit {
                            index: 1,
                            parent_order: 0,
                        },
                        vec![0],
                    ),
                    child(ReferenceGroupChildKind::Outside, vec![1, 2]),
                ],
            }],
        )?;

        assert_eq!(graph.nodes[1].parents, [2]);
        assert_eq!(graph.child_counts(), [3, 0, 1, 0, 0]);
        Ok(())
    }

    #[test]
    fn keeps_the_entrypoint_on_its_commit() -> Result<()> {
        let id = oid(1);
        let graph = apply_reference_groups(
            graph(vec![commit(id, vec![])], 0, Some("refs/heads/main")),
            vec![ReferenceGroup {
                parent: 0,
                references: vec![grouped(
                    reference("refs/heads/main", id),
                    vec![ReferenceGroupParent::Commit],
                )],
                children: vec![child(ReferenceGroupChildKind::Outside, vec![0])],
            }],
        )?;

        assert_eq!(graph.entrypoint(), &NodeGraphEntrypoint::Node(0));
        assert_eq!(
            graph.entrypoint_ref().map(ToString::to_string),
            Some("refs/heads/main".to_owned())
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_and_ambiguous_groups() {
        let id = oid(1);
        let child_id = oid(2);
        let ambiguous = ReferenceGroup {
            parent: 0,
            references: vec![grouped(
                reference("refs/heads/main", id),
                vec![ReferenceGroupParent::Commit],
            )],
            children: vec![
                child(
                    ReferenceGroupChildKind::Commit {
                        index: 1,
                        parent_order: 0,
                    },
                    vec![0],
                ),
                child(
                    ReferenceGroupChildKind::Commit {
                        index: 1,
                        parent_order: 0,
                    },
                    vec![0],
                ),
            ],
        };
        let err = apply_reference_groups(
            graph(vec![commit(id, vec![]), commit(child_id, vec![0])], 1, None),
            vec![ambiguous],
        )
        .expect_err("one parent slot cannot have two independent placements");
        assert!(err.to_string().contains("claimed more than once"));

        let invalid = ReferenceGroup {
            parent: 0,
            references: vec![grouped(
                reference("refs/heads/main", id),
                vec![ReferenceGroupParent::Reference(1)],
            )],
            children: vec![child(ReferenceGroupChildKind::Outside, vec![0])],
        };
        let err = apply_reference_groups(graph(vec![commit(id, vec![])], 0, None), vec![invalid])
            .expect_err("reference parent must exist in its group");
        assert!(err.to_string().contains("out-of-bounds reference parent"));

        let fan_out_at_commit = ReferenceGroup {
            parent: 0,
            references: vec![
                grouped(
                    reference("refs/heads/A", id),
                    vec![ReferenceGroupParent::Commit],
                ),
                grouped(
                    reference("refs/heads/B", id),
                    vec![ReferenceGroupParent::Commit],
                ),
            ],
            children: vec![child(
                ReferenceGroupChildKind::Commit {
                    index: 1,
                    parent_order: 0,
                },
                vec![0, 1],
            )],
        };
        let err = apply_reference_groups(
            graph(vec![commit(id, vec![]), commit(child_id, vec![0])], 1, None),
            vec![fan_out_at_commit],
        )
        .expect_err("one Git parent slot cannot fan out");
        assert!(err.to_string().contains("reference parents instead of one"));
    }
}
