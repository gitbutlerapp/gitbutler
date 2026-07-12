//! A graph based workspace projection, framed from the rebase [`Editor`].
//!
//! Rather than being its own graph, this points into the editor's internal commit
//! graph via [`EditorIndex`]s, so consumers can frame the mutations they're about
//! to perform against the same entries they'll act on.

use crate::graph_rebase::commits::ParentEntry;
use std::collections::{HashMap, HashSet};

use crate::graph_rebase::positions;
use crate::graph_rebase::store::RefIndex;
use anyhow::Result;
use but_core::{
    RefMetadata, WORKSPACE_REF_NAME,
    branch::resolve_tracking_branch_ref_name,
    changeset::{
        ChangeIdMode, Identity, changeset_identifier, create_similarity_lut, lookup_similar,
    },
    ui::{CommitState, PushStatus},
};
use but_graph::workspace::commit::is_managed_workspace_by_message;
use gix::prelude::ObjectIdExt;

use crate::graph_rebase::{
    Checkout, Editor, EditorIndex, EditorStore,
    traverse::{self, AheadBehind},
};

/// A structure that gives a frame of reference to a key subgraph in the
/// workspace framing. This could be the subgraph of all commits above the
/// workspace, or the entries that make up a "stack".
///
/// Rather than being a full graph structure, this provides pointers into the
/// editor's internal commit graph.
pub struct Subgraph {
    /// Entries in the subgraph that only have incoming parent entries
    pub heads: Vec<EditorIndex>,
    /// All the entries in the specified subgraph
    pub entries: HashSet<EditorIndex>,
}

impl Subgraph {
    fn empty() -> Self {
        Self {
            heads: vec![],
            entries: HashSet::new(),
        }
    }
}

/// Provides a frame of reference for the standardized view of the world.
///
/// This is intended to be used only inside the but-workspace crate.
pub struct GraphWorkspace {
    /// If we're on the workspace branch, any commits in the rev-set
    /// `HEAD ^workspace_commit ^target_sha` will be included in this subgraph.
    pub above_workspace: Subgraph,

    /// If we are on the workspace branch, and a workspace commit can be found,
    /// this will be set.
    pub workspace_commit: Option<EditorIndex>,

    /// If we're on the workspace branch, this will contain a list of subgraphs
    /// that represents a stack. These are commits that follow the rev-set
    /// `workspace_commit_parents ^target_sha`
    ///
    /// We consider a stack beneath the workspace commit to be mutually
    /// exclusive sub-graphs of commits that don't have any incoming or outgoing
    /// parent entries to other commits in other stacks.
    ///
    /// Membership is computed over commits: references are positions, not topology, so a
    /// shared ref entry (typically the target's, sitting above an excluded target commit)
    /// cannot glue two distinct stacks together. Each reference joins the stack its position
    /// belongs to — its entering child's, else its commit's — and a group hanging
    /// straight off the workspace commit keeps its own stack even without commits (an empty
    /// branch). A reference whose position lies outside every stack (e.g. the target's own
    /// ref) is in none of them.
    ///
    /// As a natural extension, if we failed to find the workspace commit, this
    /// list will be empty since all the commits will deemed "above_workspace".
    ///
    /// If we're outside of the workspace branch, there will be one stack that
    /// contains all commits in the rev-set `HEAD ^target_sha`.
    pub stacks: Vec<Subgraph>,

    /// Per-reference push and integration status for every local-branch
    /// reference in the projection, keyed by its [`EditorIndex`].
    pub reference_status: HashMap<EditorIndex, ReferenceStatus>,

    /// The [`CommitState`] of every commit (`CommitSpec`) in the projection, keyed by
    /// its [`EditorIndex`]: integrated, local-and-remote, or local-only. Commits are
    /// all local-only without a target. Per-reference integration is exposed as
    /// [`PushStatus::Integrated`].
    pub commit_state: HashMap<EditorIndex, CommitState>,
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
    commit_state: HashMap<EditorIndex, CommitState>,
    integrated_references: HashSet<EditorIndex>,
}

impl GraphWorkspace {
    fn empty() -> Self {
        Self::topology(Subgraph::empty(), None, vec![])
    }

