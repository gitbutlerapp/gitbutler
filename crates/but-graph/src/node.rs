use std::collections::{HashSet, VecDeque};

use anyhow::{Result, bail, ensure};

use crate::{CommitFlags, RefInfo, SegmentMetadata, StopCondition, init};

/// The position of a node in a [`NodeGraph`].
pub type NodeIndex = usize;

/// A commit/reference graph backed by a contiguous vector.
///
/// Parent indexes are ordered exactly like the corresponding Git commit's
/// parents. Commit annotations live in a parallel vector at the same index.
#[derive(Debug, Clone)]
#[must_use]
pub struct NodeGraph {
    pub(crate) nodes: Vec<Node>,
    pub(crate) annotations: Vec<CommitFlags>,
    pub(crate) context: ConstructionContext,
}

#[derive(Debug, Clone)]
pub(crate) struct ConstructionContext {
    pub(crate) entrypoint: NodeGraphEntrypoint,
    pub(crate) entrypoint_ref: Option<gix::refs::FullName>,
    pub(crate) managed_workspace_commit_id: Option<gix::ObjectId>,
    pub(crate) traversal_tips: Vec<init::Tip>,
    pub(crate) ad_hoc_branch_stack_orders: Vec<Vec<gix::refs::FullName>>,
    pub(crate) hard_limit_hit: bool,
    pub(crate) options: init::Options,
    pub(crate) project_meta: but_core::ref_metadata::ProjectMeta,
    pub(crate) symbolic_remote_names: Vec<String>,
}

impl NodeGraph {
    /// Return all nodes in insertion order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Return annotations parallel to [`Self::nodes()`].
    pub fn annotations(&self) -> &[CommitFlags] {
        &self.annotations
    }

    /// Return the user-facing traversal entrypoint.
    pub fn entrypoint(&self) -> &NodeGraphEntrypoint {
        &self.context.entrypoint
    }

    /// Validate the graph and return it unchanged.
    pub fn validated(self) -> Result<Self> {
        self.validate()?;
        Ok(self)
    }

    /// Count how many nodes name each node as a parent.
    pub fn child_counts(&self) -> Vec<usize> {
        let mut out = vec![0; self.nodes.len()];
        for node in &self.nodes {
            for &parent in &node.parents {
                out[parent] += 1;
            }
        }
        out
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.nodes.len() == self.annotations.len(),
            "BUG: node and annotation counts differ: {} != {}",
            self.nodes.len(),
            self.annotations.len()
        );

        if let NodeGraphEntrypoint::Unborn(reference) = &self.context.entrypoint {
            ensure!(
                self.nodes.is_empty() && self.context.traversal_tips.is_empty(),
                "BUG: unborn entrypoint requires an empty node graph and traversal context"
            );
            ensure!(
                reference.ref_info.commit_id.is_none(),
                "BUG: unborn entrypoint reference has a target commit"
            );
            ensure!(
                self.context.entrypoint_ref.as_ref() == Some(&reference.ref_info.ref_name),
                "BUG: unborn entrypoint reference {} does not match remembered entrypoint ref {:?}",
                reference.ref_info.ref_name,
                self.context.entrypoint_ref
            );
        }

        if let NodeGraphEntrypoint::Node(index) = self.context.entrypoint {
            ensure!(
                index < self.nodes.len(),
                "BUG: entrypoint node {index} is out of bounds for {} nodes",
                self.nodes.len()
            );
            ensure!(
                !matches!(self.nodes[index].kind, NodeKind::ShallowPoint { .. }),
                "BUG: entrypoint node {index} is a shallow point"
            );
        }

