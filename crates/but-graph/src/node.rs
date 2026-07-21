use std::collections::{HashSet, VecDeque};

use anyhow::{Result, bail, ensure};

use crate::{BoundaryKind, CommitFlags, RefInfo, ReferenceMetadata};

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
    pub(crate) project_meta: but_core::ref_metadata::ProjectMeta,
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

    /// Return the managed workspace commit discovered during construction, if any.
    pub fn managed_workspace_commit_id(&self) -> Option<gix::ObjectId> {
        self.context.managed_workspace_commit_id
    }

    /// Return the reference used to start traversal, if `HEAD` was symbolic.
    pub fn entrypoint_ref(&self) -> Option<&gix::refs::FullNameRef> {
        self.context
            .entrypoint_ref
            .as_ref()
            .map(|name| name.as_ref())
    }

    /// Return project-wide target and push metadata.
    pub fn project_meta(&self) -> &but_core::ref_metadata::ProjectMeta {
        &self.context.project_meta
    }

    /// Find a commit node by object ID.
    pub fn node_by_commit_id(&self, id: gix::ObjectId) -> Option<(NodeIndex, &Node)> {
        self.nodes.iter().enumerate().find(
            |(_, node)| matches!(node.kind, NodeKind::Commit { id: candidate } if candidate == id),
        )
    }

    /// Find a materialized commit or available convergence boundary by object ID.
    pub fn node_by_addressable_commit_id(&self, id: gix::ObjectId) -> Option<(NodeIndex, &Node)> {
        self.nodes
            .iter()
            .enumerate()
            .find(|(_, node)| node.kind.addressable_commit_id() == Some(id))
    }

    /// Find a reference node by full name.
    pub fn node_by_ref_name(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<(NodeIndex, &Reference)> {
        self.nodes.iter().enumerate().find_map(|(index, node)| {
            let NodeKind::Reference(reference) = &node.kind else {
                return None;
            };
            (reference.ref_info.ref_name.as_ref() == name).then_some((index, reference.as_ref()))
        })
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
                self.nodes.is_empty(),
                "BUG: unborn entrypoint requires an empty node graph"
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
            // Edits can drop the entrypoint onto a convergence boundary — a
            // real, addressable commit — so any commit-like node is legal.
            ensure!(
                is_commit_like(&self.nodes, index),
                "BUG: born entrypoint node {index} is not a commit"
            );
            let entrypoint_id = self.nodes[index]
                .kind
                .addressable_commit_id()
                .expect("commit-like nodes are addressable");
            if let Some(entrypoint_ref) = self.context.entrypoint_ref.as_ref()
                && let Some(reference) = self.nodes.iter().find_map(|node| match &node.kind {
                    NodeKind::Reference(reference)
                        if reference.ref_info.ref_name == *entrypoint_ref =>
                    {
                        Some(reference)
                    }
                    NodeKind::Commit { .. }
                    | NodeKind::Reference(_)
                    | NodeKind::Boundary { .. }
                    | NodeKind::None => None,
                })
            {
                ensure!(
                    reference.ref_info.commit_id == Some(entrypoint_id),
                    "BUG: symbolic entrypoint {entrypoint_ref} targets {:?}, not entrypoint commit {entrypoint_id}",
                    reference.ref_info.commit_id
                );
            }
        }

        let mut commit_ids = HashSet::new();
        let mut boundaries = HashSet::new();
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
                    if matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_))) {
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
                NodeKind::None => {
                    // Tombstones left behind by graph edits are legal: traversal
                    // resolves through them to their parents.
                    ensure!(
                        annotation.is_empty(),
                        "BUG: tombstone node {index} has commit annotations"
                    );
                }
                NodeKind::Boundary { id, reason } => {
                    ensure!(
                        node.parents.is_empty(),
                        "BUG: boundary node {index} has parents"
                    );
                    ensure!(
                        boundaries.insert((*id, *reason)),
                        "BUG: boundary {id} with reason {reason:?} appears more than once"
                    );
                    ensure!(
                        annotation.is_empty(),
                        "BUG: boundary node {index} has commit annotations"
                    );
                }
            }
        }

        for (id, _) in &boundaries {
            ensure!(
                !commit_ids.contains(id),
                "BUG: boundary {id} also appears as a materialized commit"
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
                matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_)))
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
            if matches!(node.kind, NodeKind::Boundary { .. }) {
                ensure!(
                    !children[index].is_empty(),
                    "BUG: boundary node {index} is not used as an omitted parent"
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
                commit_ids.contains(&expected_id)
                    || boundaries.contains(&(expected_id, BoundaryKind::Convergence)),
                "BUG: reference node {index} targets commit {expected_id}, which is not in the graph"
            );

            if matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_))) {
                let Some((own_target_parent, overlay_parents)) = node.parents.split_last() else {
                    bail!("BUG: workspace reference node {index} has no own-target parent");
                };
                ensure!(
                    self.nodes[*own_target_parent].kind.addressable_commit_id()
                        == Some(expected_id),
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
                        if matches!(parent_ref.metadata, Some(ReferenceMetadata::Workspace(_))) {
                            parents.extend(self.nodes[parent].parents.last().copied());
                        } else {
                            parents.extend(self.nodes[parent].parents.iter().copied());
                        }
                    }
                    NodeKind::Boundary {
                        id,
                        reason: BoundaryKind::Convergence,
                    } => {
                        ensure!(
                            *id == expected_id,
                            "BUG: reference node {index} targets {expected_id}, but its parent chain reaches convergence commit {id}"
                        );
                    }
                    NodeKind::Boundary {
                        reason: BoundaryKind::Shallow,
                        ..
                    } => {
                        bail!(
                            "BUG: reference node {index} targets {expected_id}, but its parent chain reaches a shallow boundary"
                        );
                    }
                    // Tombstones are transparent; a reference resolves through a
                    // tombstone's first parent slot, matching [`resolve_to_commit`].
                    NodeKind::None => {
                        parents.extend(self.nodes[parent].parents.first().copied());
                    }
                }
            }
        }

        Ok(())
    }
}

