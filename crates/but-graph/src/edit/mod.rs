//! Graph editing: the mutable stage of a [`NodeGraph`]'s lifecycle.
//!
//! The lifecycle is `NodeGraph -> MutableNodeGraph -> rebase() -> NodeGraph ->
//! materialize_changes()`: a validated graph is unlocked for mutation, edited
//! in place, rewritten by a rebase that cherry-picks changed commits into an
//! in-memory repository, and finally written back to disk as a single
//! reference transaction.
//!
//! Node indexes are stable across the whole lifecycle: mutation only appends
//! nodes or leaves tombstones ([`NodeKind::None`]), and a rebase swaps commit
//! ids in place. An index obtained before a rebase addresses the same logical
//! node afterwards.

use std::collections::{BTreeMap, HashSet};

use anyhow::{Result, anyhow, bail};
use but_core::commit::SignCommit;
use gix::refs::Category;

use crate::{
    BoundaryKind, Node, NodeGraph, NodeGraphEntrypoint, NodeIndex, NodeKind, RefInfo, Reference,
    node::{child_most, children_of, resolve_to_commit},
};

pub mod cherry_pick;
pub mod commit;
pub mod materialize;
pub mod merge_commit_changes;
pub mod mutate;
pub mod ordering;
pub mod rebase;
/// Utilities for testing
pub mod testing;
pub mod traverse;

pub use cherry_pick::{CherryPickOutcome, PickMode, TreeMergeMode, cherry_pick};
pub use materialize::{MaterializeOptions, MaterializeOutcome};
pub use merge_commit_changes::{
    MergeCommitChangesConflict, MergeCommitChangesOutcome, PlannedCommitChange,
};
pub use mutate::{
    AnySelector, InsertSide, ParentReparentingOrder, RelativeTo, RelativeToRef, SegmentDelimiter,
    SelectorSet, SomeSelectors,
};
pub use rebase::Rebased;
pub use traverse::AheadBehind;

/// Represents a commit to be cherry-picked in a rebase operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Pick {
    /// The ID of the commit getting picked
    pub id: gix::ObjectId,
    /// If we are dealing with a sub-graph with an incomplete history, we
    /// need to represent the bottom most commits in a way that we preserve
    /// their parents.
    ///
    /// If this is Some, the commit WILL NOT be picked onto the parents the
    /// graph implies but instead on to the parents listed here.
    pub preserved_parents: Option<Vec<gix::ObjectId>>,
    /// Controls under what circumstances the commit is cherry-picked.
    pub pick_mode: PickMode,
    /// Controls whether the resulting commit is signed.
    ///
    /// Note that signing a parent commit only causes descendants to be signed if those descendants
    /// are also picked with a `sign_commit` value that enables signing (e.g. [`SignCommit::Yes`]
    /// or [`SignCommit::IfSignCommitsEnabled`] with config enabled).
    pub sign_commit: SignCommit,
    /// Exclude the commit from being included in the
    /// [`Rebased::commit_mappings()`]. This is helpful if we are
    /// creating a new commit since the the mappings will be non-sensical to the
    /// frontend consumers.
    pub exclude_from_tracking: bool,
    /// If set to false, the rebase will fail if this commit results in a
    /// conflicted state. The cherry-pick still runs and creates the
    /// conflicted commit — this check happens afterwards in [`MutableNodeGraph::rebase`].
    pub conflictable: bool,
    /// Controls how parent trees are merged during cherry-pick.
    /// See [`TreeMergeMode`] for details.
    pub tree_merge_mode: TreeMergeMode,
    /// Whether the rebase may rewrite this commit.
    ///
    /// The mutable graph contains every commit in the workspace graph, but only
    /// those reachable from a mutable entrypoint (e.g. `HEAD`) should be
    /// rewritten. When `false`, the rebase copies the pick verbatim instead of
    /// cherry-picking it, preserving its id.
    pub mutable: bool,
}

