//! Editor creation: turning a display graph into an editor (see the lifecycle in
//! [`but_graph::ref_layout`]). The arena is cloned wholesale — full commit payloads
//! survive, which putting the arena back after a rebase depends on — then normalized:
//! every parent edge must point at a node in the graph, and the workspace commit gets the
//! parents its chains say it has. Each commit becomes a pick; a pick is mutable when its
//! commit is reachable from `HEAD`, extended by flooding from any extra mutable refs. The
//! reference table is a straight copy of the stored layout with commit ids mapped to pick
//! handles; nothing is re-derived on the way in.

use std::collections::HashSet;

use anyhow::{Result, anyhow, bail};
use but_core::{RefMetadata, commit::SignCommit, ref_metadata::ProjectMeta};

use crate::graph_rebase::graph_editor::{EditorIndex, GroupCarry, PickIndex, RefGroup, RefIndex};
use crate::graph_rebase::{
    Checkout, Editor, GraphEditor, Pick, RevisionHistory, Selector, SuccessfulRebase,
};

#[derive(Clone)]
/// Options for the editor.
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References to force mutable.
    ///
    /// The editor always contains every commit and ref the workspace graph
    /// carries, with only those reachable from `HEAD` being mutable. Use this
    /// to force a ref that isn't reachable from `HEAD` to be mutable so its
    /// territory can be rewritten.
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

/// Creates an editor out of the commit graph.
impl<'meta, M: RefMetadata> Editor<'meta, M> {
    /// Creates an editor out of the commit graph with the default options.
    ///
    /// The commit graph must carry the ref layout the builder stores on it.
    pub fn create(
        commit_graph: &but_graph::CommitGraph,
        project_meta: &ProjectMeta,
        meta: &'meta mut M,
        repo: &gix::Repository,
    ) -> Result<Self> {
        Self::create_with_opts(
            commit_graph,
            project_meta,
            meta,
            repo,
            &GraphEditorOptions::default(),
        )
    }

    /// Creates an editor out of the commit graph with the specified options.
    pub fn create_with_opts(
        commit_graph: &but_graph::CommitGraph,
        project_meta: &ProjectMeta,
        meta: &'meta mut M,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<Self> {
        // Not #[instrument]: its generated closure cannot return the `'meta` borrow.
        let _span = tracing::debug_span!("Editor::create").entered();
        let (graph, references, checkouts) = build_graph_editor(commit_graph, options)?;
        Ok(Self {
            graph,
            initial_references: references,
            checkouts,
            repo: repo.clone().with_object_memory(),
            history: RevisionHistory::new(),
            project_meta: project_meta.clone(),
            meta,
        })
    }
}