    /// A projection skeleton with empty status maps; they are filled in later.
    fn topology(
        above_workspace: Subgraph,
        workspace_commit: Option<EditorIndex>,
        stacks: Vec<Subgraph>,
    ) -> Self {
        Self {
            above_workspace,
            workspace_commit,
            stacks,
            reference_status: HashMap::new(),
            commit_state: HashMap::new(),
        }
    }
}

/// The index-level analog of [`Subgraph`], used internally so the traversal and
/// set-algebra stay on cheap `EditorIndex`es; converted to [`Subgraph`]s once at
/// the boundary.
struct NodeSet {
    heads: Vec<EditorIndex>,
    entries: HashSet<EditorIndex>,
}

impl NodeSet {
    fn into_subgraph(self) -> Subgraph {
        Subgraph {
            heads: self.heads.into_iter().collect(),
            entries: self.entries.into_iter().collect(),
        }
    }
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Build a graph-based workspace projection framed from this editor, with the
    /// stack partition supplied by the workspace's segment graph — the one authority
    /// for which refs and commits form which stack. The editor contributes what is
    /// editor-native: the entry address space, the above-workspace region, and the
    /// push/integration status decoration.
    pub fn graph_workspace(
        &self,
        stacks: &[but_graph::workspace::SegmentStack],
    ) -> Result<GraphWorkspace> {
        let mut ws = self.graph_workspace_topology(stacks)?;
        // Every entry in the projection, so the status walks stay scoped to
        // the workspace rather than wandering down the full history.
        let entries: HashSet<EditorIndex> = ws
            .above_workspace
            .entries
            .iter()
            .chain(ws.stacks.iter().flat_map(|stack| stack.entries.iter()))
            .copied()
            .collect();
        let integration = self.integration(&entries)?;
        ws.reference_status =
            self.reference_statuses(&entries, &integration.integrated_references)?;
        ws.commit_state = integration.commit_state;
        Ok(ws)
    }

    /// Map the segment-graph `stacks` into editor entries: per stack, every
    /// segment's naming reference and assigned commits. The head is the top segment's
    /// reference (or its first commit). Refs the editor graph doesn't carry are
    /// skipped — status decoration would skip them too.
    fn subgraphs_from_stacks(
        &self,
        stacks: &[but_graph::workspace::SegmentStack],
    ) -> Vec<Subgraph> {
        // Refs that name a stacks segment belong to exactly that stack — they never
        // ride along into a sibling that shares their anchor commit.
        let naming: HashSet<EditorIndex> = stacks
            .iter()
            .flat_map(|s| s.segments.iter())
            .filter_map(|segment| segment.ref_name())
            .filter_map(|name| self.select_reference(name).ok())
            .map(EditorIndex::from)
            .collect();
        stacks
            .iter()
            .map(|stack| {
                let mut entries: HashSet<EditorIndex> = HashSet::new();
                let mut heads = Vec::new();
                for (seg_idx, segment) in stack.segments.iter().enumerate() {
                    if let Some(name) = segment.ref_name()
                        && let Ok(sel) = self.select_reference(name)
                    {
                        if seg_idx == 0 {
                            heads.push(sel.into());
                        }
                        entries.insert(sel.into());
                    }
                    for &id in &segment.commits {
                        if let Some(sel) = self.try_select_commit(id) {
                            if heads.is_empty() && seg_idx == 0 && segment.tip() == Some(id) {
                                heads.push(sel.into());
                            }
                            entries.insert(sel.into());
                        }
                    }
                }
                if heads.is_empty()
                    && let Some(&first) = entries.iter().min()
                {
                    // Deterministic fallback: hash-set iteration order must not commit
                    // the render seed.
                    heads.push(first);
                }
                Subgraph { heads, entries }
            })
            .filter(|subgraph| !subgraph.entries.is_empty())
            .map(|mut subgraph| {
                // Riding references join the stack owning their position: the commit their
                // entering parent entries resolve through, else their resolved commit — the same
                // membership the flooded-region attachment uses.
                let commit_entries: HashSet<EditorIndex> =
                    subgraph.entries.iter().copied().collect();
                let additions: Vec<RefIndex> = self
                    .store
                    .positioned_refs()
                    .filter(|&entry| !naming.contains(&EditorIndex::from(entry)))
                    .filter(|&entry| {
                        positions::entering(&self.store, entry).iter().any(
                            |ParentEntry { child, .. }| {
                                commit_entries.contains(&EditorIndex::from(*child))
                            },
                        ) || self
                            .store
                            .resolve_to_commit(EditorIndex::from(entry))
                            .is_some_and(|commit| {
                                commit_entries.contains(&EditorIndex::from(commit))
                            })
                    })
                    .collect();
                subgraph
                    .entries
                    .extend(additions.into_iter().map(EditorIndex::from));
                subgraph
            })
            .collect()
    }

