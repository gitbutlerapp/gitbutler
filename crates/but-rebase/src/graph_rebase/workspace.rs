//! A graph based workspace projection, framed from the rebase [`Editor`].
//!
//! Rather than being its own graph, this points into the editor's internal commit
//! graph via [`Selector`]s, so consumers can frame the mutations they're about
//! to perform against the same selectors they'll act on.

use std::collections::{HashMap, HashSet};

use crate::graph_rebase::positions;
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
    Checkout, Editor, EditorGraph, EditorGraphIndex, LookupStep, Pick, Selector, Step,
    traverse::{self, AheadBehind},
};

/// A structure that gives a frame of reference to a key subgraph in the
/// workspace framing. This could be the subgraph of all commits above the
/// workspace, or the entries that make up a "stack".
///
/// Rather than being a full graph structure, this provides pointers into the
/// editor's internal commit graph.
pub struct Subgraph {
    /// Entries in the subgraph that only have incoming edges
    pub heads: Vec<Selector>,
    /// All the entries in the specified subgraph
    pub entries: HashSet<Selector>,
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
    pub workspace_commit: Option<Selector>,

    /// If we're on the workspace branch, this will contain a list of subgraphs
    /// that represents a stack. These are commits that follow the rev-set
    /// `workspace_commit_parents ^target_sha`
    ///
    /// We consider a stack beneath the workspace commit to be mutually
    /// exclusive sub-graphs of commits that don't have any incoming or outgoing
    /// edges to other commits in other stacks.
    ///
    /// Membership is computed over COMMITS: references are positions, not topology, so a
    /// shared ref entry (typically the target's, sitting above an excluded target commit)
    /// cannot glue two distinct stacks together. Each reference joins the stack its position
    /// belongs to — its entering child's, else its pick's — and a group hanging
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
    /// reference in the projection, keyed by its [`Selector`].
    pub reference_status: HashMap<Selector, ReferenceStatus>,

    /// The [`CommitState`] of every commit (`Pick`) in the projection, keyed by
    /// its [`Selector`]: integrated, local-and-remote, or local-only. Commits are
    /// all local-only without a target. Per-reference integration is exposed as
    /// [`PushStatus::Integrated`].
    pub commit_state: HashMap<Selector, CommitState>,
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
    commit_state: HashMap<Selector, CommitState>,
    integrated_references: HashSet<Selector>,
}

impl GraphWorkspace {
    fn empty() -> Self {
        Self::topology(Subgraph::empty(), None, vec![])
    }