        let mut commit_ids = HashSet::new();
        let mut shallow_points = HashSet::new();
        let mut children = vec![Vec::new(); self.nodes.len()];
        for (index, (node, annotation)) in
            self.nodes.iter().zip(self.annotations.iter()).enumerate()
        {
            ensure!(
                annotation.bits() & !CommitFlags::all().bits() == 0,
                "BUG: node {index} has temporary traversal flags in its annotation"
            );

            for &parent in &node.parents {
                ensure!(
                    parent < self.nodes.len(),
                    "BUG: node {index} has out-of-bounds parent {parent}"
                );
                children[parent].push(index);
            }

            match &node.kind {
                NodeKind::Commit { id } => {
                    ensure!(
                        commit_ids.insert(*id),
                        "BUG: commit {id} appears in more than one commit node"
                    );
                }
                NodeKind::Reference(reference) => {
                    if matches!(reference.metadata, Some(SegmentMetadata::Workspace(_))) {
                        ensure!(
                            !node.parents.is_empty(),
                            "BUG: workspace reference node {index} has no parents"
                        );
                    } else {
                        ensure!(
                            node.parents.len() == 1,
                            "BUG: ordinary reference node {index} has {} parents instead of one",
                            node.parents.len()
                        );
                    }
                    ensure!(
                        node.parents.iter().copied().collect::<HashSet<_>>().len()
                            == node.parents.len(),
                        "BUG: reference node {index} has duplicate parent indexes"
                    );
                    ensure!(
                        annotation.is_empty(),
                        "BUG: reference node {index} has commit annotations"
                    );
                }
                NodeKind::ShallowPoint { id, reason } => {
                    ensure!(
                        node.parents.is_empty(),
                        "BUG: shallow-point node {index} has parents"
                    );
                    ensure!(
                        *reason == StopCondition::Limit
                            || *reason == StopCondition::ShallowBoundary,
                        "BUG: shallow-point node {index} has invalid stop reason {reason:?}"
                    );
                    ensure!(
                        shallow_points.insert((*id, reason.bits())),
                        "BUG: shallow point {id} with reason {reason:?} appears more than once"
                    );
                    ensure!(
                        annotation.is_empty(),
                        "BUG: shallow-point node {index} has commit annotations"
                    );
                }
            }
        }

        for tip in &self.context.traversal_tips {
            ensure!(
                commit_ids.contains(&tip.id),
                "BUG: traversal tip {} is not a commit node",
                tip.id
            );
        }

        if let Some(managed_id) = self.context.managed_workspace_commit_id {
            ensure!(
                commit_ids.contains(&managed_id),
                "BUG: managed workspace commit {managed_id} is not a commit node"
            );
            let managed_index = self
                .nodes
                .iter()
                .position(|node| matches!(node.kind, NodeKind::Commit { id } if id == managed_id))
                .expect("verified managed commit exists");
            let workspace_targets = self.nodes.iter().filter_map(|node| {
                let NodeKind::Reference(reference) = &node.kind else {
                    return None;
                };
                matches!(reference.metadata, Some(SegmentMetadata::Workspace(_)))
                    .then(|| node.parents.last().copied())
                    .flatten()
            });
            let mut saw_workspace_ref = false;
            let mut saw_managed_ancestor = false;
            for target in workspace_targets {
                saw_workspace_ref = true;
                saw_managed_ancestor |= node_reaches(&self.nodes, target, managed_index);
            }
            ensure!(
                !saw_workspace_ref || saw_managed_ancestor,
                "BUG: managed workspace commit {managed_id} is not reachable from a workspace reference"
            );
        }

        for (index, node) in self.nodes.iter().enumerate() {
            if matches!(node.kind, NodeKind::ShallowPoint { .. }) {
                ensure!(
                    !children[index].is_empty(),
                    "BUG: shallow-point node {index} is not used as an omitted parent"
                );
            }
        }

        ensure_acyclic(&self.nodes, &children)?;

