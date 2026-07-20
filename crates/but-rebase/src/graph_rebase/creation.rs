use std::collections::HashSet;

use anyhow::{Result, bail};
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::{BoundaryKind, Graph, NodeGraphEntrypoint, NodeKind};
use gix::refs::Category;

use crate::graph_rebase::{
    Checkout, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, SuccessfulRebase,
    step_graph::NodeMeta,
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
    ///
    /// Reference steps themselves are only ever mutable when they are local
    /// branches; a non-local entry still makes its ancestry commits mutable.
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
        let (graph, initial_references, checkouts) =
            Self::create_from_graph(&workspace.graph, workspace_commit_id, repo, options)?;
        let project_meta = workspace.graph.project_meta().clone();
        Ok(Self {
            graph,
            initial_references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            project_meta,
            workspace,
            meta,
        })
    }

    /// Copy the node graph verbatim and annotate it with editor metadata.
    ///
    /// The editor's step indexes are the node graph's node indexes.
    fn create_from_graph(
        node_graph: &Graph,
        workspace_commit_id: Option<gix::ObjectId>,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<(StepGraph, Vec<gix::refs::FullName>, Vec<Checkout>)> {
        let mut mutable_entrypoints = Vec::new();
        let mut has_mutable_local_ref = false;
        if let NodeGraphEntrypoint::Node(entrypoint) = node_graph.entrypoint() {
            let symbolic_entrypoint = node_graph
                .entrypoint_ref()
                .and_then(|name| node_graph.node_by_ref_name(name).map(|(index, _)| index));
            mutable_entrypoints.push(symbolic_entrypoint.unwrap_or(*entrypoint));
            has_mutable_local_ref = symbolic_entrypoint.is_some_and(|_| {
                node_graph
                    .entrypoint_ref()
                    .is_some_and(|name| name.category() == Some(Category::LocalBranch))
            });
        }
        for ref_name in &options.extra_mutable_refs {
            let (index, _) = node_graph
                .node_by_ref_name(ref_name.as_ref())
                .ok_or_else(|| anyhow::anyhow!("Failed to find graph node for {ref_name}"))?;
            mutable_entrypoints.push(index);
            has_mutable_local_ref |= ref_name.category() == Some(Category::LocalBranch);
        }

        let mut mutable_nodes = HashSet::new();
        while let Some(index) = mutable_entrypoints.pop() {
            if mutable_nodes.insert(index) {
                mutable_entrypoints.extend(node_graph.nodes()[index].parents().iter().copied());
            }
        }

        // Local branches decorating mutable commits are part of the rewrite even when the node
        // graph keeps them as sibling roots instead of placing them on the entrypoint ancestry.
        if has_mutable_local_ref {
            let mutable_commit_ids = mutable_nodes
                .iter()
                .filter_map(|index| node_graph.nodes()[*index].kind().addressable_commit_id())
                .collect::<HashSet<_>>();
            for (index, node) in node_graph.nodes().iter().enumerate() {
                let NodeKind::Reference(reference) = node.kind() else {
                    continue;
                };
                if reference.ref_info.ref_name.category() == Some(Category::LocalBranch)
                    && reference
                        .ref_info
                        .commit_id
                        .is_some_and(|id| mutable_commit_ids.contains(&id))
                {
                    mutable_nodes.insert(index);
                }
            }
        }

        let addressable_commit_ids = node_graph
            .nodes()
            .iter()
            .filter_map(|node| node.kind().addressable_commit_id())
            .collect::<HashSet<_>>();

        let mut initial_references = Vec::new();
        let mut step_reference_names = HashSet::new();
        let mut meta = Vec::with_capacity(node_graph.nodes().len());
        for (index, node) in node_graph.nodes().iter().enumerate() {
            let mutable = mutable_nodes.contains(&index);
            let node_meta = match node.kind() {
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
                            NodeKind::Boundary {
                                reason: BoundaryKind::Shallow,
                                ..
                            }
                        )
                    });
                    if has_shallow_parent
                        || parent_ids
                            .iter()
                            .any(|id| !addressable_commit_ids.contains(id))
                    {
                        pick.preserved_parents = Some(parent_ids);
                    }
                    pick.mutable = mutable;
                    NodeMeta::Pick(pick.into_settings().1)
                }
                NodeKind::Reference(reference) => {
                    let refname = reference.ref_info.ref_name.clone();
                    if !step_reference_names.insert(refname.clone()) {
                        bail!("BUG: reference {refname} occurs more than once in the node graph");
                    }
                    // Materialization only ever writes local branches (and HEAD, via
                    // checkout): remote-tracking branches, tags, and custom references
                    // stay immutable no matter how they were reached.
                    let mutable = mutable && refname.category() == Some(Category::LocalBranch);
                    if mutable {
                        initial_references.push(refname);
                    }
                    NodeMeta::Reference { mutable }
                }
                NodeKind::Boundary { .. } | NodeKind::None => NodeMeta::Inert,
            };
            meta.push(node_meta);
        }

        let mut graph = StepGraph::from_parts(node_graph.nodes().to_vec(), meta);

        let checkouts = match node_graph.entrypoint() {
            NodeGraphEntrypoint::Node(index) => {
                let checkout_index = node_graph
                    .entrypoint_ref()
                    .and_then(|name| node_graph.node_by_ref_name(name))
                    .map(|(index, _)| index)
                    .unwrap_or(*index);
                vec![Checkout::Head {
                    selector: Selector {
                        id: checkout_index,
                        revision: 0,
                    },
                    merge_base_override: None,
                }]
            }
            NodeGraphEntrypoint::Unborn(reference) => {
                let refname = reference.ref_info.ref_name.clone();
                initial_references.push(refname.clone());
                let unborn = graph.add_node(Step::Reference {
                    refname,
                    mutable: true,
                });
                vec![Checkout::Head {
                    selector: Selector {
                        id: unborn,
                        revision: 0,
                    },
                    merge_base_override: None,
                }]
            }
        };

        Ok((graph, initial_references, checkouts))
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
            project_meta: self.project_meta,
            workspace: self.workspace,
            meta: self.meta,
        }
    }
}
