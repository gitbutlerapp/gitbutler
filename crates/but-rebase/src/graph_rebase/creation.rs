use std::collections::{HashMap, HashSet};

use anyhow::{Result, bail};
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::Commit;

use crate::graph_rebase::{
    Checkout, Edge, Editor, ExtraRef, Pick, RevisionHistory, Selector, Step, StepGraph,
    StepGraphIndex, SuccessfulRebase, inputs::select_branches, util,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions<'a> {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// Extra references that should be included in the editor.
    ///
    /// If the parentage of a commit in the extra references list gets modified,
    /// mutable extra references will be updated while immutable ones remain
    /// traversal-only.
    pub extra_refs: Vec<ExtraRef<'a>>,
}

impl Default for GraphEditorOptions<'_> {
    fn default() -> Self {
        Self {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            extra_refs: vec![],
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
        // The step graph is built directly from the workspace's `BranchGraph`: `select_branches`
        // picks and orders the branches the rebase operates on, then each contributes its reference
        // and pick steps. The build validates that each pick's derived parents match the commit's
        // actual parents: but-rebase trusts but-graph's commit-accurate topology, and a mismatch is
        // a but-graph bug.
        let bg = workspace.branch_graph(repo);
        let (order, connections, immutable_references) = select_branches(&bg, &options.extra_refs)?;
        let workspace_commit_id = bg.workspace_commit;
        let entrypoint_name = bg
            .branches
            .iter()
            .find(|b| b.is_entrypoint)
            .and_then(|b| b.ref_name.clone());

        let mut commits: Vec<Commit> = vec![];
        let mut commit_to_pick_ix = HashMap::<gix::ObjectId, StepGraphIndex>::new();
        let mut graph = StepGraph::new();
        let mut head_selectors = vec![];
        let mut references = vec![];
        let mut branch_nodes: Vec<Vec<StepGraphIndex>> = Vec::with_capacity(order.len());

        for &branch_idx in &order {
            let branch = &bg.branches[branch_idx];
            let mut nodes = vec![];

            if let Some(refname) = &branch.ref_name {
                references.push(refname.clone());
                let ix = graph.add_node(Step::Reference {
                    refname: refname.clone(),
                });
                if branch.ref_name == entrypoint_name {
                    head_selectors.push(Selector {
                        id: ix,
                        revision: 0,
                    });
                }
                nodes.push(ix);
            }

            for commit in &branch.commits {
                commits.push(commit.clone());

                for reference in commit.refs.iter().map(|r| r.ref_name.clone()) {
                    references.push(reference.clone());
                    let ix = graph.add_node(Step::Reference {
                        refname: reference.clone(),
                    });
                    if let Some(previous_ix) = nodes.last() {
                        graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                    }
                    nodes.push(ix);
                }

                let pick = if workspace_commit_id == Some(commit.id) {
                    Pick::new_workspace_pick(commit.id)
                } else {
                    let mut pick = Pick::new_pick(commit.id);
                    pick.sign_commit = options.default_sign_commit;
                    pick
                };
                let ix = graph.add_node(Step::Pick(pick));
                commit_to_pick_ix.insert(commit.id, ix);
                if let Some(previous_ix) = nodes.last() {
                    graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                }
                nodes.push(ix);
            }

            if nodes.is_empty() {
                tracing::debug!("Empty node added - this is probably impossible");
                let ix = graph.add_node(Step::None);
                nodes.push(ix);
            }

            branch_nodes.push(nodes);
        }

        let commit_ids = commits.iter().map(|c| c.id).collect::<HashSet<_>>();

        for c in &commits {
            let has_no_parents = c.parent_ids.is_empty();
            let missing_parent_steps = c.parent_ids.iter().any(|p| !commit_ids.contains(p));

            // If the commit has parents, but at least one of them is not
            // in the graph, this means but-graph did a partial traversal
            // and we want to preserve the commit as it is.
            if !has_no_parents && missing_parent_steps {
                let Some(idx) = commit_to_pick_ix.get(&c.id) else {
                    bail!("BUG: Listed commit does not have corresponding idx.");
                };

                let Step::Pick(pick) = &mut graph[*idx] else {
                    bail!("BUG: Listed commit does not have corresponding pick step.");
                };

                pick.preserved_parents = Some(c.parent_ids.clone());
            };
        }

        for (source_idx, target_idx, parent_order) in connections {
            let Some(source) = branch_nodes[source_idx].last() else {
                continue;
            };
            let Some(target) = branch_nodes[target_idx].first() else {
                continue;
            };
            graph.add_edge(
                *source,
                *target,
                Edge {
                    order: parent_order,
                },
            );
        }

        for c in &commits {
            if Some(c.id) == workspace_commit_id {
                continue;
            }

            let Some(&pick_ix) = commit_to_pick_ix.get(&c.id) else {
                continue;
            };

            // Skip commits with preserved parents (partial traversal — already handled above)
            if let Step::Pick(Pick {
                preserved_parents: Some(_),
                ..
            }) = &graph[pick_ix]
            {
                continue;
            }

            // Resolve what the graph thinks are the parents of this pick
            let graph_parents = util::collect_ordered_parents(&graph, pick_ix);
            let graph_parent_ids: Vec<gix::ObjectId> = graph_parents
                .iter()
                .filter_map(|idx| match &graph[*idx] {
                    Step::Pick(Pick { id, .. }) => Some(*id),
                    _ => None,
                })
                .collect();

            if graph_parent_ids == c.parent_ids {
                continue;
            }

            // The walk stops expanding once it hits its commit budget, leaving a boundary commit
            // with no recorded outgoing edges — even when its parent is present via another path, so
            // the absent-parent check above misses it. Preserve its real parents, exactly as for a
            // parent that is entirely absent, rather than treating the truncation as a bug.
            if graph_parent_ids.is_empty() {
                if let Step::Pick(pick) = &mut graph[pick_ix] {
                    pick.preserved_parents = Some(c.parent_ids.clone());
                }
                continue;
            }

            bail!(
                "but-graph produced a commit topology inconsistent with the commit graph for {}: \
                 segment-derived parents {:?} != actual parents {:?}. but-rebase trusts but-graph's \
                 (now commit-accurate) topology rather than maintaining a corrected copy, so this \
                 indicates a but-graph bug to fix at the source.",
                c.id,
                graph_parent_ids
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>(),
                c.parent_ids
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>(),
            );
        }

        Ok(Self {
            graph,
            initial_references: references,
            // TODO(CTO): We need to eventually list all worktrees that we own
            // here so we can `safe_checkout` them too.
            checkouts: head_selectors
                .into_iter()
                .map(|selector| Checkout::Head {
                    selector,
                    merge_base_override: None,
                })
                .collect(),
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            immutable_references,
            workspace,
            meta,
        })
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
            immutable_references: self.immutable_references,
            workspace: self.workspace,
            meta: self.meta,
        }
    }
}