impl Pick {
    /// Creates a pick with the expected defaults
    pub fn new_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            pick_mode: PickMode::IfChanged,
            sign_commit: SignCommit::IfSignCommitsEnabled,
            exclude_from_tracking: false,
            conflictable: true,
            tree_merge_mode: TreeMergeMode::WithRenames,
            mutable: true,
        }
    }

    /// Creates a pick with the expected defaults, but is excluded from being
    /// included from the [`Rebased::commit_mappings()`] output. This is
    /// often preferable if you are doing something like an
    /// `insert_blank_commit` operation.
    pub fn new_untracked_pick(id: gix::ObjectId) -> Self {
        let mut pick = Self::new_pick(id);
        pick.exclude_from_tracking = true;
        pick
    }

    /// Creates a pick with the defaults set for a workspace commit
    pub fn new_workspace_pick(id: gix::ObjectId) -> Self {
        Self {
            id,
            preserved_parents: None,
            pick_mode: PickMode::IfChanged,
            sign_commit: SignCommit::No,
            exclude_from_tracking: false,
            conflictable: false,
            tree_merge_mode: TreeMergeMode::WithoutRenames,
            mutable: true,
        }
    }

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

/// Edit-only payload stored parallel to the nodes.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum NodePolicy {
    /// Pick options for a commit node.
    Pick(PickSettings),
    /// Mutability of a reference node.
    Reference { mutable: bool },
    /// Boundaries and placeholders carry no payload.
    Inert,
}

/// Options for unlocking a graph for editing.
#[derive(Clone)]
pub struct GraphEditorOptions {
    /// Determines how cherry-picked commits are signed.
    pub default_sign_commit: SignCommit,
    /// References whose ancestry should be forced mutable.
    ///
    /// The mutable graph always contains every node in the workspace graph,
    /// with only those reachable from `HEAD` being mutable. Use this to force a
    /// reference and its ancestry to be mutable so they can be rewritten.
    ///
    /// Reference nodes themselves are only ever mutable when they are local
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

/// Old-to-new commit id tracking across rebases.
///
/// Unintuatively, the values are the original values, and the keys are the
/// _new_ values that they have been mapped to, so that chained rewrites keep
/// pointing at the original commit.
#[derive(Debug, Clone, Default)]
pub(crate) struct CommitMappings(BTreeMap<gix::ObjectId, gix::ObjectId>);

impl CommitMappings {
    /// If there is no entry whose old `to` that corresponds with the new
    /// `from`, then we just add a `to <- from` entry.
    /// If there is an entry whose old `to` that corresponds with the new
    /// `from`, then we replace `old_to <- old_from` with `new_to <- old_from`
    pub(crate) fn update(&mut self, from: gix::ObjectId, to: gix::ObjectId) {
        if let Some(value) = self.0.remove(&from) {
            self.0.insert(to, value);
        } else {
            self.0.insert(to, from);
        }
    }

    /// Provides a mapping from original to rewritten commit ids.
    pub(crate) fn mappings(&self) -> BTreeMap<gix::ObjectId, gix::ObjectId> {
        self.0
            .iter()
            .filter_map(|(k, v)| if k == v { None } else { Some((*v, *k)) })
            .collect()
    }
}

/// Per-edit state that travels with the graph through the whole lifecycle,
/// including across chained rebases.
#[derive(Debug, Clone)]
pub(crate) struct EditSession {
    /// The in-memory repository that the rebase engine works with.
    pub repo: gix::Repository,
    /// Mutable references present when editing started. This is used to track
    /// any references that might need deleted.
    pub initial_references: Vec<gix::refs::FullName>,
    /// Mutable references whose commit chain died during an edit. They are
    /// deliberately left untouched on disk instead of being deleted.
    pub left_behind: HashSet<gix::refs::FullName>,
    /// Old-to-new commit id tracking across rebases.
    pub commit_mappings: CommitMappings,
    /// A pre-computed merge base tree (`HEAD^{tree}` + consumed changes,
    /// additive-only) to pass through to `safe_checkout`. When set, the
    /// 3-way snapshot merge uses this as the base so consumed hunks cancel
    /// and don't reappear as uncommitted changes.
    pub merge_base_override: Option<gix::ObjectId>,
    /// The node `HEAD` was attached to when editing started. Node indexes are
    /// stable, so this keeps HEAD following that node even when the checkout
    /// reference is replaced with a differently-named one.
    pub checkout_index: Option<NodeIndex>,
    /// Workspace lanes that were not part of the managed workspace commit's
    /// merge when editing started (empty stacks exist only as overlay parents
    /// on the workspace reference). Only these may be adopted into the merge
    /// by a rebase — lanes an edit deliberately disconnects must not come
    /// back.
    pub unmerged_lanes: Vec<NodeIndex>,
}

/// A [`NodeGraph`] unlocked for mutation.
///
/// Obtained via [`NodeGraph::into_mut`]; sealed back into a validated
/// [`NodeGraph`] by [`MutableNodeGraph::rebase`].
///
/// Mutation is append-only: removing a node leaves a tombstone
/// ([`NodeKind::None`]) so node indexes stay stable for the whole edit.
#[derive(Debug)]
pub struct MutableNodeGraph {
    pub(crate) nodes: Vec<Node>,
    pub(crate) context: crate::node::ConstructionContext,
    pub(crate) policy: Vec<NodePolicy>,
    pub(crate) session: EditSession,
}

impl NodeGraph {
    /// Unlock this graph for mutation with default options.
    ///
    /// `repo` is the on-disk repository the graph was constructed from; object
    /// writes during the edit go to an in-memory clone of it.
    pub fn into_mut(self, repo: &gix::Repository) -> Result<MutableNodeGraph> {
        self.into_mut_with_opts(repo, &GraphEditorOptions::default())
    }

