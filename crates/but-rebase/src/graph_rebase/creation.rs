//! Editor creation: turning a commit graph into an editor (see the lifecycle in
//! [`but_graph::ref_layout`]). The graph is cloned wholesale — full commit records
//! survive, which putting the graph back after a rebase depends on — then normalized:
//! every parent entry must point at a node in the graph, and the workspace commit gets the
//! parents its chains say it has. Each commit becomes a commit; a commit is mutable when its
//! commit is reachable from `HEAD` and not below the workspace lower bound, extended by
//! flooding from any extra mutable refs. The
//! reference table is a straight copy of the stored layout with commit ids mapped to commit
//! indices; nothing is re-derived on the way in.

use crate::graph_rebase::commits::ParentEntry;
use std::collections::{BTreeMap, HashSet};

use anyhow::{Context as _, Result, anyhow, bail};
use but_core::{RefMetadata, commit::SignCommit, ref_metadata::ProjectMeta};

use crate::graph_rebase::commits::CommitIndex;
use crate::graph_rebase::store::{EditorIndex, GroupCarry, RefGroup, RefIndex};
use crate::graph_rebase::{
    Checkout, CommitSpec, Editor, EditorStore, RebasedEditor, RevisionHistory,
};

#[derive(Clone)]
/// Options for the editor.
pub struct EditorStoreOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References to force mutable.
    ///
    /// The editor always contains every commit and ref the workspace graph
    /// carries, with only those reachable from `HEAD` being mutable. Use this
    /// to force a ref that isn't reachable from `HEAD` to be mutable so its
    /// territory can be rewritten.
    pub extra_mutable_refs: Vec<gix::refs::FullName>,
    /// The linked worktrees whose `HEAD` should follow this edit.
    ///
    /// The commit graph records which worktree checks out which ref, but not which of
    /// them the caller considers active — archived ones are caller state, not Git's. So
    /// membership is decided here, normally by passing the graph's own
    /// [`worktree_tips`](but_graph::walk::Options::worktree_tips) straight through.
    ///
    /// Their refs become mutable (an edit that cannot move them cannot follow them), and
    /// each becomes a `Checkout::Worktree` the materialization checks out.
    pub worktree_tips: Vec<but_graph::walk::WorktreeTip>,
}

impl Default for EditorStoreOptions {
    fn default() -> Self {
        Self {
            default_sign_commit: SignCommit::IfSignCommitsEnabled,
            extra_mutable_refs: vec![],
            worktree_tips: vec![],
        }
    }
}

/// Creates an editor out of the commit graph.
impl<'meta, M: RefMetadata> Editor<'meta, M> {
    /// Creates an editor for `ws`: its commit graph, its project metadata, and — unlike
    /// [`Self::create`] — the linked worktrees the workspace was built with, so an edit
    /// that moves a branch some worktree has checked out moves that worktree along.
    ///
    /// This is what operations on a workspace want; reach for the granular constructors
    /// only when there is no workspace, or when a caller deliberately edits without
    /// touching worktrees.
    pub fn for_workspace(
        ws: &but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
    ) -> Result<Self> {
        Self::for_workspace_with_opts(ws, meta, repo, EditorStoreOptions::default())
    }