    /// A projection skeleton with empty status maps; they are filled in later.
    fn topology(
        above_workspace: Subgraph,
        workspace_commit: Option<Selector>,
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
/// set-algebra stay on cheap `EditorGraphIndex`es; converted to selectors once at
/// the boundary.
struct NodeSet {
    heads: Vec<EditorGraphIndex>,
    entries: HashSet<EditorGraphIndex>,
}

impl NodeSet {
    fn into_subgraph(self) -> Subgraph {
        Subgraph {
            heads: self.heads.into_iter().map(|id| Selector { id }).collect(),
            entries: self.entries.into_iter().map(|id| Selector { id }).collect(),
        }
    }
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Build a graph-based workspace projection framed from this editor.
    pub fn graph_workspace(&self) -> Result<GraphWorkspace> {
        let mut ws = self.graph_workspace_topology()?;
        // Every selector in the projection, so the status walks stay scoped to
        // the workspace rather than wandering down the full history.
        let entries: HashSet<Selector> = ws
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

    /// Build the topological skeleton of the projection (stacks, above-workspace,
    /// workspace commit) with an empty [`GraphWorkspace::reference_status`].
    fn graph_workspace_topology(&self) -> Result<GraphWorkspace> {
        let Some(entrypoint_ix) = self.head_index() else {
            return Ok(GraphWorkspace::empty());
        };

        // In the case of no target sha:
        // In PGM: We have one giant stack that contains all commits
        // In A workspace:
        //   If we find a workspace commit, we have stacks that reach the full history.
        //   If we don't find a workspace commit, all commits from HEAD are considered above the workspace.

        let ws_ref: gix::refs::FullName = WORKSPACE_REF_NAME.try_into()?;
        let on_workspace = self
            .graph
            .reference(entrypoint_ix)
            .is_some_and(|(refname, _)| *refname == ws_ref);

        let target_ix = self.target_selector().map(|s| s.id);

        // The entrypoint is a reference: the region floods from the pick it resolves to
        // (references carry no edges). The region is the rev-set `HEAD ^target`.
        let entrypoint_pick = positions::resolve_to_pick(&self.graph, entrypoint_ix);
        let mut region = NodeSet {
            heads: vec![entrypoint_ix],
            entries: entrypoint_pick
                .map(|pick| {
                    traverse::all_until_optional_limit(&self.graph, pick, target_ix).collect()
                })
                .unwrap_or_default(),
        };

        if on_workspace {
            // The workspace commit, if present, lives somewhere in `HEAD ^target`.
            let workspace_commit = region.entries.iter().copied().find_map(|ix| {
                let id = self.graph.commit_id(ix)?;
                let gix_commit = self.repo.find_commit(id).ok()?;
                is_managed_workspace_by_message(gix_commit.message_raw().ok()?).then_some(ix)
            });

            if let Some(workspace_commit_ix) = workspace_commit {
                let (above_workspace, stacks) =
                    divide_workspace_into_stacks(&self.graph, region, workspace_commit_ix);

                Ok(GraphWorkspace::topology(
                    above_workspace.into_subgraph(),
                    Some(self.new_selector(workspace_commit_ix)),
                    stacks.into_iter().map(|s| s.into_subgraph()).collect(),
                ))
            } else {
                attach_flooded_refs(&self.graph, &mut region.entries, Some(entrypoint_ix));
                Ok(GraphWorkspace::topology(
                    region.into_subgraph(),
                    None,
                    vec![],
                ))
            }
        } else {
            // We're pegging.
            attach_flooded_refs(&self.graph, &mut region.entries, Some(entrypoint_ix));
            Ok(GraphWorkspace::topology(
                Subgraph::empty(),
                None,
                vec![region.into_subgraph()],
            ))
        }
    }

    /// The entrypoint (`HEAD`) reference entry, or `None` if HEAD isn't on a ref.
    fn head_index(&self) -> Option<EditorGraphIndex> {
        self.checkouts
            .first()
            .map(|Checkout::Head { selector, .. }| selector.id)
    }

    /// The target commit's entry, if a target is configured and present.
    fn target_selector(&self) -> Option<Selector> {
        let target = self.project_meta.target_commit_id?;
        self.try_select_commit(target)
    }

    /// Compute the per-reference status for every local-branch reference in the
    /// projection, given the full projection `entries` and the references already
    /// classified as `integrated`.
    fn reference_statuses(
        &self,
        entries: &HashSet<Selector>,
        integrated: &HashSet<Selector>,
    ) -> Result<HashMap<Selector, ReferenceStatus>> {
        // First pass: each local-branch reference's own remote ref and push status.
        let mut remote_by_ref = HashMap::new();
        let mut status_by_ref = HashMap::new();
        for entry in entries {
            let Step::Reference { refname, .. } = self.lookup_step(*entry)? else {
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
        for selector in integrated {
            if let Some(push_status) = status_by_ref.get_mut(selector) {
                *push_status = PushStatus::Integrated;
            }
        }

        // Adjacency among projection entries, used by the combined walk to reach
        // parent references through intermediate commits.
        let mut parents_by_node: HashMap<Selector, Vec<Selector>> = HashMap::new();
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
    fn integration(&self, entries: &HashSet<Selector>) -> Result<Integration> {
        let Some(target_ref) = self.project_meta.target_ref.as_ref() else {
            return Ok(Integration::default());
        };
        let Some(target_ref_selector) = self.try_select_reference(target_ref.as_ref()) else {
            return Ok(Integration::default());
        };

        // Historical integration: everything reachable from the target ref.
        let from_target_ref: HashSet<Selector> =
            self.reachable_from(target_ref_selector)?.collect();

        let target_selector = self.target_selector();
        // Content integration: the upstream commits (target ref ahead of its
        // base) cherry-pick-equivalent to a workspace commit.
        let from_target_sha: HashSet<Selector> = match target_selector {
            Some(selector) => self.reachable_from(selector)?.collect(),
            None => HashSet::new(),
        };
        let mut upstream: Vec<Selector> = from_target_ref
            .iter()
            .copied()
            .filter(|selector| !from_target_sha.contains(selector))
            .collect();
        if upstream.is_empty() {
            upstream = from_target_ref.iter().copied().collect();
        }
        let upstream_ids = self.pick_ids(upstream.into_iter())?;
        let workspace_ids = self.pick_ids(entries.iter().copied())?;
        let content = but_core::changeset::compute_similarity_by_commit_ids(
            self.repo(),
            &upstream_ids,
            &workspace_ids,
            true,
        )?;

        let reference_names: HashMap<Selector, gix::refs::FullName> = entries
            .iter()
            .filter_map(|entry| match self.lookup_step(*entry) {
                Ok(Step::Reference { refname, .. }) => Some(Ok((*entry, refname))),
                Ok(_) => None,
                Err(err) => Some(Err(err)),
            })
            .collect::<Result<_>>()?;

        let is_commit_integrated = |selector: Selector| -> Result<bool> {
            if from_target_ref.contains(&selector) {
                return Ok(true);
            }
            Ok(match self.lookup_step(selector)? {
                Step::Pick(Pick { id, .. }) => {
                    content.matches_by_workspace_commit.contains_key(&id)
                }
                _ => false,
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
            let (_, Some(remote_selector)) = self.remote_for_reference(ref_name.as_ref()) else {
                continue;
            };
            for selector in self.all_until_optional_limit(remote_selector, target_selector)? {
                if !remote_reachable.insert(selector) {
                    continue;
                }
                if !entries.contains(&selector)
                    && let Step::Pick(Pick { id, .. }) = self.lookup_step(selector)?
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
            let Step::Pick(Pick { id, .. }) = self.lookup_step(*entry)? else {
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
        for (ref_selector, ref_name) in &reference_names {
            if ref_name.category() != Some(gix::refs::Category::LocalBranch) {
                continue;
            }
            let mut tips = vec![*ref_selector];
            let mut seen = HashSet::from([*ref_selector]);
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
                integrated_references.insert(*ref_selector);
            }
        }
        Ok(Integration {
            commit_state,
            integrated_references,
        })
    }

    /// The commit ids of the `Pick` steps among `selectors` (non-picks dropped).
    fn pick_ids(&self, selectors: impl Iterator<Item = Selector>) -> Result<Vec<gix::ObjectId>> {
        let mut out = Vec::new();
        for selector in selectors {
            if let Step::Pick(Pick { id, .. }) = self.lookup_step(selector)? {
                out.push(id);
            }
        }
        Ok(out)
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
        ref_selector: Selector,
        refname: &gix::refs::FullNameRef,
    ) -> Result<(Option<gix::refs::FullName>, PushStatus)> {
        let (remote_ref, remote_selector) = self.remote_for_reference(refname);
        let Some(remote_selector) = remote_selector else {
            // Either no tracking branch exists, or the remote exists but its
            // history is outside the workspace view and so can't be compared
            // within the editor (rare under real traversals).
            return Ok((remote_ref, PushStatus::CompletelyUnpushed));
        };

        Ok((
            remote_ref,
            push_status_from_ahead_behind(self.ahead_behind(ref_selector, remote_selector)?),
        ))
    }

    /// Resolve `refname`'s remote-tracking ref name and a selector for it in the
    /// editor graph, preferring its reference entry and falling back to its tip
    /// commit (limited traversals often drop the remote *ref* entry while keeping
    /// the commit it points at). Both are `None` when there is no tracking
    /// branch; the selector alone is `None` when the remote is outside the graph.
    fn remote_for_reference(
        &self,
        refname: &gix::refs::FullNameRef,
    ) -> (Option<gix::refs::FullName>, Option<Selector>) {
        let Ok(remote_ref) = resolve_tracking_branch_ref_name(refname, self.repo()) else {
            return (None, None);
        };
        let remote_ref = remote_ref.into_owned();
        let selector = self.try_select_reference(remote_ref.as_ref()).or_else(|| {
            let tip = self
                .repo()
                .try_find_reference(remote_ref.as_ref())
                .ok()
                .flatten()?
                .peel_to_id()
                .ok()?
                .detach();
            self.try_select_commit(tip)
        });
        (Some(remote_ref), selector)
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

/// Split the region beneath the workspace commit into mutually-exclusive stacks,
/// returning `(above_workspace, stacks)`.
///
/// Membership is computed over PICKS: references are transparent for connectivity (they are
/// positions, not topology), so a shared ref entry can no longer glue two distinct stacks
/// together — the limitation formerly documented on [`GraphWorkspace::stacks`]. After the
/// pick-flood, each reference joins the stack its position belongs to: the stack of its
/// entering child, else the stack of its resolved pick, and a group hanging directly
/// off the workspace commit keeps its own (possibly pick-less) stack — the empty-branch case.
fn divide_workspace_into_stacks(
    graph: &EditorGraph,
    head_not_target: NodeSet,
    workspace_commit_ix: EditorGraphIndex,
) -> (NodeSet, Vec<NodeSet>) {
    // Each parent of the workspace commit seeds a stack, flooded pick-to-pick: every outgoing
    // edge resolves through reference/tombstone steps to the pick beneath.
    let mut initial_stacks = graph
        .parents(workspace_commit_ix)
        .iter()
        .copied()
        .map(|head| {
            let mut entries = std::collections::HashSet::new();
            let mut tips = Vec::new();
            if let Some(pick) = positions::resolve_to_pick(graph, head)
                && head_not_target.entries.contains(&pick)
            {
                entries.insert(pick);
                tips.push(pick);
            }
            while let Some(tip) = tips.pop() {
                for parent in graph.parents(tip) {
                    let Some(pick) = positions::resolve_to_pick(graph, parent) else {
                        continue;
                    };
                    if !head_not_target.entries.contains(&pick) {
                        continue;
                    }
                    if entries.insert(pick) {
                        tips.push(pick);
                    }
                }
            }
            NodeSet {
                heads: vec![head],
                entries,
            }
        })
        .collect::<Vec<_>>();

    // Merge stacks that share any pick (they aren't actually distinct). The pop-loop takes
    // from the back, so reverse first: stacks come out in the workspace commit's slot order,
    // first parent first.
    let mut deduplicated = vec![];
    initial_stacks.reverse();
    while let Some(mut out) = initial_stacks.pop() {
        for bix in (0..initial_stacks.len()).rev() {
            #[expect(clippy::indexing_slicing)]
            if out
                .entries
                .iter()
                .any(|o| initial_stacks[bix].entries.contains(o))
            {
                let b = initial_stacks.swap_remove(bix);
                out.entries.extend(b.entries);
                out.heads.extend(b.heads);
            }
        }
        deduplicated.push(out);
    }

    // Each positioned reference joins the stack its position belongs to: the entering child's
    // stack (with a group hanging straight off the workspace commit falling back to its
    // pick's stack — the workspace commit itself is in none), else the resolved pick's stack.
    // References belonging to neither (e.g. the target's own ref above the excluded target
    // commit) stay outside every stack.
    for (entry, stored) in graph.positioned_refs() {
        let pick = positions::resolve_to_pick(graph, stored.on);
        let entering = positions::edges_through(graph, entry);
        let by_pick = |a: Option<EditorGraphIndex>| {
            a.and_then(|a| deduplicated.iter().position(|s| s.entries.contains(&a)))
        };
        // Every entering edge must agree on the stack; a group entered from several stacks
        // (or from the workspace commit itself) falls back to its pick's stack. A root group
        // (no entering edges) has no stack — no flood ever descended into it.
        let pick_in_region = pick.is_some_and(|a| head_not_target.entries.contains(&a));
        let home = match entering.as_slice() {
            // A root group: no flood ever descended into it — no stack.
            [] => None,
            // A single entering edge follows its child's stack, even onto an excluded pick (a stack
            // bottom resting on the target); a group hanging straight off the workspace
            // commit falls back to its pick's stack.
            [(child, _)] if *child != workspace_commit_ix && !stored.ambiguous => deduplicated
                .iter()
                .position(|s| s.entries.contains(child))
                .or_else(|| pick_in_region.then(|| by_pick(pick)).flatten()),
            [_] => pick_in_region.then(|| by_pick(pick)).flatten(),
            // A shared group: every edge must agree on the stack; otherwise it belongs to its
            // pick's stack when that is in region, or nowhere.
            many => {
                let homes: Vec<Option<usize>> = many
                    .iter()
                    .map(|(child, _)| {
                        if *child == workspace_commit_ix {
                            None
                        } else {
                            deduplicated.iter().position(|s| s.entries.contains(child))
                        }
                    })
                    .collect();
                match homes.as_slice() {
                    [Some(first), rest @ ..] if rest.iter().all(|h| *h == Some(*first)) => {
                        Some(*first)
                    }
                    _ => pick_in_region.then(|| by_pick(pick)).flatten(),
                }
            }
        };
        if let Some(ix) = home {
            #[expect(clippy::indexing_slicing)]
            deduplicated[ix].entries.insert(entry);
        }
    }

    let mut outside = head_not_target.entries.clone();
    attach_flooded_refs(graph, &mut outside, None);
    for stack in &deduplicated {
        outside = outside.difference(&stack.entries).copied().collect();
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
        entries: outside,
    };

    (above_workspace, deduplicated)
}

/// Insert the positioned references a downward flood over `entries` would have passed through
/// when references were edges: groups entered by an in-region child, plus — when `entry`
/// is the reference the flood started at — the entry itself and its group below it. Root
/// groups nothing descends into (e.g. a remote ref stacked above a local one) stay out,
/// exactly like the edge-era floods never reached them.
fn attach_flooded_refs(
    graph: &EditorGraph,
    entries: &mut HashSet<EditorGraphIndex>,
    entry: Option<EditorGraphIndex>,
) {
    let mut additions: Vec<EditorGraphIndex> = graph
        .positioned_refs()
        .filter_map(|(entry, _stored)| {
            // A group any in-region edge enters was flooded through before the walk
            // stopped at a boundary — membership is broader than stack assignment, which
            // stays arity- and ambiguity-aware in `divide_workspace_into_stacks`. Co-located
            // group members all share the same entering edges, so a lower member is
            // attached with the whole group, while a root ref stacked above (its own entering set
            // empty, e.g. a remote ref over the tip) stays out.
            let followed = positions::edges_through(graph, entry)
                .iter()
                .any(|(child, _)| entries.contains(child));
            followed.then_some(entry)
        })
        .collect();
    if let Some(entry) = entry
        && let Some(entry_stored) = graph.position_of(entry)
    {
        additions.push(entry);
        let entry_edges = positions::edges_through(graph, entry);
        let entry_depth = positions::ref_depth(graph, entry);
        additions.extend(graph.positioned_refs().filter_map(|(entry, stored)| {
            (stored.on == entry_stored.on
                && positions::edges_through(graph, entry) == entry_edges
                && positions::ref_depth(graph, entry) < entry_depth)
                .then_some(entry)
        }));
    }
    entries.extend(additions);
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