    /// Build the topological skeleton of the projection (stacks from the supplied
    /// stacks, above-workspace, workspace commit) with an empty
    /// [`GraphWorkspace::reference_status`].
    fn graph_workspace_topology(
        &self,
        stacks: &[but_graph::workspace::SegmentStack],
    ) -> Result<GraphWorkspace> {
        let Some(entrypoint_ix) = self.head_index() else {
            return Ok(GraphWorkspace::empty());
        };

        // Without a target sha:
        // Outside the workspace branch, there is one giant stack with every commit.
        // On the workspace branch:
        //   With a workspace commit, the stacks reach the full history.
        //   Without one, everything from HEAD counts as above the workspace.

        let ws_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
        let on_workspace = self
            .store
            .reference(entrypoint_ix)
            .is_some_and(|(refname, _)| *refname == ws_ref);

        let target_ix = self.target_entry();

        // The entrypoint is a reference: the region floods from the commit it resolves to
        // (references carry no parent entries). The region is the rev-set `HEAD ^target`.
        let entrypoint_pick = self.store.resolve_to_commit(entrypoint_ix);
        let mut region = NodeSet {
            heads: vec![entrypoint_ix],
            entries: entrypoint_pick
                .map(|commit| {
                    traverse::all_until_optional_limit(&self.store, commit.into(), target_ix)
                        .collect()
                })
                .unwrap_or_default(),
        };

        // The workspace commit, if present, lives somewhere in `HEAD ^target`.
        let workspace_commit = on_workspace
            .then(|| {
                region.entries.iter().copied().find_map(|ix| {
                    let id = self.store.commit_id(ix)?;
                    let gix_commit = self.repo.find_commit(id).ok()?;
                    is_managed_workspace_by_message(gix_commit.message_raw().ok()?).then_some(ix)
                })
            })
            .flatten();

        // The stacks come from the partition; the editor only contributes the
        // above-workspace remainder: everything flooded that no stack claimed and
        // that isn't the workspace commit itself, refs attached by position.
        let stacks = self.subgraphs_from_stacks(stacks);
        attach_flooded_refs(&self.store, &mut region.entries, Some(entrypoint_ix));
        let stack_entries: HashSet<EditorIndex> = stacks
            .iter()
            .flat_map(|s| s.entries.iter())
            .copied()
            .collect();
        // A metadata-less legacy shape can claim the workspace commit into a stack (the
        // walk starts on it); it then renders there, not as its own section.
        let workspace_commit = workspace_commit.filter(|ix| !stack_entries.contains(ix));
        let claimed: HashSet<EditorIndex> =
            stack_entries.into_iter().chain(workspace_commit).collect();
        region.entries.retain(|ix| !claimed.contains(ix));
        region.heads.retain(|ix| !claimed.contains(ix));
        if region.heads.is_empty() && !region.entries.is_empty() {
            // The entrypoint was claimed by a stack: re-seed the remainder from its
            // topmost entries (those no other remaining entry lists as a parent).
            let has_child_in_region: HashSet<EditorIndex> = region
                .entries
                .iter()
                .flat_map(|&ix| self.store.parents(ix))
                .map(EditorIndex::from)
                .collect();
            region.heads = region
                .entries
                .iter()
                .copied()
                .filter(|ix| !has_child_in_region.contains(ix))
                .collect();
        }
        Ok(GraphWorkspace::topology(
            region.into_subgraph(),
            workspace_commit,
            stacks,
        ))
    }