/// Whether the node at `index` stands for an addressable commit
/// (a materialized commit or a convergence boundary).
pub fn is_commit_like(nodes: &[Node], index: NodeIndex) -> bool {
    matches!(
        nodes[index].kind,
        NodeKind::Commit { .. }
            | NodeKind::Boundary {
                reason: BoundaryKind::Convergence,
                ..
            }
    )
}

/// The parent slots a transparent node stands on: the target (last) parent for
/// a reference, every parent for a tombstone, nothing for a shallow boundary.
pub fn expansion_slots(nodes: &[Node], index: NodeIndex) -> &[NodeIndex] {
    match &nodes[index].kind {
        NodeKind::Reference(_) => {
            let parents = nodes[index].parents();
            if parents.is_empty() {
                &[]
            } else {
                &parents[parents.len() - 1..]
            }
        }
        NodeKind::None => nodes[index].parents(),
        NodeKind::Commit { .. }
        | NodeKind::Boundary {
            reason: BoundaryKind::Convergence,
            ..
        } => unreachable!("commit-like nodes are never expanded"),
        NodeKind::Boundary {
            reason: BoundaryKind::Shallow,
            ..
        } => &[],
    }
}

/// Find the commit-like parents of a given node, in parent-slot order.
///
/// Non-commit nodes are transparent: a reference stands on its target (its
/// *last* parent), a tombstone expands into all of its parents in order, and a
/// shallow boundary contributes nothing. Commits reachable through several
/// paths are emitted once, at their first encounter.
pub fn collect_ordered_parents(nodes: &[Node], target: NodeIndex) -> Vec<NodeIndex> {
    let mut pending = nodes[target].parents().to_vec();
    pending.reverse();
    let mut seen = pending.iter().copied().collect::<HashSet<_>>();
    let mut parents = Vec::new();

    while let Some(candidate) = pending.pop() {
        if is_commit_like(nodes, candidate) {
            parents.push(candidate);
            // Don't pursue the commit's own parents.
            continue;
        }
        for slot in expansion_slots(nodes, candidate).iter().rev() {
            if seen.insert(*slot) {
                pending.push(*slot);
            }
        }
    }

    parents
}

