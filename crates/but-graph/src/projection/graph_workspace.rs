//! A graph based workspace projection, framed directly over a node graph.
//!
//! Rather than being its own graph, this points into the node graph via
//! [`NodeIndex`]es, so consumers can frame the mutations they're about to
//! perform against the same indexes they'll act on. It works over both a
//! [`crate::Graph`] and a mutable graph mid-edit, including rebased graphs
//! whose objects exist only in an in-memory repository.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use but_core::{
    WORKSPACE_REF_NAME,
    branch::resolve_tracking_branch_ref_name,
    changeset::{
        ChangeIdMode, Identity, changeset_identifier, create_similarity_lut, lookup_similar,
    },
    ui::{CommitState, PushStatus},
};
use gix::prelude::ObjectIdExt;

use crate::{
    Node, NodeGraph, NodeIndex, NodeKind,
    edit::{
        MutableNodeGraph, Rebased,
        traverse::{self, AheadBehind},
    },
    workspace::commit::is_managed_workspace_by_message,
};

/// A structure that gives a frame of reference to a key subgraph in the
/// workspace framing. This could be the subgraph of all commits above the
/// workspace, or the nodes that make up a "stack".
///
/// Rather than being a full graph structure, this provides pointers into the
/// node graph.
pub struct Subgraph {
    /// Nodes in the subgraph that only have incoming edges
    pub heads: Vec<NodeIndex>,
    /// All the nodes in the specified subgraph
    pub nodes: HashSet<NodeIndex>,
}

impl Subgraph {
    fn empty() -> Self {
        Self {
            heads: vec![],
            nodes: HashSet::new(),
        }
    }
}

/// Provides a frame of reference for the standardized view of the world.
pub struct GraphWorkspace {
    /// If we're on the workspace branch, any commits in the rev-set
    /// `HEAD ^workspace_commit ^target_sha` will be included in this subgraph.
    pub above_workspace: Subgraph,

    /// If we are on the workspace branch, and a workspace commit can be found,
    /// this will be set.
    pub workspace_commit: Option<NodeIndex>,

    /// If we're on the workspace branch, this will contain a list of subgraphs
    /// that represents a stack. These are commits that follow the rev-set
    /// `workspace_commit_parents ^target_sha`
    ///
    /// We consider a stack beneath the workspace commit to be mutually
    /// exclusive sub-graphs of commits that don't have any incoming or outgoing
    /// edges to other commits in other stacks.
    ///
    /// As a natural extension, if we failed to find the workspace commit, this
    /// list will be empty since all the commits will deemed "above_workspace".
    ///
    /// If we're outside of the workspace branch, there will be one stack that
    /// contains all commits in the rev-set `HEAD ^target_sha`.
    ///
    /// # Known limitation: stacks sharing unbounded history collapse into one
    ///
    /// Stacks that reach a shared node merge into one. Without a target to
    /// bound the traversal, stacks that branch off common history all reach
    /// the shared base commit and collapse, so a fixture can look like N
    /// obviously-distinct stacks and still come back as one. With a target,
    /// the shared history is excluded and distinct stacks stay separate.
    pub stacks: Vec<Subgraph>,

    /// Per-reference push and integration status for every local-branch
    /// reference in the projection, keyed by its [`NodeIndex`].
    pub reference_status: HashMap<NodeIndex, ReferenceStatus>,

    /// The [`CommitState`] of every commit in the projection, keyed by its
    /// [`NodeIndex`]: integrated, local-and-remote, or local-only. Commits are
    /// all local-only without a target. Per-reference integration is exposed as
    /// [`PushStatus::Integrated`].
    pub commit_state: HashMap<NodeIndex, CommitState>,
}

/// The status of a single reference in the workspace projection.
#[derive(Clone)]
pub struct ReferenceStatus {
    /// The remote-tracking branch this reference was compared against, if one
    /// could be resolved.
    pub remote_ref: Option<gix::refs::FullName>,
    /// Push status for just this reference. [`PushStatus::Integrated`] when
    /// every commit this reference exclusively owns has landed upstream.
    pub push_status: PushStatus,
    /// Push status for this reference, escalated to a force push if any parent
    /// reference below it in the stack would itself require one.
    pub combined_push_status: PushStatus,
}