    /// The entrypoint (`HEAD`) reference entry, or `None` if HEAD isn't on a ref.
    ///
    /// A linked worktree has its own `HEAD`; only the main one answers here.
    fn head_index(&self) -> Option<EditorIndex> {
        self.checkouts.iter().find_map(|checkout| match checkout {
            Checkout::Head { entry, .. } => Some(*entry),
            Checkout::Worktree { .. } => None,
        })
    }

    /// The target commit's entry, if a target is configured and present.
    fn target_entry(&self) -> Option<EditorIndex> {
        let target = self.project_meta.target_commit_id?;
        self.try_select_commit(target).map(Into::into)
    }

    /// Compute the per-reference status for every local-branch reference in the
    /// projection, given the full projection `entries` and the references already
    /// classified as `integrated`.
    fn reference_statuses(
        &self,
        entries: &HashSet<EditorIndex>,
        integrated: &HashSet<EditorIndex>,
    ) -> Result<HashMap<EditorIndex, ReferenceStatus>> {
        // First pass: each local-branch reference's own remote ref and push status.
        let mut remote_by_ref = HashMap::new();
        let mut status_by_ref = HashMap::new();
        for entry in entries {
            let Some((refname, _)) = self.store.reference(*entry) else {
                continue;
            };
            if refname.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let (remote_ref, push_status) = self.reference_push_status(*entry, refname.as_ref())?;
            remote_by_ref.insert(*entry, remote_ref);
            status_by_ref.insert(*entry, push_status);
        }

        // Integrated references override their push status: nothing to push once
        // the work has landed upstream.
        for entry in integrated {
            if let Some(push_status) = status_by_ref.get_mut(entry) {
                *push_status = PushStatus::Integrated;
            }
        }

        // Adjacency among projection entries, used by the combined walk to reach
        // parent references through intermediate commits.
        let mut parents_by_node: HashMap<EditorIndex, Vec<EditorIndex>> = HashMap::new();
        for entry in entries {
            let parents = self
                .position_parents(*entry)?
                .into_iter()
                .filter(|parent| entries.contains(parent))
                .collect();
            parents_by_node.insert(*entry, parents);
        }

        // Second pass: fold parent references into the combined status.
        status_by_ref
            .iter()
            .map(|(entry, push_status)| {
                Ok((
                    *entry,
                    ReferenceStatus {
                        remote_ref: remote_by_ref.get(entry).cloned().flatten(),
                        push_status: *push_status,
                        combined_push_status: combined_push_status(
                            *entry,
                            *push_status,
                            &parents_by_node,
                            &status_by_ref,
                        ),
                    },
                ))
            })
            .collect()
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
    ///
    /// Note: this and the upstream-integration heuristics are a read-only projection
    /// responsibility accreting on the mutation `Editor`. They could live on their own
    /// type borrowing the editor, keeping the mutation surface focused.
    fn integration(&self, entries: &HashSet<EditorIndex>) -> Result<Integration> {
        let Some(target_ref) = self.project_meta.target_ref.as_ref() else {
            return Ok(Integration::default());
        };
        let Some(target_ref_handle) = self.try_select_reference(target_ref.as_ref()) else {
            return Ok(Integration::default());
        };

        // Historical integration: everything reachable from the target ref.
        let from_target_ref: HashSet<EditorIndex> =
            self.reachable_from(target_ref_handle)?.collect();

        let target_entry = self.target_entry();
        // Content integration: the upstream commits (target ref ahead of its
        // base) cherry-commit-equivalent to a workspace commit.
        let from_target_sha: HashSet<EditorIndex> = match target_entry {
            Some(entry) => self.reachable_from(entry)?.collect(),
            None => HashSet::new(),
        };
        let mut upstream: Vec<EditorIndex> = from_target_ref
            .iter()
            .copied()
            .filter(|entry| !from_target_sha.contains(entry))
            .collect();
        if upstream.is_empty() {
            upstream = from_target_ref.iter().copied().collect();
        }
        let upstream_ids = self.commit_ids(upstream.into_iter());
        let workspace_ids = self.commit_ids(entries.iter().copied());
        let content = but_core::changeset::compute_similarity_by_commit_ids(
            self.repo(),
            &upstream_ids,
            &workspace_ids,
            true,
        )?;

        let reference_names: HashMap<EditorIndex, gix::refs::FullName> = entries
            .iter()
            .filter_map(|entry| {
                self.store
                    .reference(*entry)
                    .map(|(refname, _)| (*entry, refname.clone()))
            })
            .collect();

        let is_commit_integrated = |entry: EditorIndex| -> Result<bool> {
            if from_target_ref.contains(&entry) {
                return Ok(true);
            }
            Ok(match self.store.commit_id(entry) {
                Some(id) => content.matches_by_workspace_commit.contains_key(&id),
                None => false,
            })
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
            let (_, Some(remote_entry)) = self.remote_for_reference(ref_name.as_ref()) else {
                continue;
            };
            for entry in self.all_until_optional_limit(remote_entry, target_entry)? {
                if !remote_reachable.insert(entry) {
                    continue;
                }
                if !entries.contains(&entry)
                    && let Some(id) = self.store.commit_id(entry)
                {
                    remote_only_ids.push(id);
                }
            }
        }
        let remote_lut = self.similarity_lut(&remote_only_ids)?;

        // Per-commit state: integrated wins over local-and-remote wins over local-only.
        let mut elapsed = std::time::Duration::default();
        let mut commit_state = HashMap::new();
        for entry in entries {
            let Some(id) = self.store.commit_id(*entry) else {
                continue;
            };
            let state = if is_commit_integrated(*entry)? {
                CommitState::Integrated
            } else if remote_reachable.contains(entry) {
                CommitState::LocalAndRemote(id)
            } else if let Some(remote_id) = self.remote_similarity(id, &remote_lut, &mut elapsed)? {
                CommitState::LocalAndRemote(remote_id)
            } else {
                CommitState::LocalOnly
            };
            commit_state.insert(*entry, state);
        }

        // Per-reference: a local branch is integrated when all the commits it
        // exclusively owns (down to the next local branch) are integrated.
        let mut integrated_references = HashSet::new();
        for (ref_handle, ref_name) in &reference_names {
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let mut tips = vec![*ref_handle];
            let mut seen = HashSet::from([*ref_handle]);
            let mut all_integrated = true;
            let mut traversed_commits = false;
            'walk: while let Some(tip) = tips.pop() {
                for (parent, _) in self.direct_parents(tip)? {
                    if !entries.contains(&parent) {
                        continue;
                    }
                    // A local branch owns its own commits, so stop there. Any
                    // other reference (remote, target) acts as an integrated
                    // boundary; commits must themselves be integrated.
                    let parent_is_non_local_ref = match reference_names.get(&parent) {
                        Some(name) if name.category() == Some(gix::refs::Category::LocalBranch) => {
                            continue;
                        }
                        Some(_) => true,
                        None => {
                            traversed_commits = true;
                            false
                        }
                    };
                    if seen.insert(parent) {
                        if !(parent_is_non_local_ref || is_commit_integrated(parent)?) {
                            all_integrated = false;
                            break 'walk;
                        }
                        tips.push(parent);
                    }
                }
            }
            if traversed_commits && all_integrated {
                integrated_references.insert(*ref_handle);
            }
        }
        Ok(Integration {
            commit_state,
            integrated_references,
        })
    }

