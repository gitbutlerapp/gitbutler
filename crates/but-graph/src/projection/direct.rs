//! The workspace projection: build a [`Workspace`] directly from the commit-first walk output and
//! metadata, with no segment surgery.
//!
//! The record [`Graph`] returned alongside the [`Workspace`] is segments minted as inert
//! name/commit records so renderers and id-based tooling keep working; the commit topology lives in
//! the carried [`CommitGraph`], and the rebase reads the branch records via
//! [`BranchGraph`](crate::BranchGraph).
//!
//! [`build`] is the entry point; its stages:
//! - frame: classify the workspace via the [`Frame`] state machine — managed-owning,
//!   managed-missing-commit (anchored by attachment), ad-hoc, plus the integrated-entrypoint
//!   downgrade.
//! - naming: a run takes its owning record's name, with [`name_anonymous_run`] lifting a single
//!   local ref and disambiguating remote-scoped names.
//! - stacks: first-parent runs between provenance heads; special and remote-named runs pass
//!   through unnamed; a stack's base is the first uncollected run.
//! - lower bound: a merge-base fold over the commit store, per frame.
//! - integrated pruning: cut to the ancestor-set of the target tip.
//! - remotes: commit-store reachability from remote tips ([`enrich_with_remotes`]); local refs
//!   consumed into synthesized local-tracking records are tracked in [`consumed_local_refs`].
//! - metadata branches: [`materialize_metadata_branches`] inserts empty named segments for Applied
//!   metadata branches — above the commit their ref sits on (the last listed ref keeps the
//!   commits), at stack bottoms, or as independent empty stacks at base candidates. Stacks whose
//!   refs advanced past the workspace project siblings via [`sibling_candidates`]/
//!   [`adopt_ahead_siblings`].
//! - entrypoints: [`mark_entrypoint`] handles explicit traversal entrypoints — the
//!   entrypoint-in-workspace gate, the metadata-entrypoint downgrade exception, entrypoint-owned
//!   segments, and preferred-parent paths at merges.
//!
//! Unborn and detached heads project too.

use std::collections::{BTreeMap, HashSet};

use anyhow::Context as _;
use but_core::{RefMetadata, ref_metadata, ref_metadata::ProjectMeta};
use gix::prelude::{ObjectIdExt as _, ReferenceExt};

use crate::{
    Commit, CommitFlags, Workspace,
    commit_graph::CommitGraph,
    init::{
        self,
        commit_walk::{Context, State},
        overlay::{OverlayMetadata, OverlayRepo},
    },
    workspace::{Stack, StackCommit, StackSegment, TargetRef, WorkspaceKind},
};

impl Workspace {
    /// Like [`Self::from_head`], but starts at `tip`, with `ref_name` assumed to point at it.
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let repo = tip.repo;
        let tip = tip.detach();
        Self::from_resolved_tip(
            repo,
            tip,
            ref_name.into(),
            meta,
            project_meta,
            options,
            false,
        )
    }

    /// Resolve the workspace from an already-resolved `tip`, with `ref_name` assumed to point at it:
    /// compute the initial tips from workspace metadata and traverse directly. The shared tail of
    /// [`Self::from_commit_traversal`] and [`Self::from_head`].
    fn from_resolved_tip(
        repo: &gix::Repository,
        tip: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: init::Options,
        detached_head: bool,
    ) -> anyhow::Result<Self> {
        let (overlay_repo, overlay_meta, _entrypoint) =
            init::Overlay::default().into_parts(repo, meta);
        let tips = init::initial_tips_from_workspace_metadata(
            &overlay_repo,
            &overlay_meta,
            tip,
            ref_name.as_ref(),
            &project_meta,
            options.extra_target_commit_id,
        )?;
        Self::traverse_tips_with_overlay_impl(
            &overlay_repo,
            tips,
            &overlay_meta,
            project_meta,
            options,
            ref_name,
            detached_head,
        )
    }

    /// Build the [`Workspace`] from explicit traversal `tips`.
    ///
    /// The entrypoint is the tip whose [`Tip::is_entrypoint`](crate::init::Tip) flag is set.
    pub fn from_commit_traversal_tips(
        repo: &gix::Repository,
        tips: impl IntoIterator<Item = crate::init::Tip>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let tips: Vec<_> = tips.into_iter().collect();
        let (overlay_repo, overlay_meta, _entrypoint) =
            init::Overlay::default().into_parts(repo, meta);
        Self::traverse_tips_with_overlay_impl(
            &overlay_repo,
            tips,
            &overlay_meta,
            project_meta,
            options,
            None,
            false,
        )
    }

    /// Build the workspace of `repo`'s `HEAD` from the commit-first traversal.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let head = repo.head()?;
        let mut is_detached = false;
        let (tip, maybe_name) = match head.kind {
            gix::head::Kind::Unborn(ref_name) => {
                // An unborn branch projects as a single empty ad-hoc stack.
                let mut wt_by_branch = std::collections::BTreeMap::new();
                wt_by_branch.insert(
                    ref_name.clone(),
                    vec![crate::Worktree {
                        kind: crate::WorktreeKind::Main,
                        owned_by_repo: true,
                    }],
                );
                let ref_info = crate::RefInfo::from_ref(ref_name, None, &wt_by_branch);
                // The single ad-hoc segment; no traversal ran, and its one worktree is unique.
                let rec = 0;
                let entrypoint_ref = Some(ref_info.ref_name.clone());
                return Ok(Workspace {
                    commit_graph: None,
                    project_meta,
                    options,
                    entrypoint_ref,
                    symbolic_remote_names: Vec::new(),
                    branches: Some(vec![crate::branch_graph::Branch {
                        ref_name: Some(ref_info.ref_name.clone()),
                        commits: Vec::new(),
                        outgoing: Vec::new(),
                        is_entrypoint: true,
                    }]),
                    id: rec,
                    tip_commit_id: None,
                    ref_info: Some(ref_info.clone()),
                    kind: WorkspaceKind::AdHoc,
                    stacks: vec![Stack {
                        id: Some(but_core::ref_metadata::StackId::single_branch_id()),
                        segments: vec![StackSegment {
                            ref_info: Some(ref_info),
                            id: rec,
                            commits: Vec::new(),
                            commits_by_segment: Vec::new(),
                            metadata: None,
                            ..blank_stack_segment()
                        }],
                    }],
                    lower_bound: None,
                    lower_bound_ref_name: None,
                    target_ref: None,
                    target_commit: None,
                    integrated_target_tip_commit_id: None,
                    ancestor_workspace_commit: None,
                    named_segments: Vec::new(),
                    ref_tips: Vec::new(),
                    hard_limit_hit: false,
                    has_multiple_worktrees: false,
                    entrypoint_commit_id: None,
                    metadata: None,
                });
            }
            gix::head::Kind::Detached { target, peeled } => {
                is_detached = true;
                (peeled.unwrap_or(target).attach(repo), None)
            }
            gix::head::Kind::Symbolic(existing_reference) => {
                let mut existing_reference = existing_reference.attach(repo);
                let tip = existing_reference.peel_to_id()?;
                (tip, Some(existing_reference.inner.name))
            }
        };
        let repo = tip.repo;
        let tip = tip.detach();
        Self::from_resolved_tip(
            repo,
            tip,
            maybe_name,
            meta,
            project_meta,
            options,
            is_detached,
        )
    }
}

/// The walk's record segments plus provenance, resolved into commit-level facts.
struct Facts<'a> {
    state: &'a State,
    /// Local refs consumed into synthesized local-tracking records for workspace targets,
    /// stripped from displayed commit refs.
    consumed_local_refs: HashSet<(gix::ObjectId, gix::refs::FullName)>,
    /// Refs consumed into metadata-materialized segments, stripped from displayed commit refs.
    consumed_meta_refs: std::cell::RefCell<HashSet<gix::refs::FullName>>,
}

impl Facts<'_> {
    fn commits(&self) -> &CommitGraph {
        &self.state.commits
    }

    /// Run head commit per owning record.
    fn head_of(&self) -> &BTreeMap<usize, gix::ObjectId> {
        &self.state.head_by_owner
    }

    /// The traversal entrypoint segment.
    fn entrypoint(&self) -> Option<usize> {
        self.state.entrypoint
    }

    /// The ref the caller resolved as the entrypoint, if any.
    fn entrypoint_ref(&self) -> Option<&gix::refs::FullName> {
        self.state.entrypoint_ref.as_ref()
    }

    /// The project metadata (targets) the traversal ran with.
    fn project_meta(&self) -> &but_core::ref_metadata::ProjectMeta {
        &self.state.project_meta
    }

    /// The name the walk recorded for segment `seg`, if any.
    fn ref_info_of(&self, seg: usize) -> Option<&crate::RefInfo> {
        self.state.ref_info_by_segment.get(&seg)
    }

    /// The segment metadata the walk recorded for segment `seg`, if any.
    fn metadata_of(&self, seg: usize) -> Option<&crate::SegmentMetadata> {
        self.state.metadata_by_segment.get(&seg)
    }

    /// Every record segment the walk named, with its name. Run owners and empty named records alike.
    fn named_segments(&self) -> impl Iterator<Item = (usize, &crate::RefInfo)> {
        self.state
            .ref_info_by_segment
            .iter()
            .map(|(&s, ri)| (s, ri))
    }

    /// The traversal tips (roles + names) the walk ran with.
    fn traversal_tips(&self) -> &[crate::init::Tip] {
        &self.state.traversal_tips
    }

    /// The traversal options the walk ran with.
    fn options(&self) -> &crate::init::Options {
        &self.state.options
    }

    /// The managed-workspace record segment (the one carrying workspace metadata), with its name
    /// and that metadata. First in record-segment order, matching the record graph's iteration.
    fn workspace_segment(
        &self,
    ) -> Option<(usize, &crate::RefInfo, &but_core::ref_metadata::Workspace)> {
        self.state.metadata_by_segment.iter().find_map(|(&s, md)| {
            let crate::SegmentMetadata::Workspace(ws) = md else {
                return None;
            };
            Some((s, self.state.ref_info_by_segment.get(&s)?, ws))
        })
    }

    /// Whether `id` was reached by the walk and is present in the commit store.
    fn has_commit(&self, id: gix::ObjectId) -> bool {
        self.commits().node(id).is_some()
    }

    /// The commit `rec` attaches to via the walk's attachments, if it owns no run of its own.
    fn attach_target(&self, rec: usize) -> Option<gix::ObjectId> {
        self.state
            .attachments
            .iter()
            .find_map(|a| (a.segment == rec).then_some(a.to))
    }

    /// The commit a record resolves to: the head of the run it owns, or the commit it attaches to.
    fn record_commit(&self, rec: usize) -> Option<gix::ObjectId> {
        self.head_of()
            .get(&rec)
            .copied()
            .or_else(|| self.attach_target(rec))
    }

    /// The record segment named exactly `name`, if any (first in segment-id order).
    fn segment_named(&self, name: &gix::refs::FullNameRef) -> Option<usize> {
        self.state
            .ref_info_by_segment
            .iter()
            .find(|(_, ri)| ri.ref_name.as_ref() == name)
            .map(|(&s, _)| s)
    }

    /// The commit of the record segment named exactly `name`, if one exists and resolves.
    fn record_commit_named(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        self.record_commit(self.segment_named(name)?)
    }

    /// The record owning the run that contains `commit`, and that run's head.
    fn run_of(&self, commit: gix::ObjectId) -> Option<(usize, gix::ObjectId)> {
        self.state.run_of.get(&commit).copied()
    }

    /// The run as commit ids, head to base, stopping before the next run head.
    fn run(&self, head: gix::ObjectId) -> Vec<gix::ObjectId> {
        let mut out = Vec::new();
        let mut cur = head;
        loop {
            out.push(cur);
            cur = match self.commits().first_parent_id(cur) {
                Some(p) if !self.is_run_head(p) => p,
                _ => break,
            };
        }
        out
    }

    fn is_run_head(&self, id: gix::ObjectId) -> bool {
        self.state
            .run_of
            .get(&id)
            .is_some_and(|(_, head)| *head == id)
    }

    /// The head of the run that follows `head`'s run along the first parent, if any.
    fn next_run_head(&self, head: gix::ObjectId) -> Option<gix::ObjectId> {
        let last = *self.run(head).last().expect("never empty");
        self.commits().first_parent_id(last)
    }
}

/// Build-private scratch segment for the display projection — the subset of the record `Segment`
/// the mint pipeline actually uses. Its own type so `out` no longer pins `crate::Segment` alive
/// (which then dies with the record graph).
#[derive(Default)]
struct MintSeg {
    id: usize,
    ref_info: Option<crate::RefInfo>,
    metadata: Option<crate::SegmentMetadata>,
    commits: Vec<crate::Commit>,
}

/// Build-private node storage for the display projection, replacing the record graph: segments by
/// id, no edges (the resolution, ref tables, and branch records are all derived from facts and the
/// `BranchGraph`). Ids come from a plain counter; they never index the record graph.
#[derive(Default)]
struct NodeStore {
    nodes: BTreeMap<usize, MintSeg>,
    next: usize,
}

impl NodeStore {
    /// Store `seg` under a fresh id and return it (mirrors `Graph::insert_segment`).
    fn insert_segment(&mut self, mut seg: MintSeg) -> usize {
        let id = self.next;
        self.next += 1;
        seg.id = id;
        self.nodes.insert(id, seg);
        id
    }
    /// Store an empty segment that sits above `anchor`. The anchor is no longer recorded (empties
    /// are derived from facts now); the parameter is kept for the call sites pending out's removal.
    fn insert_segment_with_anchor(
        &mut self,
        seg: MintSeg,
        _anchor: Option<gix::ObjectId>,
    ) -> usize {
        self.insert_segment(seg)
    }
    /// All segment ids, in id order.
    fn segments(&self) -> impl Iterator<Item = usize> + '_ {
        self.nodes.keys().copied()
    }
}

impl std::ops::Index<usize> for NodeStore {
    type Output = MintSeg;
    fn index(&self, id: usize) -> &MintSeg {
        &self.nodes[&id]
    }
}

impl std::ops::IndexMut<usize> for NodeStore {
    fn index_mut(&mut self, id: usize) -> &mut MintSeg {
        self.nodes
            .get_mut(&id)
            .expect("segment id minted by this store")
    }
}

/// The minted record segment owning the run that contains `commit`, if one was minted for it.
fn minted_of(
    facts: &Facts<'_>,
    minted: &BTreeMap<gix::ObjectId, usize>,
    commit: gix::ObjectId,
) -> Option<usize> {
    facts
        .run_of(commit)
        .and_then(|(_, head)| minted.get(&head).copied())
}

/// Drop a segment's name, and its branch metadata with it: a segment that loses its name has no
/// claim to the metadata that name carried.
fn clear_segment_name(seg: &mut MintSeg) {
    seg.ref_info = None;
    if matches!(seg.metadata, Some(crate::SegmentMetadata::Branch(_))) {
        seg.metadata = None;
    }
}

/// A [`StackSegment`] with every field empty/default, for `..blank_stack_segment()` struct-update:
/// callers spell out only the fields that carry information.
fn blank_stack_segment() -> StackSegment {
    StackSegment {
        ref_info: None,
        remote_tracking_ref_name: None,
        remote_tip_id: None,
        tip_commit_id: None,
        generation: 0,
        id: usize::default(),
        commits: Vec::new(),
        commits_outside: None,
        base: None,
        base_segment_id: None,
        base_ref_name: None,
        commits_by_segment: Vec::new(),
        commits_on_remote: Vec::new(),
        metadata: None,
        is_entrypoint: false,
        projected_from_outside: false,
    }
}

/// Find the managed workspace commit in the ancestry of an advanced workspace reference (the
/// `workspace_id` segment), along with the commits sitting on top of it. This is the walk consumers
/// used to run on the record graph; the projection owns the graph, so it resolves it once here and
/// the result travels on the workspace as commit-addressed data.
fn find_ancestor_workspace_commit(
    commit_graph: &crate::commit_graph::CommitGraph,
    repo: &OverlayRepo<'_>,
    workspace_tip: Option<gix::ObjectId>,
    lower_bound: Option<gix::ObjectId>,
    generation_by_commit: &std::collections::HashMap<gix::ObjectId, usize>,
) -> Option<crate::workspace::AncestorWorkspaceCommit> {
    let lower_bound_generation = lower_bound.and_then(|lb| generation_by_commit.get(&lb).copied());
    let workspace_tip = workspace_tip?;
    // Find the managed workspace commit on the first-parent line below the tip (its natural base),
    // bounded by the lower bound so the search never runs past the integration point.
    let mut managed_commit_id = None;
    let mut cur = Some(workspace_tip);
    while let Some(id) = cur {
        if lower_bound_generation
            .is_some_and(|max_gen| generation_by_commit.get(&id).copied().unwrap_or(0) > max_gen)
        {
            break;
        }
        let message = repo
            .find_commit(id)
            .ok()
            .and_then(|c| c.message_raw().ok().map(|m| m.to_owned()));
        if message
            .as_ref()
            .is_some_and(|m| crate::workspace::commit::is_managed_workspace_by_message(m.as_ref()))
        {
            managed_commit_id = Some(id);
            break;
        }
        cur = commit_graph.first_parent_id(id);
    }
    let managed_commit_id = managed_commit_id?;
    // Every commit on top of the workspace commit: those reachable from the tip but not from the
    // managed commit — exactly the set a `git reset --soft <managed>` unwinds, merge siblings
    // included (the record graph's segment walk pruned some by generation, inconsistently).
    let commits_outside = commit_graph
        .commits_reachable_from_a_not_b(workspace_tip, managed_commit_id, false)
        .into_iter()
        .filter_map(|id| commit_graph.commit(id).cloned())
        .collect();
    Some(crate::workspace::AncestorWorkspaceCommit {
        managed_commit_id,
        commits_outside,
    })
}

/// How the workspace is anchored, decided up front and used throughout [`build`].
enum Frame {
    ManagedOwning {
        ws_commit: gix::ObjectId,
        commit_is_managed: bool,
    },
    ManagedMissing {
        anchor: gix::ObjectId,
    },
    AdHoc,
}

/// Classify the workspace: a managed workspace ref owning its commit, a managed ref whose commit
/// is missing (it attaches to an anchor), or an ad-hoc head. The integrated-entrypoint downgrade
/// happens later in [`build`], once the lower bound is known.
fn classify_frame(
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    ep_commit: gix::ObjectId,
) -> (
    Frame,
    Option<(usize, crate::RefInfo, ref_metadata::Workspace)>,
) {
    let ws = facts
        .workspace_segment()
        .map(|(rec, ref_info, md)| (rec, ref_info.clone(), md.clone()));

    // The entrypoint record only counts as the workspace when the entrypoint ref agrees: an
    // unnamed tip disambiguated onto the workspace ref still belongs to the ref the caller gave.
    let ep_in_workspace = (ws.as_ref().map(|(rec, _, _)| *rec) == facts.entrypoint()
        && facts
            .entrypoint_ref()
            .is_none_or(|ep_ref| ws.as_ref().is_some_and(|(_, ri, _)| ri.ref_name == *ep_ref)))
        || facts.commits().node(ep_commit).is_some_and(|nx| {
            facts.commits().inner[nx]
                .flags
                .contains(CommitFlags::InWorkspace)
        });
    match &ws {
        Some((rec, ref_info, metadata)) if ep_in_workspace => match facts.head_of().get(rec) {
            Some(&ws_commit) => (
                Frame::ManagedOwning {
                    ws_commit,
                    // A workspace ref on someone else's tip is not a workspace commit: the run
                    // belongs to the stack, the workspace merely attaches above it.
                    commit_is_managed: ctx
                        .repo
                        .find_commit(ws_commit)
                        .ok()
                        .and_then(|c| c.message_raw().ok().map(|m| m.to_owned()))
                        .is_some_and(|message| {
                            crate::workspace::commit::is_managed_workspace_by_message(
                                message.as_ref(),
                            )
                        }),
                },
                Some((*rec, ref_info.clone(), metadata.clone())),
            ),
            None => match facts.record_commit(*rec) {
                Some(anchor) => (
                    Frame::ManagedMissing { anchor },
                    Some((*rec, ref_info.clone(), metadata.clone())),
                ),
                None => (Frame::AdHoc, None),
            },
        },
        _ => (Frame::AdHoc, None),
    }
}