    /// Unlock this graph for mutation with the specified options.
    pub fn into_mut_with_opts(
        self,
        repo: &gix::Repository,
        options: &GraphEditorOptions,
    ) -> Result<MutableNodeGraph> {
        let NodeGraph {
            nodes,
            annotations: _,
            context,
        } = self;

        let workspace_commit_id = context.managed_workspace_commit_id;
        let mut mutable_entrypoints = Vec::new();
        let mut has_mutable_local_ref = false;
        if let NodeGraphEntrypoint::Node(entrypoint) = &context.entrypoint {
            let symbolic_entrypoint = context
                .entrypoint_ref
                .as_ref()
                .and_then(|name| node_index_by_ref_name(&nodes, name.as_ref()));
            mutable_entrypoints.push(symbolic_entrypoint.unwrap_or(*entrypoint));
            has_mutable_local_ref = symbolic_entrypoint.is_some_and(|_| {
                context
                    .entrypoint_ref
                    .as_ref()
                    .is_some_and(|name| name.category() == Some(Category::LocalBranch))
            });
        }
        for ref_name in &options.extra_mutable_refs {
            let index = node_index_by_ref_name(&nodes, ref_name.as_ref())
                .ok_or_else(|| anyhow!("Failed to find graph node for {ref_name}"))?;
            mutable_entrypoints.push(index);
            has_mutable_local_ref |= ref_name.category() == Some(Category::LocalBranch);
        }

        let mut mutable_nodes = HashSet::new();
        while let Some(index) = mutable_entrypoints.pop() {
            if mutable_nodes.insert(index) {
                mutable_entrypoints.extend(nodes[index].parents().iter().copied());
            }
        }

        // Local branches decorating mutable commits are part of the rewrite even when the node
        // graph keeps them as sibling roots instead of placing them on the entrypoint ancestry.
        if has_mutable_local_ref {
            let mutable_commit_ids = mutable_nodes
                .iter()
                .filter_map(|index| nodes[*index].kind().addressable_commit_id())
                .collect::<HashSet<_>>();
            for (index, node) in nodes.iter().enumerate() {
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

        let addressable_commit_ids = nodes
            .iter()
            .filter_map(|node| node.kind().addressable_commit_id())
            .collect::<HashSet<_>>();

        let mut initial_references = Vec::new();
        let mut step_reference_names = HashSet::new();
        let mut policy = Vec::with_capacity(nodes.len());
        for (index, node) in nodes.iter().enumerate() {
            let mutable = mutable_nodes.contains(&index);
            let node_policy = match node.kind() {
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
                            nodes[*parent].kind(),
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
                    NodePolicy::Pick(pick.into_settings().1)
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
                    NodePolicy::Reference { mutable }
                }
                NodeKind::Boundary { .. } | NodeKind::None => NodePolicy::Inert,
            };
            policy.push(node_policy);
        }

        // Resolve where `HEAD` is attached once, at creation time: node
        // indexes are stable, so materialization can keep HEAD following this
        // node even if the reference it holds is replaced or renamed.
        let checkout_index = match &context.entrypoint {
            NodeGraphEntrypoint::Node(index) => Some(
                context
                    .entrypoint_ref
                    .as_ref()
                    .and_then(|name| node_index_by_ref_name(&nodes, name.as_ref()))
                    .unwrap_or(*index),
            ),
            NodeGraphEntrypoint::Unborn(_) => None,
        };

        let mut graph = MutableNodeGraph {
            nodes,
            context,
            policy,
            session: EditSession {
                repo: repo.clone().with_object_memory(),
                initial_references,
                left_behind: HashSet::new(),
                commit_mappings: CommitMappings::default(),
                merge_base_override: None,
                checkout_index,
                unmerged_lanes: Vec::new(),
            },
        };

        // Lanes outside the workspace merge at capture time are the only ones
        // a rebase may later adopt into it.
        graph.session.unmerged_lanes = workspace_unmerged_lanes(&graph.nodes, workspace_commit_id)
            .map(|(_, lanes)| lanes)
            .unwrap_or_default();

        // An unborn entrypoint gets a mutable reference node so edits can
        // build history on it.
        if let NodeGraphEntrypoint::Unborn(reference) = &graph.context.entrypoint {
            let refname = reference.ref_info.ref_name.clone();
            graph.session.initial_references.push(refname.clone());
            let unborn = graph.add_reference(refname);
            graph.session.checkout_index = Some(unborn);
        }

        Ok(graph)
    }
}

pub(crate) fn node_index_by_ref_name(
    nodes: &[Node],
    name: &gix::refs::FullNameRef,
) -> Option<NodeIndex> {
    nodes.iter().position(|node| {
        matches!(node.kind(), NodeKind::Reference(reference) if reference.ref_info.ref_name.as_ref() == name)
    })
}

/// Locate the managed workspace merge and its lanes.
///
/// Graph construction keeps the workspace's metadata stacks as overlay parents
/// on the workspace *reference* node (own-target commit last), while the
/// workspace *commit* node keeps its on-disk parents. Returns the workspace
/// commit's node and the lanes not sitting on one of the commit's parent
/// paths. That includes empty lanes standing on commits the merge already
/// reaches (e.g. a fresh stack on the merge base): they must stay capturable
/// so [`adopt_workspace_lanes`] can adopt them once an edit gives them
/// commits — adoption re-checks reachability itself.
fn workspace_unmerged_lanes(
    nodes: &[Node],
    workspace_commit_id: Option<gix::ObjectId>,
) -> Option<(NodeIndex, Vec<NodeIndex>)> {
    let managed_id = workspace_commit_id?;
    let ws_ref = nodes.iter().position(|node| {
        let NodeKind::Reference(reference) = node.kind() else {
            return false;
        };
        let is_workspace =
            matches!(
                reference.metadata,
                Some(crate::ReferenceMetadata::Workspace(_))
            ) || but_core::is_workspace_ref_name(reference.ref_info.ref_name.as_ref());
        is_workspace
            && node.parents().split_last().is_some_and(|(own_target, _)| {
                matches!(nodes[*own_target].kind(), NodeKind::Commit { id } if *id == managed_id)
            })
    })?;
    let (own_target, overlay_parents) = nodes[ws_ref]
        .parents()
        .split_last()
        .map(|(own_target, overlay_parents)| (*own_target, overlay_parents.to_vec()))?;

    let ws_commit = own_target;
    let mut reachable = HashSet::new();
    extend_reachable(&mut reachable, nodes[ws_commit].parents(), nodes);

    let mut unmerged = Vec::new();
    for lane in overlay_parents {
        if reachable.contains(&lane) {
            continue;
        }
        if resolve_to_commit(nodes, lane).is_none() {
            continue;
        }
        // Lanes standing on an already-reachable commit stay listed: an empty
        // stack on the merge base is trivially "in" the merge now, but must
        // remain adoptable once an edit gives it commits.
        // `adopt_workspace_lanes` re-checks reachability before adopting.
        unmerged.push(lane);
    }
    Some((ws_commit, unmerged))
}

fn extend_reachable(reachable: &mut HashSet<NodeIndex>, roots: &[NodeIndex], nodes: &[Node]) {
    let mut pending = roots.to_vec();
    while let Some(index) = pending.pop() {
        if reachable.insert(index) {
            pending.extend(nodes[index].parents().iter().copied());
        }
    }
}

/// Adopt workspace lanes that gained commits during the edit into the managed
/// workspace commit's merge.
///
/// A lane that now stands on a commit the merge does not already reach becomes
/// a new last parent of the workspace commit — this is how a commit inserted
/// into a previously empty lane enters the workspace merge. Only lanes listed
/// in `allowed` (those outside the merge when editing began, see
/// [`EditSession::unmerged_lanes`]) are considered: a lane an edit
/// deliberately disconnected from the merge must not come back. Lanes still
/// standing on commits the merge already reaches are left alone, so running
/// this on every rebase of a chain is safe.
pub(crate) fn adopt_workspace_lanes(
    nodes: &mut [Node],
    workspace_commit_id: Option<gix::ObjectId>,
    allowed: &[NodeIndex],
) {
    if allowed.is_empty() {
        return;
    }
    let Some((ws_commit, unmerged)) = workspace_unmerged_lanes(nodes, workspace_commit_id) else {
        return;
    };
    let mut reachable = HashSet::new();
    extend_reachable(&mut reachable, nodes[ws_commit].parents(), nodes);
    let mut adopted = Vec::new();
    for lane in unmerged {
        if !allowed.contains(&lane) || reachable.contains(&lane) {
            continue;
        }
        if resolve_to_commit(nodes, lane).is_some_and(|target| reachable.contains(&target)) {
            continue;
        }
        extend_reachable(&mut reachable, &[lane], nodes);
        adopted.push(lane);
    }
    nodes[ws_commit].parents_mut().extend(adopted);
}

/// Synthesize the [`Pick`] stored at `index` from a node and its policy.
///
/// A convergence boundary reads as an immutable pick (it is a real,
/// addressable commit); references, shallow boundaries, and tombstones carry
/// no pick.
pub(crate) fn pick_of(nodes: &[Node], policy: &[NodePolicy], index: NodeIndex) -> Option<Pick> {
    match nodes[index].kind() {
        NodeKind::Commit { id } => match &policy[index] {
            NodePolicy::Pick(settings) => Some(settings.with_id(*id)),
            NodePolicy::Reference { .. } | NodePolicy::Inert => {
                debug_assert!(false, "BUG: commit node {index} without pick settings");
                Some(Pick::new_pick(*id))
            }
        },
        // A convergence boundary is a real shared commit: keep it addressable
        // so deleting the commit above it can reconnect references to this base.
        NodeKind::Boundary {
            id,
            reason: BoundaryKind::Convergence,
        } => {
            let mut pick = Pick::new_pick(*id);
            pick.mutable = false;
            Some(pick)
        }
        NodeKind::Reference(_)
        | NodeKind::Boundary {
            reason: BoundaryKind::Shallow,
            ..
        }
        | NodeKind::None => None,
    }
}

/// Read the mutability of the reference node at `index`, or `None` if the
/// node is not a reference.
pub(crate) fn reference_mutability_of(
    nodes: &[Node],
    policy: &[NodePolicy],
    index: NodeIndex,
) -> Option<bool> {
    match nodes[index].kind() {
        NodeKind::Reference(_) => match policy[index] {
            NodePolicy::Reference { mutable } => Some(mutable),
            NodePolicy::Pick(_) | NodePolicy::Inert => {
                debug_assert!(false, "BUG: reference node {index} without policy");
                Some(false)
            }
        },
        NodeKind::Commit { .. } | NodeKind::Boundary { .. } | NodeKind::None => None,
    }
}

impl MutableNodeGraph {
    /// Return all nodes in insertion order.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// The number of nodes, including tombstones.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the graph holds no nodes at all.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// All node indexes.
    pub fn indices(&self) -> std::ops::Range<NodeIndex> {
        0..self.nodes.len()
    }