    /// Like [`Self::for_workspace`], with room for extra options. The workspace's linked
    /// worktrees are filled in regardless — an editor for a workspace always follows them.
    pub fn for_workspace_with_opts(
        ws: &but_graph::Workspace,
        meta: &'meta mut M,
        repo: &gix::Repository,
        mut options: EditorStoreOptions,
    ) -> Result<Self> {
        options.worktree_tips = ws.options().worktree_tips.clone();
        Self::create_with_opts(ws.commit_graph(), ws.project_meta(), meta, repo, &options)
    }

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
            &EditorStoreOptions::default(),
        )
    }

    /// Creates an editor out of the commit graph with the specified options.
    pub fn create_with_opts(
        commit_graph: &but_graph::CommitGraph,
        project_meta: &ProjectMeta,
        meta: &'meta mut M,
        repo: &gix::Repository,
        options: &EditorStoreOptions,
    ) -> Result<Self> {
        // Not #[instrument]: its generated closure cannot return the `'meta` borrow.
        let _span = tracing::debug_span!("Editor::create").entered();
        let (graph, checkouts) = build_store(commit_graph, options)?;
        Ok(Self {
            store: graph,
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
fn build_store(
    cg: &but_graph::CommitGraph,
    options: &EditorStoreOptions,
) -> Result<(EditorStore, Vec<Checkout>)> {
    let Some(stored) = cg.layout() else {
        bail!("editor creation requires the ref layout the builder stores on the CommitGraph");
    };

    let workspace_commit_id = stored.amended_ws_parents.as_ref().map(|m| m.commit);

    // A parent outside the graph means the traversal was partial here: the editor's parent list
    // must all be present, so those parent numbers are dropped — the raw parent list is preserved so
    // the rebase keeps the commit's real ancestry.
    let traversal_was_partial_at = |id: gix::ObjectId| {
        let raw_parents = &cg.node(id).expect("iterating graph ids").parent_ids;
        (!raw_parents.is_empty() && raw_parents.iter().any(|p| cg.node(*p).is_none()))
            .then(|| raw_parents.clone())
    };
    let commit_index_of = |id: gix::ObjectId| -> Result<CommitIndex> {
        cg.index_of(id)
            .map(CommitIndex)
            .ok_or_else(|| anyhow!("stored position {id} is not a commit in the graph"))
    };

    let mut graph = cg.clone();
    for (i, id) in cg.commit_ids().enumerate() {
        // The workspace commit takes its chain parents from the stored layout (duplicates
        // and all); everything else keeps its present parents in order.
        if workspace_commit_id == Some(id) {
            let chain_parents = stored
                .amended_ws_parents
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
            graph.set_parents(i, targets);
        } else if traversal_was_partial_at(id).is_some() {
            graph.set_parents(i, cg.present_parent_indices(i));
        }
    }

    // Mutability follows reachability, but stops at the workspace lower bound: the
    // entrypoint's reach runs all the way to the root, so seeding mutability from it alone
    // would make the target's whole ancestry rewritable and drag the rebase into re-creating
    // ancient merges (whose already-resolved parents then re-conflict). Commits strictly
    // below the base — flagged BelowBound during derivation — are fixed anchors instead;
    // the base itself and everything above it (including integrated tips like origin/main
    // when they are the merge-base) stay editable, so operations can reorder around them.
    // The set is later extended below by flooding from the extra mutable refs once ingested.
    let mut mutable_commits: HashSet<gix::ObjectId> = stored
        .reachable_commits
        .iter()
        .copied()
        .filter(|id| {
            !cg.node(*id)
                .is_some_and(|n| n.flags.contains(but_graph::CommitFlags::BelowBound))
        })
        .collect();

    let mut step_graph = EditorStore::adopt(graph);
    // Split the workspace commit's ingested parent entries into real parents (on disk) and minted ones —
    // the amended-list entries that exist only in the declaration, one per empty chain. The merge
    // formula makes the split exact: the amended list is real plus minted as a multiset, with the
    // real ones keeping their relative order, so a greedy in-order match against the disk array
    // identifies every entry. Recording both is what lets the write rule stay per-parent precise — a
    // mint and a real parent can name the same commit, and asking by commit cannot tell them
    // apart, which is how a written mint used to read back as ancestry and become permanent.
    if let Some(ws_id) = workspace_commit_id
        && let Ok(ws_commit) = commit_index_of(ws_id)
    {
        let chain_parents = stored
            .amended_ws_parents
            .as_ref()
            .map(|m| m.parents.as_slice())
            .unwrap_or_default();
        let mut disk: std::collections::VecDeque<gix::ObjectId> = cg
            .node(ws_id)
            .map(|n| n.parent_ids.iter().copied().collect())
            .unwrap_or_default();
        let ws_parents = step_graph.parents(ws_commit);
        for (index, parent) in chain_parents.iter().enumerate() {
            let is_real = disk.front() == Some(parent);
            if is_real {
                disk.pop_front();
            }
            if let (Some(entry), Some(&target)) = (
                step_graph.commits.entry_id_at(ws_commit, index),
                ws_parents.get(index),
            ) {
                if is_real {
                    step_graph.ws_real_parents.push((entry, target));
                } else {
                    step_graph.ws_minted_parents.push((entry, target));
                }
            }
        }
    }
    // A ref another worktree has checked out may still be moved — that is how a linked
    // worktree follows a rewrite — but it must never be deleted out from under that
    // checkout, so it stays out of the initial-reference list below.
    let refs_of_foreign_worktrees: HashSet<gix::refs::FullName> = cg
        .commit_ids()
        .collect::<Vec<_>>()
        .into_iter()
        .filter_map(|id| cg.node(id))
        .flat_map(|node| &node.refs)
        .filter(|info| {
            info.worktree
                .as_ref()
                .is_some_and(|worktree| !worktree.owned_by_repo)
        })
        .map(|info| info.ref_name.clone())
        .collect();
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
    // commit ids become commit indices, everything else transfers verbatim.
    let ref_ixs: Vec<RefIndex> = stored
        .facts
        .iter()
        .zip(&ref_mutable)
        .map(|((name, _), mutable)| step_graph.add_reference(name.clone(), *mutable, true))
        .collect();
    for ((_, facts), &entry) in stored.facts.iter().zip(&ref_ixs) {
        step_graph.set_ref_ambiguous(entry, facts.ambiguous);
    }
    for (on, commit_groups) in stored.groups.iter() {
        let key = commit_index_of(on)?;
        let groups = commit_groups
            .iter()
            .map(|group| {
                Ok(RefGroup {
                    members: group.members.clone(),
                    carry: match &group.carry {
                        but_graph::ref_layout::GroupCarry::None => GroupCarry::None,
                        but_graph::ref_layout::GroupCarry::All => GroupCarry::All,
                        but_graph::ref_layout::GroupCarry::Entries(entries) => GroupCarry::Entries(
                            entries
                                .iter()
                                .map(|entry| {
                                    // The stored coordinate resolves to the ingested
                                    // parent entry's stable id; a coordinate past the live
                                    // parent count has no parent entry to name and drops.
                                    Ok(commit_index_of(entry.child).ok().and_then(|child| {
                                        step_graph.commits.entry_id_at(child, entry.index)
                                    }))
                                })
                                .collect::<Result<Vec<_>>>()?
                                .into_iter()
                                .flatten()
                                .collect(),
                        ),
                    },
                    attach: group.attach.clone(),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        step_graph.insert_groups(key, groups);
    }

    // A linked worktree can follow a branch the workspace layout never placed — it is
    // checked out over there, not here. The editor still has to be able to move that ref,
    // so register it against the commit it points at; without an entry the checkout has
    // nothing to follow.
    for tip in &options.worktree_tips {
        let Some(ref_name) = tip.ref_name.as_ref() else {
            continue;
        };
        if step_graph.entry_of(ref_name.as_ref()).is_some() {
            continue;
        }
        let Some(commit) = cg.commit_by_ref(ref_name.as_ref()) else {
            continue;
        };
        step_graph.add_reference(ref_name.clone(), true, true);
        step_graph.insert_groups(
            commit_index_of(commit)?,
            vec![RefGroup {
                members: vec![ref_name.clone()],
                carry: GroupCarry::None,
                attach: None,
            }],
        );
    }

    // The extra-mutable flood, over the editor's own queries: down below-links and onto
    // the ref's commit, then per parent entry across the refs that carry it. Remote refs
    // are traversed but never marked (the category gate above).
    // A linked worktree's branch floods too: an edit that cannot move it cannot make the
    // worktree follow the rewrite. Unlike an explicitly requested ref, a worktree whose
    // branch this edit doesn't contain is simply not this edit's concern.
    let worktree_refs: Vec<&gix::refs::FullName> = options
        .worktree_tips
        .iter()
        .filter_map(|tip| tip.ref_name.as_ref())
        .collect();
    if !options.extra_mutable_refs.is_empty() || !worktree_refs.is_empty() {
        let mut queue: Vec<EditorIndex> = Vec::new();
        for ref_name in &options.extra_mutable_refs {
            let Some(entry) = step_graph.entry_of(ref_name.as_ref()) else {
                bail!("Failed to find corresponding reference for {ref_name}");
            };
            queue.push(entry.into());
        }
        queue.extend(
            worktree_refs
                .into_iter()
                .filter_map(|ref_name| step_graph.entry_of(ref_name.as_ref()))
                .map(EditorIndex::from),
        );
        let mut seen_refs = HashSet::new();
        let mut seen_commits = HashSet::new();
        while let Some(visit) = queue.pop() {
            match (visit.as_ref(), visit.as_commit()) {
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
                (_, Some(commit)) => {
                    if !seen_commits.insert(commit) {
                        continue;
                    }
                    if let Some(id) = step_graph.commit_id(commit) {
                        mutable_commits.insert(id);
                    }
                    for (parent_number, parent) in
                        step_graph.parents(commit).into_iter().enumerate()
                    {
                        let carriers: Vec<RefIndex> = step_graph
                            .entry_carriers(
                                parent,
                                ParentEntry {
                                    child: commit,
                                    number: parent_number,
                                },
                            )
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
        let ix = CommitIndex(i);
        let mut spec = if workspace_commit_id == Some(id) {
            CommitSpec::workspace(id)
        } else {
            let mut spec = CommitSpec::new(id);
            spec.sign_commit = options.default_sign_commit;
            spec
        };
        spec.mutable = mutable_commits.contains(&id);
        step_graph.commits.set_commit(ix, spec);
        if let Some(raw_parents) = traversal_was_partial_at(id) {
            step_graph
                .commits
                .set_preserved_parents(ix, Some(raw_parents));
        }
    }

    // A foreign worktree's ref is not ours to delete: it leaves the deletion universe by
    // shedding its creation mark, however the edit ends.
    for entry in &ref_ixs {
        if step_graph
            .state_of((*entry).into())
            .is_some_and(|state| refs_of_foreign_worktrees.contains(&state.refname))
        {
            step_graph.clear_existed_at_creation(*entry);
        }
    }
    // Worktree checkouts come first so a materialization moves the linked worktrees
    // before this repository's own `HEAD`.
    let mut worktree_checkouts = BTreeMap::new();
    for tip in &options.worktree_tips {
        let id = match &tip.ref_name {
            Some(ref_name) => step_graph
                .entry_of(ref_name.as_ref())
                .map(EditorIndex::from)
                .with_context(|| {
                    format!(
                        "Visible worktree {} reference {ref_name} is missing from the editor",
                        tip.name
                    )
                })?,
            // A detached worktree has no ref to follow, so its commit is the anchor.
            None => EditorIndex::from(commit_index_of(tip.id).with_context(|| {
                format!(
                    "Visible detached worktree {} HEAD {} is missing from the editor",
                    tip.name, tip.id
                )
            })?),
        };
        let checkout = Checkout::Worktree {
            worktree_name: tip.name.clone(),
            entry: id,
            ref_name: tip.ref_name.clone(),
            initial_head: tip.id,
            merge_base_override: None,
        };
        if worktree_checkouts
            .insert(tip.name.clone(), checkout)
            .is_some()
        {
            bail!("Visible worktree {} was listed more than once", tip.name);
        }
    }
    let checkouts = worktree_checkouts
        .into_values()
        .chain(
            stored
                .head_refs
                .iter()
                .filter_map(|name| step_graph.entry_of(name.as_ref()))
                .map(|entry| Checkout::Head {
                    entry: entry.into(),
                    merge_base_override: None,
                }),
        )
        .collect();

    crate::graph_rebase::positions::assert_positions_total(&step_graph)?;
    Ok((step_graph, checkouts))
}

impl<'meta, M: RefMetadata> RebasedEditor<'meta, M> {
    /// Converts a RebasedEditor back into another editor for multi-step operations.
    ///
    /// This is the normalization path for callers that want to chain
    /// additional editor-based operations and need the editor graph plus
    /// in-memory repository to agree on ancestry.
    pub fn into_editor(self) -> Editor<'meta, M> {
        self.editor
    }
}