/// The per-commit states and integrated references in a projection, computed together.
#[derive(Default)]
struct Integration {
    commit_state: HashMap<NodeIndex, CommitState>,
    integrated_references: HashSet<NodeIndex>,
}

impl GraphWorkspace {
    fn empty() -> Self {
        Self {
            above_workspace: Subgraph::empty(),
            workspace_commit: None,
            stacks: vec![],
            reference_status: HashMap::new(),
            commit_state: HashMap::new(),
        }
    }
}

/// The index-level node set used internally so the traversal and set-algebra
/// stay on cheap indexes.
struct NodeSet {
    heads: Vec<NodeIndex>,
    nodes: HashSet<NodeIndex>,
}

impl NodeSet {
    fn into_subgraph(self) -> Subgraph {
        Subgraph {
            heads: self.heads,
            nodes: self.nodes,
        }
    }
}

impl MutableNodeGraph {
    /// Build a graph-based workspace projection framed from this graph,
    /// resolving objects through the edit's in-memory repository.
    pub fn graph_workspace(&self) -> Result<GraphWorkspace> {
        View {
            nodes: self.nodes(),
            repo: self.repo(),
            head: self.head_index(),
            target_ref: self.project_meta().target_ref.clone(),
            target_commit_id: self.project_meta().target_commit_id,
        }
        .graph_workspace()
    }
}

impl Rebased {
    /// Build a graph-based workspace projection over the rebased graph,
    /// resolving objects through the in-memory repository.
    pub fn graph_workspace(&self) -> Result<GraphWorkspace> {
        self.graph.graph_workspace(self.repo())
    }
}

impl NodeGraph {
    /// Build a graph-based workspace projection framed from this graph.
    ///
    /// `repo` must be able to resolve every commit the graph refers to; for a
    /// freshly constructed graph that is the repository it was constructed
    /// from, for a rebased graph the in-memory repository of its [`Rebased`].
    pub fn graph_workspace(&self, repo: &gix::Repository) -> Result<GraphWorkspace> {
        View {
            nodes: self.nodes(),
            repo,
            head: self.head_index(),
            target_ref: self.project_meta().target_ref.clone(),
            target_commit_id: self.project_meta().target_commit_id,
        }
        .graph_workspace()
    }
}

/// Everything the projection needs, independent of which graph flavour it is
/// framed over.
struct View<'a> {
    nodes: &'a [Node],
    repo: &'a gix::Repository,
    head: Option<NodeIndex>,
    target_ref: Option<gix::refs::FullName>,
    target_commit_id: Option<gix::ObjectId>,
}

