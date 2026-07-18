use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::{Graph, NodeGraphEntrypoint, NodeKind, Reference, ReferenceMetadata};

use crate::graph_rebase::{
    Checkout, Edge, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, StepGraphIndex,
    SuccessfulRebase,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References whose ancestry should be forced mutable.
    ///
    /// The editor always contains every node in the workspace graph, with
    /// only those reachable from `HEAD` being mutable. Use this to force a
    /// reference and its ancestry to be mutable so they can be rewritten.
    pub extra_mutable_refs: Vec<gix::refs::FullName>,
}

impl Default for GraphEditorOptions {
    fn default() -> Self {
        Self {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            extra_mutable_refs: vec![],
        }
    }
}

/// Creates an editor out of the workspace graph.
impl<'ws, 'meta, M: RefMetadata> Editor<'ws, 'meta, M> {
    /// Creates an editor out of the workspace graph with the default options.
    pub fn create(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
    ) -> Result<Self> {
        Self::create_with_opts(workspace, meta, repo, &GraphEditorOptions::default())
    }

    /// Creates an editor out of the workspace graph with the specified options.
    pub fn create_with_opts(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<Self> {
        let workspace_commit_id = workspace.graph.managed_workspace_commit_id();
        if workspace_commit_id.is_none()
            && workspace.graph.nodes().iter().any(|node| {
                matches!(
                    node.kind(),
                    NodeKind::Reference(reference)
                        if matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_)))
                )
            })
        {
            bail!(
                "Editor construction does not yet support a workspace reference without a managed workspace commit"
            );
        }
        let (graph, initial_references, checkouts) =
            Self::create_from_graph(&workspace.graph, workspace_commit_id, repo, options)?;
        Ok(Self {
            graph,
            initial_references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            workspace,
            meta,
        })
    }

    fn create_from_graph(
        node_graph: &Graph,
        workspace_commit_id: Option<gix::ObjectId>,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<(StepGraph, Vec<gix::refs::FullName>, Vec<Checkout>)> {
        let mut mutable_nodes = HashSet::new();
        let mut mutable_entrypoints = Vec::new();
        let mut has_mutable_local_ref = false;
        if let NodeGraphEntrypoint::Node(entrypoint) = node_graph.entrypoint() {
            mutable_entrypoints.push(*entrypoint);
            has_mutable_local_ref = matches!(
                node_graph.nodes()[*entrypoint].kind(),
                NodeKind::Reference(reference)
                    if reference.ref_info.ref_name.category()
                        == Some(gix::refs::Category::LocalBranch)
            );
        }
        for ref_name in &options.extra_mutable_refs {
            let index = node_graph
                .nodes()
                .iter()
                .position(|node| {
                    matches!(
                        node.kind(),
                        NodeKind::Reference(reference)
                            if reference.ref_info.ref_name == *ref_name
                    )
                })
                .ok_or_else(|| anyhow::anyhow!("Failed to find graph node for {ref_name}"))?;
            mutable_entrypoints.push(index);
            has_mutable_local_ref |= ref_name.category() == Some(gix::refs::Category::LocalBranch);
        }

        // A reference with multiple commit children cannot occupy one NodeGraph parent slot.
        // Route commit children through the sole local branch to retain Editor segment boundaries.
        let mut local_refs_by_commit = HashMap::<usize, Vec<usize>>::new();
        for (index, node) in node_graph.nodes().iter().enumerate() {
            let NodeKind::Reference(reference) = node.kind() else {
                continue;
            };
            let [target] = node.parents() else {
                continue;
            };
            if reference.ref_info.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                && matches!(node_graph.nodes()[*target].kind(), NodeKind::Commit { .. })
            {
                local_refs_by_commit.entry(*target).or_default().push(index);
            }
        }
        let owning_ref_by_commit = local_refs_by_commit
            .into_iter()
            .filter_map(|(commit, refs)| (refs.len() == 1).then_some((commit, refs[0])))
            .collect::<HashMap<_, _>>();
        let effective_parents = node_graph
            .nodes()
            .iter()
            .map(|node| match node.kind() {
                NodeKind::Commit { .. } => node
                    .parents()
                    .iter()
                    .map(|parent| owning_ref_by_commit.get(parent).copied().unwrap_or(*parent))
                    .collect(),
                NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => node.parents().to_vec(),
            })
            .collect::<Vec<Vec<_>>>();
        while let Some(index) = mutable_entrypoints.pop() {
            if mutable_nodes.insert(index) {
                mutable_entrypoints.extend(&effective_parents[index]);
            }
        }

        // Local branches decorating mutable commits are part of the rewrite even when the node
        // graph keeps them as sibling roots instead of placing them on the entrypoint ancestry.
        // Remote-tracking branches, tags, and custom references remain immutable.
        if has_mutable_local_ref {
            let mutable_commit_ids = mutable_nodes
                .iter()
                .filter_map(|index| match node_graph.nodes()[*index].kind() {
                    NodeKind::Commit { id } => Some(*id),
                    NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
                })
                .collect::<HashSet<_>>();
            for (index, node) in node_graph.nodes().iter().enumerate() {
                let NodeKind::Reference(reference) = node.kind() else {
                    continue;
                };
                if reference.ref_info.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                    && reference
                        .ref_info
                        .commit_id
                        .is_some_and(|id| mutable_commit_ids.contains(&id))
                {
                    mutable_nodes.insert(index);
                }
            }
        }

        let commit_ids = node_graph
            .nodes()
            .iter()
            .filter_map(|node| match node.kind() {
                NodeKind::Commit { id } => Some(*id),
                NodeKind::Reference(_) | NodeKind::ShallowPoint { .. } => None,
            })
            .collect::<HashSet<_>>();

        let mut graph = StepGraph::new();
        let mut node_to_step = vec![None; node_graph.nodes().len()];
        let mut initial_references = Vec::new();
        let mut step_reference_names = HashSet::new();
        let unborn_head = match node_graph.entrypoint() {
            NodeGraphEntrypoint::Unborn(reference) => {
                let refname = reference.ref_info.ref_name.clone();
                step_reference_names.insert(refname.clone());
                initial_references.push(refname.clone());
                Some(graph.add_node(Step::Reference {
                    refname,
                    mutable: true,
                }))
            }
            NodeGraphEntrypoint::Node(_) => None,
        };
        for (index, node) in node_graph.nodes().iter().enumerate() {
            let mutable = mutable_nodes.contains(&index);
            let step = match node.kind() {
                NodeKind::Commit { id } => {
                    let mut pick = if Some(*id) == workspace_commit_id {
                        Pick::new_workspace_pick(*id)
                    } else {
                        let mut pick = Pick::new_pick(*id);
                        pick.sign_commit = options.default_sign_commit;
                        pick
                    };
                    let parent_ids = repo
                        .find_commit(*id)?
                        .parent_ids()
                        .map(|id| id.detach())
                        .collect::<Vec<_>>();
                    let has_shallow_parent = node.parents().iter().any(|parent| {
                        matches!(
                            node_graph.nodes()[*parent].kind(),
                            NodeKind::ShallowPoint { .. }
                        )
                    });
                    if has_shallow_parent || parent_ids.iter().any(|id| !commit_ids.contains(id)) {
                        pick.preserved_parents = Some(parent_ids);
                    }
                    pick.mutable = mutable;
                    Some(Step::Pick(pick))
                }
                NodeKind::Reference(reference) => {
                    let refname = reference.ref_info.ref_name.clone();
                    if !step_reference_names.insert(refname.clone()) {
                        bail!("BUG: reference {refname} occurs more than once in the node graph");
                    }
                    if mutable {
                        initial_references.push(refname.clone());
                    }
                    Some(Step::Reference { refname, mutable })
                }
                NodeKind::ShallowPoint { .. } => None,
            };
            if let Some(step) = step {
                node_to_step[index] = Some(graph.add_node(step));
            }
        }

        let managed_workspace_parents = workspace_commit_id.and_then(|managed_id| {
            node_graph.nodes().iter().find_map(|node| {
                let NodeKind::Reference(reference) = node.kind() else {
                    return None;
                };
                if !is_workspace_reference(reference) {
                    return None;
                }
                let (own_target, overlay_parents) = node.parents().split_last()?;
                matches!(
                    node_graph.nodes()[*own_target].kind(),
                    NodeKind::Commit { id } if *id == managed_id
                )
                .then(|| overlay_parents.to_vec())
            })
        });

        for (index, node) in node_graph.nodes().iter().enumerate() {
            let Some(source) = node_to_step[index] else {
                continue;
            };

            match node.kind() {
                NodeKind::Commit { id }
                    if Some(*id) == workspace_commit_id && managed_workspace_parents.is_some() =>
                {
                    let overlay_parents =
                        managed_workspace_parents.as_ref().expect("checked above");
                    let mut claimed_parent_slots = HashSet::new();
                    let mut next_parent_order = node.parents().len();
                    for parent in overlay_parents {
                        let parent_order = node_target_id(node_graph, *parent)
                            .and_then(|id| {
                                node.parents()
                                    .iter()
                                    .enumerate()
                                    .find_map(|(order, candidate)| {
                                        (!claimed_parent_slots.contains(&order)
                                            && node_target_id(node_graph, *candidate) == Some(id))
                                        .then_some(order)
                                    })
                            })
                            .unwrap_or_else(|| {
                                let order = next_parent_order;
                                next_parent_order += 1;
                                order
                            });
                        claimed_parent_slots.insert(parent_order);
                        if let Some(target) = node_to_step[*parent] {
                            graph.add_edge(
                                source,
                                target,
                                Edge {
                                    order: parent_order,
                                },
                            );
                        }
                    }
                    for (order, parent) in node.parents().iter().copied().enumerate() {
                        if claimed_parent_slots.contains(&order) {
                            continue;
                        }
                        if let Some(target) = node_to_step[parent] {
                            graph.add_edge(source, target, Edge { order });
                        }
                    }
                }
                NodeKind::Reference(reference) if is_workspace_reference(reference) => {
                    let Some((own_target, overlay_parents)) = node.parents().split_last() else {
                        bail!("BUG: workspace reference node {index} has no target");
                    };
                    let target_is_managed = workspace_commit_id.is_some_and(|managed_id| {
                        matches!(
                            node_graph.nodes()[*own_target].kind(),
                            NodeKind::Commit { id } if *id == managed_id
                        )
                    });
                    let parents = std::iter::once(*own_target).chain(
                        (!target_is_managed)
                            .then_some(overlay_parents)
                            .into_iter()
                            .flatten()
                            .copied(),
                    );
                    add_parent_edges(&mut graph, source, parents, &node_to_step);
                }
                NodeKind::Commit { .. } | NodeKind::Reference(_) => {
                    add_parent_edges(
                        &mut graph,
                        source,
                        effective_parents[index].iter().copied(),
                        &node_to_step,
                    );
                }
                NodeKind::ShallowPoint { .. } => unreachable!("shallow points have no step"),
            }
        }

        let checkouts = match node_graph.entrypoint() {
            NodeGraphEntrypoint::Node(index) => node_to_step[*index]
                .map(|id| Checkout::Head {
                    selector: Selector { id, revision: 0 },
                    merge_base_override: None,
                })
                .into_iter()
                .collect(),
            NodeGraphEntrypoint::Unborn(_) => unborn_head
                .map(|id| Checkout::Head {
                    selector: Selector { id, revision: 0 },
                    merge_base_override: None,
                })
                .into_iter()
                .collect(),
        };

        Ok((graph, initial_references, checkouts))
    }
}