        for (index, node) in self.nodes.iter().enumerate() {
            let NodeKind::Reference(reference) = &node.kind else {
                continue;
            };
            let Some(expected_id) = reference.ref_info.commit_id else {
                bail!("BUG: reference node {index} has no target commit");
            };
            ensure!(
                commit_ids.contains(&expected_id),
                "BUG: reference node {index} targets commit {expected_id}, which is not in the graph"
            );

            if matches!(reference.metadata, Some(SegmentMetadata::Workspace(_))) {
                let Some((own_target_parent, overlay_parents)) = node.parents.split_last() else {
                    bail!("BUG: workspace reference node {index} has no own-target parent");
                };
                ensure!(
                    matches!(
                        self.nodes[*own_target_parent].kind,
                        NodeKind::Commit { id } if id == expected_id
                    ),
                    "BUG: workspace reference node {index} must retain its direct target {expected_id} as the final parent"
                );
                for &parent in overlay_parents {
                    ensure!(
                        matches!(self.nodes[parent].kind, NodeKind::Reference(_)),
                        "BUG: workspace reference node {index} overlay parent {parent} is not a reference"
                    );
                }
                continue;
            }

            let mut parents = node.parents.clone();
            let mut seen = HashSet::new();
            while let Some(parent) = parents.pop() {
                if !seen.insert(parent) {
                    continue;
                }
                match &self.nodes[parent].kind {
                    NodeKind::Commit { id } => {
                        ensure!(
                            *id == expected_id,
                            "BUG: reference node {index} targets {expected_id}, but its parent chain reaches {id}"
                        );
                    }
                    NodeKind::Reference(parent_ref) => {
                        ensure!(
                            parent_ref.ref_info.commit_id == Some(expected_id),
                            "BUG: reference node {index} targets {expected_id}, but reference parent {parent} targets {:?}",
                            parent_ref.ref_info.commit_id
                        );
                        if matches!(parent_ref.metadata, Some(SegmentMetadata::Workspace(_))) {
                            parents.extend(self.nodes[parent].parents.last().copied());
                        } else {
                            parents.extend(self.nodes[parent].parents.iter().copied());
                        }
                    }
                    NodeKind::ShallowPoint { .. } => {
                        bail!(
                            "BUG: reference node {index} targets {expected_id}, but its parent chain reaches a shallow point"
                        );
                    }
                }
            }
        }

        Ok(())
    }
}

fn node_reaches(nodes: &[Node], start: NodeIndex, wanted: NodeIndex) -> bool {
    let mut pending = vec![start];
    let mut seen = HashSet::new();
    while let Some(index) = pending.pop() {
        if index == wanted {
            return true;
        }
        if seen.insert(index) {
            pending.extend(nodes[index].parents.iter().copied());
        }
    }
    false
}

fn ensure_acyclic(nodes: &[Node], children: &[Vec<NodeIndex>]) -> Result<()> {
    let mut remaining_parents = nodes
        .iter()
        .map(|node| node.parents.len())
        .collect::<Vec<_>>();
    let mut ready = remaining_parents
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut seen = 0;

    while let Some(parent) = ready.pop_front() {
        seen += 1;
        for &child in &children[parent] {
            remaining_parents[child] -= 1;
            if remaining_parents[child] == 0 {
                ready.push_back(child);
            }
        }
    }

    ensure!(seen == nodes.len(), "BUG: node graph contains a cycle");
    Ok(())
}

/// A node and its ordered parent indexes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub(crate) kind: NodeKind,
    pub(crate) parents: Vec<NodeIndex>,
}

impl Node {
    /// Return the data stored at this node.
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    /// Return ordered parent indexes.
    pub fn parents(&self) -> &[NodeIndex] {
        &self.parents
    }
}

/// Data stored in a [`Node`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeKind {
    /// A traversed Git commit.
    Commit {
        /// The commit object ID.
        id: gix::ObjectId,
    },
    /// A reference placed above one or more paths to its target commit.
    Reference(Box<Reference>),
    /// An omitted-parent sentinel below a separately represented commit node.
    ///
    /// It is never an entrypoint, reference target, or traversal tip.
    ShallowPoint {
        /// The object ID of the parent that traversal omitted.
        id: gix::ObjectId,
        /// Why traversal stopped here.
        reason: StopCondition,
    },
}

/// Information attached to a reference node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The reference name, resolved target, and worktree association.
    pub ref_info: RefInfo,
    /// Legacy metadata used to place the reference in a workspace stack.
    pub metadata: Option<SegmentMetadata>,
    /// The configured remote-tracking reference, if one was resolved.
    pub remote_tracking_ref_name: Option<gix::refs::FullName>,
}