/// Build the editor graph from the data the builder stores on the
/// [`but_graph::CommitGraph`] — no segment is read; the module doc spells the ingest contract.
fn build_graph_editor(
    cg: &but_graph::CommitGraph,
    options: &GraphEditorOptions,
) -> Result<(GraphEditor, Vec<gix::refs::FullName>, Vec<Checkout>)> {
    let Some(stored) = cg.layout() else {
        bail!("editor creation requires the ref layout the builder stores on the CommitGraph");
    };

    let workspace_commit_id = stored.materialized_ws_parents.as_ref().map(|m| m.commit);

    // A parent outside the graph means the traversal was partial here: the editor's parent list
    // must all be present, so those parent numbers are dropped — the raw parent list is preserved so
    // the rebase keeps the commit's real ancestry.
    let traversal_was_partial_at = |id: gix::ObjectId| {
        let raw_parents = &cg.node(id).expect("iterating graph ids").parent_ids;
        (!raw_parents.is_empty() && raw_parents.iter().any(|p| cg.node(*p).is_none()))
            .then(|| raw_parents.clone())
    };
    let node_of = |id: gix::ObjectId| -> Result<PickIndex> {
        cg.index_of(id)
            .map(PickIndex)
            .ok_or_else(|| anyhow!("stored position {id} is not a commit in the graph"))
    };

    let mut arena = cg.clone();
    for (i, id) in cg.commit_ids().enumerate() {
        // The workspace commit takes its chain parents from the stored layout (duplicates
        // and all); everything else keeps its present parents in order.
        if workspace_commit_id == Some(id) {
            let chain_parents = stored
                .materialized_ws_parents
                .as_ref()
                .map(|m| m.parents.as_slice())
                .unwrap_or_default();
            let mut targets = Vec::new();
            for parent in chain_parents {
                let Some(target) = cg.index_of(*parent) else {
                    bail!("stored ws parent {parent} is not a commit in the graph");
                };
                targets.push(target);
            }
            arena.set_parents(i, targets);
        } else if traversal_was_partial_at(id).is_some() {
            arena.set_parents(i, cg.present_parent_indices(i));
        }
    }

    // Mutability follows reachability: the stored entrypoint reach, extended below by
    // flooding from the extra mutable refs once the layout is ingested.
    let mut mutable_commits: HashSet<gix::ObjectId> =
        stored.reachable_commits.iter().copied().collect();

    let mut step_graph = GraphEditor::adopt(arena);
    // Remote-category refs are never mutable — the editor cannot move or delete the remote.
    let ref_mutable: Vec<bool> = stored
        .facts
        .iter()
        .map(|(name, facts)| {
            facts.reachable && name.as_ref().category() != Some(gix::refs::Category::RemoteBranch)
        })
        .collect();
    // Register every reference (facts order = table order), then copy the stored groups
    // in: the layout's shape IS the editor's shape, so ingest is an id-mapping copy —
    // commit ids become pick handles, everything else transfers verbatim.
    let ref_ixs: Vec<RefIndex> = stored
        .facts
        .iter()
        .zip(&ref_mutable)
        .map(|((name, _), mutable)| step_graph.add_reference(name.clone(), *mutable))
        .collect();
    for ((_, facts), &entry) in stored.facts.iter().zip(&ref_ixs) {
        step_graph.set_ref_ambiguous(entry, facts.ambiguous);
    }
    for (on, commit_groups) in &stored.groups {
        let key = node_of(*on)?;
        let groups = commit_groups
            .iter()
            .map(|group| {
                Ok(RefGroup {
                    members: group.members.clone(),
                    carry: match &group.carry {
                        but_graph::ref_layout::GroupCarry::None => GroupCarry::None,
                        but_graph::ref_layout::GroupCarry::All => GroupCarry::All,
                        but_graph::ref_layout::GroupCarry::Edges(edges) => GroupCarry::Edges(
                            edges
                                .iter()
                                .map(|&(id, parent_number)| Ok((node_of(id)?, parent_number)))
                                .collect::<Result<Vec<_>>>()?,
                        ),
                    },
                    attach: group.attach.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        step_graph.insert_groups(key, groups);
    }

    // The extra-mutable flood, over the editor's own queries: down below-links and onto
    // the ref's commit, then per parent edge across the refs that carry it. Remote refs
    // are traversed but never marked (the category gate above).
    if !options.extra_mutable_refs.is_empty() {
        let mut queue: Vec<EditorIndex> = Vec::new();
        for ref_name in &options.extra_mutable_refs {
            let Some(entry) = step_graph.entry_of(ref_name.as_ref()) else {
                bail!("Failed to find corresponding reference for {ref_name}");
            };
            queue.push(entry.into());
        }
        let mut seen_refs = HashSet::new();
        let mut seen_picks = HashSet::new();
        while let Some(visit) = queue.pop() {
            match (visit.as_ref(), visit.as_pick()) {
                (Some(entry), _) => {
                    if !seen_refs.insert(entry) {
                        continue;
                    }
                    let is_remote = step_graph.state_of(entry.into()).is_some_and(|state| {
                        state.refname.category() == Some(gix::refs::Category::RemoteBranch)
                    });
                    if !is_remote {
                        step_graph.set_ref_mutable(entry, true);
                    }
                    if let Some(below) = step_graph.below_of(entry) {
                        queue.push(below.into());
                    }
                    if let Some(on) = step_graph.positioned_on(entry) {
                        queue.push(on.into());
                    }
                }
                (_, Some(pick)) => {
                    if !seen_picks.insert(pick) {
                        continue;
                    }
                    if let Some(id) = step_graph.commit_id(pick) {
                        mutable_commits.insert(id);
                    }
                    for (parent_number, parent) in step_graph.parents(pick).into_iter().enumerate()
                    {
                        let carriers: Vec<RefIndex> = step_graph
                            .edge_carriers(parent, pick, parent_number)
                            .collect();
                        for carrier in carriers {
                            queue.push(carrier.into());
                        }
                        queue.push(parent.into());
                    }
                }
                _ => {}
            }
        }
    }

    for (i, id) in cg.commit_ids().enumerate() {
        let ix = PickIndex(i);
        let mut pick = if workspace_commit_id == Some(id) {
            Pick::new_workspace_pick(id)
        } else {
            let mut pick = Pick::new_pick(id);
            pick.sign_commit = options.default_sign_commit;
            pick
        };
        pick.mutable = mutable_commits.contains(&id);
        step_graph.set_step(ix, Some(pick));
        if let Some(raw_parents) = traversal_was_partial_at(id) {
            step_graph.set_preserved_parents(ix, Some(raw_parents));
        }
    }

    let references = ref_ixs
        .iter()
        .filter(|&&entry| {
            step_graph
                .state_of(entry.into())
                .is_some_and(|state| state.mutable)
        })
        .filter_map(|&entry| {
            step_graph
                .state_of(entry.into())
                .map(|state| state.refname.clone())
        })
        .collect();
    let checkouts = stored
        .head_refs
        .iter()
        .filter_map(|name| step_graph.entry_of(name.as_ref()))
        .map(|entry| Checkout::Head {
            selector: Selector { id: entry.into() },
            merge_base_override: None,
        })
        .collect();

    crate::graph_rebase::positions::debug_assert_positions_total(&step_graph);
    Ok((step_graph, references, checkouts))
}

impl<'meta, M: RefMetadata> SuccessfulRebase<'meta, M> {
    /// Converts a SuccessfulRebase back into another editor for multi-step operations.
    ///
    /// This is the normalization path for callers that want to chain
    /// additional editor-based operations and need the editor graph plus
    /// in-memory repository to agree on ancestry.
    pub fn into_editor(self) -> Editor<'meta, M> {
        Editor {
            graph: self.graph,
            initial_references: self.initial_references,
            checkouts: self.checkouts,
            repo: self.repo,
            history: self.history,
            project_meta: self.project_meta,
            meta: self.meta,
        }
    }
}