impl View<'_> {
    fn commit_id_at(&self, index: NodeIndex) -> Option<gix::ObjectId> {
        self.nodes[index].kind().addressable_commit_id()
    }

    fn ref_name_at(&self, index: NodeIndex) -> Option<&gix::refs::FullName> {
        match self.nodes[index].kind() {
            NodeKind::Reference(reference) => Some(&reference.ref_info.ref_name),
            _ => None,
        }
    }

    fn try_select_commit(&self, target: gix::ObjectId) -> Option<NodeIndex> {
        self.nodes
            .iter()
            .position(|node| node.kind().addressable_commit_id() == Some(target))
    }

    fn try_select_reference(&self, target: &gix::refs::FullNameRef) -> Option<NodeIndex> {
        crate::edit::node_index_by_ref_name(self.nodes, target)
    }

    fn direct_parents(&self, index: NodeIndex) -> &[NodeIndex] {
        self.nodes[index].parents()
    }

    /// The target commit's node, if a target is configured and present.
    fn target_index(&self) -> Option<NodeIndex> {
        self.target_commit_id
            .and_then(|id| self.try_select_commit(id))
            .or_else(|| {
                let target_ref = self.target_ref.as_ref()?;
                let index = self.try_select_reference(target_ref.as_ref())?;
                crate::node::resolve_to_commit(self.nodes, index)
            })
    }

    fn graph_workspace(&self) -> Result<GraphWorkspace> {
        let mut ws = self.graph_workspace_topology()?;
        // Every node in the projection, so the status walks stay scoped to
        // the workspace rather than wandering down the full history.
        let nodes: HashSet<NodeIndex> = ws
            .above_workspace
            .nodes
            .iter()
            .chain(ws.stacks.iter().flat_map(|stack| stack.nodes.iter()))
            .copied()
            .collect();
        let integration = self.integration(&nodes)?;
        ws.reference_status =
            self.reference_statuses(&nodes, &integration.integrated_references)?;
        ws.commit_state = integration.commit_state;
        Ok(ws)
    }

    /// Build the topological skeleton of the projection (stacks, above-workspace,
    /// workspace commit) with an empty [`GraphWorkspace::reference_status`].
    fn graph_workspace_topology(&self) -> Result<GraphWorkspace> {
        let Some(entrypoint_ix) = self.head else {
            return Ok(GraphWorkspace::empty());
        };

        // In the case of no target sha:
        // In PGM: We have one giant stack that contains all commits
        // In A workspace:
        //   If we find a workspace commit, we have stacks that reach the full history.
        //   If we don't find a workspace commit, all commits from HEAD are considered above the workspace.

        let ws_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
        let on_workspace = self.ref_name_at(entrypoint_ix) == Some(&ws_ref);

        let target_ix = self.target_index();

        if on_workspace {
            let head_not_target_commit =
                all_until_optional_limit(self.nodes, entrypoint_ix, target_ix);

            // The workspace commit, if present, lives somewhere in `HEAD ^target`.
            let workspace_commit = head_not_target_commit.nodes.iter().copied().find_map(|ix| {
                let id = self.commit_id_at(ix)?;
                let gix_commit = self.repo.find_commit(id).ok()?;
                is_managed_workspace_by_message(gix_commit.message_raw().ok()?).then_some(ix)
            });

            if let Some(workspace_commit_ix) = workspace_commit {
                let (above_workspace, stacks) = divide_workspace_into_stacks(
                    self.nodes,
                    head_not_target_commit,
                    workspace_commit_ix,
                );

                Ok(GraphWorkspace {
                    above_workspace: above_workspace.into_subgraph(),
                    workspace_commit: Some(workspace_commit_ix),
                    stacks: stacks.into_iter().map(|s| s.into_subgraph()).collect(),
                    reference_status: HashMap::new(),
                    commit_state: HashMap::new(),
                })
            } else {
                Ok(GraphWorkspace {
                    above_workspace: head_not_target_commit.into_subgraph(),
                    workspace_commit: None,
                    stacks: vec![],
                    reference_status: HashMap::new(),
                    commit_state: HashMap::new(),
                })
            }
        } else {
            // We're pegging.
            let stack = all_until_optional_limit(self.nodes, entrypoint_ix, target_ix);

            Ok(GraphWorkspace {
                above_workspace: Subgraph::empty(),
                workspace_commit: None,
                stacks: vec![stack.into_subgraph()],
                reference_status: HashMap::new(),
                commit_state: HashMap::new(),
            })
        }
    }

    /// Compute the per-reference status for every local-branch reference in the
    /// projection, given the full projection `nodes` and the references already
    /// classified as `integrated`.
    fn reference_statuses(
        &self,
        nodes: &HashSet<NodeIndex>,
        integrated: &HashSet<NodeIndex>,
    ) -> Result<HashMap<NodeIndex, ReferenceStatus>> {
        // First pass: each local-branch reference's own remote ref and push status.
        let mut remote_by_ref = HashMap::new();
        let mut status_by_ref = HashMap::new();
        for node in nodes {
            let Some(refname) = self.ref_name_at(*node) else {
                continue;
            };
            if refname.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let (remote_ref, push_status) = self.reference_push_status(*node, refname.as_ref())?;
            remote_by_ref.insert(*node, remote_ref);
            status_by_ref.insert(*node, push_status);
        }

        // Integrated references override their push status: nothing to push once
        // the work has landed upstream.
        for index in integrated {
            if let Some(push_status) = status_by_ref.get_mut(index) {
                *push_status = PushStatus::Integrated;
            }
        }

        // Adjacency among projection nodes, used by the combined walk to reach
        // parent references through intermediate commits.
        let mut parents_by_node: HashMap<NodeIndex, Vec<NodeIndex>> = HashMap::new();
        for node in nodes {
            let parents = self
                .direct_parents(*node)
                .iter()
                .filter(|parent| nodes.contains(parent))
                .copied()
                .collect();
            parents_by_node.insert(*node, parents);
        }

        // Second pass: fold parent references into the combined status.
        Ok(status_by_ref
            .iter()
            .map(|(node, push_status)| {
                (
                    *node,
                    ReferenceStatus {
                        remote_ref: remote_by_ref.get(node).cloned().flatten(),
                        push_status: *push_status,
                        combined_push_status: combined_push_status(
                            *node,
                            *push_status,
                            &parents_by_node,
                            &status_by_ref,
                        ),
                    },
                )
            })
            .collect())
    }

    /// Classify which commits and which local-branch references in the projection
    /// have landed upstream, following the commit-ownership branch of upstream
    /// integration's `reference_integrated` rule.
    ///
    /// A commit is integrated when it is reachable from the target ref
    /// (historically integrated) or content-equivalent to an upstream commit
    /// (via the changeset-similarity engine). A reference is integrated when
    /// every commit it owns down to the next local branch is integrated. A
    /// reference owning no commits is never marked integrated here: the
    /// empty-branch remote-tip fallback that `reference_integrated` has is
    /// intentionally not ported. Without a target there is nothing to integrate
    /// into, so both sets are empty.
    fn integration(&self, nodes: &HashSet<NodeIndex>) -> Result<Integration> {
        let Some(target_ref) = self.target_ref.as_ref() else {
            return Ok(Integration::default());
        };
        let Some(target_ref_index) = self.try_select_reference(target_ref.as_ref()) else {
            return Ok(Integration::default());
        };

        // Historical integration: everything reachable from the target ref.
        let from_target_ref: HashSet<NodeIndex> =
            traverse::reachable_from(self.nodes, target_ref_index).collect();

        let target_index = self.target_index();
        // Content integration: the upstream commits (target ref ahead of its
        // base) cherry-pick-equivalent to a workspace commit.
        let from_target_sha: HashSet<NodeIndex> = match target_index {
            Some(index) => traverse::reachable_from(self.nodes, index).collect(),
            None => HashSet::new(),
        };
        let mut upstream: Vec<NodeIndex> = from_target_ref
            .iter()
            .copied()
            .filter(|index| !from_target_sha.contains(index))
            .collect();
        if upstream.is_empty() {
            upstream = from_target_ref.iter().copied().collect();
        }
        let upstream_ids = self.pick_ids(upstream.into_iter());
        let workspace_ids = self.pick_ids(nodes.iter().copied());
        let content = but_core::changeset::compute_similarity_by_commit_ids(
            self.repo,
            &upstream_ids,
            &workspace_ids,
            true,
        )?;

        let reference_names: HashMap<NodeIndex, gix::refs::FullName> = nodes
            .iter()
            .filter_map(|node| Some((*node, self.ref_name_at(*node)?.clone())))
            .collect();

        let is_commit_integrated = |index: NodeIndex| -> bool {
            if from_target_ref.contains(&index) {
                return true;
            }
            match self.commit_id_at(index) {
                Some(id) => content.matches_by_workspace_commit.contains_key(&id),
                None => false,
            }
        };

        // Commits present on some local branch's remote-tracking branch, used to
        // distinguish `LocalAndRemote` from `LocalOnly`. `remote_reachable` is the
        // identity match (the remote holds this exact commit); `remote_only_ids`
        // are the remote's other commits, against which we content-match local
        // commits to catch rebased-but-pushed commits (the similarity match).
        let mut remote_reachable = HashSet::new();
        let mut remote_only_ids = Vec::new();
        for ref_name in reference_names.values() {
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let (_, Some(remote_index)) = self.remote_for_reference(ref_name.as_ref()) else {
                continue;
            };
            for index in traverse::all_until_optional_limit(self.nodes, remote_index, target_index)
            {
                if !remote_reachable.insert(index) {
                    continue;
                }
                if !nodes.contains(&index)
                    && let Some(id) = self.commit_id_at(index)
                {
                    remote_only_ids.push(id);
                }
            }
        }
        let remote_lut = self.similarity_lut(&remote_only_ids)?;

        // Per-commit state: integrated wins over local-and-remote wins over local-only.
        let mut elapsed = std::time::Duration::default();
        let mut commit_state = HashMap::new();
        for node in nodes {
            let Some(id) = self.commit_id_at(*node) else {
                continue;
            };
            let state = if is_commit_integrated(*node) {
                CommitState::Integrated
            } else if remote_reachable.contains(node) {
                CommitState::LocalAndRemote(id)
            } else if let Some(remote_id) = self.remote_similarity(id, &remote_lut, &mut elapsed)? {
                CommitState::LocalAndRemote(remote_id)
            } else {
                CommitState::LocalOnly
            };
            commit_state.insert(*node, state);
        }

        // Per-reference: a local branch is integrated when all the commits it
        // exclusively owns (down to the next local branch) are integrated.
        let mut integrated_references = HashSet::new();
        for (ref_index, ref_name) in &reference_names {
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let mut tips = vec![*ref_index];
            let mut seen = HashSet::from([*ref_index]);
            let mut all_integrated = true;
            let mut traversed_commits = false;
            'walk: while let Some(tip) = tips.pop() {
                for parent in self.direct_parents(tip) {
                    if !nodes.contains(parent) {
                        continue;
                    }
                    // A local branch owns its own commits, so stop there. Any
                    // other reference (remote, target) acts as an integrated
                    // boundary; commits must themselves be integrated.
                    let parent_is_non_local_ref = match reference_names.get(parent) {
                        Some(name) if name.category() == Some(gix::refs::Category::LocalBranch) => {
                            continue;
                        }
                        Some(_) => true,
                        None => {
                            traversed_commits = true;
                            false
                        }
                    };
                    if seen.insert(*parent) {
                        if !(parent_is_non_local_ref || is_commit_integrated(*parent)) {
                            all_integrated = false;
                            break 'walk;
                        }
                        tips.push(*parent);
                    }
                }
            }
            if traversed_commits && all_integrated {
                integrated_references.insert(*ref_index);
            }
        }
        Ok(Integration {
            commit_state,
            integrated_references,
        })
    }

    /// The commit ids of the commit-like nodes among `indexes` (others dropped).
    fn pick_ids(&self, indexes: impl Iterator<Item = NodeIndex>) -> Vec<gix::ObjectId> {
        indexes
            .filter_map(|index| self.commit_id_at(index))
            .collect()
    }

    /// Build a changeset-similarity lookup table over `commit_ids`.
    fn similarity_lut(&self, commit_ids: &[gix::ObjectId]) -> Result<Identity> {
        let cost_info = (
            commit_ids.len(),
            self.repo.index_or_empty()?.entries().len(),
        );
        create_similarity_lut(
            self.repo,
            commit_ids
                .iter()
                .filter_map(|id| but_core::Commit::from_id(id.attach(self.repo)).ok()),
            cost_info,
            true,
        )
    }

    /// The id of the commit in `lut` that is content-equivalent to the commit
    /// `id` (by change-id, commit data, or changeset id), if any. `elapsed`
    /// bounds the wall-clock spent on the expensive changeset computation.
    fn remote_similarity(
        &self,
        id: gix::ObjectId,
        lut: &Identity,
        elapsed: &mut std::time::Duration,
    ) -> Result<Option<gix::ObjectId>> {
        let commit = but_core::Commit::from_id(id.attach(self.repo))?;
        let expensive = changeset_identifier(self.repo, Some(&commit), elapsed)?;
        Ok(lookup_similar(lut, &commit, expensive.as_ref(), ChangeIdMode::Use).copied())
    }

    /// The push status for a single local branch reference, derived from how it
    /// diverges from its remote-tracking branch.
    fn reference_push_status(
        &self,
        ref_index: NodeIndex,
        refname: &gix::refs::FullNameRef,
    ) -> Result<(Option<gix::refs::FullName>, PushStatus)> {
        let (remote_ref, remote_index) = self.remote_for_reference(refname);
        let Some(remote_index) = remote_index else {
            // Either no tracking branch exists, or the remote exists but its
            // history is outside the workspace view and so can't be compared
            // (rare under real traversals).
            return Ok((remote_ref, PushStatus::CompletelyUnpushed));
        };

        let ahead_behind = AheadBehind {
            ahead: traverse::count_picks(
                self.nodes,
                traverse::a_not_b(self.nodes, ref_index, remote_index),
            ),
            behind: traverse::count_picks(
                self.nodes,
                traverse::a_not_b(self.nodes, remote_index, ref_index),
            ),
        };
        Ok((remote_ref, push_status_from_ahead_behind(ahead_behind)))
    }

    /// Resolve `refname`'s remote-tracking ref name and its node in the graph,
    /// preferring its reference node and falling back to its tip commit
    /// (limited traversals often drop the remote *ref* node while keeping the
    /// commit it points at). Both are `None` when there is no tracking branch;
    /// the node alone is `None` when the remote is outside the graph.
    fn remote_for_reference(
        &self,
        refname: &gix::refs::FullNameRef,
    ) -> (Option<gix::refs::FullName>, Option<NodeIndex>) {
        let Ok(remote_ref) = resolve_tracking_branch_ref_name(refname, self.repo) else {
            return (None, None);
        };
        let remote_ref = remote_ref.into_owned();
        let index = self.try_select_reference(remote_ref.as_ref()).or_else(|| {
            let tip = self
                .repo
                .try_find_reference(remote_ref.as_ref())
                .ok()
                .flatten()?
                .peel_to_id()
                .ok()?
                .detach();
            self.try_select_commit(tip)
        });
        (Some(remote_ref), index)
    }
}