/// The entrypoint of a [`NodeGraph`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeGraphEntrypoint {
    /// The node representing the resolved entrypoint.
    Node(NodeIndex),
    /// A reference with no target commit yet.
    Unborn(Box<Reference>),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oid(hex: &str) -> gix::ObjectId {
        gix::ObjectId::from_hex(hex.as_bytes()).expect("valid test object id")
    }

    fn reference(name: &str, id: gix::ObjectId) -> NodeKind {
        NodeKind::Reference(Box::new(Reference {
            ref_info: RefInfo {
                ref_name: name.try_into().expect("valid full ref name"),
                commit_id: Some(id),
                worktree: None,
            },
            metadata: None,
            remote_tracking_ref_name: None,
        }))
    }

    fn workspace_reference(name: &str, id: gix::ObjectId) -> NodeKind {
        let NodeKind::Reference(mut reference) = reference(name, id) else {
            unreachable!()
        };
        reference.metadata = Some(SegmentMetadata::Workspace(Default::default()));
        NodeKind::Reference(reference)
    }

    fn unborn(name: &str) -> NodeGraphEntrypoint {
        NodeGraphEntrypoint::Unborn(Box::new(Reference {
            ref_info: RefInfo {
                ref_name: name.try_into().expect("valid full ref name"),
                commit_id: None,
                worktree: None,
            },
            metadata: None,
            remote_tracking_ref_name: None,
        }))
    }

    fn graph(
        nodes: Vec<Node>,
        annotations: Vec<CommitFlags>,
        entrypoint: NodeGraphEntrypoint,
    ) -> NodeGraph {
        let entrypoint_ref = match &entrypoint {
            NodeGraphEntrypoint::Unborn(reference) => Some(reference.ref_info.ref_name.clone()),
            NodeGraphEntrypoint::Node(_) => None,
        };
        let tip = nodes.iter().find_map(|node| match node.kind {
            NodeKind::Commit { id } => Some(init::Tip::new(id)),
            _ => None,
        });
        NodeGraph {
            nodes,
            annotations,
            context: ConstructionContext {
                entrypoint,
                entrypoint_ref,
                managed_workspace_commit_id: None,
                traversal_tips: tip.into_iter().collect(),
                ad_hoc_branch_stack_orders: Vec::new(),
                hard_limit_hit: false,
                options: init::Options::default(),
                project_meta: Default::default(),
                symbolic_remote_names: Vec::new(),
            },
        }
    }

    #[test]
    fn validates_nodes_and_counts_children() -> Result<()> {
        let root = oid("1111111111111111111111111111111111111111");
        let left = oid("2222222222222222222222222222222222222222");
        let right = oid("3333333333333333333333333333333333333333");
        let valid_graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: root },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: left },
                    parents: vec![0],
                },
                Node {
                    kind: NodeKind::Commit { id: right },
                    parents: vec![0],
                },
            ],
            vec![
                CommitFlags::Integrated,
                CommitFlags::InWorkspace,
                CommitFlags::default(),
            ],
            NodeGraphEntrypoint::Node(1),
        )
        .validated()?;

        assert_eq!(valid_graph.child_counts(), vec![2, 0, 0]);
        assert_eq!(valid_graph.nodes()[1].parents(), &[0]);
        assert_eq!(valid_graph.annotations()[0], CommitFlags::Integrated);
        Ok(())
    }

    #[test]
    fn validates_reference_chains() -> Result<()> {
        let id = oid("1111111111111111111111111111111111111111");
        let _graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/bottom", id),
                    parents: vec![0],
                },
                Node {
                    kind: reference("refs/heads/top", id),
                    parents: vec![1],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(2),
        )
        .validated()?;
        Ok(())
    }

    #[test]
    fn validates_managed_workspace_commit_context() -> Result<()> {
        let managed_id = oid("1111111111111111111111111111111111111111");
        let other_id = oid("2222222222222222222222222222222222222222");
        let mut valid = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: managed_id },
                    parents: vec![],
                },
                Node {
                    kind: workspace_reference("refs/heads/gitbutler/workspace", managed_id),
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        valid.context.managed_workspace_commit_id = Some(managed_id);
        let _ = valid.validated()?;

        let mut advanced = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: managed_id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: other_id },
                    parents: vec![0],
                },
                Node {
                    kind: workspace_reference("refs/heads/gitbutler/workspace", other_id),
                    parents: vec![1],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(2),
        );
        advanced.context.managed_workspace_commit_id = Some(managed_id);
        let _ = advanced.validated()?;

        let mut missing = graph(
            vec![Node {
                kind: NodeKind::Commit { id: managed_id },
                parents: vec![],
            }],
            vec![CommitFlags::default()],
            NodeGraphEntrypoint::Node(0),
        );
        missing.context.managed_workspace_commit_id = Some(other_id);
        assert!(
            missing
                .validated()
                .unwrap_err()
                .to_string()
                .contains("is not a commit node")
        );

        let mut wrong_workspace_target = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: managed_id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: other_id },
                    parents: vec![],
                },
                Node {
                    kind: workspace_reference("refs/heads/gitbutler/workspace", other_id),
                    parents: vec![1],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(2),
        );
        wrong_workspace_target.context.managed_workspace_commit_id = Some(managed_id);
        assert!(
            wrong_workspace_target
                .validated()
                .unwrap_err()
                .to_string()
                .contains("is not reachable from a workspace reference")
        );
        Ok(())
    }

    #[test]
    fn workspace_overlay_keeps_one_own_target_and_rejects_ordinary_cross_target_refs() -> Result<()>
    {
        let id = oid("1111111111111111111111111111111111111111");
        let other_id = oid("2222222222222222222222222222222222222222");
        let valid_graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: other_id },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/left", id),
                    parents: vec![0],
                },
                Node {
                    kind: reference("refs/heads/right", other_id),
                    parents: vec![1],
                },
                Node {
                    kind: workspace_reference("refs/heads/workspace", id),
                    parents: vec![2, 3, 0],
                },
                Node {
                    kind: reference("refs/remotes/origin/workspace", id),
                    parents: vec![4],
                },
            ],
            vec![CommitFlags::default(); 6],
            NodeGraphEntrypoint::Node(5),
        )
        .validated()?;
        assert_eq!(valid_graph.nodes[4].parents, [2, 3, 0]);

        let missing_own_target = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: other_id },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/left", id),
                    parents: vec![0],
                },
                Node {
                    kind: reference("refs/heads/right", other_id),
                    parents: vec![1],
                },
                Node {
                    kind: workspace_reference("refs/heads/workspace", id),
                    parents: vec![2, 3],
                },
            ],
            vec![CommitFlags::default(); 5],
            NodeGraphEntrypoint::Node(4),
        );
        assert!(
            missing_own_target
                .validated()
                .unwrap_err()
                .to_string()
                .contains("must retain its direct target")
        );

        let ordinary_cross_target = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: other_id },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/other", other_id),
                    parents: vec![1],
                },
                Node {
                    kind: reference("refs/heads/main", id),
                    parents: vec![2],
                },
            ],
            vec![CommitFlags::default(); 4],
            NodeGraphEntrypoint::Node(3),
        );
        assert!(
            ordinary_cross_target
                .validated()
                .unwrap_err()
                .to_string()
                .contains("reference parent 2 targets")
        );
        Ok(())
    }

    #[test]
    fn only_workspace_references_may_fan_out_and_reference_parents_are_unique() -> Result<()> {
        let root = oid("1111111111111111111111111111111111111111");
        let child = oid("2222222222222222222222222222222222222222");
        let _valid_graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: root },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: child },
                    parents: vec![0, 0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        )
        .validated()?;

        let ordinary = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: root },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/main", root),
                    parents: vec![0, 0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            ordinary
                .validated()
                .unwrap_err()
                .to_string()
                .contains("ordinary reference node 1")
        );

        let workspace = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id: root },
                    parents: vec![],
                },
                Node {
                    kind: workspace_reference("refs/heads/workspace", root),
                    parents: vec![0, 0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            workspace
                .validated()
                .unwrap_err()
                .to_string()
                .contains("duplicate parent indexes")
        );
        Ok(())
    }

    #[test]
    fn unborn_entrypoint_requires_an_empty_graph() -> Result<()> {
        let valid_graph = graph(vec![], vec![], unborn("refs/heads/main")).validated()?;
        assert!(matches!(
            valid_graph.entrypoint(),
            NodeGraphEntrypoint::Unborn(reference)
                if reference.ref_info.ref_name.to_string() == "refs/heads/main"
        ));

        let invalid = graph(
            vec![Node {
                kind: NodeKind::Commit {
                    id: oid("1111111111111111111111111111111111111111"),
                },
                parents: vec![],
            }],
            vec![CommitFlags::default()],
            unborn("refs/heads/main"),
        );
        assert!(
            invalid
                .validated()
                .unwrap_err()
                .to_string()
                .contains("requires an empty node graph")
        );

        let mut mismatched_ref = graph(vec![], vec![], unborn("refs/heads/main"));
        mismatched_ref.context.entrypoint_ref =
            Some("refs/heads/other".try_into().expect("valid full ref name"));
        assert!(
            mismatched_ref
                .validated()
                .unwrap_err()
                .to_string()
                .contains("does not match remembered entrypoint ref")
        );
        Ok(())
    }

    #[test]
    fn rejects_misaligned_annotations() {
        let graph = graph(
            vec![Node {
                kind: NodeKind::Commit {
                    id: oid("1111111111111111111111111111111111111111"),
                },
                parents: vec![],
            }],
            vec![],
            NodeGraphEntrypoint::Node(0),
        );

        assert!(
            graph
                .validated()
                .unwrap_err()
                .to_string()
                .contains("node and annotation counts differ")
        );
    }

    #[test]
    fn rejects_invalid_parent_and_entrypoint_indexes() {
        let id = oid("1111111111111111111111111111111111111111");
        let invalid_parent = graph(
            vec![Node {
                kind: NodeKind::Commit { id },
                parents: vec![1],
            }],
            vec![CommitFlags::default()],
            NodeGraphEntrypoint::Node(0),
        );
        assert!(
            invalid_parent
                .validated()
                .unwrap_err()
                .to_string()
                .contains("out-of-bounds parent")
        );

        let invalid_entrypoint = graph(
            vec![Node {
                kind: NodeKind::Commit { id },
                parents: vec![],
            }],
            vec![CommitFlags::default()],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            invalid_entrypoint
                .validated()
                .unwrap_err()
                .to_string()
                .contains("entrypoint node 1 is out of bounds")
        );
    }

    #[test]
    fn rejects_cycles() {
        let graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit {
                        id: oid("1111111111111111111111111111111111111111"),
                    },
                    parents: vec![1],
                },
                Node {
                    kind: NodeKind::Commit {
                        id: oid("2222222222222222222222222222222222222222"),
                    },
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(0),
        );

        assert!(
            graph
                .validated()
                .unwrap_err()
                .to_string()
                .contains("contains a cycle")
        );
    }

    #[test]
    fn rejects_reference_cycles_before_following_parent_chains() {
        let id = oid("1111111111111111111111111111111111111111");
        let graph = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: reference("refs/heads/one", id),
                    parents: vec![2],
                },
                Node {
                    kind: reference("refs/heads/two", id),
                    parents: vec![1],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(1),
        );

        assert!(
            graph
                .validated()
                .unwrap_err()
                .to_string()
                .contains("contains a cycle")
        );
    }

    #[test]
    fn shallow_point_is_an_omitted_parent_sentinel() -> Result<()> {
        let omitted_parent = oid("1111111111111111111111111111111111111111");
        let tip = oid("2222222222222222222222222222222222222222");
        let valid_graph = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id: omitted_parent,
                        reason: StopCondition::ShallowBoundary,
                    },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: tip },
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(), CommitFlags::ShallowBoundary],
            NodeGraphEntrypoint::Node(1),
        )
        .validated()?;

        assert_eq!(valid_graph.child_counts(), vec![1, 0]);
        assert!(matches!(
            valid_graph.nodes()[0].kind(),
            NodeKind::ShallowPoint { id, .. } if *id == omitted_parent
        ));

        let mut invalid_entrypoint = valid_graph.clone();
        invalid_entrypoint.context.entrypoint = NodeGraphEntrypoint::Node(0);
        assert!(
            invalid_entrypoint
                .validated()
                .unwrap_err()
                .to_string()
                .contains("is a shallow point")
        );

        let mut invalid_tip = valid_graph.clone();
        invalid_tip.context.traversal_tips = vec![init::Tip::new(omitted_parent)];
        assert!(
            invalid_tip
                .validated()
                .unwrap_err()
                .to_string()
                .contains("is not a commit node")
        );

        let invalid_target = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id: omitted_parent,
                        reason: StopCondition::ShallowBoundary,
                    },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: tip },
                    parents: vec![0],
                },
                Node {
                    kind: reference("refs/heads/main", omitted_parent),
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            invalid_target
                .validated()
                .unwrap_err()
                .to_string()
                .contains("which is not in the graph")
        );
        Ok(())
    }

    #[test]
    fn rejects_invalid_reference_and_shallow_points() {
        let id = oid("1111111111111111111111111111111111111111");
        let wrong_id = oid("2222222222222222222222222222222222222222");
        let invalid_reference = graph(
            vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Reference(Box::new(Reference {
                        ref_info: RefInfo {
                            ref_name: "refs/heads/main".try_into().expect("valid full ref name"),
                            commit_id: Some(wrong_id),
                            worktree: None,
                        },
                        metadata: None,
                        remote_tracking_ref_name: None,
                    })),
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            invalid_reference
                .validated()
                .unwrap_err()
                .to_string()
                .contains("which is not in the graph")
        );

        let invalid_shallow = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id,
                        reason: StopCondition::FirstCommit,
                    },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: wrong_id },
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            invalid_shallow
                .validated()
                .unwrap_err()
                .to_string()
                .contains("invalid stop reason")
        );

        let combined_reason = graph(
            vec![
                Node {
                    kind: NodeKind::ShallowPoint {
                        id,
                        reason: StopCondition::Limit | StopCondition::ShallowBoundary,
                    },
                    parents: vec![],
                },
                Node {
                    kind: NodeKind::Commit { id: wrong_id },
                    parents: vec![0],
                },
            ],
            vec![CommitFlags::default(); 2],
            NodeGraphEntrypoint::Node(1),
        );
        assert!(
            combined_reason
                .validated()
                .unwrap_err()
                .to_string()
                .contains("invalid stop reason")
        );
    }

    #[test]
    fn shallow_point_identity_is_id_and_reason() -> Result<()> {
        let omitted = oid("1111111111111111111111111111111111111111");
        let tip = oid("2222222222222222222222222222222222222222");
        let nodes = vec![
            Node {
                kind: NodeKind::ShallowPoint {
                    id: omitted,
                    reason: StopCondition::Limit,
                },
                parents: vec![],
            },
            Node {
                kind: NodeKind::ShallowPoint {
                    id: omitted,
                    reason: StopCondition::ShallowBoundary,
                },
                parents: vec![],
            },
            Node {
                kind: NodeKind::Commit { id: omitted },
                parents: vec![],
            },
            Node {
                kind: NodeKind::Commit { id: tip },
                parents: vec![0, 1],
            },
        ];
        let _graph = graph(
            nodes.clone(),
            vec![CommitFlags::default(); nodes.len()],
            NodeGraphEntrypoint::Node(3),
        )
        .validated()?;

        let duplicate = graph(
            vec![
                nodes[0].clone(),
                nodes[0].clone(),
                Node {
                    kind: NodeKind::Commit { id: tip },
                    parents: vec![0, 1],
                },
            ],
            vec![CommitFlags::default(); 3],
            NodeGraphEntrypoint::Node(2),
        );
        assert!(
            duplicate
                .validated()
                .unwrap_err()
                .to_string()
                .contains("appears more than once")
        );
        Ok(())
    }
}