fn is_workspace_reference(reference: &Reference) -> bool {
    matches!(reference.metadata, Some(ReferenceMetadata::Workspace(_)))
        || but_core::is_workspace_ref_name(reference.ref_info.ref_name.as_ref())
}

fn node_target_id(node_graph: &Graph, index: usize) -> Option<gix::ObjectId> {
    match node_graph.nodes()[index].kind() {
        NodeKind::Commit { id } => Some(*id),
        NodeKind::Reference(reference) => reference.ref_info.commit_id,
        NodeKind::ShallowPoint { .. } => None,
    }
}

fn add_parent_edges(
    graph: &mut StepGraph,
    source: StepGraphIndex,
    parents: impl IntoIterator<Item = usize>,
    node_to_step: &[Option<StepGraphIndex>],
) {
    for (order, parent) in parents.into_iter().enumerate() {
        if let Some(target) = node_to_step[parent] {
            graph.add_edge(source, target, Edge { order });
        }
    }
}

impl<'ws, 'meta, M: RefMetadata> SuccessfulRebase<'ws, 'meta, M> {
    /// Converts a SuccessfulRebase back into another editor for multi-step operations.
    ///
    /// This is the normalization path for callers that want to chain
    /// additional editor-based operations and need the editor graph plus
    /// in-memory repository to agree on ancestry.
    pub fn into_editor(self) -> Editor<'ws, 'meta, M> {
        Editor {
            graph: self.graph,
            initial_references: self.initial_references,
            checkouts: self.checkouts,
            repo: self.repo,
            history: self.history,
            workspace: self.workspace,
            meta: self.meta,
        }
    }
}
