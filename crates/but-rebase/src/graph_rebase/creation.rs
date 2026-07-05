use std::collections::HashMap;

use anyhow::{Context as _, Result, bail};
use but_core::{RefMetadata, commit::SignCommit};

use crate::graph_rebase::{
    Checkout, Editor, Pick, RevisionHistory, Selector, Step, StepGraph, StepGraphIndex,
    SuccessfulRebase, placements,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References whose segment should be forced mutable.
    ///
    /// The editor always contains every segment in the workspace graph, with
    /// only those reachable from `HEAD` being mutable. Use this to force a
    /// segment that isn't reachable from `HEAD` to be mutable so it can be
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
        // The editor graph is built NATIVELY: the ref-placement ledger derives from the
        // segment graph and create_native builds picks straight from the carried CommitGraph.
        let ledger = placements::derive(workspace, repo, options)?;
        let (graph, references, checkouts) = create_native(workspace, repo, options, &ledger)?;
        Ok(Self {
            graph,
            initial_references: references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            workspace,
            meta,
        })
    }
}

/// Build the editor graph by ADOPTION: the carried [`but_graph::CommitGraph`] is cloned
/// wholesale into the arena — full commit payloads (flags, refs, generation) survive, which
/// the write-through put-back depends on — then normalized to editor shape (every parent
/// slot present, the ws commit on its ledger lane slots) and dressed with pick settings and
/// the ledger's reference positions.
fn create_native(
    workspace: &but_graph::Workspace,
    repo: &gix::Repository,
    options: &GraphEditorOptions,
    ledger: &placements::RefPlacements,
) -> Result<(StepGraph, Vec<gix::refs::FullName>, Vec<Checkout>)> {
    let Some(cg) = workspace.graph.commit_graph() else {
        bail!("native creation requires the graph to carry its CommitGraph");
    };
    let workspace_commit_id = workspace
        .graph
        .managed_entrypoint_commit(repo)?
        .map(|c| c.id);
    // A parent outside the graph means the traversal was partial here: the editor's slots
    // must all be present, so those slots are dropped — the raw parent list is preserved so
    // the rebase keeps the commit's real ancestry.
    let traversal_was_partial_at = |id: gix::ObjectId| {
        let raw_parents = &cg.node(id).expect("iterating graph ids").commit.parent_ids;
        (!raw_parents.is_empty() && raw_parents.iter().any(|p| cg.node(*p).is_none()))
            .then(|| raw_parents.clone())
    };

    let mut arena = cg.clone();
    for (i, id) in cg.commit_ids().enumerate() {
        // The ws commit takes its LANE slots from the ledger (one per workspace lane, dups
        // and all); everything else keeps the PRESENT parents in slot order — the same
        // presence filter the segment walk's parent-fixup pass applied.
        if workspace_commit_id == Some(id) {
            let mut targets = Vec::new();
            for parent in ledger.ws_parents.as_deref().unwrap_or_default() {
                let Some(target) = cg.index_of(*parent) else {
                    bail!("ledger ws parent {parent} is not a commit in the graph");
                };
                targets.push(target);
            }
            arena.set_parents(i, targets);
        } else if traversal_was_partial_at(id).is_some() {
            arena.set_parents(i, cg.present_parent_indices(i));
        }
    }

    let mut graph = StepGraph::adopt(arena);
    for (i, id) in cg.commit_ids().enumerate() {
        let ix = StepGraphIndex::Node(i);
        let mut pick = if workspace_commit_id == Some(id) {
            Pick::new_workspace_pick(id)
        } else {
            let mut pick = Pick::new_pick(id);
            pick.sign_commit = options.default_sign_commit;
            pick
        };
        pick.mutable = ledger.mutable_commits.contains(&id);
        graph.set_step(ix, Step::Pick(pick));
        if let Some(raw_parents) = traversal_was_partial_at(id) {
            graph.set_preserved_parents(ix, Some(raw_parents));
        }
    }

    // Two passes: refs stack top-down in the ledger (a ref's `below` has a HIGHER index), so
    // every node must exist before positions can name it.
    let mut ref_by_name = HashMap::<gix::refs::FullName, StepGraphIndex>::new();
    for placed in &ledger.refs {
        let ix = graph.add_reference(placed.name.clone(), placed.mutable);
        ref_by_name.insert(placed.name.clone(), ix);
    }
    for placed in &ledger.refs {
        // Unborn refs (no anchor) keep no stored position.
        let Some(anchor_id) = placed.anchor else {
            continue;
        };
        let node = ref_by_name[&placed.name];
        let Some(anchor) = cg.index_of(anchor_id).map(StepGraphIndex::Node) else {
            bail!("ledger anchor {anchor_id} is not a commit in the graph");
        };
        let below =
            match &placed.below {
                Some(name) => Some(*ref_by_name.get(name).with_context(|| {
                    format!("ledger below {name} is not a reference in the graph")
                })?),
                None => None,
            };
        let mut approach = Vec::with_capacity(placed.approach.len());
        for (source, slot) in &placed.approach {
            let Some(source_ix) = cg.index_of(*source).map(StepGraphIndex::Node) else {
                bail!("ledger approach source {source} is not a commit in the graph");
            };
            approach.push((source_ix, *slot));
        }
        graph.set_position(node, anchor, &approach, placed.ambiguous, below);
    }

    let references = ledger
        .refs
        .iter()
        .filter(|r| r.mutable)
        .map(|r| r.name.clone())
        .collect();
    let checkouts = ledger
        .head_refs
        .iter()
        .map(|name| {
            let Some(&id) = ref_by_name.get(name) else {
                bail!("ledger head ref {name} is not a reference in the graph");
            };
            Ok(Checkout::Head {
                selector: Selector { id },
                merge_base_override: None,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    crate::graph_rebase::positions::debug_assert_positions_total(&graph);
    Ok((graph, references, checkouts))
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
