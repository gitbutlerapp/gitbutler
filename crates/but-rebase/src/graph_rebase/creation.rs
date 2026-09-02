use std::collections::{BTreeMap, HashMap, HashSet};

use anyhow::{Context, Result, bail};
use but_core::{RefMetadata, commit::SignCommit};
use but_graph::Commit;

use crate::graph_rebase::{
    Checkout, Edge, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, StepGraphIndex,
    SuccessfulRebase, inputs::select_branches, util,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References whose branch should be forced mutable.
    ///
    /// The editor always contains every branch in the workspace graph, with
    /// only those reachable from `HEAD` being mutable. Use this to force a
    /// branch that isn't reachable from `HEAD` to be mutable so it can be
    /// rewritten.
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

/// Whether the repository that built `workspace` owns the checkout of `ref_name`, i.e. the branch
/// is not checked out in another (linked) worktree.
///
/// The projection records the worktree on every named segment and commit ref it displays; a
/// branch it doesn't display is owned by this repository unless a linked worktree tip lists it.
fn ref_owned_by_repo(workspace: &but_graph::Workspace, ref_name: &gix::refs::FullName) -> bool {
    let recorded = workspace
        .stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .flat_map(|segment| {
            segment
                .ref_info
                .iter()
                .chain(segment.commits.iter().flat_map(|c| c.refs.iter()))
        })
        .find(|ri| ri.ref_name == *ref_name);
    match recorded {
        Some(ri) => ri
            .worktree
            .as_ref()
            .is_none_or(|worktree| worktree.owned_by_repo),
        None => !workspace
            .worktree_tips
            .iter()
            .any(|tip| tip.ref_name.as_ref() == Some(ref_name)),
    }
}

/// Creates an editor out of the workspace graph.
impl<'ws, 'meta, M: RefMetadata> Editor<'ws, 'meta, M> {
    /// Creates an editor out of the workspace graph with the default options.
    pub fn create(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
        db: &'meta mut but_db::DbHandle,
    ) -> Result<Self> {
        let editor =
            Self::create_with_opts(workspace, meta, repo, db, &GraphEditorOptions::default())?;
        Ok(editor)
    }

    /// Creates an editor out of the workspace graph with the specified options.
    pub fn create_with_opts(
        workspace: &'ws mut but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
        db: &'meta mut but_db::DbHandle,
        options: &GraphEditorOptions,
    ) -> Result<Self> {
        // The step graph is built directly from the workspace's `BranchGraph`: `select_branches`
        // orders the branches and decides which the editor may rewrite, then each contributes its
        // reference and pick steps. Branches reachable from `HEAD`, from an extra mutable ref, or
        // from a branch checked out in a linked worktree are mutable; every other branch is still
        // part of the editor, but immutable. The build validates that each pick's derived parents
        // match the commit's actual parents: but-rebase trusts but-graph's commit-accurate
        // topology, and a mismatch is a but-graph bug.
        let worktree_tips = workspace.worktree_tips.clone();
        let bg = workspace.branch_graph(repo);
        let selected = select_branches(
            &bg,
            options
                .extra_mutable_refs
                .iter()
                .chain(worktree_tips.iter().filter_map(|tip| tip.ref_name.as_ref())),
        )?;
        let workspace_commit_id = bg.workspace_commit;
        let entrypoint_name = bg
            .branches
            .iter()
            .find(|b| b.is_entrypoint)
            .and_then(|b| b.ref_name.clone());

        let mut commits: Vec<Commit> = vec![];
        let mut commit_to_pick_ix = HashMap::<gix::ObjectId, StepGraphIndex>::new();
        let mut reference_to_ix = HashMap::<gix::refs::FullName, StepGraphIndex>::new();
        let mut graph = StepGraph::new();
        let mut head_selectors = vec![];
        let mut references = vec![];
        let mut branch_nodes: Vec<Vec<StepGraphIndex>> = Vec::with_capacity(selected.order.len());

        for (order_idx, &branch_idx) in selected.order.iter().enumerate() {
            let branch = &bg.branches[branch_idx];
            let branch_mutable = selected.mutable[order_idx];
            let mut nodes = vec![];

            if let Some(refname) = &branch.ref_name {
                let reference_mutable =
                    branch_mutable && refname.category() == Some(gix::refs::Category::LocalBranch);
                // Only mutable references are tracked for potential deletion.
                if reference_mutable && ref_owned_by_repo(workspace, refname) {
                    references.push(refname.clone());
                }
                let ix = graph.add_node(Step::Reference {
                    refname: refname.clone(),
                    mutable: reference_mutable,
                });
                reference_to_ix.insert(refname.clone(), ix);
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

                for ref_info in &commit.refs {
                    let refname = ref_info.ref_name.clone();
                    let reference_mutable = branch_mutable
                        && refname.category() == Some(gix::refs::Category::LocalBranch);
                    if reference_mutable
                        && ref_info
                            .worktree
                            .as_ref()
                            .is_none_or(|worktree| worktree.owned_by_repo)
                    {
                        references.push(refname.clone());
                    }
                    let ix = graph.add_node(Step::Reference {
                        refname: refname.clone(),
                        mutable: reference_mutable,
                    });
                    reference_to_ix.insert(refname, ix);
                    if let Some(previous_ix) = nodes.last() {
                        graph.add_edge(*previous_ix, ix, Edge { order: 0 });
                    }
                    nodes.push(ix);
                }

                let mut pick = if workspace_commit_id == Some(commit.id) {
                    Pick::new_workspace_pick(commit.id)
                } else {
                    let mut pick = Pick::new_pick(commit.id);
                    pick.sign_commit = options.default_sign_commit;
                    pick
                };
                pick.mutable = branch_mutable;
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

        for (source_idx, target_idx, parent_order) in selected.connections {
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

        let mut worktree_checkouts = BTreeMap::new();
        for tip in worktree_tips {
            let selector = match &tip.ref_name {
                Some(ref_name) => *reference_to_ix.get(ref_name).with_context(|| {
                    format!(
                        "Visible worktree {} reference {ref_name} is missing from the editor",
                        tip.name
                    )
                })?,
                None => *commit_to_pick_ix.get(&tip.id).with_context(|| {
                    format!(
                        "Visible detached worktree {} HEAD {} is missing from the editor",
                        tip.name, tip.id
                    )
                })?,
            };
            let name = tip.name.clone();
            let checkout = Checkout::Worktree {
                worktree_name: name.clone(),
                selector: Selector {
                    id: selector,
                    revision: 0,
                },
                ref_name: tip.ref_name,
                initial_head: tip.id,
                merge_base_override: None,
            };
            if worktree_checkouts.insert(name.clone(), checkout).is_some() {
                bail!("Visible worktree {name} was listed more than once");
            }
        }

        let checkouts = worktree_checkouts
            .into_values()
            .chain(head_selectors.into_iter().map(|selector| Checkout::Head {
                selector,
                merge_base_override: None,
            }))
            .collect();

        Ok(Self {
            graph,
            initial_references: references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            workspace,
            meta,
            db,
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
            workspace: self.workspace,
            meta: self.meta,
            db: self.db,
        }
    }
}