    /// The ordered parent indexes of `index`.
    pub fn parents(&self, index: NodeIndex) -> &[NodeIndex] {
        self.nodes[index].parents()
    }

    /// Mutable access to the ordered parent indexes of `index`.
    pub fn parents_mut(&mut self, index: NodeIndex) -> &mut Vec<NodeIndex> {
        self.nodes[index].parents_mut()
    }

    /// All `(child, parent_slot)` pairs naming `index` as a parent.
    pub fn children(&self, index: NodeIndex) -> Vec<(NodeIndex, usize)> {
        children_of(&self.nodes, index)
    }

    /// All nodes that no other node names as a parent.
    pub fn child_most(&self) -> Vec<NodeIndex> {
        child_most(&self.nodes)
    }

    /// Returns a reference to the in-memory repository.
    ///
    /// Objects written during the edit exist only here until
    /// [`Rebased::materialize_changes`] persists them.
    pub fn repo(&self) -> &gix::Repository {
        &self.session.repo
    }

    /// Set a merge-base override for checkout so that consumed worktree
    /// changes don't reappear as uncommitted after materialization.
    pub fn set_merge_base_override(&mut self, tree_id: gix::ObjectId) {
        self.session.merge_base_override = Some(tree_id);
    }

    /// The graph's traversal entrypoint (`HEAD`).
    pub fn entrypoint(&self) -> &NodeGraphEntrypoint {
        &self.context.entrypoint
    }