/// Resolve the target: the record named after the configured target ref, resolved to its commit;
/// or, with no configured target and no workspace-metadata tip, a named integrated traversal tip.
/// The configured target: the record named after the configured target ref, resolved to its commit
/// (from metadata, before any integrated-tip fallback). Kept separate so a downgrade to
/// single-branch can preserve it while dropping the fallback.
fn resolve_configured_target(
    facts: &Facts<'_>,
) -> Option<(usize, gix::refs::FullName, gix::ObjectId)> {
    facts
        .project_meta()
        .target_ref
        .as_ref()
        .and_then(|target_ref| {
            facts
                .segment_named(target_ref.as_ref())
                .map(|rec| (rec, target_ref.clone()))
        })
        .and_then(|(rec, ref_name)| {
            facts
                .record_commit(rec)
                .map(|commit| (rec, ref_name, commit))
        })
}

fn resolve_target(facts: &Facts<'_>) -> Option<(usize, gix::refs::FullName, gix::ObjectId)> {
    resolve_configured_target(facts)
        // `integrated_tip_target_ref`: with no configured target and no workspace-metadata
        // tip, a named integrated traversal tip (an explicit `Tip::integrated`) is the target.
        .or_else(|| {
            let has_ws_md_tip = facts
                .traversal_tips()
                .iter()
                .any(|tip| matches!(tip.metadata, Some(crate::SegmentMetadata::Workspace(_))));
            if has_ws_md_tip {
                return None;
            }
            facts
                .traversal_tips()
                .iter()
                .filter(|tip| tip.role.is_integrated())
                .find_map(|tip| {
                    let ref_name = tip.ref_name.clone()?;
                    let rec = facts.segment_named(ref_name.as_ref())?;
                    facts
                        .record_commit(rec)
                        .map(|commit| (rec, ref_name, commit))
                })
        })
}

/// Stack tips per frame: the workspace commit's in-graph parents in parent order, or the single
/// anchor/entrypoint commit.
fn stack_tips_for_frame(
    frame: &Frame,
    facts: &Facts<'_>,
    ep_commit: gix::ObjectId,
) -> Vec<gix::ObjectId> {
    match frame {
        Frame::ManagedOwning {
            ws_commit,
            commit_is_managed: true,
        } => facts
            .commits()
            .node_data(*ws_commit)
            .parent_ids
            .iter()
            .copied()
            .filter(|id| facts.has_commit(*id))
            .collect(),
        Frame::ManagedOwning {
            ws_commit,
            commit_is_managed: false,
        } => vec![*ws_commit],
        Frame::ManagedMissing { anchor } => vec![*anchor],
        Frame::AdHoc => vec![ep_commit],
    }
}

/// An ad-hoc head without a configured target falls back to its own remote tracking branch as the
/// target, like the single-branch auto-target.
fn resolve_auto_target(
    frame: &Frame,
    target: &Option<(usize, gix::refs::FullName, gix::ObjectId)>,
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    ep_commit: gix::ObjectId,
) -> anyhow::Result<Option<(gix::refs::FullName, gix::ObjectId)>> {
    if !(matches!(frame, Frame::AdHoc) && target.is_none()) {
        return Ok(None);
    }
    let entry_name = facts
        .run_of(ep_commit)
        .and_then(|(owner, _)| facts.ref_info_of(owner).cloned())
        .map(|ri| ri.ref_name)
        .or_else(|| facts.entrypoint_ref().cloned());
    if let Some(local) = entry_name
        && let Some(remote_ref) = crate::init::remotes::lookup_remote_tracking_branch_or_deduce_it(
            ctx.repo,
            local.as_ref(),
            ctx.symbolic_remote_names,
            ctx.configured_remote_tracking_branches,
        )?
        && let Some(remote_tip) = facts.record_commit_named(remote_ref.as_ref())
    {
        return Ok(Some((remote_ref, remote_tip)));
    }
    Ok(None)
}

/// The lower bound: a merge-base fold over the stack tips and the target tip. A workspace needs at
/// least two candidates; an ad-hoc head computes against any target context it has. `integrated_tips`
/// (past target positions) join the fold so the workspace does not appear to lose now-reachable stacks.
// Arguments are the resolved workspace context; bundling them into a struct would require migrating
// the heavily-threaded `target`/`frame` across build(), deferred as a decomposition follow-up.
#[allow(clippy::too_many_arguments)]
fn compute_lower_bound(
    frame: &Frame,
    stack_tips: &[gix::ObjectId],
    target: &Option<(usize, gix::refs::FullName, gix::ObjectId)>,
    target_commit_id: Option<gix::ObjectId>,
    auto_target: &Option<(gix::refs::FullName, gix::ObjectId)>,
    integrated_tips: &[gix::ObjectId],
    facts: &Facts<'_>,
    ep_commit: gix::ObjectId,
) -> Option<gix::ObjectId> {
    let fold_candidates = |candidates: &[gix::ObjectId]| -> Option<gix::ObjectId> {
        let mut iter = candidates.iter().copied();
        let first = iter.next()?;
        Some(iter.fold(first, |base, next| {
            facts.commits().merge_base(base, next).unwrap_or(base)
        }))
    };
    match frame {
        Frame::ManagedOwning { .. } | Frame::ManagedMissing { .. } => {
            // A single stack tip folds only against target context; multiple tips bound each
            // other like the lowest-base computation over workspace children.
            let has_target_context =
                target.is_some() || target_commit_id.is_some() || !integrated_tips.is_empty();
            let candidates: Vec<gix::ObjectId> = stack_tips
                .iter()
                .copied()
                .chain(target.as_ref().map(|(_, _, c)| *c))
                .chain(target_commit_id)
                .chain(integrated_tips.iter().copied())
                .collect();
            fold_candidates(&candidates)
                .filter(|_| (has_target_context || stack_tips.len() >= 2) && candidates.len() >= 2)
        }
        Frame::AdHoc => {
            let target_tip = target
                .as_ref()
                .map(|(_, _, c)| *c)
                .or(auto_target.as_ref().map(|(_, c)| *c))
                .or(target_commit_id);
            if target_tip.is_some() || !integrated_tips.is_empty() {
                let candidates: Vec<gix::ObjectId> = std::iter::once(ep_commit)
                    .chain(target_tip)
                    .chain(integrated_tips.iter().copied())
                    .collect();
                fold_candidates(&candidates)
            } else {
                None
            }
        }
    }
}

/// In managed frames the workspace segment leads; ad-hoc reuses the first stack segment. Mints or
/// renames the leading segment in `out` and returns it.
fn lead_workspace_segment(
    frame: &Frame,
    ws_info: &Option<(usize, crate::RefInfo, ref_metadata::Workspace)>,
    facts: &Facts<'_>,
    minted: &BTreeMap<gix::ObjectId, usize>,
    out: &mut NodeStore,
) -> anyhow::Result<Option<usize>> {
    Ok(match (frame, ws_info) {
        (
            Frame::ManagedOwning {
                ws_commit,
                commit_is_managed: true,
            },
            Some((_, ref_info, _)),
        ) => {
            let sidx = minted_of(facts, minted, *ws_commit)
                .context("BUG: the workspace commit is owned by a run")?;
            out[sidx].ref_info = Some(ref_info.clone());
            Some(sidx)
        }
        (
            Frame::ManagedOwning {
                ws_commit,
                commit_is_managed: false,
            },
            Some((rec, ref_info, _)),
        ) => {
            // The run belongs to the stack: take the workspace name off the canonical run and
            // attach an empty workspace record above it, like an attached workspace ref.
            if let Some(sidx) = minted_of(facts, minted, *ws_commit)
                && out[sidx]
                    .ref_info
                    .as_ref()
                    .is_some_and(|ri| ri.ref_name == ref_info.ref_name)
            {
                clear_segment_name(&mut out[sidx]);
            }
            let sidx = out.insert_segment_with_anchor(
                MintSeg {
                    ref_info: Some(ref_info.clone()),
                    metadata: facts.metadata_of(*rec).cloned(),
                    ..Default::default()
                },
                Some(*ws_commit),
            );
            Some(sidx)
        }
        (Frame::ManagedMissing { anchor }, Some((rec, ref_info, _))) => {
            // The attached record pass usually minted the workspace segment already.
            let existing = out.segments().find(|s| {
                out[*s]
                    .ref_info
                    .as_ref()
                    .is_some_and(|ri| ri.ref_name == ref_info.ref_name)
            });
            let sidx = existing.unwrap_or_else(|| {
                out.insert_segment_with_anchor(
                    MintSeg {
                        ref_info: Some(ref_info.clone()),
                        metadata: facts.metadata_of(*rec).cloned(),
                        ..Default::default()
                    },
                    Some(*anchor),
                )
            });
            Some(sidx)
        }
        _ => None,
    })
}

/// Mint the node storage from the walk: one segment per run (named from its record, commits from
/// facts), plus the empty named record segments (attached by anchor). Names anonymous runs and
/// splits the managed workspace commit onto its own segment. No edges — the record graph's
/// adjacency is gone; empties record their anchor commit for target resolution instead.
fn mint_segments<T: RefMetadata>(
    out: &mut NodeStore,
    minted: &mut BTreeMap<gix::ObjectId, usize>,
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    meta: &OverlayMetadata<'_, T>,
    frame: &Frame,
) -> anyhow::Result<()> {
    let mut idx_of_record: BTreeMap<usize, usize> = BTreeMap::new();
    for (&owner, &head) in facts.head_of().iter() {
        let name = facts.ref_info_of(owner).cloned();
        let sidx = out.insert_segment(MintSeg {
            ref_info: name.clone(),
            metadata: facts.metadata_of(owner).cloned(),
            commits: facts
                .run(head)
                .into_iter()
                .map(|id| graph_commit(facts, id, name.as_ref().map(|ri| &ri.ref_name)))
                .collect(),
            ..Default::default()
        });
        minted.insert(head, sidx);
        idx_of_record.insert(owner, sidx);
    }
    let named: Vec<(usize, crate::RefInfo)> = facts
        .named_segments()
        .map(|(s, ri)| (s, ri.clone()))
        .collect();
    for (rec, ref_info) in named {
        if idx_of_record.contains_key(&rec) {
            continue;
        }
        let to = facts.attach_target(rec);
        let sidx = out.insert_segment_with_anchor(
            MintSeg {
                ref_info: Some(ref_info),
                metadata: facts.metadata_of(rec).cloned(),
                ..Default::default()
            },
            to,
        );
        idx_of_record.insert(rec, sidx);
    }
    if !facts
        .options()
        .dangerously_skip_postprocessing_for_debugging
    {
        // A later remote commit carrying a remote-tracking ref splits into its own segment.
        split_remote_runs(out, facts);
        // A managed workspace commit owns its segment alone; the rest of its run splits into an
        // anonymous segment the stacks resolve to.
        if let Frame::ManagedOwning {
            ws_commit,
            commit_is_managed: true,
        } = frame
            && let Some(&ws_sidx) = facts
                .run_of(*ws_commit)
                .and_then(|(_, head)| minted.get(&head))
            && out[ws_sidx].commits.len() > 1
            && out[ws_sidx].commits.first().map(|c| c.id) == Some(*ws_commit)
        {
            let tail_head = out[ws_sidx].commits[1].id;
            let tail = split_out_segment(out, ws_sidx, 1, None);
            minted.insert(tail_head, tail);
        }
        // Every anonymous run takes its unambiguous name up front, so later passes key on it.
        for (&_, &head) in facts.head_of().iter() {
            let Some(&sidx) = minted.get(&head) else {
                continue;
            };
            if out[sidx].ref_info.is_some() {
                continue;
            }
            if let Some(ri) = name_anonymous_run(facts, ctx, head)? {
                let md = meta
                    .branch_opt(ri.ref_name.as_ref())
                    .ok()
                    .flatten()
                    .map(|md| crate::SegmentMetadata::Branch(ref_metadata::Branch::clone(&md)));
                apply_name_to_canonical(out, sidx, ri);
                if out[sidx].metadata.is_none() {
                    out[sidx].metadata = md;
                }
            }
        }
    }
    Ok(())
}

/// A later remote commit that carries a remote-tracking ref starts its own segment named by that
/// ref; additional refs at the same commit become empty virtual segments anchored at it. Keeps the
/// remote runs as isolated, findable segments (branch records reads them). No edges.
fn split_remote_runs(out: &mut NodeStore, facts: &Facts<'_>) {
    let sidxs: Vec<usize> = out.segments().collect();
    for sidx in sidxs {
        if out[sidx].commits.len() < 2 {
            continue;
        }
        let mut splits: Vec<(usize, gix::refs::FullName, Vec<gix::refs::FullName>)> = Vec::new();
        for (cidx, commit) in out[sidx].commits.iter().enumerate() {
            if cidx == 0 || !commit.flags.is_remote() {
                continue;
            }
            let mut remote_refs: Vec<gix::refs::FullName> = facts
                .commits()
                .node(commit.id)
                .map(|nx| {
                    facts.commits().inner[nx]
                        .refs
                        .iter()
                        .filter(|ri| {
                            ri.ref_name.category() == Some(gix::refs::Category::RemoteBranch)
                        })
                        .map(|ri| ri.ref_name.clone())
                        .collect()
                })
                .unwrap_or_default();
            if remote_refs.is_empty() {
                continue;
            }
            let owner_ref = remote_refs.remove(0);
            splits.push((cidx, owner_ref, remote_refs));
        }
        for (cidx, owner_ref, virtual_refs) in splits.into_iter().rev() {
            let tip = out[sidx].commits[cidx].id;
            split_out_segment(
                out,
                sidx,
                cidx,
                Some(crate::RefInfo {
                    ref_name: owner_ref,
                    commit_id: Some(tip),
                    worktree: None,
                }),
            );
            for virtual_name in virtual_refs {
                out.insert_segment_with_anchor(
                    MintSeg {
                        ref_info: Some(crate::RefInfo {
                            ref_name: virtual_name,
                            commit_id: Some(tip),
                            worktree: None,
                        }),
                        ..Default::default()
                    },
                    Some(tip),
                );
            }
        }
    }
}

/// Wire each segment's base from the next segment's pre-trim head; the last rests on the lower
/// bound, unless that commit is a traversal dead-end (no in-graph parents and nothing below).
fn wire_segment_bases(
    segments: &mut [StackSegment],
    heads: &[gix::ObjectId],
    base_next: Option<gix::ObjectId>,
    lower_bound_segment_id: Option<usize>,
    facts: &Facts<'_>,
    minted: &BTreeMap<gix::ObjectId, usize>,
) {
    for i in 0..segments.len() {
        let (base, base_segment_id) = match segments.get(i + 1) {
            Some(next) => (heads.get(i + 1).copied(), Some(next.id)),
            None => (
                // The run below where collection stopped, unless it is a traversal dead-end
                // (a lone cutoff commit with unwalked parents). A cut commit that owns a walked
                // run still rests the stack on the segment below it.
                base_next.filter(|b| {
                    let node = facts.commits().node_data(*b);
                    node.parent_ids.is_empty()
                        || facts.run_of(*b).is_some_and(|(_, head)| head == *b)
                        || facts.run(*b).len() > 1
                        || facts
                            .run(*b)
                            .last()
                            .is_some_and(|last| facts.commits().first_parent_id(*last).is_some())
                }),
                base_next
                    .and_then(|b| minted.get(&b).copied())
                    .or(lower_bound_segment_id),
            ),
        };
        segments[i].base = base;
        segments[i].base_segment_id = base_segment_id;
    }
}

/// Collect one stack by walking first-parent runs from `tip` to the lower bound, minting its
/// segments into `out` and returning the stack (or `None` if it collapses to nothing).
#[allow(clippy::too_many_arguments)] // resolved workspace context, like compute_lower_bound
fn collect_one_stack<T: RefMetadata>(
    tip: gix::ObjectId,
    tip_idx: usize,
    frame: &Frame,
    lower_bound: Option<gix::ObjectId>,
    lower_bound_segment_id: Option<usize>,
    stack_tips: &[gix::ObjectId],
    meta_lifted: &HashSet<gix::refs::FullName>,
    adhoc_name: &Option<crate::RefInfo>,
    parent_hints: &BTreeMap<gix::ObjectId, usize>,
    sibling_of: &BTreeMap<gix::ObjectId, (crate::RefInfo, gix::ObjectId)>,
    ep_run_head: Option<gix::ObjectId>,
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    meta: &OverlayMetadata<'_, T>,
    out: &mut NodeStore,
    minted: &mut BTreeMap<gix::ObjectId, usize>,
    head_by_segment: &mut BTreeMap<usize, gix::ObjectId>,
) -> anyhow::Result<Option<Stack>> {
    let mut segments: Vec<StackSegment> = Vec::new();
    // The pre-trim head commit per stack segment, for base wiring.
    let mut heads: Vec<gix::ObjectId> = Vec::new();
    let mut base_next = None;
    let mut cur = Some(tip);
    'runs: while let Some(head) = cur {
        // An ad-hoc entry segment is kept even when it sits on the bound; managed stacks
        // that start at the bound are discarded by collection.
        let keep_empty_entry = matches!(frame, Frame::AdHoc) && segments.is_empty();
        if Some(head) == lower_bound && !keep_empty_entry {
            base_next = Some(head);
            break;
        }
        let (owner, _) = facts.run_of(head).context("BUG: stack tips are visited")?;
        // A lifted run keeps its boundary only where it is not itself a stack tip: a run
        // also walked as its own stack duplicates into sharing stacks instead.
        let lifted = facts
            .ref_info_of(owner)
            .is_some_and(|ri| meta_lifted.contains(&ri.ref_name))
            && !stack_tips.contains(&head);
        let mut name = match facts.ref_info_of(owner).cloned() {
            // Internal refs (GitButler's own + remote-named stand-ins) never shape user-visible stacks.
            Some(ri) if is_internal_ref(ri.ref_name.as_ref()) => None,
            // Metadata-lifted names walk anonymously; their run keeps its boundary and
            // restoration re-applies the name — like the named segment the walk produces.
            Some(ri) if meta_lifted.contains(&ri.ref_name) => None,
            Some(ri) => Some(ri),
            // An anonymous run can be named from its head commit: a single local ref, or
            // exactly one ref with a remote tracking branch known to the traversal.
            None if facts
                .options()
                .dangerously_skip_postprocessing_for_debugging =>
            {
                None
            }
            None => name_anonymous_run(facts, ctx, head)?
                .filter(|ri| !meta_lifted.contains(&ri.ref_name)),
        };
        if segments.is_empty() && tip_idx == 0 && name.is_none() {
            name = adhoc_name.clone();
        }
        let metadata = name
            .as_ref()
            .filter(|_| {
                facts.ref_info_of(owner).is_none()
                    || !matches!(
                        facts.metadata_of(owner),
                        Some(crate::SegmentMetadata::Branch(_))
                    )
            })
            .and_then(|ri| meta.branch_opt(ri.ref_name.as_ref()).ok().flatten())
            .map(|md| crate::SegmentMetadata::Branch(md.clone()))
            .or_else(|| {
                // A mid-run tip under the workspace record must not inherit its
                // workspace metadata; only branch metadata travels onto stacks, and
                // only together with a name — anonymous segments stay bare.
                name.as_ref()
                    .and(facts.metadata_of(owner).cloned())
                    .filter(|md| matches!(md, crate::SegmentMetadata::Branch(_)))
            });
        let run = facts.run(head);
        let next = match parent_hints.get(run.last().expect("never empty")).copied() {
            Some(order) => facts
                .commits()
                .node_data(*run.last().expect("never empty"))
                .parent_ids
                .get(order)
                .copied()
                .filter(|p| facts.has_commit(*p)),
            None => facts.next_run_head(head),
        };

        // The run ends at the lower bound; integrated pruning happens after collection.
        let mut commits = Vec::new();
        for id in run {
            if Some(id) == lower_bound {
                if push_stack_run(
                    out,
                    facts,
                    minted,
                    &mut segments,
                    name,
                    metadata,
                    head,
                    commits,
                    sibling_of.contains_key(&head)
                        || lifted
                        || (Some(head) == ep_run_head && head != tip),
                ) {
                    heads.push(head);
                    if let Some(seg) = segments.last() {
                        head_by_segment.insert(seg.id, head);
                    }
                }
                // Internal GitButler runs are folded into stack segments, so the base resolves
                // through them to what a full collection would rest on.
                base_next = {
                    let mut below = next;
                    while let Some(b) = below
                        && facts.run_of(b).is_some_and(|(owner, run_head)| {
                            run_head == b
                                && facts.ref_info_of(owner).is_some_and(|ri| {
                                    ri.ref_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                                })
                        })
                    {
                        below = facts.next_run_head(b);
                    }
                    below
                };
                break 'runs;
            }
            commits.push(id);
        }
        if push_stack_run(
            out,
            facts,
            minted,
            &mut segments,
            name,
            metadata,
            head,
            commits,
            sibling_of.contains_key(&head) || lifted || (Some(head) == ep_run_head && head != tip),
        ) {
            heads.push(head);
            if let Some(seg) = segments.last() {
                head_by_segment.insert(seg.id, head);
            }
        }
        // Another stack tip below means this stack ends here, resting on it — unless that
        // tip's name is metadata-lifted, in which case its history is shared and duplicated.
        if let Some(n) = next
            && n != tip
            && stack_tips.contains(&n)
            && !facts
                .run_of(n)
                .and_then(|(o, _)| facts.ref_info_of(o))
                .is_some_and(|ri| meta_lifted.contains(&ri.ref_name))
        {
            base_next = Some(n);
            break;
        }
        base_next = next;
        cur = next;
    }
    if segments.iter().all(|s| s.commits.is_empty())
        && !matches!(frame, Frame::AdHoc)
        && !segments.iter().any(|s| {
            head_by_segment
                .get(&s.id)
                .is_some_and(|h| sibling_of.contains_key(h))
        })
    {
        return Ok(None);
    }
    // Wire bases from the next segment's pre-trim head; the last rests on the lower bound,
    // unless that commit is a traversal dead-end (no in-graph parents and nothing below).
    wire_segment_bases(
        &mut segments,
        &heads,
        base_next,
        lower_bound_segment_id,
        facts,
        minted,
    );
    if segments.is_empty() {
        return Ok(None);
    }
    Ok(Some(Stack {
        id: matches!(frame, Frame::AdHoc).then(but_core::ref_metadata::StackId::single_branch_id),
        segments,
    }))
}