    /// The commit ids of the live commits among `entries` (references and tombstones dropped).
    fn commit_ids(&self, entries: impl Iterator<Item = EditorIndex>) -> Vec<gix::ObjectId> {
        entries
            .filter_map(|entry| self.store.commit_id(entry))
            .collect()
    }

    /// Build a changeset-similarity lookup table over `commit_ids`.
    fn similarity_lut(&self, commit_ids: &[gix::ObjectId]) -> Result<Identity> {
        let cost_info = (
            commit_ids.len(),
            self.repo().index_or_empty()?.entries().len(),
        );
        create_similarity_lut(
            self.repo(),
            commit_ids
                .iter()
                .filter_map(|id| but_core::Commit::from_id(id.attach(self.repo())).ok()),
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
        let commit = but_core::Commit::from_id(id.attach(self.repo()))?;
        let expensive = changeset_identifier(self.repo(), Some(&commit), elapsed)?;
        Ok(lookup_similar(lut, &commit, expensive.as_ref(), ChangeIdMode::Use).copied())
    }

    /// The push status for a single local branch reference, derived from how it
    /// diverges from its remote-tracking branch via [`Editor::ahead_behind`].
    fn reference_push_status(
        &self,
        ref_handle: EditorIndex,
        refname: &gix::refs::FullNameRef,
    ) -> Result<(Option<gix::refs::FullName>, PushStatus)> {
        let (remote_ref, remote_entry) = self.remote_for_reference(refname);
        let Some(remote_entry) = remote_entry else {
            // Either no tracking branch exists, or the remote exists but its
            // history is outside the workspace view and so can't be compared
            // within the editor (rare under real traversals).
            return Ok((remote_ref, PushStatus::CompletelyUnpushed));
        };

        Ok((
            remote_ref,
            push_status_from_ahead_behind(self.ahead_behind(ref_handle, remote_entry)?),
        ))
    }

    /// Resolve `refname`'s remote-tracking ref name and an entry for it in the
    /// editor graph, preferring its reference entry and falling back to its tip
    /// commit (limited traversals often drop the remote *ref* entry while keeping
    /// the commit it points at). Both are `None` when there is no tracking
    /// branch; the entry alone is `None` when the remote is outside the graph.
    fn remote_for_reference(
        &self,
        refname: &gix::refs::FullNameRef,
    ) -> (Option<gix::refs::FullName>, Option<EditorIndex>) {
        let Ok(remote_ref) = resolve_tracking_branch_ref_name(refname, self.repo()) else {
            return (None, None);
        };
        let remote_ref = remote_ref.into_owned();
        let entry = self
            .try_select_reference(remote_ref.as_ref())
            .map(EditorIndex::from)
            .or_else(|| {
                let tip = self
                    .repo()
                    .try_find_reference(remote_ref.as_ref())
                    .ok()
                    .flatten()?
                    .peel_to_id()
                    .ok()?
                    .detach();
                self.try_select_commit(tip).map(EditorIndex::from)
            });
        (Some(remote_ref), entry)
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
/// Generic over the entry key so the force-escalation walk can be exercised with
/// plain keys in tests. `parents_by_node` may include non-reference entries
/// (commits) as intermediate hops; only keys present in `status_by_ref` count as
/// references.
fn combined_push_status<K: Copy + Eq + std::hash::Hash>(
    reference: K,
    own_status: PushStatus,
    parents_by_node: &HashMap<K, Vec<K>>,
    status_by_ref: &HashMap<K, PushStatus>,
) -> PushStatus {
    // An integrated reference isn't pushed at all, so parents can't change that.
    if matches!(own_status, PushStatus::Integrated) {
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
    own_status
}

/// Add the references that belong with the walked commits: groups entered by an in-region
/// child, plus — when the walk started at a reference — that reference and its group
/// members below it. Root groups nothing descends into (e.g. a remote ref stacked above a
/// local one) stay out; no walk ever passes through them.
fn attach_flooded_refs(
    store: &EditorStore,
    entries: &mut HashSet<EditorIndex>,
    entry: Option<EditorIndex>,
) {
    let mut additions: Vec<RefIndex> = store
        .positioned_refs()
        .filter(|&entry| {
            // A group any in-region parent entry enters was flooded through before the walk
            // stopped at a boundary — membership is broader than stack assignment, which
            // the workspace's segment-graph partition decides. Co-located
            // group members all share the same entering parent entries, so a lower member is
            // attached with the whole group, while a root ref stacked above (its own entering set
            // empty, e.g. a remote ref over the tip) stays out.
            positions::entering(store, entry)
                .iter()
                .any(|ParentEntry { child, .. }| entries.contains(&EditorIndex::from(*child)))
        })
        .collect();
    if let Some(entry) = entry.and_then(|e| e.as_ref())
        && let Some(entry_on) = store.positioned_on(entry)
    {
        additions.push(entry);
        let entry_edges = positions::entering(store, entry);
        let entry_depth = positions::ref_depth(store, entry);
        additions.extend(store.positioned_refs().filter(|&other| {
            store.positioned_on(other) == Some(entry_on)
                && positions::entering(store, other) == entry_edges
                && positions::ref_depth(store, other) < entry_depth
        }));
    }
    entries.extend(additions.into_iter().map(EditorIndex::from));
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use super::{combined_push_status, push_status_from_ahead_behind};
    use crate::graph_rebase::traverse::AheadBehind;
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