    /// Return the reference used to start traversal, if `HEAD` was symbolic.
    pub fn entrypoint_ref(&self) -> Option<&gix::refs::FullNameRef> {
        self.context
            .entrypoint_ref
            .as_ref()
            .map(|name| name.as_ref())
    }

    /// Return the managed workspace commit discovered during construction, if any.
    pub fn managed_workspace_commit_id(&self) -> Option<gix::ObjectId> {
        self.context.managed_workspace_commit_id
    }

    /// Return project-wide target and push metadata.
    pub fn project_meta(&self) -> &but_core::ref_metadata::ProjectMeta {
        &self.context.project_meta
    }

    /// Synthesize the [`Pick`] stored at `index`.
    ///
    /// A convergence boundary reads as an immutable pick (it is a real,
    /// addressable commit); references, shallow boundaries, and tombstones
    /// return `None`.
    pub fn pick_at(&self, index: NodeIndex) -> Option<Pick> {
        pick_of(&self.nodes, &self.policy, index)
    }

    /// Read the mutability of the reference node at `index`, or `None` if the
    /// node is not a reference.
    pub fn reference_mutability(&self, index: NodeIndex) -> Option<bool> {
        reference_mutability_of(&self.nodes, &self.policy, index)
    }

    /// Turn the node at `index` into a commit holding `pick`.
    pub(crate) fn install_pick(&mut self, index: NodeIndex, pick: Pick) {
        let (id, settings) = pick.into_settings();
        self.nodes[index].set_kind(NodeKind::Commit { id });
        self.policy[index] = NodePolicy::Pick(settings);
    }