/// Resolve `index` itself to the commit-like node it stands on, following
/// transparent nodes.
///
/// For a reference this is its target commit; for a tombstone the first
/// commit its ordered parents resolve to.
pub fn resolve_to_commit(nodes: &[Node], index: NodeIndex) -> Option<NodeIndex> {
    let mut pending = vec![index];
    let mut seen = HashSet::new();
    while let Some(candidate) = pending.pop() {
        if !seen.insert(candidate) {
            continue;
        }
        if is_commit_like(nodes, candidate) {
            return Some(candidate);
        }
        for slot in expansion_slots(nodes, candidate).iter().rev() {
            pending.push(*slot);
        }
    }
    None
}

/// All `(child, parent_slot)` pairs naming `index` as a parent.
pub fn children_of(nodes: &[Node], index: NodeIndex) -> Vec<(NodeIndex, usize)> {
    let mut out = Vec::new();
    for (child, node) in nodes.iter().enumerate() {
        for (slot, parent) in node.parents().iter().enumerate() {
            if *parent == index {
                out.push((child, slot));
            }
        }
    }
    out
}

/// All nodes that no other node names as a parent.
pub fn child_most(nodes: &[Node]) -> Vec<NodeIndex> {
    let mut has_child = vec![false; nodes.len()];
    for node in nodes {
        for parent in node.parents() {
            has_child[*parent] = true;
        }
    }
    has_child
        .into_iter()
        .enumerate()
        .filter_map(|(index, has_child)| (!has_child).then_some(index))
        .collect()
}