/// Map a reference's divergence from its remote into a [`PushStatus`].
fn push_status_from_ahead_behind(ahead_behind: AheadBehind) -> PushStatus {
    if ahead_behind.behind > 0 {
        // The remote has commits we don't, so pushing rewrites its history.
        PushStatus::UnpushedCommitsRequiringForce
    } else if ahead_behind.ahead > 0 {
        PushStatus::UnpushedCommits
    } else {
        PushStatus::NothingToPush
    }
}

/// Fold a reference's own push status with those of the references below it: if
/// any parent reference requires a force push, so does this one.
///
/// Generic over the node key so the force-escalation walk can be exercised with
/// plain keys in tests. `parents_by_node` may include non-reference nodes
/// (commits) as intermediate hops; only keys present in `status_by_ref` count as
/// references.
fn combined_push_status<K: Copy + Eq + std::hash::Hash>(
    reference: K,
    own: PushStatus,
    parents_by_node: &HashMap<K, Vec<K>>,
    status_by_ref: &HashMap<K, PushStatus>,
) -> PushStatus {
    // An integrated reference isn't pushed at all, so parents can't change that.
    if matches!(own, PushStatus::Integrated) {
        return PushStatus::Integrated;
    }
    let mut tips = vec![reference];
    let mut seen = HashSet::from([reference]);
    while let Some(tip) = tips.pop() {
        for parent in parents_by_node.get(&tip).into_iter().flatten() {
            if !seen.insert(*parent) {
                continue;
            }
            if status_by_ref.get(parent) == Some(&PushStatus::UnpushedCommitsRequiringForce) {
                return PushStatus::UnpushedCommitsRequiringForce;
            }
            tips.push(*parent);
        }
    }
    own
}

