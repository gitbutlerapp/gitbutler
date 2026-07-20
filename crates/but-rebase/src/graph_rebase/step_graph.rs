//! The editor's graph: [`but_graph::Node`]s as the single source of topology,
//! with editor-only payload kept in a parallel metadata vector.
//!
//! The public exchange type remains [`Step`]: it is synthesized from a node's
//! [`NodeKind`] plus its metadata, and decomposed back on writes. Two node
//! kinds have a fixed lens: a convergence boundary reads as an immutable pick
//! (it is a real, addressable commit), and a shallow boundary reads as
//! [`Step::None`] (unavailable history behaves like a removed node).

use but_core::commit::SignCommit;
use but_graph::{Node, NodeKind, RefInfo, Reference};

use crate::graph_rebase::{
    Pick, Step,
    cherry_pick::{PickMode, TreeMergeMode},
};

/// The position of a step in a [`StepGraph`].
///
/// Freshly created editors share their index space with the
/// [`but_graph::NodeGraph`] they were created from.
pub(crate) type StepGraphIndex = usize;

/// Editor-only payload stored parallel to the nodes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodeMeta {
    /// Pick options for a commit node.
    Pick(PickSettings),
    /// Mutability of a reference node.
    Reference {
        mutable: bool,
    },
    /// Boundaries and placeholders carry no payload.
    Inert,
}

/// Everything a [`Pick`] holds except the commit id, which lives in the node.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PickSettings {
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    pub pick_mode: PickMode,
    pub sign_commit: SignCommit,
    pub exclude_from_tracking: bool,
    pub conflictable: bool,
    pub tree_merge_mode: TreeMergeMode,
    pub mutable: bool,
}

impl Pick {
    pub(crate) fn into_settings(self) -> (gix::ObjectId, PickSettings) {
        let Pick {
            id,
            preserved_parents,
            pick_mode,
            sign_commit,
            exclude_from_tracking,
            conflictable,
            tree_merge_mode,
            mutable,
        } = self;
        (
            id,
            PickSettings {
                preserved_parents,
                pick_mode,
                sign_commit,
                exclude_from_tracking,
                conflictable,
                tree_merge_mode,
                mutable,
            },
        )
    }
}

impl PickSettings {
    pub(crate) fn with_id(&self, id: gix::ObjectId) -> Pick {
        let PickSettings {
            preserved_parents,
            pick_mode,
            sign_commit,
            exclude_from_tracking,
            conflictable,
            tree_merge_mode,
            mutable,
        } = self.clone();
        Pick {
            id,
            preserved_parents,
            pick_mode,
            sign_commit,
            exclude_from_tracking,
            conflictable,
            tree_merge_mode,
            mutable,
        }
    }
}

/// A vector-backed graph of steps, sharing its topology representation (and,
/// at creation time, its index space) with [`but_graph::NodeGraph`].
#[derive(Debug, Clone, Default)]
pub(crate) struct StepGraph {
    nodes: Vec<Node>,
    meta: Vec<NodeMeta>,
}

impl StepGraph {
    #[cfg(test)]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_parts(nodes: Vec<Node>, meta: Vec<NodeMeta>) -> Self {
        debug_assert_eq!(nodes.len(), meta.len());
        Self { nodes, meta }
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn indices(&self) -> std::ops::Range<StepGraphIndex> {
        0..self.nodes.len()
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The ordered parent indexes of `index`.
    pub fn parents(&self, index: StepGraphIndex) -> &[StepGraphIndex] {
        self.nodes[index].parents()
    }

    /// Mutable access to the ordered parent indexes of `index`.
    pub fn parents_mut(&mut self, index: StepGraphIndex) -> &mut Vec<StepGraphIndex> {
        self.nodes[index].parents_mut()
    }

    /// All `(child, parent_slot)` pairs naming `index` as a parent.
    pub fn children(&self, index: StepGraphIndex) -> Vec<(StepGraphIndex, usize)> {
        let mut out = Vec::new();
        for (child, node) in self.nodes.iter().enumerate() {
            for (slot, parent) in node.parents().iter().enumerate() {
                if *parent == index {
                    out.push((child, slot));
                }
            }
        }
        out
    }

    /// All nodes that no other node names as a parent.
    pub fn child_most(&self) -> Vec<StepGraphIndex> {
        let mut has_child = vec![false; self.nodes.len()];
        for node in &self.nodes {
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

    /// Synthesize the step stored at `index`.
    pub fn step(&self, index: StepGraphIndex) -> Step {
        match self.nodes[index].kind() {
            NodeKind::Commit { id } => match &self.meta[index] {
                NodeMeta::Pick(settings) => Step::Pick(settings.with_id(*id)),
                NodeMeta::Reference { .. } | NodeMeta::Inert => {
                    debug_assert!(false, "BUG: commit node {index} without pick settings");
                    Step::Pick(Pick::new_pick(*id))
                }
            },
            NodeKind::Reference(reference) => {
                let mutable = match self.meta[index] {
                    NodeMeta::Reference { mutable } => mutable,
                    NodeMeta::Pick(_) | NodeMeta::Inert => {
                        debug_assert!(false, "BUG: reference node {index} without meta");
                        false
                    }
                };
                Step::Reference {
                    refname: reference.ref_info.ref_name.clone(),
                    mutable,
                }
            }
            // A convergence boundary is a real shared commit: keep it addressable
            // so deleting the commit above it can reconnect references to this base.
            NodeKind::Boundary {
                id,
                reason: but_graph::BoundaryKind::Convergence,
            } => {
                let mut pick = Pick::new_pick(*id);
                pick.mutable = false;
                Step::Pick(pick)
            }
            // Unavailable history behaves like a removed node.
            NodeKind::Boundary {
                reason: but_graph::BoundaryKind::Shallow,
                ..
            }
            | NodeKind::None => Step::None,
        }
    }

    /// Replace the step stored at `index`, returning the previous step.
    ///
    /// Replacing a reference with a reference of the same name keeps the
    /// node's discovered reference information.
    pub fn set_step(&mut self, index: StepGraphIndex, step: Step) -> Step {
        let previous = self.step(index);
        match step {
            Step::Pick(pick) => {
                let (id, settings) = pick.into_settings();
                self.nodes[index].set_kind(NodeKind::Commit { id });
                self.meta[index] = NodeMeta::Pick(settings);
            }
            Step::Reference { refname, mutable } => {
                let keep_node = matches!(
                    self.nodes[index].kind(),
                    NodeKind::Reference(reference) if reference.ref_info.ref_name == refname
                );
                if !keep_node {
                    self.nodes[index].set_kind(NodeKind::Reference(Box::new(Reference {
                        ref_info: RefInfo {
                            ref_name: refname,
                            commit_id: None,
                            worktree: None,
                        },
                        metadata: None,
                        remote_tracking_ref_name: None,
                    })));
                }
                self.meta[index] = NodeMeta::Reference { mutable };
            }
            Step::None => {
                self.nodes[index].set_kind(NodeKind::None);
                self.meta[index] = NodeMeta::Inert;
            }
        }
        previous
    }

    /// Append a disconnected node holding `step`.
    pub fn add_node(&mut self, step: Step) -> StepGraphIndex {
        let index = self.nodes.len();
        self.nodes.push(Node::new(NodeKind::None, Vec::new()));
        self.meta.push(NodeMeta::Inert);
        self.set_step(index, step);
        index
    }
}