/// Build a [`Workspace`] from the commit-first walk output: `state` is the commit store with
/// provenance, plus the per-segment naming/metadata and traversal scalars the walk recorded.
pub(crate) fn build<T: RefMetadata>(
    state: State,
    _repo: &OverlayRepo<'_>,
    meta: &OverlayMetadata<'_, T>,
    ctx: &Context<'_>,
    tip: gix::ObjectId,
    detached_head: bool,
) -> anyhow::Result<Workspace> {
    let facts = Facts {
        state: &state,
        consumed_local_refs: consumed_local_refs(&state, ctx)?,
        consumed_meta_refs: Default::default(),
    };

    // ---- The workspace frame ----
    //
    // Classify how the workspace is anchored; the integrated-entrypoint downgrade happens later,
    // once the lower bound is known.
    let ep_commit = tip;
    let (mut frame, ws_info) = classify_frame(&facts, ctx, ep_commit);

    // The target: the record named after the configured target ref, resolved to its commit.
    let configured_target = resolve_configured_target(&facts);
    let mut target = resolve_target(&facts);

    // The stored target commit: a remembered past target position that extends the workspace. It
    // counts when it leads a run or carries a ref — the points at which a segment can begin.
    let target_commit_id = facts.project_meta().target_commit_id.filter(|id| {
        facts.run_of(*id).is_some_and(|(_, head)| head == *id)
            || facts
                .commits()
                .node(*id)
                .is_some_and(|nx| !facts.commits().inner[nx].refs.is_empty())
    });

    // Stack tips per frame: the workspace commit's in-graph parents in parent order, or the
    // single anchor/entrypoint commit.
    let mut stack_tips = stack_tips_for_frame(&frame, &facts, ep_commit);

    // An ad-hoc head without a configured target falls back to its own remote tracking branch
    // as the target, like the single-branch auto-target.
    let auto_target = resolve_auto_target(&frame, &target, &facts, ctx, ep_commit)?;

    // Extra integrated tips (past target positions) join the fold so the workspace does not
    // appear to lose stacks that are now reachable from them.
    let integrated_tips: Vec<gix::ObjectId> = facts
        .traversal_tips()
        .iter()
        .filter(|tip| tip.role.is_integrated())
        .filter(|tip| facts.has_commit(tip.id))
        .filter(|tip| Some(tip.id) != target.as_ref().map(|(_, _, c)| *c))
        .map(|tip| tip.id)
        .collect();
    // The lower bound: a merge-base fold over the stack tips and the target tip.
    let mut lower_bound = compute_lower_bound(
        &frame,
        &stack_tips,
        &target,
        target_commit_id,
        &auto_target,
        &integrated_tips,
        &facts,
        ep_commit,
    );

    // The downgrade: the entrypoint resolved into a workspace above it, but it is integrated and
    // at (or cannot reach) the workspace bound — it is outside the workspace, so present it alone.
    let entrypoint_is_separate = ws_info
        .as_ref()
        .is_some_and(|(rec, _, _)| facts.entrypoint() != Some(*rec));
    let entrypoint_owns_its_commit = facts.entrypoint().is_some_and(|ep_rec| {
        facts.head_of().get(&ep_rec) == Some(&ep_commit)
            || facts.run_of(ep_commit).is_some_and(|(o, _)| o == ep_rec)
    });
    let entrypoint_is_metadata_branch =
        ws_info
            .as_ref()
            .zip(facts.entrypoint_ref())
            .is_some_and(|((_, _, ws_md), ep_ref)| {
                ws_md
                    .stacks(ref_metadata::StackKind::Applied)
                    .any(|ms| ms.branches.iter().any(|b| b.ref_name == *ep_ref))
            });
    if !matches!(frame, Frame::AdHoc)
        && entrypoint_is_separate
        && entrypoint_owns_its_commit
        && !entrypoint_is_metadata_branch
        && facts
            .commits()
            .node_data(ep_commit)
            .flags
            .contains(CommitFlags::Integrated)
        && lower_bound
            .is_some_and(|lb| lb == ep_commit || !first_parent_reaches(&facts, ep_commit, lb))
    {
        frame = Frame::AdHoc;
        // Keep the configured target on downgrade — a downgraded single-branch view still knows its
        // target (upstream: "keep the target in sbm"). Resolve it from the repo when it has no
        // traversal record (the usual case here, since the target sits outside the lone branch);
        // only the integrated-tip fallback is dropped. The segment index is vestigial — the target
        // output re-resolves by ref name.
        target = configured_target.or_else(|| {
            let target_ref = facts.project_meta().target_ref.clone()?;
            let commit = ctx
                .repo
                .try_find_reference(target_ref.as_ref())
                .ok()
                .flatten()?
                .peel_to_id()
                .ok()?
                .detach();
            Some((0, target_ref, commit))
        });
        lower_bound = None;
        stack_tips = vec![ep_commit];
    }

    let kind = match (&frame, &ws_info) {
        (Frame::ManagedOwning { .. }, Some((_, ref_info, _))) => WorkspaceKind::Managed {
            ref_info: ref_info.clone(),
        },
        (Frame::ManagedMissing { .. }, Some((_, ref_info, _))) => {
            WorkspaceKind::ManagedMissingWorkspaceCommit {
                ref_info: ref_info.clone(),
            }
        }
        _ => WorkspaceKind::AdHoc,
    };

    // Metadata-listed names whose ref sits on a run head are lifted into empty segments above the
    // commit, leaving the commit segment anonymous — so runs named by them walk as anonymous and
    // other stacks pass through them, duplicating shared history.
    let meta_lifted: HashSet<gix::refs::FullName> = ws_info
        .as_ref()
        .map(|(_, _, ws_md)| {
            ws_md
                .stacks(ref_metadata::StackKind::Applied)
                .flat_map(|ms| ms.branches.iter())
                .filter(|b| !b.archived)
                .filter(|b| {
                    facts.state.commits.inner.node_indices().any(|nx| {
                        let node = &facts.state.commits.inner[nx];
                        node.flags.contains(CommitFlags::InWorkspace)
                            && !node.flags.contains(CommitFlags::Integrated)
                            && facts
                                .state
                                .run_of
                                .get(&node.id)
                                .is_some_and(|(_, head)| *head == node.id)
                            && node.refs.iter().any(|ri| ri.ref_name == b.ref_name)
                    })
                })
                .map(|b| b.ref_name.clone())
                .collect()
        })
        .unwrap_or_default();

    // Sibling candidates: anonymous run heads with a second in-graph child whose upward path
    // (through out-of-workspace commits) reaches a run named by a metadata-known ref.
    let sibling_of: BTreeMap<gix::ObjectId, (crate::RefInfo, gix::ObjectId)> = ws_info
        .as_ref()
        .filter(|_| !matches!(frame, Frame::AdHoc))
        .map(|(_, _, ws_md)| sibling_candidates(&facts, ws_md))
        .unwrap_or_default();

    // The path from the workspace tip to the entrypoint, as parent choices per merge commit. The
    // stack walk follows these hints so the entrypoint's side of a merge becomes part of the stack.
    let parent_hints: BTreeMap<gix::ObjectId, usize> =
        if entrypoint_is_separate && !matches!(frame, Frame::AdHoc) {
            let start = match &frame {
                Frame::ManagedOwning { ws_commit, .. } => *ws_commit,
                Frame::ManagedMissing { anchor } => *anchor,
                Frame::AdHoc => ep_commit,
            };
            let mut hints = BTreeMap::new();
            fn dfs(
                facts: &Facts<'_>,
                cur: gix::ObjectId,
                target: gix::ObjectId,
                seen: &mut HashSet<gix::ObjectId>,
                hints: &mut BTreeMap<gix::ObjectId, usize>,
            ) -> bool {
                if cur == target {
                    return true;
                }
                if !seen.insert(cur) {
                    return false;
                }
                let parents = facts.commits().node_data(cur).parent_ids.clone();
                for (order, parent) in parents.iter().enumerate() {
                    if facts.commits().node(*parent).is_none() {
                        continue;
                    }
                    if dfs(facts, *parent, target, seen, hints) {
                        hints.insert(cur, order);
                        return true;
                    }
                }
                false
            }
            let mut seen = HashSet::new();
            dfs(&facts, start, ep_commit, &mut seen, &mut hints);
            hints
        } else {
            Default::default()
        };

    // The ad-hoc entry name: what `HEAD` pointed at.
    let adhoc_name = matches!(frame, Frame::AdHoc)
        .then(|| {
            facts
                .entrypoint_ref()
                .map(|rn| crate::RefInfo::from_ref(rn.clone(), ep_commit, &ctx.worktree_by_branch))
        })
        .flatten();

    // Build each stack by walking first-parent runs from its tip until the lower bound.
    // The canonical record graph: one inert segment per run with its full commits, every empty
    // named record with its attachment, edges replayed from the walk, and generations — the
    // full traversed topology as a derived artifact, never mutated by reconciliation.
    //
    // This is the shared intermediate both workspace views project from: the stack view
    // (`StackSegment`) is assembled as segments are minted into it below, and the full-topology
    // view (the `BranchGraph`'s `Branch` records) is derived from it afterwards by `branch_records`.
    // Node storage for the display projection: segments only, no edges (the resolution, ref tables,
    // and branch records are derived from facts and the BranchGraph).
    let mut out = NodeStore::default();
    let mut minted = BTreeMap::new();
    mint_segments(&mut out, &mut minted, &facts, ctx, meta, &frame)?;

    // In managed frames the workspace segment leads; ad-hoc reuses the first stack segment.
    let pre_ws_out = lead_workspace_segment(&frame, &ws_info, &facts, &minted, &mut out)?;

    let mut walked_stack_count = usize::MAX;
    let mut lower_bound_segment_id = None;
    let mut head_by_segment: BTreeMap<usize, gix::ObjectId> = BTreeMap::new();
    let mut stacks = Vec::new();
    // A separate entrypoint's run keeps its own segment: stack collection splits at the entry tip.
    let ep_run_head = facts.run_of(ep_commit).map(|(_, head)| head);
    for (tip_idx, tip) in stack_tips.iter().copied().enumerate() {
        if let Some(stack) = collect_one_stack(
            tip,
            tip_idx,
            &frame,
            lower_bound,
            lower_bound_segment_id,
            &stack_tips,
            &meta_lifted,
            &adhoc_name,
            &parent_hints,
            &sibling_of,
            ep_run_head,
            &facts,
            ctx,
            meta,
            &mut out,
            &mut minted,
            &mut head_by_segment,
        )? {
            stacks.push(stack);
        }
    }

    // A detached head keeps its entry segment anonymous; the name the walk gave it moves back
    // onto the commit, appended to its refs.
    if (detached_head || ctx.detach_entrypoint)
        && matches!(frame, Frame::AdHoc)
        && let Some(seg) = stacks.first_mut().and_then(|s| s.segments.first_mut())
        && let Some(name) = seg.ref_info.take()
    {
        clear_segment_name(&mut out[seg.id]);
        if let Some(first) = out[seg.id]
            .commits
            .first_mut()
            .filter(|c| c.id == ep_commit && !c.refs.iter().any(|ri| ri.ref_name == name.ref_name))
        {
            first.refs.push(name.clone());
        }
        if let Some(first) = seg
            .commits
            .first_mut()
            .filter(|c| c.id == ep_commit && !c.refs.iter().any(|ri| ri.ref_name == name.ref_name))
        {
            first.refs.push(name);
        }
    }

    let ws_out = match pre_ws_out {
        Some(id) => id,
        None => stacks
            .first()
            .and_then(|s| s.segments.first())
            .map(|s| s.id)
            .context("seed: an ad-hoc workspace needs at least one stack segment")?,
    };
    // Sibling projection for stacks whose tip ref advanced beyond the workspace: an anonymous
    // tip segment with a second incoming path adopts the out-of-workspace segment that names it.
    if !sibling_of.is_empty()
        && !facts
            .options()
            .dangerously_skip_postprocessing_for_debugging
    {
        adopt_ahead_siblings(&facts, meta, &sibling_of, &head_by_segment, &mut stacks)?;
    }

    // Metadata materialization for refs at stack bottoms and base commits: every Applied metadata
    // branch whose ref sits on an in-store commit becomes an empty named segment — appended to the
    // stack whose bottom it annotates, or forming an independent empty stack — and the ref
    // disappears from display. Mid-stack splits are not implemented yet.
    if let Some((_, _, ws_md)) = ws_info.as_ref().filter(|_| {
        !facts
            .options()
            .dangerously_skip_postprocessing_for_debugging
    }) {
        let missing_anchor = match &frame {
            Frame::ManagedMissing { anchor } if target.is_none() => Some(*anchor),
            _ => None,
        };
        // The workspace commit and its parents are the de-facto stacks; metadata only creates a
        // stack anchored at one of them (or the workspace commit itself).
        let ws_commit = match &frame {
            Frame::ManagedOwning { ws_commit, .. } => Some(*ws_commit),
            Frame::ManagedMissing { anchor } => Some(*anchor),
            Frame::AdHoc => None,
        };
        walked_stack_count = stacks.len();
        if let Some(consumed_anchor) = materialize_metadata_branches(
            &mut out,
            &facts,
            meta,
            &meta_lifted,
            target.is_none() && target_commit_id.is_none() && integrated_tips.is_empty(),
            ws_md,
            &mut stacks,
            lower_bound,
            missing_anchor,
            &stack_tips,
            ws_commit,
            &mut minted,
            matches!(frame, Frame::AdHoc),
            matches!(
                frame,
                Frame::ManagedOwning {
                    commit_is_managed: true,
                    ..
                }
            ),
        )? {
            // Independent metadata stacks consumed the anchor: it becomes the lower bound and
            // the plain anchor stack vanishes.
            lower_bound = Some(consumed_anchor);
            stacks.retain(|stack| {
                !stack
                    .segments
                    .iter()
                    .all(|seg| seg.commits.iter().all(|c| c.id == consumed_anchor))
                    || stack.segments.iter().any(|seg| {
                        seg.ref_name().is_some_and(|rn| {
                            ws_md
                                .stacks(ref_metadata::StackKind::Applied)
                                .any(|ms| ms.branches.iter().any(|b| b.ref_name.as_ref() == rn))
                        })
                    })
            });
        }
    }

    // The entrypoint inside the workspace gets its own, named segment and the marker.
    if entrypoint_is_separate && !matches!(frame, Frame::AdHoc) {
        mark_entrypoint(
            &mut out,
            &facts,
            &minted,
            facts.entrypoint_ref(),
            ep_commit,
            &mut stacks,
        );
    } else if matches!(frame, Frame::AdHoc)
        && let Some(seg) = stacks.iter_mut().find_map(|stack| {
            stack
                .segments
                .iter_mut()
                .find(|seg| seg.commits.first().is_some_and(|c| c.id == ep_commit))
        })
        && seg.commits.first().map(|c| c.id) != stack_tips.first().copied()
    {
        // An ad-hoc entry buried in the stack carries the marker on its own run.
        seg.is_entrypoint = true;
    }

    // Stack identity and order: match each stack against workspace metadata via
    // `find_matching_stack_id`, then order matched stacks by their metadata position.
    if let Some((_, _, ws_md)) = ws_info.as_ref()
        && !matches!(frame, Frame::AdHoc)
    {
        let mut used = std::collections::BTreeSet::new();
        for stack in stacks.iter_mut() {
            stack.id = find_matching_stack_id(Some(ws_md), &stack.segments, &mut used)
                .map(|(id, _in_ws)| id);
        }

        // Order like the workspace-edge reorder: one stable sort over the edge-iteration
        // order, where a stack whose tip segment is named by a metadata stack's first branch
        // keys by that metadata position, and an unmatched stack keys by its iteration
        // position — the two keyspaces interleave. Iteration order is petgraph's
        // newest-edge-first: materialized stacks in reverse creation order, then the walked
        // stacks in collection order.
        let meta_pos_of = |stack: &Stack| {
            stack
                .segments
                .first()
                .and_then(|s| s.ref_name())
                .and_then(|rn| {
                    ws_md.stacks.iter().position(|ms| {
                        ms.is_in_workspace()
                            && ms.branches.iter().any(|b| b.ref_name.as_ref() == rn)
                    })
                })
        };
        let all = std::mem::take(&mut stacks);
        let walked_stack_count = walked_stack_count.min(all.len());
        let mut iteration: Vec<Stack> = Vec::with_capacity(all.len());
        let mut materialized: Vec<Stack> = Vec::new();
        for (i, stack) in all.into_iter().enumerate() {
            if i < walked_stack_count {
                iteration.push(stack);
            } else {
                materialized.push(stack);
            }
        }
        // Owning frames iterate real workspace edges newest-first, putting late-created
        // (materialized) stacks ahead; a workspace without its own commit has no such edges,
        // and chains collect in creation order.
        let mut iteration: Vec<Stack> = if matches!(frame, Frame::ManagedOwning { .. }) {
            materialized.into_iter().rev().chain(iteration).collect()
        } else {
            iteration.into_iter().chain(materialized).collect()
        };
        let mut keyed: Vec<(usize, usize, Stack)> = iteration
            .drain(..)
            .enumerate()
            .map(|(pos, stack)| {
                let key = meta_pos_of(&stack).unwrap_or(pos);
                (key, pos, stack)
            })
            .collect();
        keyed.sort_by_key(|(key, pos, _)| (*key, *pos));
        stacks = keyed.into_iter().map(|(_, _, stack)| stack).collect();

        // Archived pruning: an archived branch whose segment and everything below it are empty
        // truncates the stack there.
        for ms in ws_md.stacks(ref_metadata::StackKind::Applied) {
            for b in ms.branches.iter().filter(|b| b.archived) {
                let Some((stack_idx, seg_idx)) =
                    stacks.iter().enumerate().find_map(|(si, stack)| {
                        stack
                            .segments
                            .iter()
                            .position(|seg| seg.ref_name() == Some(b.ref_name.as_ref()))
                            .map(|gi| (si, gi))
                    })
                else {
                    continue;
                };
                let stack = &mut stacks[stack_idx];
                if !stack.segments[seg_idx..]
                    .iter()
                    .all(|s| s.commits.is_empty())
                {
                    continue;
                }
                stack.segments.truncate(seg_idx);
            }
        }
        stacks.retain(|stack| !stack.segments.is_empty());
    }

    // Integrated pruning: nothing without a target; extra integrated tips beyond the target mean
    // upstream advanced, which floors each stack at its fork point on the target's first-parent
    // trunk while keeping fully-integrated stacks alive; otherwise everything at or below the
    // target goes. This runs for every frame.
    if target.is_some() {
        let upstream_advanced = upstream_advanced_past_target(&facts, target.as_ref());
        // A nameless integrated tip that leads its run is the resolved target position —
        // the same fallback that becomes `target_commit` — and anchors pruning like a
        // stored one.
        let effective_target_commit_id = target_commit_id.or_else(|| {
            facts
                .traversal_tips()
                .iter()
                .filter(|tip| tip.role.is_integrated() && tip.ref_name.is_none())
                .filter(|tip| target.as_ref().map(|(_, _, c)| *c) != Some(tip.id))
                .filter(|tip| facts.run_of(tip.id).is_some_and(|(_, h)| h == tip.id))
                .max_by_key(|tip| {
                    // The minted run segments never carried a generation (always 0), so this only
                    // ever distinguished "has a minted run" (Some(0)) from not (None) — preserved.
                    facts
                        .run_of(tip.id)
                        .and_then(|(_, h)| minted.get(&h))
                        .map(|_| 0usize)
                })
                .map(|tip| tip.id)
        });
        if !(effective_target_commit_id.is_none() && upstream_advanced) {
            let anchor = effective_target_commit_id.or(target.as_ref().map(|(_, _, c)| *c));
            let prune_set: HashSet<gix::ObjectId> = match anchor {
                Some(anchor) if upstream_advanced => {
                    // The target's first-parent trunk only.
                    let mut set = HashSet::new();
                    let mut cur = Some(anchor);
                    while let Some(id) = cur {
                        set.insert(id);
                        cur = facts.commits().first_parent_id(id);
                    }
                    set
                }
                Some(anchor) => facts.commits().ancestor_ids(anchor),
                None => Default::default(),
            };
            for stack in stacks.iter_mut() {
                prune_integrated_stack(&facts, stack, &prune_set, upstream_advanced);
            }
        }
        // Empty segments survive only when the stack's own matched metadata pins them.
        if let Some((_, _, ws_md)) = ws_info.as_ref().filter(|_| !matches!(frame, Frame::AdHoc)) {
            for stack in stacks.iter_mut() {
                let own = stack.id.and_then(|id| {
                    ws_md
                        .stacks(ref_metadata::StackKind::Applied)
                        .find(|ms| ms.id == id)
                });
                stack.segments.retain(|seg| {
                    !seg.commits.is_empty()
                        || own.as_ref().is_some_and(|ms| {
                            seg.ref_info.as_ref().is_some_and(|ri| {
                                ms.branches
                                    .iter()
                                    .any(|b| b.ref_name == ri.ref_name && !b.archived)
                            })
                        })
                });
            }
            stacks.retain(|stack| !stack.segments.is_empty());
        }
    }

    // Mark the last commit of a stack segment that the traversal limit cut (it has unwalked
    // parents), so consumers can show the early end.
    for stack in stacks.iter_mut() {
        for segment in stack.segments.iter_mut() {
            let Some(last) = segment.commits.last_mut() else {
                continue;
            };
            let cut = facts.commits().node(last.id).is_some_and(|nx| {
                let node = &facts.commits().inner[nx];
                !node.parent_ids.is_empty()
                    && !node.flags.contains(CommitFlags::ShallowBoundary)
                    && facts
                        .commits()
                        .inner
                        .neighbors_directed(nx, petgraph::Direction::Outgoing)
                        .next()
                        .is_none()
            });
            if cut {
                last.flags |= crate::workspace::StackCommitFlags::EarlyEnd;
            }
        }
    }

    // The walk's canonical name per run head, computed from facts before the readers so the
    // BranchGraph (and, in turn, the readers) can resolve names without the minted graph: forced
    // tip names, else metadata-disambiguated local, else the picked remote-tracking ref.
    let canonical_name_by_head: BTreeMap<gix::ObjectId, gix::refs::FullName> = {
        let is_local =
            |rn: &gix::refs::FullName| rn.category() == Some(gix::refs::Category::LocalBranch);
        let forced: BTreeMap<gix::ObjectId, gix::refs::FullName> = facts
            .traversal_tips()
            .iter()
            .filter_map(|t| {
                if matches!(t.role, crate::init::TipRole::Workspace) {
                    return None;
                }
                t.ref_name.clone().filter(&is_local).map(|rn| (t.id, rn))
            })
            .collect();
        let workspace_forced: BTreeMap<gix::ObjectId, gix::refs::FullName> = facts
            .traversal_tips()
            .iter()
            .filter_map(|t| {
                matches!(t.role, crate::init::TipRole::Workspace)
                    .then(|| t.ref_name.clone().filter(&is_local).map(|rn| (t.id, rn)))
                    .flatten()
            })
            .collect();
        // The managed workspace commit (a GitButler-created commit) is named by the workspace ref,
        // ahead of any branch sharing its tip — matching the walk, which forces the name there.
        let managed_ws_commit: Option<gix::ObjectId> = match &frame {
            Frame::ManagedOwning {
                ws_commit,
                commit_is_managed: true,
            } => Some(*ws_commit),
            _ => None,
        };
        let name_of = |head: gix::ObjectId| -> Option<gix::refs::FullName> {
            if managed_ws_commit == Some(head)
                && let Some(n) = workspace_forced.get(&head)
            {
                return Some(n.clone());
            }
            let node = facts.commits().node_data(head);
            forced.get(&head).cloned().or_else(|| {
                let locals = node.ref_name_iter().filter(|rn| is_local(rn));
                crate::init::disambiguate_refs_by_branch_metadata(locals, meta)
                    .map(|(rn, _)| rn)
                    .or_else(|| {
                        name_anonymous_run(&facts, ctx, head)
                            .ok()
                            .flatten()
                            .map(|ri| ri.ref_name)
                    })
                    .or_else(|| {
                        let remotes: Vec<gix::refs::FullName> = node
                            .ref_name_iter()
                            .filter(|rn| rn.category() == Some(gix::refs::Category::RemoteBranch))
                            .cloned()
                            .collect();
                        if remotes.is_empty() {
                            return None;
                        }
                        // The target remote wins; else the default remote's ref; else a lone remote.
                        if let Some(t) = facts.project_meta().target_ref.as_ref()
                            && let Some(r) = remotes.iter().find(|r| *r == t)
                        {
                            return Some(r.clone());
                        }
                        if let Some(def) = ctx.symbolic_remote_names.first() {
                            let prefix = format!("refs/remotes/{def}/");
                            if let Some(r) = remotes
                                .iter()
                                .find(|r| r.as_bstr().starts_with(prefix.as_bytes()))
                            {
                                return Some(r.clone());
                            }
                        }
                        (remotes.len() == 1).then(|| remotes[0].clone())
                    })
                    .or_else(|| workspace_forced.get(&head).cloned())
            })
        };
        let mut map: BTreeMap<gix::ObjectId, gix::refs::FullName> = BTreeMap::new();
        for (&_owner, &head) in facts.head_of().iter() {
            if let Some(name) = name_of(head) {
                map.insert(head, name);
            }
        }
        // Lower-bound clear (direct.rs:1952): the base run drops a metadata-stack name (it moves to
        // the materialized empty segment) and re-derives via name_anonymous_run.
        let meta_stacks_flat: HashSet<gix::refs::FullName> = ws_info
            .as_ref()
            .map(|(_, _, md)| {
                md.stacks(but_core::ref_metadata::StackKind::Applied)
                    .flat_map(|ms| ms.branches.iter().map(|b| b.ref_name.clone()))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(lb) = lower_bound
            && let Some((_, lb_head)) = facts.run_of(lb)
            && map
                .get(&lb_head)
                .is_some_and(|n| meta_stacks_flat.contains(n))
        {
            map.remove(&lb_head);
            if let Some(ri) = name_anonymous_run(&facts, ctx, lb_head).ok().flatten() {
                map.insert(lb_head, ri.ref_name);
            }
        }
        map
    };

    // The empty named segments the mint materialized without a record counterpart, derived from
    // facts (branch_records skips any already present, so a superset is safe): the consumed
    // local-tracking refs of integrated targets, the target ref itself, and split_remote_runs'
    // virtual stand-ins (a remote commit past a run head carrying >1 remote ref keeps its extra
    // refs as empties anchored at that commit).
    let materialized_empties: Vec<(gix::refs::FullName, Option<gix::ObjectId>)> = {
        let mut empties: Vec<(gix::refs::FullName, Option<gix::ObjectId>)> = Vec::new();
        for (local_tip, local_ref) in &facts.consumed_local_refs {
            empties.push((local_ref.clone(), Some(*local_tip)));
        }
        // The target ref (raw, before the reader resolves it into `target_ref`).
        if let Some((name, commit)) = target
            .as_ref()
            .map(|(_, n, c)| (n.clone(), *c))
            .or_else(|| auto_target.as_ref().map(|(n, c)| (n.clone(), *c)))
        {
            empties.push((name, Some(commit)));
        }
        for nx in facts.commits().inner.node_indices() {
            let head = facts.commits().inner[nx].id;
            if !facts.is_run_head(head) {
                continue;
            }
            for (cidx, cid) in facts.run(head).into_iter().enumerate() {
                if cidx == 0 {
                    continue;
                }
                let Some(cnx) = facts.commits().node(cid) else {
                    continue;
                };
                if !facts.commits().inner[cnx].flags.is_remote() {
                    continue;
                }
                let remote_refs = facts.commits().inner[cnx]
                    .refs
                    .iter()
                    .filter(|ri| ri.ref_name.category() == Some(gix::refs::Category::RemoteBranch))
                    .map(|ri| ri.ref_name.clone());
                for virtual_name in remote_refs.skip(1) {
                    empties.push((virtual_name, Some(cid)));
                }
            }
        }
        empties
    };

    // The BranchGraph: the single rich segment structure, derived from facts + the canonical names
    // + the materialized empties. The readers below resolve segment-by-name lookups against it.
    let branches = branch_records(
        &canonical_name_by_head,
        &facts,
        &materialized_empties,
        ws_info.as_ref().map(|(_, _, md)| md),
    );

    // Remote enrichment: pair each named local stack segment with its remote tracking branch,
    // collect remote-only commits, and flag local commits reachable from a remote — all over the
    // commit store.
    if !facts
        .options()
        .dangerously_skip_postprocessing_for_debugging
    {
        enrich_with_remotes(&facts, ctx, &mut stacks)?;
    }

    // The lower-bound segment: the canonical run segment owning the bound commit. A workspace
    // metadata name on the bound passes to its materialized segment — ownership of the bound
    // commit moves to an unnamed segment — and an anonymous bound then takes a name by the same
    // single-ref/remote-scoped rules that name stack runs, since consumers select bases by
    // segment name.
    if let Some(lb) = lower_bound {
        lower_bound_segment_id = minted_of(&facts, &minted, lb);
        if let Some((_, head)) = facts.run_of(lb)
            && let Some(&sidx) = minted.get(&head)
        {
            if let Some((_, _, ws_md)) = ws_info.as_ref()
                && out[sidx].ref_info.as_ref().is_some_and(|ri| {
                    ws_md
                        .stacks(ref_metadata::StackKind::Applied)
                        .flat_map(|ms| ms.branches.iter())
                        .any(|b| b.ref_name == ri.ref_name)
                })
            {
                clear_segment_name(&mut out[sidx]);
            }
            if out[sidx].ref_info.is_none()
                && let Some(ri) = name_anonymous_run(&facts, ctx, head)?
            {
                apply_name_to_canonical(&mut out, sidx, ri);
            }
        }
        for stack in &mut stacks {
            if let Some(last) = stack.segments.last_mut()
                && last.base == lower_bound
            {
                last.base_segment_id = lower_bound_segment_id;
            }
        }
    }

    // The target record for `TargetRef` — configured, or the ad-hoc auto-target. The canonical
    // graph carries the attached target record; only an unrepresented target mints a new one.
    // Per-commit generation (CommitGraph topological depth), resolved before `facts` is dropped;
    // used by the target/ancestor walks and the StackSegment resolution.
    let generation_by_commit = facts.commits().generation_by_commit_id();
    let target_ref_commit = target.as_ref().map(|(_, _, c)| *c);
    let target_ref = target
        .map(|(_, ref_name, commit)| (ref_name, commit))
        .or(auto_target)
        .map(|(ref_name, commit)| {
            // Resolve where a ref points across the BranchGraph (a run it owns → that run's tip)
            // and facts (a consumed local-tracking ref, else a walked commit carrying it) — the
            // segment-by-name lookup the record graph used to serve from `out`.
            let resolve_ref_commit = |name: &gix::refs::FullNameRef| -> Option<gix::ObjectId> {
                branches
                    .iter()
                    .find(|b| b.ref_name.as_ref().is_some_and(|n| n.as_ref() == name))
                    .and_then(|b| b.commits.first().map(|c| c.id))
                    .or_else(|| {
                        facts
                            .consumed_local_refs
                            .iter()
                            .find(|(_, r)| r.as_ref() == name)
                            .map(|(tip, _)| *tip)
                    })
                    .or_else(|| {
                        facts.commits().inner.node_indices().find_map(|nx| {
                            let node = &facts.commits().inner[nx];
                            node.refs
                                .iter()
                                .any(|ri| ri.ref_name.as_ref() == name)
                                .then_some(node.id)
                        })
                    })
            };
            // Commits the target is ahead of the workspace by: the standard git ahead-count
            // `lower_bound..target` (commits reachable from the target but not from the integration
            // point), minus any already in the workspace. This replaces the record graph's
            // `visit_upstream_commits` segment walk; its segment-generation prune relied on synthetic
            // workspace→target base edges that the commit graph doesn't have (and that made a
            // *disjoint* target wrongly read as 0-ahead — here it counts its unintegrated commits).
            let not_in_workspace = |id: &gix::ObjectId| {
                !facts
                    .commits()
                    .node_data(*id)
                    .flags
                    .contains(CommitFlags::InWorkspace)
            };
            let commits_ahead = match lower_bound {
                Some(lb) => facts
                    .commits()
                    .commits_reachable_from_a_not_b(commit, lb, false)
                    .into_iter()
                    .filter(not_in_workspace)
                    .count(),
                None => facts
                    .commits()
                    .ancestor_ids(commit)
                    .into_iter()
                    .filter(not_in_workspace)
                    .count(),
            };
            // The target tip: where the target ref points, the target commit otherwise. The
            // local-tracking sibling (checkout fallbacks resolve through it) resolves by name.
            let tip_commit_id = resolve_ref_commit(ref_name.as_ref()).or(Some(commit));
            // The target's local-tracking sibling (checkout fallbacks resolve through it). First the
            // local→remote pairing the inline pass uses (a local branch whose remote-tracking ref is
            // this target — config-aware), else bare prefix-stripping; resolved over the BranchGraph.
            let local_tracking = branches
                .iter()
                .find_map(|b| {
                    let local = b.ref_name.clone()?;
                    if local.category() != Some(gix::refs::Category::LocalBranch) {
                        return None;
                    }
                    let remote = crate::init::remotes::lookup_remote_tracking_branch_or_deduce_it(
                        ctx.repo,
                        local.as_ref(),
                        ctx.symbolic_remote_names,
                        ctx.configured_remote_tracking_branches,
                    )
                    .ok()??;
                    (remote == ref_name).then_some(local)
                })
                .or_else(|| deduce_local_of_remote(ctx.repo, ref_name.as_ref()))
                .and_then(|local| {
                    let tip = resolve_ref_commit(local.as_ref())?;
                    Some(crate::RefInfo::from_ref(
                        local,
                        tip,
                        &ctx.worktree_by_branch,
                    ))
                });
            TargetRef {
                ref_name,
                tip_commit_id,
                local_tracking,
                commits_ahead,
            }
        });

    // A single ad-hoc stack whose head is the bound is fully integrated inline: show it empty.
    if stacks.len() == 1
        && let Some(first) = stacks[0].segments.first()
        && lower_bound.is_some()
        && first.commits.first().map(|c| c.id) == lower_bound
    {
        stacks[0].segments.drain(1..);
        let first = stacks[0].segments.first_mut().expect("non-empty");
        first.commits.clear();
        first.commits_by_segment.clear();
    }

    // Reconcile branch metadata: a segment named after a metadata branch should carry that
    // branch's metadata, but the various naming sites don't all attach it. Attach it once, here,
    // to every local-branch segment that still lacks it.
    if !facts
        .options()
        .dangerously_skip_postprocessing_for_debugging
    {
        let named: Vec<usize> = out
            .segments()
            .filter(|s| {
                out[*s].metadata.is_none()
                    && out[*s].ref_info.as_ref().is_some_and(|ri| {
                        ri.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                    })
            })
            .collect();
        for sidx in named {
            let name = out[sidx]
                .ref_info
                .as_ref()
                .expect("filtered")
                .ref_name
                .clone();
            if let Some(md) = meta.branch_opt(name.as_ref()).ok().flatten() {
                out[sidx].metadata = Some(crate::SegmentMetadata::Branch(
                    ref_metadata::Branch::clone(&md),
                ));
            }
        }
    }

    // The lower bound's canonical name, resolved from facts before `facts` is dropped.
    let lower_bound_ref_name = lower_bound
        .and_then(|lb| facts.run_of(lb))
        .and_then(|(_, h)| canonical_name_by_head.get(&h).cloned());
    // The walk's commit graph: merge-base/reachability and the workspace carry it directly instead
    // of re-deriving one from the segments; `facts` is done borrowing `state` here.
    drop(facts);
    let commit_graph = state.commits;

    let target_commit = {
        let resolve = |commit_id: gix::ObjectId| {
            // The commit only counts when it leads a branch.
            branches
                .iter()
                .any(|b| b.commits.first().map(|c| c.id) == Some(commit_id))
                .then_some(crate::workspace::TargetCommit { commit_id })
        };
        target_commit_id.and_then(resolve).or_else(|| {
            // `integrated_tip_target_commit`: an integrated traversal tip is fallback
            // target context, deepest generation first, unless the target ref already
            // points there.
            state
                .traversal_tips
                .iter()
                .filter(|tip| tip.role.is_integrated())
                // Ref-named tips belong to target-ref resolution; only nameless tips
                // (extra targets, stored positions) provide commit-level context.
                .filter(|tip| tip.ref_name.is_none())
                .filter(|tip| target_ref_commit != Some(tip.id))
                .filter_map(|tip| resolve(tip.id))
                .max_by_key(|tc| {
                    generation_by_commit
                        .get(&tc.commit_id)
                        .copied()
                        .unwrap_or_default()
                })
        })
    };

    // Resolve commit-addressed values that consumers would otherwise navigate the record graph
    // for: each stack segment's own tip (skip-empty, ref-info fallback), its remote tracking tip,
    // and its generation. build owns the graph, so it resolves these once; the projected output
    // then carries them directly. The skip-empty tip is derived from the BranchGraph the workspace
    // carries (navigate `outgoing` past empty branches; ambiguous = ≠1 outgoing → None), matching
    // `Graph::tip_skip_empty` without a record-graph lookup.
    let branch_by_ref: std::collections::HashMap<gix::refs::FullName, usize> = branches
        .iter()
        .enumerate()
        .filter_map(|(i, b)| b.ref_name.clone().map(|rn| (rn, i)))
        .collect();
    let skip_empty_tip = |start: usize| -> Option<gix::ObjectId> {
        let mut idx = start;
        for _ in 0..branches.len().max(1) {
            let b = branches.get(idx)?;
            if let Some(c) = b.commits.first() {
                return Some(c.id);
            }
            match b.outgoing.as_slice() {
                [(next, _)] => idx = *next,
                _ => return None,
            }
        }
        None
    };
    for stack in &mut stacks {
        let n = stack.segments.len();
        for i in 0..n {
            let tip_commit_id = stack.segments[i]
                .commits
                .first()
                .map(|c| c.id)
                .or_else(|| {
                    stack.segments[i]
                        .ref_info
                        .as_ref()
                        .and_then(|ri| branch_by_ref.get(&ri.ref_name).copied())
                        .and_then(|idx| skip_empty_tip(idx))
                })
                .or_else(|| {
                    stack.segments[i]
                        .ref_info
                        .as_ref()
                        .and_then(|ri| ri.commit_id)
                });
            // The base segment is the next one down in the stack; the bottom rests on the lower bound.
            let base_ref_name = match stack.segments.get(i + 1) {
                Some(below) => below.ref_info.as_ref().map(|ri| ri.ref_name.clone()),
                None => stack.segments[i].base.and(lower_bound_ref_name.clone()),
            };
            let seg = &mut stack.segments[i];
            seg.tip_commit_id = tip_commit_id;
            seg.base_ref_name = base_ref_name;
            // A segment carries `commits_outside` exactly when it adopted an out-of-workspace sibling.
            seg.projected_from_outside = seg.commits_outside.is_some();
            seg.remote_tip_id = seg
                .remote_tracking_ref_name
                .as_ref()
                .and_then(|rn| branch_by_ref.get(rn).copied())
                .and_then(|idx| skip_empty_tip(idx));
            seg.generation = tip_commit_id
                .and_then(|id| generation_by_commit.get(&id).copied())
                .unwrap_or(0);
        }
    }
    // The ws ref_info comes from ws_info (managed frames) or the first stack segment (ad-hoc),
    // mirroring lead_workspace_segment, which sets out[ws_out].ref_info from exactly that.
    let workspace_ref_info = match (&frame, &ws_info) {
        (Frame::ManagedOwning { .. } | Frame::ManagedMissing { .. }, Some((_, ri, _))) => {
            Some(ri.clone())
        }
        _ => stacks
            .first()
            .and_then(|s| s.segments.first())
            .and_then(|seg| seg.ref_info.clone()),
    };
    // The workspace tip, navigating the BranchGraph rather than the record graph: the branch leading
    // the frame's workspace/anchor commit (the first stack's named segment for ad-hoc), then
    // skip-empty past empty branches, ref-info commit as the final fallback. Matches
    // `Graph::tip_skip_empty(ws_out)`.
    let workspace_tip_commit_id = {
        let ws_anchor = match &frame {
            Frame::ManagedOwning { ws_commit, .. } => Some(*ws_commit),
            Frame::ManagedMissing { anchor } => Some(*anchor),
            Frame::AdHoc => None,
        };
        ws_anchor
            .and_then(|wc| {
                branches
                    .iter()
                    .position(|b| b.commits.first().map(|c| c.id) == Some(wc))
            })
            .or_else(|| {
                stacks
                    .first()
                    .and_then(|s| s.segments.first())
                    .and_then(|seg| seg.ref_info.as_ref())
                    .and_then(|ri| branch_by_ref.get(&ri.ref_name).copied())
            })
            .or_else(|| branches.iter().position(|b| b.is_entrypoint))
            .and_then(|i| skip_empty_tip(i))
            .or_else(|| workspace_ref_info.as_ref().and_then(|ri| ri.commit_id))
    };
    // The integrated target tip: the first integrated traversal tip that leads a branch, then
    // skip-empty. Matches `integrated_tip_segments()` + `tip_skip_empty`.
    let integrated_target_tip_commit_id = state
        .traversal_tips
        .iter()
        .filter(|tip| tip.role.is_integrated())
        .find_map(|tip| {
            let idx = branches
                .iter()
                .position(|b| b.commits.first().map(|c| c.id) == Some(tip.id))?;
            skip_empty_tip(idx)
        });
    // The name and resolved tip of every named branch, so consumers can resolve a ref to its
    // segment tip without the record graph (what segment_by_ref_name + tip_skip_empty did). Derived
    // from the BranchGraph: a ref that shares its commit with others is a distinct branch here, so it
    // resolves correctly even in ambiguous cases (e.g. a dependent branch sharing a commit's refs).
    let named_segments: Vec<(gix::refs::FullName, gix::ObjectId)> = branches
        .iter()
        .enumerate()
        .filter_map(|(i, b)| Some((b.ref_name.clone()?, skip_empty_tip(i)?)))
        .collect();
    // Every ref name to its resolved commit, mirroring segment_and_commit_by_ref_name: a branch
    // name resolves to its segment tip, any other ref to the commit that carries it; first hit in
    // branch order wins. Derived from the BranchGraph, like named_segments.
    let ref_tips: Vec<(gix::refs::FullName, gix::ObjectId)> = {
        let mut seen = std::collections::HashSet::new();
        let mut ref_tips = Vec::new();
        for (i, b) in branches.iter().enumerate() {
            if let (Some(name), Some(tip)) = (b.ref_name.clone(), skip_empty_tip(i))
                && seen.insert(name.clone())
            {
                ref_tips.push((name, tip));
            }
            for commit in &b.commits {
                for ri in &commit.refs {
                    if seen.insert(ri.ref_name.clone()) {
                        ref_tips.push((ri.ref_name.clone(), commit.id));
                    }
                }
            }
        }
        ref_tips
    };
    let hard_limit_hit = ctx.hard_limit;
    // Multiple worktrees: ≥2 distinct worktree kinds are checked out across the tracked branches.
    let has_multiple_worktrees = {
        let mut first: Option<&crate::WorktreeKind> = None;
        let mut multiple = false;
        for wt in ctx.worktree_by_branch.values().flatten() {
            match first {
                Some(f) if *f != wt.kind => multiple = true,
                None => first = Some(&wt.kind),
                _ => {}
            }
        }
        multiple
    };
    // The entrypoint is always seeded at `ep_commit` (see the `AtCommit` the traversal recorded).
    let entrypoint_commit_id = Some(ep_commit);
    // For advanced (non-managed-commit) workspaces, resolve the managed commit in the ancestry and
    // the commits on top of it: a first-parent walk from the workspace tip down to the lower bound.
    let ancestor_workspace_commit = (!kind.has_managed_commit())
        .then(|| {
            find_ancestor_workspace_commit(
                &commit_graph,
                ctx.repo,
                workspace_tip_commit_id,
                lower_bound,
                &generation_by_commit,
            )
        })
        .flatten();
    // The entrypoint segment's ref name, so re-traversal without an overlay-supplied entrypoint
    // reseeds from the same ref. This is the ref the traversal entered at.
    let rebuild_entrypoint_ref = state.entrypoint_ref.clone();
    // The rebuild context: enough to re-run the traversal and to serve commit-level queries without
    // the record graph.
    let rebuild_commit_graph = Some(commit_graph);
    let rebuild_project_meta = state.project_meta.clone();
    let rebuild_options = state.options.clone();
    let rebuild_symbolic_remote_names: Vec<String> = Vec::new();

    Ok(Workspace {
        commit_graph: rebuild_commit_graph,
        project_meta: rebuild_project_meta,
        options: rebuild_options,
        entrypoint_ref: rebuild_entrypoint_ref,
        symbolic_remote_names: rebuild_symbolic_remote_names,
        branches: Some(branches),
        id: ws_out,
        tip_commit_id: workspace_tip_commit_id,
        ref_info: workspace_ref_info,
        kind,
        stacks,
        lower_bound,
        lower_bound_ref_name,
        target_ref,
        target_commit,
        integrated_target_tip_commit_id,
        ancestor_workspace_commit,
        named_segments,
        ref_tips,
        hard_limit_hit,
        has_multiple_worktrees,
        entrypoint_commit_id,
        metadata: ws_info
            .filter(|_| !matches!(frame, Frame::AdHoc))
            .map(|(_, _, md)| md),
    })
}

/// Append `commits` as a stack segment named `name`, or merge them into the previous segment when
/// anonymous, mirroring how stack collection aggregates unnamed graph segments.
#[allow(clippy::too_many_arguments)]
fn push_stack_run(
    out: &mut NodeStore,
    facts: &Facts<'_>,
    minted: &mut BTreeMap<gix::ObjectId, usize>,
    segments: &mut Vec<StackSegment>,
    name: Option<crate::RefInfo>,
    metadata: Option<crate::SegmentMetadata>,
    run_head: gix::ObjectId,
    commits: Vec<gix::ObjectId>,
    own_segment: bool,
) -> bool {
    let strip = name.as_ref().map(|ri| ri.ref_name.clone());
    let stack_commits: Vec<StackCommit> = commits
        .iter()
        .map(|id| StackCommit::from_graph_commit(&graph_commit(facts, *id, strip.as_ref())))
        .collect();
    let mut mint =
        |out: &mut NodeStore, name: Option<crate::RefInfo>, md: Option<crate::SegmentMetadata>| {
            // An entry truncated at the bound carries no commits but still identifies with the
            // canonical run segment: truncation only clears the commits, not the identity.
            let head = commits.first().copied().unwrap_or(run_head);
            match minted.get(&head) {
                Some(&sidx) => {
                    // The canonical record segment adopts the projection-resolved name, consuming the
                    // ref off the commit.
                    if let Some(ri) = name {
                        apply_name_to_canonical(out, sidx, ri);
                    }
                    if md.is_some() {
                        out[sidx].metadata = md;
                    }
                    sidx
                }
                None => {
                    let strip = name.as_ref().map(|ri| ri.ref_name.clone());
                    let sidx = out.insert_segment(MintSeg {
                        ref_info: name,
                        metadata: md,
                        commits: commits
                            .iter()
                            .map(|id| graph_commit(facts, *id, strip.as_ref()))
                            .collect(),
                        ..Default::default()
                    });
                    minted.insert(head, sidx);
                    sidx
                }
            }
        };

    if name.is_none()
        && !own_segment
        && let Some(prev) = segments.last_mut()
    {
        let rec = mint(out, None, None);
        let offset = prev.commits.len();
        prev.commits.extend(stack_commits);
        prev.commits_by_segment.push((rec, offset));
        return false;
    }

    // The display metadata is this run's branch metadata — the caller-computed `metadata`, not a
    // read-back of out's accumulated value.
    let seg_metadata = match &metadata {
        Some(crate::SegmentMetadata::Branch(md)) => Some(md.clone()),
        _ => None,
    };
    let rec = mint(out, name.clone(), metadata);
    segments.push(StackSegment {
        ref_info: name,
        id: rec,
        commits: stack_commits,
        commits_by_segment: vec![(rec, 0)],
        metadata: seg_metadata,
        ..blank_stack_segment()
    });
    true
}

/// For every non-first remote commit in a multi-commit segment that carried a remote-tracking ref
/// in the walk, split the segment there with the new lower segment named by the first such ref,
/// and turn additional refs at the same commit into empty virtual segments pointing at it. The
/// displayed commits already dropped remote refs, so the walk store provides them.
/// Split `sidx` at `cidx`: the tail commits move into a new segment (named by `ref_info`),
/// every outgoing edge moves onto it leaving from its last commit, and the top connects above
/// the tail.
fn split_out_segment(
    out: &mut NodeStore,
    sidx: usize,
    cidx: usize,
    ref_info: Option<crate::RefInfo>,
) -> usize {
    let bottom_commits: Vec<Commit> = out[sidx].commits.drain(cidx..).collect();
    out.insert_segment(MintSeg {
        ref_info,
        commits: bottom_commits,
        ..Default::default()
    })
}

/// A ref both projections treat as internal — GitButler's own workspace refs and remote-tracking
/// branches never name a user-visible segment, in the display stacks or the rebase branch records.
/// The single decision both `graph_commit`/`branch_records` and `collect_one_stack` share.
fn is_internal_ref(name: &gix::refs::FullNameRef) -> bool {
    name.as_bstr().starts_with(b"refs/heads/gitbutler/")
        || name.category() == Some(gix::refs::Category::RemoteBranch)
}

fn graph_commit(
    facts: &Facts<'_>,
    id: gix::ObjectId,
    strip: Option<&gix::refs::FullName>,
) -> Commit {
    let node = facts.commits().node_data(id);
    Commit {
        id,
        parent_ids: node.parent_ids.clone(),
        flags: node.flags,
        refs: node
            .refs
            .iter()
            .filter(|ri| {
                Some(&ri.ref_name) != strip
                    && !is_internal_ref(ri.ref_name.as_ref())
                    && !facts
                        .consumed_local_refs
                        .contains(&(id, ri.ref_name.clone()))
                    && !facts.consumed_meta_refs.borrow().contains(&ri.ref_name)
            })
            .cloned()
            .collect(),
    }
}

/// Pair local stack segments with their remote tracking branches and enrich the stacks with
/// remote-only commits and remote-reachability flags.
fn enrich_with_remotes(
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    stacks: &mut [Stack],
) -> anyhow::Result<()> {
    use crate::workspace::StackCommitFlags;

    // Local name → (remote ref, remote tip commit), via repository configuration, resolved
    // against the walk's discovered remote records.
    let mut remote_of = BTreeMap::new();
    for stack in stacks.iter() {
        for segment in &stack.segments {
            let Some(local) = segment.ref_name() else {
                continue;
            };
            let Some(remote_ref) =
                crate::init::remotes::lookup_remote_tracking_branch_or_deduce_it(
                    ctx.repo,
                    local,
                    ctx.symbolic_remote_names,
                    ctx.configured_remote_tracking_branches,
                )?
            else {
                continue;
            };
            // The remote tip: the record named after the remote ref, or a commit carrying it.
            // Without one, the name still pairs up, but there is nothing to walk.
            let tip = facts.record_commit_named(remote_ref.as_ref());
            remote_of.insert(segment.id, (remote_ref, tip));
        }
    }

    // Wire links and collect remote-only commits per paired segment.
    for stack in stacks.iter_mut() {
        let mut above_commit_ids: HashSet<gix::ObjectId> = HashSet::new();
        for segment in &mut stack.segments {
            let Some((remote_ref, remote_tip)) = remote_of.get(&segment.id).cloned() else {
                above_commit_ids.extend(segment.commits.iter().map(|c| c.id));
                continue;
            };
            segment.remote_tracking_ref_name = Some(remote_ref.clone());
            let Some(remote_tip) = remote_tip else {
                above_commit_ids.extend(segment.commits.iter().map(|c| c.id));
                continue;
            };
            // Remote-only commits: walk run-wise from the remote tip while commits are
            // remote-only, stopping at runs owned by other remote-tracking records — the same
            // order as the segment walk, with each run-owner's name consumed.
            let mut remote_commits = Vec::new();
            let mut seen_runs = HashSet::new();
            let mut run_queue = std::collections::VecDeque::new();
            if facts.has_commit(remote_tip) {
                run_queue.push_back(remote_tip);
                seen_runs.insert(remote_tip);
            }
            while let Some(run_head) = run_queue.pop_front() {
                if !facts.commits().node_data(run_head).flags.is_remote() {
                    continue;
                }
                // A run owned by another remote-named record is that remote's territory.
                let owner_name = facts
                    .run_of(run_head)
                    .and_then(|(owner, _)| facts.ref_info_of(owner).map(|ri| ri.ref_name.clone()));
                if run_head != remote_tip
                    && owner_name.as_ref().is_some_and(|rn| {
                        rn.category() == Some(gix::refs::Category::RemoteBranch)
                            && *rn != remote_ref
                    })
                {
                    continue;
                }
                let mut all_remote = true;
                for id in facts.run(run_head) {
                    let node = facts.commits().node_data(id);
                    if !node.flags.is_remote() {
                        all_remote = false;
                        break;
                    }
                    // A later commit carrying a foreign remote-tracking ref starts that
                    // remote's own split segment; its territory ends the walk.
                    if id != remote_tip
                        && node.refs.iter().any(|ri| {
                            ri.ref_name.category() == Some(gix::refs::Category::RemoteBranch)
                                && ri.ref_name != remote_ref
                        })
                    {
                        all_remote = false;
                        break;
                    }
                    remote_commits.push(StackCommit::from_graph_commit(&graph_commit(
                        facts,
                        id,
                        owner_name.as_ref().or(Some(&remote_ref)),
                    )));
                    for parent in &node.parent_ids {
                        if facts.has_commit(*parent)
                            && facts
                                .run_of(*parent)
                                .is_some_and(|(_, head)| head == *parent)
                            && seen_runs.insert(*parent)
                        {
                            run_queue.push_back(*parent);
                        }
                    }
                }
                // A run continuing into its first parent (not a run head) is the same segment;
                // only fully-remote runs keep walking.
                let _ = all_remote;
            }

            // The branch-split case: non-integrated commits from upper stack segments that are
            // still reachable first-parent from the remote tip.
            if !above_commit_ids.is_empty() {
                let mut known: HashSet<_> = remote_commits.iter().map(|c| c.id).collect();
                let mut cur = facts.commits().node(remote_tip).map(|_| remote_tip);
                while let Some(id) = cur {
                    let node = facts.commits().node_data(id);
                    if above_commit_ids.contains(&id)
                        && !node.flags.contains(CommitFlags::Integrated)
                        && known.insert(id)
                    {
                        let owner_name = facts.run_of(id).and_then(|(owner, _)| {
                            facts.ref_info_of(owner).map(|ri| ri.ref_name.clone())
                        });
                        remote_commits.push(StackCommit::from_graph_commit(&graph_commit(
                            facts,
                            id,
                            owner_name.as_ref(),
                        )));
                    }
                    cur = facts
                        .commits()
                        .first_parent_id(id)
                        .filter(|p| facts.has_commit(*p));
                }
            }
            segment.commits_on_remote = remote_commits;
            above_commit_ids.extend(segment.commits.iter().map(|c| c.id));
        }
    }

    // Remote reachability: walking from each remote tip, the first non-remote commit reached and
    // everything below it in its stack is reachable by that remote. Sibling-adopted segments carry
    // their pairing for display only and are excluded — they're exactly the ones adopt gave
    // `commits_outside` (the output marker for an adopted out-of-workspace sibling).
    let adopted: HashSet<usize> = stacks
        .iter()
        .flat_map(|stack| stack.segments.iter())
        .filter(|seg| seg.commits_outside.is_some())
        .map(|seg| seg.id)
        .collect();
    let pairs: Vec<(gix::refs::FullName, gix::ObjectId)> = remote_of
        .iter()
        .filter(|(sid, _)| !adopted.contains(*sid))
        .filter_map(|(_, (rn, tip))| tip.map(|t| (rn.clone(), t)))
        .collect();
    for (remote_ref, remote_tip) in pairs {
        let mut link_points = Vec::new();
        let mut seen = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        if facts.has_commit(remote_tip) {
            queue.push_back(remote_tip);
            seen.insert(remote_tip);
        }
        while let Some(id) = queue.pop_front() {
            let node = facts.commits().node_data(id);
            if !node.flags.is_remote() {
                link_points.push(id);
                continue;
            }
            for parent in &node.parent_ids {
                if facts.has_commit(*parent) && seen.insert(*parent) {
                    queue.push_back(*parent);
                }
            }
        }
        for link in link_points {
            for stack in stacks.iter_mut() {
                let Some((seg_idx, commit_idx)) =
                    stack.segments.iter().enumerate().find_map(|(si, seg)| {
                        seg.commits
                            .iter()
                            .position(|c| c.id == link)
                            .map(|ci| (si, ci))
                    })
                else {
                    continue;
                };
                let mut first = Some(commit_idx);
                for segment in &mut stack.segments[seg_idx..] {
                    let flags = if segment.remote_tracking_ref_name.as_ref() == Some(&remote_ref) {
                        StackCommitFlags::ReachableByMatchingRemote
                    } else {
                        StackCommitFlags::empty()
                    } | StackCommitFlags::ReachableByRemote;
                    for commit in &mut segment.commits[first.take().unwrap_or_default()..] {
                        commit.flags |= flags;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Whether walking first parents from `from` reaches `to`.
fn first_parent_reaches(facts: &Facts<'_>, from: gix::ObjectId, to: gix::ObjectId) -> bool {
    let mut cur = Some(from);
    while let Some(id) = cur {
        if id == to {
            return true;
        }
        cur = facts.commits().first_parent_id(id);
    }
    false
}

/// Name an anonymous run from its head commit: a single local-branch ref is lifted directly;
/// otherwise a ref with a remote tracking branch known to the traversal wins if it is the only
/// such candidate.
/// Name a canonical segment with a projection-resolved ref and consume that ref from its head
/// commit's displayed refs, like walk-time naming does at construction.
fn apply_name_to_canonical(out: &mut NodeStore, sidx: usize, ri: crate::RefInfo) {
    let name = ri.ref_name.clone();
    out[sidx].ref_info = Some(ri);
    if let Some(first) = out[sidx].commits.first_mut() {
        first.refs.retain(|r| r.ref_name != name);
    }
}

/// A `RefInfo` for `name` that adopts the worktree recorded on the walked commit ref, so
/// checked-out branches keep their marker when the projection re-creates their segment.
fn ref_info_adopting_worktree(
    facts: &Facts<'_>,
    name: &gix::refs::FullName,
    commit_id: Option<gix::ObjectId>,
) -> crate::RefInfo {
    let worktree = facts.state.commits.inner.node_indices().find_map(|nx| {
        facts.state.commits.inner[nx]
            .refs
            .iter()
            .find(|ri| ri.ref_name == *name)
            .and_then(|ri| ri.worktree.clone())
    });
    crate::RefInfo {
        ref_name: name.clone(),
        commit_id,
        worktree,
    }
}

fn name_anonymous_run(
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    head: gix::ObjectId,
) -> anyhow::Result<Option<crate::RefInfo>> {
    let refs = &facts.commits().node_data(head).refs;
    let local: Vec<&crate::RefInfo> = refs
        .iter()
        .filter(|ri| {
            ri.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                && !ri.ref_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
        })
        .collect();
    match local.len() {
        0 => Ok(None),
        1 => Ok(Some(local[0].clone())),
        _ => {
            let mut candidates = Vec::new();
            for ri in local {
                let Some(remote_ref) =
                    crate::init::remotes::lookup_remote_tracking_branch_or_deduce_it(
                        ctx.repo,
                        ri.ref_name.as_ref(),
                        ctx.symbolic_remote_names,
                        ctx.configured_remote_tracking_branches,
                    )?
                else {
                    continue;
                };
                let remote_tip = facts.record_commit_named(remote_ref.as_ref());
                if remote_tip.is_some_and(|tip| first_non_remote_from(facts, tip) == Some(head)) {
                    candidates.push(ri.clone());
                }
            }
            Ok(if candidates.len() == 1 {
                candidates.pop()
            } else {
                None
            })
        }
    }
}

/// The first non-remote commit reached when walking ancestors from `tip` through remote-only
/// commits, if it is unique — the commit a remote tracking branch "links onto".
fn first_non_remote_from(facts: &Facts<'_>, tip: gix::ObjectId) -> Option<gix::ObjectId> {
    let mut hits = Vec::new();
    let mut seen = HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    if facts.has_commit(tip) {
        queue.push_back(tip);
        seen.insert(tip);
    }
    while let Some(id) = queue.pop_front() {
        let node = facts.commits().node_data(id);
        if !node.flags.is_remote() {
            hits.push(id);
            continue;
        }
        for parent in &node.parent_ids {
            if facts.has_commit(*parent) && seen.insert(*parent) {
                queue.push_back(*parent);
            }
        }
    }
    match hits.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// Local refs consumed into synthesized empty local-tracking records for workspace-target
/// remotes: the configured local branch of a target remote, when no record carries that name and
/// its tip is the head of a run.
fn consumed_local_refs(
    state: &State,
    ctx: &Context<'_>,
) -> anyhow::Result<HashSet<(gix::ObjectId, gix::refs::FullName)>> {
    let mut out = HashSet::new();
    for tip in state
        .traversal_tips
        .iter()
        .filter(|tip| tip.role.is_integrated())
    {
        // Only remote-tracking targets have a local-tracking branch to consume; an explicit
        // integrated tip can also name a local branch, which has no upstream-of-tracking.
        let Some(remote_ref) = tip
            .ref_name
            .as_ref()
            .filter(|rn| rn.category() == Some(gix::refs::Category::RemoteBranch))
        else {
            continue;
        };
        let Some((local_ref, _remote)) = ctx
            .repo
            .upstream_branch_and_remote_for_tracking_branch(remote_ref.as_ref())?
        else {
            continue;
        };
        if state
            .ref_info_by_segment
            .values()
            .any(|ri| ri.ref_name == local_ref)
        {
            continue;
        }
        let Some(local_tip) = ctx
            .repo
            .try_find_reference(local_ref.as_ref())?
            .map(|mut r| r.peel_to_id().map(|id| id.detach()))
            .transpose()?
        else {
            continue;
        };
        if state
            .run_of
            .get(&local_tip)
            .is_some_and(|(_, head)| *head == local_tip)
        {
            out.insert((local_tip, local_ref));
        }
    }
    Ok(out)
}

/// Materialize Applied metadata branches whose refs sit on in-workspace commits. A ref on a
/// commit inside an existing stack inserts empty named segments above that commit — the last
/// listed ref names the segment that keeps the commits. Refs at a stack's base append empty
/// segments at its bottom. Metadata stacks owning no traversed commits become independent stacks
/// of empty segments. Mid-run splits are not implemented yet.
#[allow(clippy::too_many_arguments)]
fn materialize_metadata_branches<T: RefMetadata>(
    out: &mut NodeStore,
    facts: &Facts<'_>,
    meta: &OverlayMetadata<'_, T>,
    meta_lifted: &HashSet<gix::refs::FullName>,
    targetless: bool,
    ws_md: &ref_metadata::Workspace,
    stacks: &mut Vec<Stack>,
    lower_bound: Option<gix::ObjectId>,
    missing_anchor: Option<gix::ObjectId>,
    ws_parents: &[gix::ObjectId],
    ws_commit: Option<gix::ObjectId>,
    minted: &mut BTreeMap<gix::ObjectId, usize>,
    adhoc: bool,
    restrict_to_ws_parents: bool,
) -> anyhow::Result<Option<gix::ObjectId>> {
    if adhoc {
        return Ok(None);
    }
    let mut consumed_anchor = None;
    // Per-commit arbitration like the per-commit scan's `.next()`: the first metadata stack in
    // workspace order to match a commit claims it; later stacks' names there stay
    // unmaterialized until flows update the metadata.
    let mut commit_claims: std::collections::HashMap<
        gix::ObjectId,
        but_core::ref_metadata::StackId,
    > = std::collections::HashMap::new();
    // Scan visibility: a ref participates where it remains displayed, which is everywhere except
    // on a commit whose run-owning record bears that very name — queued tips consume their ref
    // into their record. An owner-consumed name resurfaces at stack bases and the lower bound,
    // latching onto the base segment's name.
    let ref_in_store = |name: &gix::refs::FullName| -> Option<(gix::ObjectId, bool)> {
        facts.state.commits.inner.node_indices().find_map(|nx| {
            let node = &facts.state.commits.inner[nx];
            if !node.flags.contains(CommitFlags::InWorkspace)
                || !node.refs.iter().any(|ri| ri.ref_name == *name)
            {
                return None;
            }
            let owner_named = facts.run_of(node.id).is_some_and(|(owner, _)| {
                facts
                    .ref_info_of(owner)
                    .is_some_and(|ri| ri.ref_name == *name)
            }) && !meta_lifted.contains(name);
            Some((node.id, owner_named))
        })
    };
    let branch_md = |meta: &OverlayMetadata<'_, T>,
                     name: &gix::refs::FullName|
     -> Option<ref_metadata::Branch> {
        meta.branch_opt(name.as_ref())
            .ok()
            .flatten()
            .map(|md| ref_metadata::Branch::clone(&md))
    };
    let mk_segment = |out: &mut NodeStore,
                      _minted: &BTreeMap<gix::ObjectId, usize>,
                      name: &gix::refs::FullName,
                      commits: Vec<StackCommit>,
                      commits_by_segment_src: Vec<(usize, usize)>,
                      anchor: Option<gix::ObjectId>|
     -> StackSegment {
        let md = branch_md(meta, name);
        // The empty record sits above its anchor commit; the anchor lets `materialized_empties`
        // resolve its target without a graph edge.
        let rec = out.insert_segment_with_anchor(
            MintSeg {
                ref_info: Some(ref_info_adopting_worktree(facts, name, None)),
                metadata: md.clone().map(crate::SegmentMetadata::Branch),
                ..Default::default()
            },
            anchor,
        );
        StackSegment {
            ref_info: out[rec].ref_info.clone(),
            id: rec,
            commits,
            commits_by_segment: commits_by_segment_src,
            metadata: md,
            ..blank_stack_segment()
        }
    };

    let mut present: HashSet<gix::refs::FullName> = stacks
        .iter()
        .flat_map(|s| s.segments.iter())
        .filter_map(|s| s.ref_name().map(|rn| rn.to_owned()))
        .collect();

    // Names can repeat across (or within) metadata stacks, even with unsound metadata mid-edit;
    // the `planned` set ensures each materializes at most once.
    let mut planned: HashSet<gix::refs::FullName> = HashSet::new();
    for ms in ws_md.stacks(ref_metadata::StackKind::Applied) {
        let names: Vec<&gix::refs::FullName> = ms
            .branches
            .iter()
            .filter(|b| !present.contains(&b.ref_name) && planned.insert(b.ref_name.clone()))
            .map(|b| &b.ref_name)
            .collect();
        if names.is_empty() {
            continue;
        }
        // Group the missing names by the commit their ref sits on.
        let mut by_commit: Vec<(Option<gix::ObjectId>, Vec<&gix::refs::FullName>)> = Vec::new();
        for name in names {
            let at = ref_in_store(name).and_then(|(id, owner_named)| {
                if owner_named {
                    // Owner-consumed names resurface at bases and the bound (the base
                    // segment's name channel), and inside stacks that this metadata stack
                    // already owns (the stack-limited dependent channel) — never inside
                    // foreign stacks.
                    let at_base_or_bound = Some(id) == lower_bound
                        || stacks.iter().any(|stack| {
                            stack
                                .segments
                                .last()
                                .is_some_and(|seg| seg.base == Some(id))
                        });
                    let in_own_stack = stacks.iter().any(|stack| {
                        stack
                            .segments
                            .iter()
                            .any(|seg| seg.commits.iter().any(|c| c.id == id))
                            && stack.segments.iter().any(|seg| {
                                seg.ref_name().is_some_and(|rn| {
                                    ms.branches.iter().any(|b| b.ref_name.as_ref() == rn)
                                })
                            })
                    });
                    (at_base_or_bound || in_own_stack).then_some(id)
                } else {
                    Some(id)
                }
            });
            match by_commit.iter_mut().find(|(existing, _)| *existing == at) {
                Some((_, group)) => group.push(name),
                None => by_commit.push((at, vec![name])),
            }
        }

        for (at, group) in by_commit {
            let Some(at) = at else {
                continue;
            };

            // The single-local-ref lift applies to the bound run too: a lone metadata ref on an
            // anonymous bound names that segment instead of materializing an empty.
            let group: Vec<&gix::refs::FullName> = group
                .into_iter()
                .filter(|name| {
                    // The walk already consumed this ref as the name of the run owning its
                    // commit. It still materializes when it is the first branch of its metadata
                    // stack (the independent path chains the segment name unconditionally) or
                    // when the commit carries other refs (the dependent path's non-empty-refs
                    // gate) — only a dependent name on an otherwise bare commit stays consumed.
                    let owner_already_named = facts.run_of(at).is_some_and(|(owner, _)| {
                        facts
                            .ref_info_of(owner)
                            .is_some_and(|ri| ri.ref_name == **name)
                    });
                    let is_stack_tip_name = ws_md
                        .stacks(ref_metadata::StackKind::Applied)
                        .any(|ms| ms.branches.first().is_some_and(|b| b.ref_name == **name));
                    let other_refs_remain = facts.commits().node(at).is_some_and(|nx| {
                        facts.commits().inner[nx].refs.iter().any(|ri| {
                            ri.ref_name != **name
                                && ri.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                                && !ri.ref_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                        })
                    });
                    let inside_a_stack = stacks.iter().any(|stack| {
                        stack
                            .segments
                            .iter()
                            .any(|seg| seg.commits.iter().any(|c| c.id == at))
                    });
                    if owner_already_named
                        && !is_stack_tip_name
                        && !other_refs_remain
                        && !inside_a_stack
                    {
                        present.insert((**name).clone());
                        return false;
                    }
                    let lifts_onto_bound = Some(at) == lower_bound
                        && !is_stack_tip_name
                        && facts.run_of(at).is_some_and(|(owner, head)| {
                            head == at && facts.ref_info_of(owner).is_none()
                        })
                        && facts.commits().node(at).is_some_and(|nx| {
                            let locals: Vec<_> = facts.commits().inner[nx]
                                .refs
                                .iter()
                                .filter(|ri| {
                                    ri.ref_name.category() == Some(gix::refs::Category::LocalBranch)
                                        && !ri
                                            .ref_name
                                            .as_bstr()
                                            .starts_with(b"refs/heads/gitbutler/")
                                })
                                .collect();
                            locals.len() == 1 && locals[0].ref_name == **name
                        });
                    if lifts_onto_bound {
                        facts
                            .consumed_meta_refs
                            .borrow_mut()
                            .insert((**name).clone());
                        // Name the canonical bound segment.
                        if let Some(sidx) = minted_of(facts, minted, at)
                            && out[sidx].ref_info.is_none()
                        {
                            apply_name_to_canonical(
                                out,
                                sidx,
                                ref_info_adopting_worktree(facts, name, Some(at)),
                            );
                        }
                        present.insert((**name).clone());
                    }
                    !lifts_onto_bound
                })
                .collect();
            if group.is_empty() {
                continue;
            }
            // A workspace anchor without a commit of its own: metadata stacks at it become
            // independent and consume it, when no target exists. A single claiming stack
            // instead absorbs the anchor through the regular insertion below.
            let anchor_claimants = ws_md
                .stacks(ref_metadata::StackKind::Applied)
                .filter(|ms| {
                    ms.branches.iter().any(|b| {
                        facts.commits().node(at).is_some_and(|nx| {
                            facts.commits().inner[nx]
                                .refs
                                .iter()
                                .any(|ri| ri.ref_name == b.ref_name)
                        })
                    })
                })
                .count();
            // The same consumption applies when an anonymous walked stack's tip commit hosts
            // two or more claiming metadata stacks: each becomes an independent chained from the
            // workspace, the original edge is cut, and the commit becomes the bound everything
            // rests on.
            let anon_ws_child_tip = targetless
                .then(|| {
                    stacks.iter().position(|stack| {
                        stack.segments.first().is_some_and(|seg| {
                            seg.ref_info.is_none()
                                && seg.commits.first().is_some_and(|c| c.id == at)
                        })
                    })
                })
                .flatten();
            if (missing_anchor == Some(at) || anon_ws_child_tip.is_some()) && anchor_claimants > 1 {
                if let Some(idx) = anon_ws_child_tip {
                    stacks.remove(idx);
                }
                let mut segments = Vec::new();
                for name in &group {
                    let seg = mk_segment(out, minted, name, Vec::new(), Vec::new(), Some(at));
                    present.insert((*name).clone());
                    segments.push(seg);
                }
                wire_pairwise_bases(&mut segments, facts, minted, Some(at));
                if !segments.is_empty() {
                    consumed_anchor = Some(at);
                    let mut consumed = facts.consumed_meta_refs.borrow_mut();
                    for name in &group {
                        consumed.insert((**name).clone());
                    }
                    stacks.push(Stack {
                        id: Some(ms.id),
                        segments,
                    });
                }
                continue;
            }
            // A commit inside an existing stack: insert the empties above it; the last ref takes
            // over the commits from that position within its stack segment.
            let inside = stacks.iter_mut().find_map(|stack| {
                stack
                    .segments
                    .iter()
                    .position(|seg| seg.commits.iter().any(|c| c.id == at))
                    .map(|seg_idx| (stack, seg_idx))
            });
            let inside = inside.filter(|_| match commit_claims.entry(at) {
                std::collections::hash_map::Entry::Occupied(claim) => *claim.get() == ms.id,
                claim => {
                    claim.or_insert(ms.id);
                    true
                }
            });
            if let Some((stack, seg_idx)) = inside {
                {
                    let mut consumed = facts.consumed_meta_refs.borrow_mut();
                    for name in &group {
                        consumed.insert((**name).clone());
                    }
                }
                // A named segment whose name is part of the group yields to the re-split: the
                // name re-materializes as one of the group empties (or takes the commits if
                // listed last), and the original becomes anonymous.
                if stack.segments[seg_idx]
                    .ref_name()
                    .is_some_and(|rn| group.iter().any(|name| name.as_ref() == rn))
                {
                    stack.segments[seg_idx].ref_info = None;
                    stack.segments[seg_idx].metadata = None;
                }
                // The base the split segment rested on — inherited by whichever new segment
                // ends up last in the split range.
                let split_base = stack.segments[seg_idx].base;
                let split_base_id = stack.segments[seg_idx].base_segment_id;
                let commit_idx = stack.segments[seg_idx]
                    .commits
                    .iter()
                    .position(|c| c.id == at)
                    .expect("just found");
                let tail: Vec<StackCommit> = stack.segments[seg_idx]
                    .commits
                    .drain(commit_idx..)
                    .collect();
                let tail_by_segment: Vec<(usize, usize)> = {
                    let seg = &mut stack.segments[seg_idx];
                    let split: Vec<_> = seg
                        .commits_by_segment
                        .iter()
                        .filter(|(_, ofs)| *ofs >= commit_idx)
                        .map(|(sidx, ofs)| (*sidx, ofs - commit_idx))
                        .collect();
                    seg.commits_by_segment.retain(|(_, ofs)| *ofs < commit_idx);
                    split
                };
                for (insert_at, (i, name)) in (seg_idx + 1..).zip(group.iter().enumerate()) {
                    let is_last = i + 1 == group.len();
                    let (commits, cbs) = if is_last {
                        (tail.clone(), tail_by_segment.clone())
                    } else {
                        (Vec::new(), Vec::new())
                    };
                    let adopt = is_last && commit_idx == 0 && minted.contains_key(&at);
                    let seg = if adopt {
                        // The run is the segment: adopt the canonical record — naming it when
                        // the walk left it anonymous — instead of minting a parallel record off
                        // the line. A canonical named by another group member yields its name to
                        // the re-split.
                        let canonical = minted[&at];
                        let md = branch_md(meta, name);
                        if out[canonical]
                            .ref_info
                            .as_ref()
                            .is_some_and(|ri| ri.ref_name != **name)
                        {
                            // The yielding name keeps a presence in the graph: an empty segment
                            // attached above the commit it pointed at.
                            let old = out[canonical].ref_info.take().expect("checked");
                            let old_md = out[canonical].metadata.take();
                            out.insert_segment_with_anchor(
                                MintSeg {
                                    ref_info: Some(old),
                                    metadata: old_md,
                                    ..Default::default()
                                },
                                Some(at),
                            );
                        }
                        if out[canonical].ref_info.is_none() {
                            apply_name_to_canonical(
                                out,
                                canonical,
                                ref_info_adopting_worktree(facts, name, None),
                            );
                            out[canonical].metadata =
                                md.clone().map(crate::SegmentMetadata::Branch);
                        }
                        StackSegment {
                            ref_info: out[canonical].ref_info.clone(),
                            id: canonical,
                            commits,
                            commits_by_segment: cbs,
                            metadata: md,
                            ..blank_stack_segment()
                        }
                    } else if is_last
                        && let Some((canonical, pos)) = minted
                            .get(&at)
                            .copied()
                            .or_else(|| minted_of(facts, minted, at))
                            .and_then(|c| {
                                out[c]
                                    .commits
                                    .iter()
                                    .position(|x| x.id == at)
                                    .map(|p| (c, p))
                            })
                            .filter(|(_, pos)| *pos > 0)
                    {
                        // The commits move out of the canonical into the named segment — a
                        // mid-segment split for a later group.
                        let md = branch_md(meta, name);
                        let tail_sidx = split_out_segment(
                            out,
                            canonical,
                            pos,
                            Some(ref_info_adopting_worktree(facts, name, Some(at))),
                        );
                        out[tail_sidx].metadata = md.clone().map(crate::SegmentMetadata::Branch);
                        if let Some(first) = out[tail_sidx].commits.first_mut() {
                            first.refs.retain(|r| r.ref_name != **name);
                        }
                        minted.insert(at, tail_sidx);
                        let cbs_len = commits.len();
                        StackSegment {
                            ref_info: out[tail_sidx].ref_info.clone(),
                            id: tail_sidx,
                            commits,
                            commits_by_segment: if cbs_len > 0 {
                                vec![(tail_sidx, 0)]
                            } else {
                                Vec::new()
                            },
                            metadata: md,
                            ..blank_stack_segment()
                        }
                    } else {
                        mk_segment(out, minted, name, commits, cbs, Some(at))
                    };
                    present.insert((*name).clone());
                    stack.segments.insert(insert_at, seg);
                }
                // Rewire only the split range [seg_idx ..= seg_idx + inserted]: each rests on
                // its immediate successor's first commit (none when that segment is empty, the
                // pairwise rule), and the last inherits the split segment's original base.
                let range_end = seg_idx + group.len();
                for i in seg_idx..=range_end {
                    let (base, base_id) = match stack.segments.get(i + 1) {
                        Some(next) if i < range_end => {
                            (next.commits.first().map(|c| c.id), Some(next.id))
                        }
                        _ => (split_base, split_base_id),
                    };
                    stack.segments[i].base = base;
                    stack.segments[i].base_segment_id = base_id;
                }
                // An emptied unnamed segment in front contributes nothing anymore; a named one
                // whose name is not part of the group is not collected either — its record
                // keeps the name off the line.
                if stack.segments[seg_idx].commits.is_empty()
                    && stack.segments[seg_idx]
                        .ref_name()
                        .is_none_or(|rn| !group.iter().any(|n| n.as_ref() == rn))
                {
                    stack.segments.remove(seg_idx);
                }
                continue;
            }
            // At a stack's base: append empties at the bottom of the stack that carries one of
            // this metadata stack's names — but never at another stack's tip commit, which its
            // own stack's claims govern.
            let at_is_foreign_tip = stacks.iter().any(|stack| {
                stack
                    .segments
                    .first()
                    .and_then(|s| s.commits.first())
                    .is_some_and(|c| c.id == at)
            });
            let basing = stacks
                .iter_mut()
                .filter(|_| !at_is_foreign_tip)
                .find(|stack| {
                    stack.segments.last().is_some_and(|s| s.base == Some(at))
                        && stack.segments.iter().any(|seg| {
                            seg.ref_name().is_some_and(|rn| {
                                ms.branches.iter().any(|b| b.ref_name.as_ref() == rn)
                            })
                        })
                });
            if let Some(stack) = basing {
                let base = stack.segments.last().and_then(|s| s.base);
                let base_id = stack.segments.last().and_then(|s| s.base_segment_id);
                if let Some(above) = stack.segments.last_mut() {
                    above.base = None;
                    above.base_segment_id = None;
                }
                let mut appended = Vec::new();
                for name in group {
                    facts.consumed_meta_refs.borrow_mut().insert(name.clone());
                    let seg = mk_segment(out, minted, name, Vec::new(), Vec::new(), Some(at));
                    present.insert(name.clone());
                    appended.push(seg);
                }
                wire_pairwise_bases(&mut appended, facts, minted, base);
                if let (Some(last), Some(base_id)) = (appended.last_mut(), base_id) {
                    last.base_segment_id = Some(base_id);
                }
                stack.segments.extend(appended);
                continue;
            }
            // Otherwise: an independent stack of empty segments — but only anchored at candidate
            // commits: existing stack bases or the lower bound. Another stack's tip commit is
            // never an independent base, even when a stack ends there — the per-commit claims of
            // its own stack govern that commit.
            let is_candidate = (Some(at) == lower_bound
                || stacks
                    .iter()
                    .any(|stack| stack.segments.last().is_some_and(|s| s.base == Some(at))))
                // With a managed workspace commit, stacks are projected only from its parents: an
                // independent metadata stack must anchor on a parent (or the commit itself).
                // Metadata never invents a lane the commit graph lacks — a branch mid-lane or on a
                // non-parent base belongs to the lane it's on. Several empty stacks are fine; the
                // octopus lists the base once per empty stack (git permits a repeated parent).
                // Without a managed octopus (a degenerate single-head workspace) there are no
                // parents to derive from, so metadata is the only source and the gate is lifted.
                && (!restrict_to_ws_parents
                    || ws_parents.contains(&at)
                    || ws_commit == Some(at))
                && !stacks.iter().any(|stack| {
                    stack
                        .segments
                        .first()
                        .and_then(|s| s.commits.first())
                        .is_some_and(|c| c.id == at)
                });
            if !is_candidate {
                continue;
            }
            let dead_end = {
                let node = facts.commits().node_data(at);
                !node.parent_ids.is_empty()
                    && facts.run(at).len() == 1
                    && facts.commits().first_parent_id(at).is_none()
            };
            let mut segments = Vec::new();
            for name in group {
                facts.consumed_meta_refs.borrow_mut().insert(name.clone());
                let seg = mk_segment(out, minted, name, Vec::new(), Vec::new(), Some(at));
                present.insert(name.clone());
                segments.push(seg);
            }
            wire_pairwise_bases(&mut segments, facts, minted, Some(at).filter(|_| !dead_end));
            if !segments.is_empty() {
                stacks.push(Stack {
                    id: Some(ms.id),
                    segments,
                });
            }
        }
    }
    // Applied names leave the displayed commit refs only now that application decided them.
    {
        let consumed = facts.consumed_meta_refs.borrow();
        if !consumed.is_empty() {
            for stack in stacks.iter_mut() {
                for segment in stack.segments.iter_mut() {
                    for commit in segment.commits.iter_mut() {
                        commit.refs.retain(|ri| !consumed.contains(&ri.ref_name));
                    }
                }
            }
            let all: Vec<usize> = out.segments().collect();
            for sidx in all {
                for commit in out[sidx].commits.iter_mut() {
                    commit.refs.retain(|ri| !consumed.contains(&ri.ref_name));
                }
            }
        }
    }
    Ok(consumed_anchor)
}

/// Sibling candidates over the whole walk: anonymous run heads with at least two in-graph
/// children whose upward path through out-of-workspace commits reaches a run named by a
/// metadata-known ref — with the restriction that Applied stack-tip names only project from a
/// direct parent that has out-of-workspace commits.
fn sibling_candidates(
    facts: &Facts<'_>,
    ws_md: &ref_metadata::Workspace,
) -> BTreeMap<gix::ObjectId, (crate::RefInfo, gix::ObjectId)> {
    use but_core::ref_metadata::StackKind::{Applied, AppliedAndUnapplied};
    let mut out = BTreeMap::new();
    for (&owner, &head) in facts.head_of().iter() {
        if facts.ref_info_of(owner).is_some() {
            continue;
        }
        let Some(head_nx) = facts.commits().node(head) else {
            continue;
        };
        if facts
            .commits()
            .inner
            .neighbors_directed(head_nx, petgraph::Direction::Incoming)
            .count()
            < 2
        {
            continue;
        }
        let mut named: Option<(crate::RefInfo, gix::ObjectId)> = None;
        let mut seen = HashSet::new();
        let mut queue: std::collections::VecDeque<gix::ObjectId> = facts
            .commits()
            .inner
            .neighbors_directed(head_nx, petgraph::Direction::Incoming)
            .map(|nx| facts.commits().inner[nx].id)
            .collect();
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) || named.is_some() {
                continue;
            }
            let node = facts.commits().node_data(id);
            if node.flags.contains(CommitFlags::InWorkspace) {
                continue;
            }
            if let Some((run_owner, run_head)) = facts.run_of(id)
                && let Some(ri) = facts.ref_info_of(run_owner)
                && ws_md.contains_ref(ri.ref_name.as_ref(), AppliedAndUnapplied)
            {
                named = Some((ri.clone(), run_head));
                continue;
            }
            if let Some(nx) = facts.commits().node(id) {
                queue.extend(
                    facts
                        .commits()
                        .inner
                        .neighbors_directed(nx, petgraph::Direction::Incoming)
                        .map(|nx| facts.commits().inner[nx].id),
                );
            }
        }
        let Some((ref_info, sibling_head)) = named else {
            continue;
        };
        let is_stack_tip = ws_md.stacks(Applied).any(|ms| {
            ms.branches
                .first()
                .is_some_and(|b| b.ref_name == ref_info.ref_name)
        });
        if is_stack_tip {
            let direct_parent = facts
                .commits()
                .inner
                .neighbors_directed(head_nx, petgraph::Direction::Incoming)
                .any(|nx| {
                    facts
                        .run_of(facts.commits().inner[nx].id)
                        .is_some_and(|(_, rh)| rh == sibling_head)
                });
            let has_outside = !facts
                .commits()
                .node_data(sibling_head)
                .flags
                .contains(CommitFlags::InWorkspace);
            if !(direct_parent && has_outside) {
                continue;
            }
        }
        out.insert(head, (ref_info, sibling_head));
    }
    out
}

/// Adopt sibling candidates onto their anonymous stack segments: name, branch metadata, the
/// sibling record, and the out-of-workspace commits as `commits_outside`.
#[allow(clippy::too_many_arguments)]
fn adopt_ahead_siblings<T: RefMetadata>(
    facts: &Facts<'_>,
    meta: &OverlayMetadata<'_, T>,
    sibling_of: &BTreeMap<gix::ObjectId, (crate::RefInfo, gix::ObjectId)>,
    head_by_segment: &BTreeMap<usize, gix::ObjectId>,
    stacks: &mut [Stack],
) -> anyhow::Result<()> {
    for stack in stacks.iter_mut() {
        for segment in stack.segments.iter_mut() {
            if segment.ref_info.is_some() {
                continue;
            }
            let Some((ref_info, sibling_head)) = head_by_segment
                .get(&segment.id)
                .and_then(|head| sibling_of.get(head))
            else {
                continue;
            };
            let md = meta
                .branch_opt(ref_info.ref_name.as_ref())?
                .map(|md| ref_metadata::Branch::clone(&md));
            segment.ref_info = Some(ref_info.clone());
            segment.metadata = md;
            let mut outside = Vec::new();
            let mut seen = HashSet::new();
            let mut queue = std::collections::VecDeque::new();
            queue.push_back(*sibling_head);
            seen.insert(*sibling_head);
            while let Some(id) = queue.pop_front() {
                let node = facts.commits().node_data(id);
                if node.flags.contains(CommitFlags::InWorkspace) {
                    continue;
                }
                let owner_name = facts
                    .run_of(id)
                    .and_then(|(owner, _)| facts.ref_info_of(owner).map(|ri| ri.ref_name.clone()));
                outside.push(StackCommit::from_graph_commit(&graph_commit(
                    facts,
                    id,
                    owner_name.as_ref().or(Some(&ref_info.ref_name)),
                )));
                for parent in &node.parent_ids {
                    if facts.has_commit(*parent) && seen.insert(*parent) {
                        queue.push_back(*parent);
                    }
                }
            }
            if !outside.is_empty() {
                segment.commits_outside = Some(outside);
            }
        }
    }
    Ok(())
}

/// Give the entrypoint its own named segment if its ref is buried in a commit's refs, then set
/// [`StackSegment::is_entrypoint`] on the segment named by the entrypoint ref — or the one whose
/// first commit is the entrypoint.
fn mark_entrypoint(
    out: &mut NodeStore,
    facts: &Facts<'_>,
    minted: &BTreeMap<gix::ObjectId, usize>,
    entrypoint_ref: Option<&gix::refs::FullName>,
    ep_commit: gix::ObjectId,
    stacks: &mut [Stack],
) {
    // The forced split: no segment carries the entrypoint name, but a commit does.
    if let Some(ep_ref) = entrypoint_ref
        && !stacks.iter().any(|stack| {
            stack
                .segments
                .iter()
                .any(|seg| seg.ref_name() == Some(ep_ref.as_ref()))
        })
    {
        'split: for stack in stacks.iter_mut() {
            for seg_idx in 0..stack.segments.len() {
                let Some(commit_idx) = stack.segments[seg_idx].commits.iter().position(|c| {
                    c.id == ep_commit
                        && facts
                            .commits()
                            .node_data(c.id)
                            .refs
                            .iter()
                            .any(|ri| ri.ref_name == *ep_ref)
                }) else {
                    continue;
                };
                if commit_idx == 0 && stack.segments[seg_idx].ref_info.is_none() {
                    // An anonymous segment starting at the entrypoint just takes the name —
                    // applied to the canonical run segment so the graph names it too.
                    let ri = ref_info_adopting_worktree(facts, ep_ref, Some(ep_commit));
                    let canonical = minted
                        .get(&ep_commit)
                        .copied()
                        .or_else(|| minted_of(facts, minted, ep_commit));
                    let rec = match canonical.filter(|&c| {
                        out[c].commits.first().map(|x| x.id) == Some(ep_commit)
                            && out[c].ref_info.is_none()
                    }) {
                        Some(canonical) => {
                            apply_name_to_canonical(out, canonical, ri.clone());
                            canonical
                        }
                        None => out.insert_segment(MintSeg {
                            ref_info: Some(ri.clone()),
                            ..Default::default()
                        }),
                    };
                    let seg = &mut stack.segments[seg_idx];
                    seg.ref_info = Some(ri);
                    seg.id = rec;
                    if let Some(first) = seg.commits.first_mut() {
                        first.refs.retain(|ri| ri.ref_name != *ep_ref);
                    }
                    break 'split;
                }
                if commit_idx == 0 {
                    // A named segment keeps its commits: the entrypoint becomes an empty
                    // segment above it, moving the ref off the first commit.
                    let ri = ref_info_adopting_worktree(facts, ep_ref, Some(ep_commit));
                    let rec = out.insert_segment(MintSeg {
                        ref_info: Some(ri.clone()),
                        ..Default::default()
                    });
                    let canonical = stack.segments[seg_idx].id;
                    if let Some(first) = stack.segments[seg_idx].commits.first_mut() {
                        first.refs.retain(|ri| ri.ref_name != *ep_ref);
                    }
                    if let Some(first) = out[canonical].commits.first_mut() {
                        first.refs.retain(|r| r.ref_name != *ep_ref);
                    }
                    stack.segments.insert(
                        seg_idx,
                        StackSegment {
                            ref_info: Some(ri),
                            id: rec,
                            commits: Vec::new(),
                            base: Some(ep_commit),
                            base_segment_id: Some(canonical),
                            commits_by_segment: Vec::new(),
                            metadata: None,
                            ..blank_stack_segment()
                        },
                    );
                    break 'split;
                }
                // Split: the entrypoint and everything below it in this segment move into a
                // new segment named after the entrypoint ref.
                let tail: Vec<StackCommit> = stack.segments[seg_idx]
                    .commits
                    .drain(commit_idx..)
                    .collect();
                let tail_by_segment: Vec<(usize, usize)> = {
                    let seg = &mut stack.segments[seg_idx];
                    let split: Vec<_> = seg
                        .commits_by_segment
                        .iter()
                        .filter(|(_, ofs)| *ofs >= commit_idx)
                        .map(|(sidx, ofs)| (*sidx, ofs - commit_idx))
                        .collect();
                    seg.commits_by_segment.retain(|(_, ofs)| *ofs < commit_idx);
                    split
                };
                let mut tail = tail;
                if let Some(first) = tail.first_mut() {
                    first.refs.retain(|ri| ri.ref_name != *ep_ref);
                }
                let ri = ref_info_adopting_worktree(facts, ep_ref, Some(ep_commit));
                let rec = out.insert_segment(MintSeg {
                    ref_info: Some(ri.clone()),
                    ..Default::default()
                });
                let base = stack.segments[seg_idx].base;
                let base_id = stack.segments[seg_idx].base_segment_id;
                stack.segments[seg_idx].base = Some(ep_commit);
                stack.segments[seg_idx].base_segment_id = Some(rec);
                stack.segments.insert(
                    seg_idx + 1,
                    StackSegment {
                        ref_info: Some(ri),
                        id: rec,
                        commits: tail,
                        base,
                        base_segment_id: base_id,
                        commits_by_segment: tail_by_segment,
                        metadata: None,
                        ..blank_stack_segment()
                    },
                );
                break 'split;
            }
        }
    }

    // The marker: by name first — every segment carrying the name, since shared history
    // duplicates a segment into multiple stacks and each copy is the entrypoint — else by
    // owning the entrypoint commit.
    let mut named_any = false;
    if let Some(ep_ref) = entrypoint_ref {
        for stack in stacks.iter_mut() {
            for seg in stack.segments.iter_mut() {
                if seg.ref_name() == Some(ep_ref.as_ref()) {
                    seg.is_entrypoint = true;
                    named_any = true;
                }
            }
        }
    }
    if !named_any
        && let Some(seg) = stacks.iter_mut().find_map(|stack| {
            stack
                .segments
                .iter_mut()
                .find(|seg| seg.commits.first().is_some_and(|c| c.id == ep_commit))
        })
    {
        seg.is_entrypoint = true;
    }
}

/// Distill the walk's topology into inert [`crate::branch_graph::Branch`] records: one per
/// run (named by its owner), plus every empty named record with its attachment, connected the
/// way the edge log connects them. These are the flat adjacency list the [`BranchGraph`] carries.
fn branch_records(
    canonical_name_by_head: &BTreeMap<gix::ObjectId, gix::refs::FullName>,
    facts: &Facts<'_>,
    materialized_empties: &[(gix::refs::FullName, Option<gix::ObjectId>)],
    ws_md: Option<&ref_metadata::Workspace>,
) -> Vec<crate::branch_graph::Branch> {
    use crate::branch_graph::Branch;
    let entrypoint_rec = facts.entrypoint();
    let ws_head = entrypoint_rec.and_then(|rec| facts.head_of().get(&rec).copied());
    let meta_stacks: Vec<Vec<gix::refs::FullName>> = ws_md
        .map(|ws_md| {
            ws_md
                .stacks(but_core::ref_metadata::StackKind::Applied)
                .map(|ms| ms.branches.iter().map(|b| b.ref_name.clone()).collect())
                .collect()
        })
        .unwrap_or_default();
    // The walk's name for a run = the facts-derived canonical name (forced/disambiguated local,
    // else the picked remote-tracking ref). A remote-only run takes its remote name so it owns its
    // commits as a named, findable segment.
    let walk_name = |head: gix::ObjectId| -> Option<gix::refs::FullName> {
        canonical_name_by_head.get(&head).cloned()
    };

    // Runs first, in record order; then empty named attached records.
    let mut list: Vec<Branch> = Vec::new();
    let mut index_of_run_head: BTreeMap<gix::ObjectId, usize> = BTreeMap::new();
    let mut target_of_run_head: BTreeMap<gix::ObjectId, usize> = BTreeMap::new();
    let mut index_of_record: BTreeMap<usize, usize> = BTreeMap::new();
    let mut splice_route: BTreeMap<gix::ObjectId, Vec<usize>> = BTreeMap::new();
    for (&owner, &head) in facts.head_of().iter() {
        // Metadata-stacked names at the run's head, in metadata order: all but the last become
        // empty records chained above the run, the last names the run itself.
        // Projection-resolved names (bound naming, lifts, adoption) live on the canonical
        // segment; prefer them so editors can select refs the records left anonymous or
        // remote-named.
        let at_head = |name: &gix::refs::FullName| {
            facts.commits().node(head).is_some_and(|nx| {
                facts.commits().inner[nx]
                    .refs
                    .iter()
                    .any(|ri| ri.ref_name == *name)
            })
        };
        let canonical_name = canonical_name_by_head.get(&head).cloned();
        let canonical_local_name = canonical_name
            .clone()
            .filter(|name| name.category() == Some(gix::refs::Category::LocalBranch));
        // A remote-named or anonymous run whose head carries exactly one workspace branch takes
        // that name: the commit goes to the metadata branch and the remote keeps its own empty
        // segment.
        let unique_ws_md_name_at_head = || {
            let mut names = meta_stacks.iter().flatten().filter(|name| at_head(name));
            names.next().filter(|_| names.next().is_none()).cloned()
        };
        let stacks_at_head = meta_stacks
            .iter()
            .filter(|names| names.iter().any(&at_head))
            .count();
        let mut owner_name = canonical_local_name
            .clone()
            .or_else(unique_ws_md_name_at_head)
            .or(walk_name(head));
        // Several stacks share this head and nothing canonical names it: the run stays anonymous
        // and every stack gets its own empty chain.
        if stacks_at_head >= 2
            && canonical_local_name.is_none()
            && owner_name
                .as_ref()
                .is_some_and(|name| meta_stacks.iter().flatten().any(|n| n == name))
        {
            owner_name = None;
        }
        // Only the run's own meta stack lifts into the chain above it.
        let mut meta_chain: Vec<gix::refs::FullName> = meta_stacks
            .iter()
            .find(|names| owner_name.as_ref().is_some_and(|o| names.contains(o)))
            .map(|names| {
                names
                    .iter()
                    .filter(|name| Some(*name) == owner_name.as_ref() || at_head(name))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let run_name = if meta_chain.len() > 1 {
            meta_chain.pop()
        } else {
            meta_chain.clear();
            None
        }
        .or(owner_name.clone())
        // The entrypoint record carries the entrypoint ref even when the walk left the run
        // anonymous, so editors can select `HEAD`.
        .or_else(|| {
            (entrypoint_rec == Some(owner))
                .then(|| facts.entrypoint_ref().cloned())
                .flatten()
        });
        let chain_above: Vec<gix::refs::FullName> = meta_chain
            .into_iter()
            .filter(|name| Some(name) != run_name.as_ref())
            .collect();
        // Meta stacks whose tips were deduplicated onto this head get an empty chain per stack,
        // spliced into the workspace-commit parent edge (ws -> tip -> run), fanning out when
        // several stacks share the head.
        let splice_chains: Vec<Vec<gix::refs::FullName>> = meta_stacks
            .iter()
            .filter(|names| !owner_name.as_ref().is_some_and(|o| names.contains(o)))
            .map(|names| {
                let mut names: Vec<_> = names
                    .iter()
                    .filter(|name| at_head(name) && Some(*name) != run_name.as_ref())
                    .cloned()
                    .collect();
                names.dedup();
                names
            })
            .filter(|names| !names.is_empty())
            .collect();
        let chain_top_in_list = list.len();
        let run_idx_in_list = list.len() + chain_above.len();
        for name in chain_above.iter() {
            list.push(Branch {
                ref_name: Some(name.clone()),
                commits: Vec::new(),
                outgoing: vec![(list.len() + 1, 0)],
                is_entrypoint: false,
            });
        }
        // Incoming connections route through the chain; the run's own connections leave from
        // the run element itself.
        target_of_run_head.insert(head, chain_top_in_list);
        index_of_run_head.insert(head, run_idx_in_list);
        index_of_record.insert(owner, run_idx_in_list);
        // Names the run name superseded (the records owner or a remote canonical name) stay
        // selectable as empty records above the run.
        // Remote-tracking refs at the head that the run name didn't take stay selectable as empty
        // records above it (mirroring split_remote_runs' virtual segments for additional refs).
        let head_remotes = facts
            .commits()
            .node_data(head)
            .ref_name_iter()
            .filter(|rn| rn.category() == Some(gix::refs::Category::RemoteBranch))
            .cloned()
            .collect::<Vec<_>>();
        let mut displaced_seen = HashSet::new();
        let displaced_names: Vec<gix::refs::FullName> = canonical_name
            .into_iter()
            .chain(walk_name(head))
            .chain(head_remotes)
            .filter(|name| {
                Some(name) != run_name.as_ref()
                    && !chain_above.contains(name)
                    && !splice_chains.iter().flatten().any(|n| n == name)
                    && displaced_seen.insert(name.clone())
            })
            .collect();
        let strip_names: Vec<gix::refs::FullName> = chain_above
            .iter()
            .chain(splice_chains.iter().flatten())
            .chain(displaced_names.iter())
            .cloned()
            .chain(run_name.clone())
            .collect();
        list.push(Branch {
            ref_name: run_name,
            commits: facts
                .run(head)
                .into_iter()
                .map(|id| {
                    // Unlike the displayed projection, the carrier keeps metadata-consumed
                    // refs on commits — except those lifted into the chain above, whose
                    // Reference steps the chain provides.
                    let node = facts.commits().node_data(id);
                    crate::Commit {
                        id,
                        parent_ids: node.parent_ids.clone(),
                        flags: node.flags,
                        refs: node
                            .refs
                            .iter()
                            .filter(|ri| {
                                (id != head || !strip_names.contains(&ri.ref_name))
                                    && !is_internal_ref(ri.ref_name.as_ref())
                                    && !facts
                                        .consumed_local_refs
                                        .contains(&(id, ri.ref_name.clone()))
                            })
                            .cloned()
                            .collect(),
                    }
                })
                .collect(),
            outgoing: Vec::new(),
            is_entrypoint: entrypoint_rec == Some(owner),
        });
        for names in &splice_chains {
            let top = list.len();
            for (i, name) in names.iter().enumerate() {
                let next = if i + 1 == names.len() {
                    chain_top_in_list
                } else {
                    list.len() + 1
                };
                list.push(Branch {
                    ref_name: Some(name.clone()),
                    commits: Vec::new(),
                    outgoing: vec![(next, 0)],
                    is_entrypoint: false,
                });
            }
            splice_route.entry(head).or_default().push(top);
        }
        for name in displaced_names {
            if list.iter().any(|s| s.ref_name.as_ref() == Some(&name)) {
                continue;
            }
            list.push(Branch {
                ref_name: Some(name),
                commits: Vec::new(),
                outgoing: vec![(chain_top_in_list, 0)],
                is_entrypoint: false,
            });
        }
    }
    for (rec, ref_name) in facts
        .named_segments()
        .map(|(s, ri)| (s, ri.ref_name.clone()))
        .collect::<Vec<_>>()
    {
        if index_of_record.contains_key(&rec) {
            continue;
        }
        // A record whose name a run already carries would only duplicate its reference step.
        if list.iter().any(|s| s.ref_name.as_ref() == Some(&ref_name)) {
            continue;
        }
        let Some(to) = facts.attach_target(rec) else {
            continue;
        };
        let Some((_, run_head)) = facts.run_of(to) else {
            continue;
        };
        let Some(&target) = target_of_run_head.get(&run_head) else {
            continue;
        };
        index_of_record.insert(rec, list.len());
        list.push(Branch {
            ref_name: Some(ref_name),
            commits: Vec::new(),
            outgoing: vec![(target, 0)],
            is_entrypoint: entrypoint_rec == Some(rec),
        });
    }

    // Named empty segments the projection materialized without a record counterpart (e.g. a
    // deduplicated target tip) still need to be selectable. The caller supplies them as
    // (name, the commit they route down to), so this doesn't navigate the record graph.
    {
        let present: HashSet<gix::refs::FullName> = list
            .iter()
            .filter_map(|s| s.ref_name.clone())
            .chain(
                list.iter()
                    .flat_map(|s| s.commits.iter())
                    .flat_map(|c| c.refs.iter().map(|ri| ri.ref_name.clone())),
            )
            .collect();
        for &(ref name, target_id) in materialized_empties {
            if present.contains(name) {
                continue;
            }
            let outgoing = target_id
                .and_then(|id| facts.run_of(id))
                .and_then(|(_, run_head)| target_of_run_head.get(&run_head).copied())
                .map(|target| vec![(target, 0)])
                .unwrap_or_default();
            list.push(Branch {
                ref_name: Some(name.clone()),
                commits: Vec::new(),
                outgoing,
                is_entrypoint: false,
            });
        }
    }

    // Connections: every parent edge that crosses runs, in commit-graph (walk) order; first-parent
    // continuations within a run are implicit in its commit list.
    for (child, parent, parent_order) in facts.commits().parent_edges() {
        let Some(&target) = target_of_run_head.get(&parent) else {
            continue;
        };
        let Some((_, child_run_head)) = facts.run_of(child) else {
            continue;
        };
        let Some(&source) = index_of_run_head.get(&child_run_head) else {
            continue;
        };
        // The workspace commit's edge routes through the spliced stack-tip chains when they
        // exist, fanning out one per stack.
        if Some(child) == ws_head
            && let Some(tops) = splice_route.get(&parent)
            && !tops.is_empty()
        {
            // Reversed to match the edge replay order, so stacks keep their metadata order.
            for &top in tops.iter().rev() {
                list[source].outgoing.push((top, parent_order));
            }
            continue;
        }
        // A first-parent edge from a run's last commit to the next head is the run continuing.
        list[source].outgoing.push((target, parent_order));
    }
    list
}

/// Whether traversal tips with target context exist beyond the target ref's own commit — the
/// signal that upstream advanced past the stored target.
fn upstream_advanced_past_target(
    facts: &Facts<'_>,
    target: Option<&(usize, gix::refs::FullName, gix::ObjectId)>,
) -> bool {
    facts
        .traversal_tips()
        .iter()
        .filter(|tip| tip.role.is_integrated())
        .filter(|tip| facts.has_commit(tip.id))
        .any(|tip| Some(tip.id) != target.map(|(_, _, c)| *c))
}

/// Prune integrated stack segments and recompute the base: walk stack segments bottom-up, cut at
/// the first block that is integrated trunk, keep a fully-integrated stack alive while upstream
/// is ahead, and rest the new bottom on its first-parent neighbour.
fn prune_integrated_stack(
    facts: &Facts<'_>,
    stack: &mut Stack,
    prune_set: &HashSet<gix::ObjectId>,
    keep_if_fully_integrated: bool,
) {
    use crate::workspace::StackCommitFlags;
    let integrated = |commits: &[StackCommit]| {
        commits
            .iter()
            .all(|c| c.flags.contains(StackCommitFlags::Integrated))
    };
    let mut cut: Option<(usize, usize)> = None;
    let mut has_surviving_commit = false;
    'outer: for seg_idx in (0..stack.segments.len()).rev() {
        let seg = &stack.segments[seg_idx];
        if seg.commits.is_empty() {
            continue;
        }
        // Blocks are the per-run chunks recorded in commits_by_segment, bottom-up.
        let blocks: Vec<(usize, usize)> = {
            let mut offsets: Vec<usize> =
                seg.commits_by_segment.iter().map(|(_, ofs)| *ofs).collect();
            if offsets.is_empty() {
                offsets.push(0);
            }
            offsets
                .iter()
                .enumerate()
                .map(|(i, &start)| {
                    let end = offsets.get(i + 1).copied().unwrap_or(seg.commits.len());
                    (start, end)
                })
                .collect()
        };
        for &(start, end) in blocks.iter().rev() {
            let commits = &seg.commits[start..end.min(seg.commits.len())];
            if commits.is_empty() {
                continue;
            }
            if integrated(commits) && commits.iter().all(|c| prune_set.contains(&c.id)) {
                cut = Some((seg_idx, start));
            } else {
                has_surviving_commit = true;
                break 'outer;
            }
        }
    }
    let Some((cut_seg_idx, cut_offset)) = cut else {
        return;
    };
    if keep_if_fully_integrated && !has_surviving_commit {
        return;
    }
    stack.segments[cut_seg_idx].commits.truncate(cut_offset);
    stack.segments[cut_seg_idx]
        .commits_by_segment
        .retain(|(_, offset)| *offset < cut_offset);
    let keep = if stack.segments[cut_seg_idx].commits.is_empty() && cut_seg_idx > 0 {
        cut_seg_idx
    } else {
        cut_seg_idx + 1
    };
    stack.segments.truncate(keep);
    if keep_if_fully_integrated {
        // Upstream is ahead: the stack's bottom moved, rest it on its first-parent neighbour.
        if let Some(last) = stack.segments.last_mut() {
            let below = last
                .commits
                .last()
                .and_then(|c| facts.commits().first_parent_id(c.id));
            last.base = below;
            last.base_segment_id = None;
        }
    }
}

/// Wire a freshly-created empty chain pairwise: each segment rests on its successor (no commit,
/// since they are empty), and the last on the canonical segment owning `anchor` — setting
/// `base_segment_id` even where the base commit is `None`.
fn wire_pairwise_bases(
    segments: &mut [StackSegment],
    facts: &Facts<'_>,
    minted: &BTreeMap<gix::ObjectId, usize>,
    anchor: Option<gix::ObjectId>,
) {
    let next_ids: Vec<Option<usize>> = segments
        .iter()
        .skip(1)
        .map(|s| Some(s.id))
        .chain(std::iter::once(None))
        .collect();
    for (seg, next_id) in segments.iter_mut().zip(next_ids) {
        match next_id {
            Some(next) => {
                seg.base = None;
                seg.base_segment_id = Some(next);
            }
            None => {
                seg.base = anchor;
                seg.base_segment_id = anchor.and_then(|at| minted_of(facts, minted, at));
            }
        }
    }
}

/// The local branch name a remote tracking ref maps back to, via the `remotes/<name>/`
/// convention over the repository's configured remotes.
fn deduce_local_of_remote(
    repo: &crate::init::overlay::OverlayRepo<'_>,
    remote: &gix::refs::FullNameRef,
) -> Option<gix::refs::FullName> {
    use bstr::ByteSlice as _;
    let (category, shorthand) = remote.category_and_short_name()?;
    if category != gix::refs::Category::RemoteBranch {
        return None;
    }
    for remote_name in repo.for_find_only().remote_names() {
        let Some(rest) = shorthand
            .as_bstr()
            .strip_prefix(remote_name.as_bstr().as_bytes())
            .and_then(|rest| rest.strip_prefix(b"/"))
        else {
            continue;
        };
        let name = format!("refs/heads/{}", rest.as_bstr());
        if let Ok(full) = gix::refs::FullName::try_from(name) {
            return Some(full);
        }
    }
    None
}

/// Match a stack's segments against workspace metadata stacks by branch-name overlap, preferring
/// applied stacks and first-branch matches, and avoiding `seen` stack ids.
fn find_matching_stack_id(
    metadata: Option<&ref_metadata::Workspace>,
    segments: &[StackSegment],
    seen: &mut std::collections::BTreeSet<but_core::ref_metadata::StackId>,
) -> Option<(but_core::ref_metadata::StackId, bool)> {
    use but_core::ref_metadata::StackKind::AppliedAndUnapplied;
    use itertools::Itertools as _;
    let metadata = metadata?;

    fn ref_names_with_weight(
        s: &StackSegment,
    ) -> impl Iterator<Item = (u64, &gix::refs::FullNameRef)> {
        s.ref_info
            .as_ref()
            .map(|ri| (100_000, ri.ref_name.as_ref()))
            .into_iter()
            .chain(
                s.commits
                    .iter()
                    .flat_map(|c| c.refs.iter().map(|ri| (1, ri.ref_name.as_ref()))),
            )
    }

    segments
        .iter()
        .flat_map(|s| {
            ref_names_with_weight(s).filter_map(|(weight, rn)| {
                metadata.stacks(AppliedAndUnapplied).find_map(|meta_stack| {
                    if let Some(bidx) = meta_stack
                        .branches
                        .iter()
                        .enumerate()
                        .find_map(|(bidx, b)| (rn == b.ref_name.as_ref()).then_some(bidx))
                    {
                        let priority = if bidx == 0 { 3 } else { 1 };
                        Some((
                            if meta_stack.is_in_workspace() {
                                weight * 2
                            } else {
                                weight
                            } * priority,
                            meta_stack.id,
                            meta_stack.is_in_workspace(),
                        ))
                    } else {
                        None
                    }
                })
            })
        })
        .sorted_by(|l, r| l.0.cmp(&r.0).reverse())
        .map(|(_weight, stack_id, in_workspace)| (stack_id, in_workspace))
        .find(|(stack_id, _)| seen.insert(*stack_id))
}
