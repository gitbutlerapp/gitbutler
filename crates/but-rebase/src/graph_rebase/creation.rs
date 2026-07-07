use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow, bail};
use but_core::{RefMetadata, commit::SignCommit, ref_metadata::ProjectMeta};

use crate::graph_rebase::{
    Checkout, Editor, EditorGraph, EditorGraphIndex, Pick, RevisionHistory, Selector, Step,
    SuccessfulRebase,
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
        let (graph, references, checkouts) = create_native(commit_graph, options)?;
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

/// Build the editor graph from the DATA the builder stores on the
/// [`but_graph::CommitGraph`] — no segment is read. The arena is the commit graph cloned
/// wholesale — full commit payloads (flags, refs, generation) survive, which the
/// write-through put-back depends on — then normalized to editor shape (every parent slot
/// present, the ws commit on its chain slots), dressed with pick settings, and the reference
/// table translated 1:1 from the stored ref layout
/// ([`RefPositions`](but_graph::ref_arrangement::RefPositions)).
fn create_native(
    cg: &but_graph::CommitGraph,
    options: &GraphEditorOptions,
) -> Result<(EditorGraph, Vec<gix::refs::FullName>, Vec<Checkout>)> {
    let Some(stored) = cg.arrangement().and_then(|a| a.positions.as_ref()) else {
        bail!("native creation requires the ref layout the builder stores on the CommitGraph");
    };

    let workspace_commit_id = stored.ws_chain_slots.as_ref().map(|(id, _)| *id);

    // A parent outside the graph means the traversal was partial here: the editor's slots
    // must all be present, so those slots are dropped — the raw parent list is preserved so
    // the rebase keeps the commit's real ancestry.
    let traversal_was_partial_at = |id: gix::ObjectId| {
        let raw_parents = &cg.row(id).expect("iterating graph ids").commit.parent_ids;
        (!raw_parents.is_empty() && raw_parents.iter().any(|p| cg.row(*p).is_none()))
            .then(|| raw_parents.clone())
    };
    let node_of = |id: gix::ObjectId| -> Result<EditorGraphIndex> {
        cg.index_of(id)
            .map(EditorGraphIndex::Row)
            .ok_or_else(|| anyhow!("stored position {id} is not a commit in the graph"))
    };

    let mut arena = cg.clone();
    for (i, id) in cg.commit_ids().enumerate() {
        // The ws commit takes its CHAIN slots from the stored layout (dups and all); everything
        // else keeps the PRESENT parents in slot order.
        if workspace_commit_id == Some(id) {
            let slots = stored
                .ws_chain_slots
                .as_ref()
                .map(|(_, slots)| slots.as_slice())
                .unwrap_or_default();
            let mut targets = Vec::new();
            for parent in slots {
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

    // MUTABILITY: the stored entrypoint reach, extended by flooding the stored position
    // topology from every extra mutable ref — down each ref's below-links, through the on-commit's
    // parent slots (crossing the refs entering them), over the NORMALIZED arena parents.
    let mut mutable_refs: Vec<bool> = stored.refs.iter().map(|r| r.reachable).collect();
    let mut mutable_commits: HashSet<gix::ObjectId> =
        stored.reachable_commits.iter().copied().collect();
    if !options.extra_mutable_refs.is_empty() {
        enum Visit {
            Ref(usize),
            Commit(gix::ObjectId),
        }
        let ordinal_by_name: HashMap<_, _> = stored
            .refs
            .iter()
            .enumerate()
            .map(|(r, pr)| (pr.name.as_ref(), r))
            .collect();
        let mut entering_index: HashMap<(gix::ObjectId, usize), Vec<usize>> = HashMap::new();
        for (r, pr) in stored.refs.iter().enumerate() {
            for &(child, slot) in pr.position.iter().flat_map(|p| &p.entering) {
                entering_index.entry((child, slot)).or_default().push(r);
            }
        }
        let mut queue = Vec::new();
        for ref_name in &options.extra_mutable_refs {
            let Some(&seed) = ordinal_by_name.get(ref_name.as_ref()) else {
                bail!("Failed to find corresponding reference for {ref_name}");
            };
            queue.push(Visit::Ref(seed));
        }
        let mut seen_refs = vec![false; stored.refs.len()];
        let mut seen_commits = HashSet::new();
        while let Some(visit) = queue.pop() {
            match visit {
                Visit::Ref(r) => {
                    if std::mem::replace(&mut seen_refs[r], true) {
                        continue;
                    }
                    mutable_refs[r] = true;
                    if let Some(position) = &stored.refs[r].position {
                        if let Some(below) = position.below {
                            queue.push(Visit::Ref(below));
                        }
                        queue.push(Visit::Commit(position.on));
                    }
                }
                Visit::Commit(id) => {
                    if !seen_commits.insert(id) {
                        continue;
                    }
                    mutable_commits.insert(id);
                    let Some(ci) = arena.index_of(id) else {
                        continue;
                    };
                    for (slot, parent) in arena.parent_indices(ci).into_iter().enumerate() {
                        for &r in entering_index.get(&(id, slot)).into_iter().flatten() {
                            queue.push(Visit::Ref(r));
                        }
                        if let Some(parent_id) = arena.row_payload(parent) {
                            queue.push(Visit::Commit(parent_id));
                        }
                    }
                }
            }
        }
    }

    let mut editor_graph = EditorGraph::adopt(arena);
    for (i, id) in cg.commit_ids().enumerate() {
        let ix = EditorGraphIndex::Row(i);
        let mut pick = if workspace_commit_id == Some(id) {
            Pick::new_workspace_pick(id)
        } else {
            let mut pick = Pick::new_pick(id);
            pick.sign_commit = options.default_sign_commit;
            pick
        };
        pick.mutable = mutable_commits.contains(&id);
        editor_graph.set_step(ix, Step::Pick(pick));
        if let Some(raw_parents) = traversal_was_partial_at(id) {
            editor_graph.set_preserved_parents(ix, Some(raw_parents));
        }
    }

    // Remote-category refs are never mutable — the editor cannot move or delete the remote.
    let ref_mutable: Vec<bool> = stored
        .refs
        .iter()
        .enumerate()
        .map(|(r, pr)| {
            mutable_refs[r]
                && pr.name.as_ref().category() != Some(gix::refs::Category::RemoteBranch)
        })
        .collect();
    // Two passes: refs stack top-down in the stored layout (a ref's `below` has a HIGHER
    // ordinal), so every entry must exist before positions can name it.
    let ref_ixs: Vec<EditorGraphIndex> = stored
        .refs
        .iter()
        .zip(&ref_mutable)
        .map(|(pr, mutable)| editor_graph.add_reference(pr.name.clone(), *mutable))
        .collect();
    for (r, pr) in stored.refs.iter().enumerate() {
        // Unborn refs (no position) keep no stored position.
        let Some(position) = &pr.position else {
            continue;
        };
        let on = node_of(position.on)?;
        let below = position.below.map(|b| ref_ixs[b]);
        let entering = position
            .entering
            .iter()
            .map(|&(id, slot)| Ok((node_of(id)?, slot)))
            .collect::<Result<Vec<_>>>()?;
        editor_graph.set_position(ref_ixs[r], on, &entering, position.ambiguous, below);
    }

    let references = stored
        .refs
        .iter()
        .zip(&ref_mutable)
        .filter(|(_, mutable)| **mutable)
        .map(|(pr, _)| pr.name.clone())
        .collect();
    let checkouts = stored
        .head_refs
        .iter()
        .map(|&r| Checkout::Head {
            selector: Selector { id: ref_ixs[r] },
            merge_base_override: None,
        })
        .collect();

    crate::graph_rebase::positions::debug_assert_positions_total(&editor_graph);
    Ok((editor_graph, references, checkouts))
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