    /// Turn the node at `index` into a mutable reference named `refname`.
    ///
    /// Installing a reference of the node's current name keeps the node's
    /// discovered reference information.
    pub(crate) fn install_reference(&mut self, index: NodeIndex, refname: gix::refs::FullName) {
        let keep_node = matches!(
            self.nodes[index].kind(),
            NodeKind::Reference(reference) if reference.ref_info.ref_name == refname
        );
        if !keep_node {
            // When the checked-out reference is replaced in place by a
            // differently named reference, `HEAD` follows the
            // replacement.
            if let NodeKind::Reference(old) = self.nodes[index].kind()
                && self.context.entrypoint_ref.as_ref() == Some(&old.ref_info.ref_name)
            {
                self.context.entrypoint_ref = Some(refname.clone());
            }
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
        self.policy[index] = NodePolicy::Reference { mutable: true };
    }

    /// Turn the node at `index` into a tombstone.
    pub(crate) fn install_none(&mut self, index: NodeIndex) {
        self.nodes[index].set_kind(NodeKind::None);
        self.policy[index] = NodePolicy::Inert;
    }

    fn push_inert_node(&mut self) -> NodeIndex {
        let index = self.nodes.len();
        self.nodes.push(Node::new(NodeKind::None, Vec::new()));
        self.policy.push(NodePolicy::Inert);
        index
    }

    /// Append a disconnected commit node holding `pick`, returning its index.
    ///
    /// Almost always you really want to use an `insert_commit*` function
    /// instead.
    pub fn add_commit(&mut self, pick: Pick) -> NodeIndex {
        let index = self.push_inert_node();
        self.install_pick(index, pick);
        index
    }

    /// Append a disconnected mutable reference node, returning its index.
    ///
    /// Almost always you really want to use [`Self::insert_reference`]
    /// instead.
    pub fn add_reference(&mut self, refname: gix::refs::FullName) -> NodeIndex {
        let index = self.push_inert_node();
        self.install_reference(index, refname);
        index
    }

    /// The entrypoint node preferring the symbolic entrypoint reference's node,
    /// or `None` for an unborn entrypoint whose reference node vanished.
    pub(crate) fn head_index(&self) -> Option<NodeIndex> {
        let symbolic = self
            .context
            .entrypoint_ref
            .as_ref()
            .and_then(|name| node_index_by_ref_name(&self.nodes, name.as_ref()));
        match (&self.context.entrypoint, symbolic) {
            (_, Some(symbolic)) => Some(symbolic),
            (NodeGraphEntrypoint::Node(index), None) => Some(*index),
            (NodeGraphEntrypoint::Unborn(reference), None) => {
                node_index_by_ref_name(&self.nodes, reference.ref_info.ref_name.as_ref())
            }
        }
    }
}

/// Convert a structure to a node index for a particular mutable graph.
pub trait ToSelector {
    /// Converts a given object into a node index. Calling `to_selector` on an
    /// object asserts that the receiver is an object that is selectable in the
    /// graph.
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex>;
}

/// Convert a type to a node index, and ensure that it is a commit.
pub trait ToCommitSelector {
    /// Converts a given object into a node index. Calling `to_commit_selector`
    /// on an object asserts that the receiver is a selectable, addressable
    /// commit in the graph.
    fn to_commit_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex>;
}

/// Convert a type to a node index, and ensure that it is a reference.
pub trait ToReferenceSelector {
    /// Converts a given object into a node index. Calling
    /// `to_reference_selector` on an object asserts that the receiver is a
    /// selectable reference node in the graph.
    fn to_reference_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex>;
}

impl ToSelector for NodeIndex {
    fn to_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        if *self >= graph.nodes.len() {
            bail!("Node index {self} is out of bounds for the graph");
        }
        Ok(*self)
    }
}

impl ToCommitSelector for NodeIndex {
    fn to_commit_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        if graph.pick_at(*self).is_none() {
            let kind = graph.nodes[*self].kind();
            bail!("Expected selector for {kind:?} to refer to a commit");
        }
        Ok(*self)
    }
}

impl ToReferenceSelector for NodeIndex {
    fn to_reference_selector(&self, graph: &MutableNodeGraph) -> Result<NodeIndex> {
        let kind = graph.nodes[*self].kind();
        if !matches!(kind, NodeKind::Reference(_)) {
            bail!("Expected selector for {kind:?} to refer to a reference");
        }
        Ok(*self)
    }
}

/// The dead-chain resolution result for a reference during sealing.
pub(crate) fn resolve_reference_targets(
    nodes: &mut Vec<Node>,
    policy: &mut [NodePolicy],
    left_behind: &mut HashSet<gix::refs::FullName>,
) {
    for index in 0..nodes.len() {
        let NodeKind::Reference(_) = nodes[index].kind() else {
            continue;
        };
        let target = resolve_to_commit(nodes, index)
            .and_then(|target| nodes[target].kind().addressable_commit_id());
        match target {
            Some(id) => {
                let NodeKind::Reference(reference) = &mut nodes[index].kind else {
                    unreachable!("checked above");
                };
                reference.ref_info.commit_id = Some(id);
            }
            None => {
                let NodeKind::Reference(reference) = nodes[index].kind() else {
                    unreachable!("checked above");
                };
                let name = reference.ref_info.ref_name.clone();
                let mutable = matches!(policy[index], NodePolicy::Reference { mutable: true });
                tracing::warn!(
                    reference = %name,
                    "reference lost its commit chain during an edit; leaving it behind untouched"
                );
                if mutable {
                    left_behind.insert(name);
                }
                nodes[index].set_kind(NodeKind::None);
                policy[index] = NodePolicy::Inert;
            }
        }
    }
}

pub(crate) fn find_commit_node(nodes: &[Node], id: gix::ObjectId) -> Option<NodeIndex> {
    nodes.iter().position(
        |node| matches!(node.kind(), NodeKind::Commit { id: candidate } if *candidate == id),
    )
}

/// Recompute [`crate::CommitFlags`] annotations for a sealed graph.
///
/// This approximates construction-time annotations by flooding reachability
/// from the entrypoint and target: commits get flagged, references are
/// transparent through their target (last) parent, tombstones through all
/// parents, boundaries stop the walk. The post-materialize disk retraverse
/// remains the canonical source; these flags serve in-memory previews.
pub(crate) fn recompute_annotations(
    nodes: &[Node],
    context: &crate::node::ConstructionContext,
) -> Vec<crate::CommitFlags> {
    use crate::CommitFlags;
    let mut annotations = vec![CommitFlags::empty(); nodes.len()];

    let flood = |roots: Vec<NodeIndex>, flag: CommitFlags, annotations: &mut Vec<CommitFlags>| {
        let mut pending = roots;
        let mut seen = HashSet::new();
        while let Some(index) = pending.pop() {
            if !seen.insert(index) {
                continue;
            }
            match nodes[index].kind() {
                NodeKind::Commit { .. } => {
                    annotations[index] |= flag;
                    pending.extend(nodes[index].parents().iter().copied());
                }
                NodeKind::Reference(_) | NodeKind::None => {
                    pending.extend(crate::node::expansion_slots(nodes, index).iter().copied());
                }
                NodeKind::Boundary { .. } => {}
            }
        }
    };

    if let NodeGraphEntrypoint::Node(entrypoint) = &context.entrypoint {
        flood(
            vec![*entrypoint],
            CommitFlags::EntrypointSide,
            &mut annotations,
        );
    }

    let mut target_roots = Vec::new();
    if let Some(target_ref) = context.project_meta.target_ref.as_ref()
        && let Some(index) = node_index_by_ref_name(nodes, target_ref.as_ref())
    {
        target_roots.push(index);
    }
    if let Some(target_id) = context.project_meta.target_commit_id
        && let Some(index) = find_commit_node(nodes, target_id)
    {
        target_roots.push(index);
    }
    if !target_roots.is_empty() {
        flood(target_roots, CommitFlags::TargetSide, &mut annotations);
    }

    annotations
}

/// I wanted to assert _somewhere_ the defaults for non-workspace & workspace commits.
#[cfg(test)]
mod test {
    use std::str::FromStr;

    use but_core::commit::SignCommit;

    use super::{Pick, PickMode, TreeMergeMode};

    #[test]
    fn workspace_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_workspace_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                pick_mode: PickMode::IfChanged,
                sign_commit: SignCommit::No,
                exclude_from_tracking: false,
                conflictable: false,
                tree_merge_mode: TreeMergeMode::WithoutRenames,
                mutable: true,
            }
        );

        Ok(())
    }

    #[test]
    fn regular_commit_defaults() -> anyhow::Result<()> {
        let object_id = gix::ObjectId::from_str("1000000000000000000000000000000000000000")?;

        assert_eq!(
            Pick::new_pick(object_id),
            Pick {
                id: object_id,
                preserved_parents: None,
                pick_mode: PickMode::IfChanged,
                sign_commit: SignCommit::IfSignCommitsEnabled,
                exclude_from_tracking: false,
                conflictable: true,
                tree_merge_mode: TreeMergeMode::WithRenames,
                mutable: true,
            }
        );

        Ok(())
    }
}