/// All node indexes ordered parents-first, so every node is visited only after
/// all of its parents. Errors when the graph contains a cycle.
pub fn topological_order(nodes: &[Node]) -> Result<Vec<NodeIndex>> {
    let mut remaining_parents = nodes
        .iter()
        .map(|node| node.parents().len())
        .collect::<Vec<_>>();
    let mut children: Vec<Vec<NodeIndex>> = vec![Vec::new(); nodes.len()];
    for (index, node) in nodes.iter().enumerate() {
        for parent in node.parents() {
            children[*parent].push(index);
        }
    }

    let mut ready = remaining_parents
        .iter()
        .enumerate()
        .filter_map(|(index, count)| (*count == 0).then_some(index))
        .collect::<VecDeque<_>>();
    let mut ordered = Vec::with_capacity(nodes.len());
    while let Some(index) = ready.pop_front() {
        ordered.push(index);
        for child in &children[index] {
            remaining_parents[*child] -= 1;
            if remaining_parents[*child] == 0 {
                ready.push_back(*child);
            }
        }
    }
    ensure!(
        ordered.len() == nodes.len(),
        "BUG: the node graph contains a cycle"
    );
    Ok(ordered)
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
    /// Create a node from its data and ordered parent indexes.
    pub fn new(kind: NodeKind, parents: Vec<NodeIndex>) -> Self {
        Self { kind, parents }
    }

    /// Return the data stored at this node.
    pub fn kind(&self) -> &NodeKind {
        &self.kind
    }

    /// Replace the data stored at this node.
    pub fn set_kind(&mut self, kind: NodeKind) -> NodeKind {
        std::mem::replace(&mut self.kind, kind)
    }

    /// Return ordered parent indexes.
    pub fn parents(&self) -> &[NodeIndex] {
        &self.parents
    }

    /// Return mutable access to the ordered parent indexes.
    pub fn parents_mut(&mut self) -> &mut Vec<NodeIndex> {
        &mut self.parents
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
    /// It is never an entrypoint or traversal tip. A convergence boundary can be a reference
    /// target because it represents an available shared commit; a shallow boundary cannot.
    Boundary {
        /// The object ID of the parent that traversal omitted.
        id: gix::ObjectId,
        /// Why traversal stopped here.
        reason: BoundaryKind,
    },
    /// A placeholder left behind when an editor removes a commit or reference.
    ///
    /// Traversal resolves through it to its parents. Graph construction never
    /// produces this kind, and validated graphs must not contain it.
    None,
}

impl NodeKind {
    /// Return the object ID when this node directly represents an addressable commit.
    ///
    /// Convergence boundaries represent real shared commits that were omitted from one traversal
    /// path, while shallow boundaries represent unavailable history and are not addressable.
    pub fn addressable_commit_id(&self) -> Option<gix::ObjectId> {
        match self {
            NodeKind::Commit { id }
            | NodeKind::Boundary {
                id,
                reason: BoundaryKind::Convergence,
            } => Some(*id),
            NodeKind::Reference(_)
            | NodeKind::Boundary {
                reason: BoundaryKind::Shallow,
                ..
            }
            | NodeKind::None => None,
        }
    }
}

/// Information attached to a reference node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reference {
    /// The reference name, resolved target, and worktree association.
    pub ref_info: RefInfo,
    /// Metadata used to place the reference in a workspace stack.
    pub metadata: Option<ReferenceMetadata>,
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

    #[test]
    fn a_boundary_id_cannot_also_be_a_commit() {
        let id = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")
            .expect("valid object ID");
        let graph = NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id },
                    parents: vec![1],
                },
                Node {
                    kind: NodeKind::Boundary {
                        id,
                        reason: BoundaryKind::Convergence,
                    },
                    parents: Vec::new(),
                },
            ],
            annotations: vec![CommitFlags::empty(); 2],
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(0),
                entrypoint_ref: None,
                managed_workspace_commit_id: None,
                project_meta: Default::default(),
            },
        };

        assert!(
            graph
                .validated()
                .expect_err("overlapping IDs must fail validation")
                .to_string()
                .contains("also appears as a materialized commit")
        );
    }

    #[test]
    fn convergence_boundaries_are_addressable_but_shallow_boundaries_are_not() {
        let commit_id = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")
            .expect("valid object ID");
        let convergence_id = gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")
            .expect("valid object ID");
        let shallow_id = gix::ObjectId::from_hex(b"3333333333333333333333333333333333333333")
            .expect("valid object ID");
        let graph = NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id: commit_id },
                    parents: vec![1, 2],
                },
                Node {
                    kind: NodeKind::Boundary {
                        id: convergence_id,
                        reason: BoundaryKind::Convergence,
                    },
                    parents: Vec::new(),
                },
                Node {
                    kind: NodeKind::Boundary {
                        id: shallow_id,
                        reason: BoundaryKind::Shallow,
                    },
                    parents: Vec::new(),
                },
            ],
            annotations: vec![CommitFlags::empty(); 3],
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(0),
                entrypoint_ref: None,
                managed_workspace_commit_id: None,
                project_meta: Default::default(),
            },
        }
        .validated()
        .expect("valid graph");

        assert!(graph.node_by_commit_id(convergence_id).is_none());
        assert!(
            graph
                .node_by_addressable_commit_id(convergence_id)
                .is_some()
        );
        assert!(graph.node_by_addressable_commit_id(shallow_id).is_none());
    }

    #[test]
    fn references_can_target_convergence_but_not_shallow_boundaries() {
        let commit_id = gix::ObjectId::from_hex(b"1111111111111111111111111111111111111111")
            .expect("valid object ID");
        let boundary_id = gix::ObjectId::from_hex(b"2222222222222222222222222222222222222222")
            .expect("valid object ID");
        let graph_with_boundary = |reason| NodeGraph {
            nodes: vec![
                Node {
                    kind: NodeKind::Commit { id: commit_id },
                    parents: vec![2],
                },
                Node {
                    kind: NodeKind::Boundary {
                        id: boundary_id,
                        reason,
                    },
                    parents: Vec::new(),
                },
                Node {
                    kind: NodeKind::Reference(Box::new(Reference {
                        ref_info: RefInfo {
                            ref_name: "refs/heads/base".try_into().expect("valid reference"),
                            commit_id: Some(boundary_id),
                            worktree: None,
                        },
                        metadata: None,
                        remote_tracking_ref_name: None,
                    })),
                    parents: vec![1],
                },
            ],
            annotations: vec![CommitFlags::empty(); 3],
            context: ConstructionContext {
                entrypoint: NodeGraphEntrypoint::Node(0),
                entrypoint_ref: None,
                managed_workspace_commit_id: None,
                project_meta: Default::default(),
            },
        };

        let _ = graph_with_boundary(BoundaryKind::Convergence)
            .validated()
            .expect("a convergence boundary is an available reference target");
        assert!(
            graph_with_boundary(BoundaryKind::Shallow)
                .validated()
                .expect_err("a shallow boundary is not an available reference target")
                .to_string()
                .contains("which is not in the graph")
        );
    }
}