/// All nodes in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`.
fn all_until_optional_limit(nodes: &[Node], start: NodeIndex, limit: Option<NodeIndex>) -> NodeSet {
    NodeSet {
        heads: vec![start],
        nodes: traverse::all_until_optional_limit(nodes, start, limit).collect(),
    }
}

/// Split the region beneath the workspace commit into mutually-exclusive stacks,
/// returning `(above_workspace, stacks)`.
fn divide_workspace_into_stacks(
    nodes: &[Node],
    head_not_target: NodeSet,
    workspace_commit_ix: NodeIndex,
) -> (NodeSet, Vec<NodeSet>) {
    // Each parent of the workspace commit seeds a stack.
    let mut initial_stacks = nodes[workspace_commit_ix]
        .parents()
        .iter()
        .map(|parent| NodeSet {
            heads: vec![*parent],
            nodes: [*parent].into(),
        })
        .collect::<Vec<_>>();

    for stack in &mut initial_stacks {
        let mut tips = stack.heads.clone();
        while let Some(tip) = tips.pop() {
            for parent in nodes[tip].parents() {
                if !head_not_target.nodes.contains(parent) {
                    continue;
                }
                if stack.nodes.insert(*parent) {
                    tips.push(*parent);
                }
            }
        }
    }

    // Merge stacks that share any node (they aren't actually distinct).
    //
    // NOTE: a shared node here includes *reference* nodes, not just commits,
    // so an inline reference on both ancestries merges its stacks even when
    // the commit below it is excluded by the target.
    initial_stacks.reverse();
    let mut deduplicated = vec![];
    while let Some(mut out) = initial_stacks.pop() {
        for bix in (0..initial_stacks.len()).rev() {
            #[expect(clippy::indexing_slicing)]
            if out
                .nodes
                .iter()
                .any(|o| initial_stacks[bix].nodes.contains(o))
            {
                let b = initial_stacks.remove(bix);
                out.nodes.extend(b.nodes);
                out.heads.extend(b.heads);
            }
        }
        deduplicated.push(out);
    }

    let mut outside = head_not_target.nodes.clone();
    for stack in &deduplicated {
        outside = outside.difference(&stack.nodes).copied().collect();
    }
    outside.remove(&workspace_commit_ix);

    let above_workspace = NodeSet {
        // The entrypoint is the tip of everything above the workspace commit.
        heads: head_not_target
            .heads
            .iter()
            .cloned()
            .filter(|h| *h != workspace_commit_ix)
            .collect(),
        nodes: outside,
    };

    (above_workspace, deduplicated)
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::{combined_push_status, push_status_from_ahead_behind};
    use crate::edit::traverse::AheadBehind;
    use but_core::ui::PushStatus;

    #[test]
    fn push_status_mapping() {
        let status = |ahead, behind| push_status_from_ahead_behind(AheadBehind { ahead, behind });
        assert_eq!(status(0, 0), PushStatus::NothingToPush);
        assert_eq!(status(2, 0), PushStatus::UnpushedCommits);
        assert_eq!(status(0, 1), PushStatus::UnpushedCommitsRequiringForce);
        // Behind dominates: a diverged branch always needs a force push.
        assert_eq!(status(3, 2), PushStatus::UnpushedCommitsRequiringForce);
    }

    #[test]
    fn combined_status_escalates_from_force_parent() {
        // Stack (child -> parent): top(1) -> commit(2) -> bottom(3) -> main(4).
        // `bottom` requires a force push; `top` sits above it (through a commit
        // hop, which is not a reference) and must inherit force.
        let parents: HashMap<usize, Vec<usize>> =
            HashMap::from([(1, vec![2]), (2, vec![3]), (3, vec![4]), (4, vec![])]);
        let statuses: HashMap<usize, PushStatus> = HashMap::from([
            (1, PushStatus::UnpushedCommits),
            (3, PushStatus::UnpushedCommitsRequiringForce),
            (4, PushStatus::NothingToPush),
        ]);

        // `top` escalates because a force push lives below it.
        assert_eq!(
            combined_push_status(1, PushStatus::UnpushedCommits, &parents, &statuses),
            PushStatus::UnpushedCommitsRequiringForce
        );
        // `bottom` keeps its own force status.
        assert_eq!(
            combined_push_status(
                3,
                PushStatus::UnpushedCommitsRequiringForce,
                &parents,
                &statuses
            ),
            PushStatus::UnpushedCommitsRequiringForce
        );
        // `main` has nothing forcing below it, so it stays as-is.
        assert_eq!(
            combined_push_status(4, PushStatus::NothingToPush, &parents, &statuses),
            PushStatus::NothingToPush
        );
    }
}
