//! The workspace projection: build a [`Workspace`] directly from the commit-first walk output and
//! metadata, with no segment surgery.
//!
//! The commit topology lives in the carried [`CommitGraph`]; the display stacks and the
//! full-topology [`BranchGraph`](crate::BranchGraph) the rebase reads are both projected from it
//! plus the per-segment facts the walk recorded.
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

use std::collections::{BTreeMap, BTreeSet, HashSet};

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
    ///
    /// `db` backs the [discovery of active linked worktrees](init::Options::worktrees), whose
    /// `HEAD`s are seeded as [extra traversal tips](Self::worktree_tips) - discovery may run the
    /// one-time worktree adoption and thus write to the database.
    pub fn from_commit_traversal(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        db: &mut but_db::DbHandle,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        Self::from_commit_traversal_with_extra_tips(
            tip,
            ref_name,
            None::<init::Tip>,
            meta,
            project_meta,
            db,
            options,
        )
    }

    /// Like [`Self::from_commit_traversal()`], but additionally traverse `extra_tips`.
    ///
    /// This is useful for callers that want the normal workspace-metadata-derived
    /// traversal while assuring additional branch tips are part of the graph,
    /// even if they are not reachable from the workspace or entrypoint.
    ///
    /// Extra tips that duplicate a metadata-derived traversal seed, or whose
    /// ref name is already claimed by `ref_name` or a metadata-derived tip —
    /// including the names embedded in [`TipRole::WorkspaceStackBranch`](init::TipRole) and
    /// [`TipRole::TargetLocal`](init::TipRole) — are skipped: callers cannot know which tips
    /// metadata resolves to, so such overlap means the branch is already covered
    /// by the normal traversal.
    /// Extra tips must not be entrypoints; the entrypoint is always `tip`.
    ///
    /// With no `extra_tips`, this is equivalent to [`Self::from_commit_traversal()`].
    #[allow(clippy::too_many_arguments)]
    pub fn from_commit_traversal_with_extra_tips(
        tip: gix::Id<'_>,
        ref_name: impl Into<Option<gix::refs::FullName>>,
        extra_tips: impl IntoIterator<Item = init::Tip>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        db: &mut but_db::DbHandle,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let repo = tip.repo;
        let worktree_tips = init::discover_worktree_tips(repo, db, options.worktrees)?;
        let tip = tip.detach();
        Self::from_resolved_tip(
            repo,
            tip,
            ref_name.into(),
            extra_tips,
            meta,
            project_meta,
            worktree_tips,
            options,
            false,
        )
    }

    /// Resolve the workspace from an already-resolved `tip`, with `ref_name` assumed to point at it:
    /// compute the initial tips from workspace metadata and traverse directly. The shared tail of
    /// [`Self::from_commit_traversal`] and [`Self::from_head`].
    #[allow(clippy::too_many_arguments)]
    fn from_resolved_tip(
        repo: &gix::Repository,
        tip: gix::ObjectId,
        ref_name: Option<gix::refs::FullName>,
        extra_tips: impl IntoIterator<Item = init::Tip>,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        worktree_tips: Vec<init::WorktreeTip>,
        options: init::Options,
        detached_head: bool,
    ) -> anyhow::Result<Self> {
        let (overlay_repo, overlay_meta, _entrypoint) =
            init::Overlay::default().into_parts(repo, meta);
        let mut tips = init::initial_tips_from_workspace_metadata(
            &overlay_repo,
            &overlay_meta,
            tip,
            ref_name.as_ref(),
            &project_meta,
            options.extra_target_commit_id,
        )?;
        init::merge_extra_tips(&mut tips, extra_tips, ref_name.as_ref())?;
        Self::traverse_tips_with_overlay_impl(
            &overlay_repo,
            tips,
            &overlay_meta,
            project_meta,
            options,
            worktree_tips,
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
        db: &mut but_db::DbHandle,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let tips: Vec<_> = tips.into_iter().collect();
        let worktree_tips = init::discover_worktree_tips(repo, db, options.worktrees)?;
        let (overlay_repo, overlay_meta, _entrypoint) =
            init::Overlay::default().into_parts(repo, meta);
        Self::traverse_tips_with_overlay_impl(
            &overlay_repo,
            tips,
            &overlay_meta,
            project_meta,
            options,
            worktree_tips,
            None,
            false,
        )
    }

    /// Build the workspace of `repo`'s `HEAD` from the commit-first traversal.
    pub fn from_head(
        repo: &gix::Repository,
        meta: &impl RefMetadata,
        project_meta: ProjectMeta,
        db: &mut but_db::DbHandle,
        options: init::Options,
    ) -> anyhow::Result<Self> {
        let worktree_tips = init::discover_worktree_tips(repo, db, options.worktrees)?;
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
                    worktree_tips: Vec::new(),
                    branches: Some(vec![crate::branch_graph::Branch {
                        ref_name: Some(ref_info.ref_name.clone()),
                        commits: Vec::new(),
                        outgoing: Vec::new(),
                        is_entrypoint: true,
                        worktree: ref_info.worktree.clone(),
                        metadata: None,
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
            None::<init::Tip>,
            meta,
            project_meta,
            worktree_tips,
            options,
            is_detached,
        )
    }
}

/// The walk's per-segment facts (naming, metadata, ownership) plus provenance, exposed as
/// commit-level queries for the projection.
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
    /// and that metadata. First in segment-id order.
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

    /// The commit of the segment named exactly `name`, if one exists and resolves.
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

/// Build-private scratch segment for the display projection: a run's name, metadata, and commits,
/// assembled into the [`Workspace`]'s stacks. Its id is the [`NodeStore`] key, not stored here.
#[derive(Default)]
struct MintSeg {
    ref_info: Option<crate::RefInfo>,
    metadata: Option<crate::SegmentMetadata>,
    commits: Vec<crate::Commit>,
}

/// Build-private node storage for the display projection: segments by id, no edges (the resolution,
/// ref tables, and branch records are all derived from facts and the `BranchGraph`). Ids come from
/// a plain counter.
#[derive(Default)]
struct NodeStore {
    nodes: BTreeMap<usize, MintSeg>,
    next: usize,
}

impl NodeStore {
    /// Store `seg` under a fresh id and return it (mirrors `Graph::insert_segment`).
    fn insert_segment(&mut self, seg: MintSeg) -> usize {
        let id = self.next;
        self.next += 1;
        self.nodes.insert(id, seg);
        id
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
/// `workspace_id` segment), along with the commits sitting on top of it. The projection resolves it once here and the
/// result travels on the workspace as commit-addressed data.
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
    // included (the old segment walk pruned some by generation, inconsistently).
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
        Some((rec, ref_info, metadata)) if ep_in_workspace => {
            let is_managed_msg = |id: gix::ObjectId| -> bool {
                ctx.repo
                    .find_commit(id)
                    .ok()
                    .and_then(|c| c.message_raw().ok().map(|m| m.to_owned()))
                    .is_some_and(|message| {
                        crate::workspace::commit::is_managed_workspace_by_message(message.as_ref())
                    })
            };
            // A workspace ref with metadata is a managed workspace whether or not a managed
            // commit exists yet: a freshly initialized workspace, or one holding only empty
            // branches, has no workspace commit and is anchored by the commit its ref points at.
            match facts.head_of().get(rec) {
                Some(&ws_commit) => (
                    Frame::ManagedOwning {
                        ws_commit,
                        // A workspace ref on someone else's tip is not a workspace commit: the run
                        // belongs to the stack, the workspace merely attaches above it.
                        commit_is_managed: is_managed_msg(ws_commit),
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
            }
        }
        _ => (Frame::AdHoc, None),
    }
}

/// Resolve the target: the segment named after the configured target ref, resolved to its commit;
/// or, with no configured target and no workspace-metadata tip, a named integrated traversal tip.
/// The configured target: the segment named after the configured target ref, resolved to its commit
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
            let sidx = out.insert_segment(MintSeg {
                ref_info: Some(ref_info.clone()),
                metadata: facts.metadata_of(*rec).cloned(),
                ..Default::default()
            });
            Some(sidx)
        }
        (Frame::ManagedMissing { anchor: _ }, Some((rec, ref_info, _))) => {
            // The attached record pass usually minted the workspace segment already.
            let existing = out.segments().find(|s| {
                out[*s]
                    .ref_info
                    .as_ref()
                    .is_some_and(|ri| ri.ref_name == ref_info.ref_name)
            });
            let sidx = existing.unwrap_or_else(|| {
                out.insert_segment(MintSeg {
                    ref_info: Some(ref_info.clone()),
                    metadata: facts.metadata_of(*rec).cloned(),
                    ..Default::default()
                })
            });
            Some(sidx)
        }
        _ => None,
    })
}

/// Mint the node storage from the walk: one segment per run (named from its record, commits from
/// facts), plus the empty named segments. Names anonymous runs and splits the managed workspace
/// commit onto its own segment. No edges; targets are resolved from facts.
fn mint_segments(
    out: &mut NodeStore,
    minted: &mut BTreeMap<gix::ObjectId, usize>,
    facts: &Facts<'_>,
) {
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
        let sidx = out.insert_segment(MintSeg {
            ref_info: Some(ref_info),
            metadata: facts.metadata_of(rec).cloned(),
            ..Default::default()
        });
        idx_of_record.insert(rec, sidx);
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
        // The ad-hoc entry IS what HEAD points at, so prefer the entrypoint ref over the walk's
        // disambiguated name: the walk may have named the entry after a sibling sharing its commit
        // (e.g. a fresh `new-branch` checked out at the target commit, where the walk picked the
        // target's local `main`). Falls back to the walk's name when HEAD is detached (no ref).
        if segments.is_empty() && tip_idx == 0 && adhoc_name.is_some() {
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

    // The target: the segment named after the configured target ref, resolved to its commit.
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
    let has_target_ref = target.is_some() || auto_target.is_some();

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

    // Ad-hoc/single-branch mode: the persisted GitButler-created branch order of the checked-out
    // branch's chain, tip to base. Same-tip runs of it become empty segments above the last member,
    // which owns the commit. Refs that don't exist locally are skipped without leaving phantoms.
    let (adhoc_branch_order, adhoc_order_starts_at_checkout): (Vec<gix::refs::FullName>, bool) =
        if matches!(frame, Frame::AdHoc) {
            adhoc_branch_order(&facts, ctx, meta)?
        } else {
            (Vec::new(), false)
        };

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
    //
    // `out` is the shared intermediate both workspace views project from: the stack view
    // (`StackSegment`) is assembled as segments are minted into it below, and the full-topology
    // view (the `BranchGraph`'s `Branch` records) is derived from it afterwards by `branch_records`.
    // Segments only, no edges (the resolution, ref tables, and branch records are derived from
    // facts and the `BranchGraph`).
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
    let mut branches = branch_records(
        &canonical_name_by_head,
        &facts,
        &materialized_empties,
        ws_info.as_ref().map(|(_, ri, _)| ri.ref_name.as_ref()),
        ws_info.as_ref().map(|(_, _, md)| md),
        match &frame {
            Frame::ManagedOwning { ws_commit, .. } => Some(*ws_commit),
            Frame::ManagedMissing { .. } | Frame::AdHoc => None,
        },
        &target
            .as_ref()
            .map(|(_, _, c)| *c)
            .into_iter()
            .chain(auto_target.as_ref().map(|(_, c)| *c))
            .collect::<Vec<_>>(),
        lower_bound,
        &adhoc_branch_order,
        &ctx.worktree_by_branch,
    );

    let mut out = NodeStore::default();
    let mut minted = BTreeMap::new();
    mint_segments(&mut out, &mut minted, &facts);

    // In managed frames the workspace segment leads; ad-hoc reuses the first stack segment.
    let pre_ws_out = lead_workspace_segment(&frame, &ws_info, &facts, &minted, &mut out)?;

    let mut walked_stack_count = usize::MAX;
    let mut lower_bound_segment_id = None;
    let mut head_by_segment: BTreeMap<usize, gix::ObjectId> = BTreeMap::new();
    let mut stacks = Vec::new();
    // A separate entrypoint's run keeps its own segment: stack collection splits at the entry tip.
    let ep_run_head = facts.run_of(ep_commit).map(|(_, head)| head);
    for (tip_idx, tip) in stack_tips.iter().copied().enumerate() {
        // A persisted single-branch order with only a stored target commit shows its members in
        // full: the stored commit flags integration but bounds nothing.
        let (collect_bound, collect_bound_segment_id) =
            if matches!(frame, Frame::AdHoc) && !has_target_ref && !adhoc_branch_order.is_empty() {
                (None, None)
            } else {
                (lower_bound, lower_bound_segment_id)
            };
        if let Some(stack) = collect_one_stack(
            tip,
            tip_idx,
            &frame,
            collect_bound,
            collect_bound_segment_id,
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
        && let Some(first) = seg
            .commits
            .first_mut()
            .filter(|c| c.id == ep_commit && !c.refs.iter().any(|ri| ri.ref_name == name.ref_name))
    {
        first.refs.push(name);
    }

    // An ad-hoc entry whose tip is the bound owns no commits and rests on the commit it points to,
    // not on whatever lies below the bound.
    if matches!(frame, Frame::AdHoc)
        && let Some(lb) = lower_bound
        && let Some(stack) = stacks.first_mut()
        && let [seg] = stack.segments.as_mut_slice()
        && seg.commits.is_empty()
        && seg.tip_commit_id.is_none()
    {
        seg.base = Some(lb);
        seg.base_segment_id = Some(seg.id);
    }

    // The persisted ad-hoc branch order: same-tip members become empty segments above the last
    // one, which keeps the commit; the checked-out empty branch is the workspace's own ref.
    if !adhoc_branch_order.is_empty() {
        apply_adhoc_branch_order(
            &mut out,
            &facts,
            ctx,
            meta,
            &adhoc_branch_order,
            adhoc_order_starts_at_checkout,
            lower_bound,
            &mut stacks,
        );
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
            target.as_ref().map(|(_, _, c)| *c),
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
                .or_else(|| {
                    // Metadata may omit a physical top branch. Then a named lower branch's
                    // stack keys the position, without reordering anonymous stacks.
                    stack.segments.first()?.ref_name()?;
                    let id = stack.id?;
                    ws_md.stacks.iter().position(|ms| {
                        ms.id == id
                            && ms.branches.first().is_some_and(|b| {
                                stack
                                    .segments
                                    .iter()
                                    .skip(1)
                                    .any(|seg| seg.ref_name() == Some(b.ref_name.as_ref()))
                            })
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
        // An ad-hoc view with a stored target commit but no target ref only flags integration:
        // nothing is pruned and the stack rests on nothing.
        let adhoc_without_target_ref =
            matches!(frame, Frame::AdHoc) && !has_target_ref && !adhoc_branch_order.is_empty();
        if adhoc_without_target_ref {
            for stack in stacks.iter_mut() {
                if let Some(last) = stack.segments.last_mut() {
                    last.base = None;
                    last.base_segment_id = None;
                }
            }
        }
        if !(adhoc_without_target_ref
            || (effective_target_commit_id.is_none() && upstream_advanced))
        {
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

    // Path A feasibility: does the BranchGraph faithfully mirror facts' runs (commits + first-parent
    // edge)? If so, collect can read the BranchGraph as a clean source-swap. Env-gated diagnostic.
    #[cfg(debug_assertions)]
    // WIP two-builder unification: compare the shadow stacks-derived-from-BranchGraph to the live
    // `out`-built stacks. Env-gated; drives the shadow to parity before it replaces the pipeline.
    #[cfg(debug_assertions)]
    // WIP flip trial: replace the `out`-built stacks with the BranchGraph-derived shadow stacks,
    // assigning synthetic sequential segment ids (consumers key on id only for uniqueness, not the
    // old graph NodeIndex). The remaining post-passes (enrich/tip/base_ref/generation) fill the
    // rest. Env-gated so the full test suite can run against the shadow as the production projection.
    #[cfg(debug_assertions)]
    // Remote enrichment: pair each named local stack segment with its remote tracking branch,
    // collect remote-only commits, and flag local commits reachable from a remote — all over the
    // commit store.
    if !facts
        .options()
        .dangerously_skip_postprocessing_for_debugging
    {
        enrich_with_remotes(&facts, ctx, &mut stacks)?;
    }

    // Branches checked out in linked worktrees leave the lanes: being checked out somewhere is
    // transient state that decides where a branch is drawn and which checkout follows a rewrite,
    // never what the lanes contain. Each such branch becomes an empty fork onto its commit in the
    // branch records and disappears from the stack rows. Runs after the remote pairing, which
    // would otherwise re-name the vacated owner from the refs left on its commit.
    if !facts
        .options()
        .dangerously_skip_postprocessing_for_debugging
    {
        let worktree_refs: Vec<gix::refs::FullName> = {
            let mut seen = HashSet::new();
            facts
                .state
                .worktree_tips
                .iter()
                .filter_map(|tip| tip.ref_name.clone())
                .filter(|rn| {
                    !ctx.worktree_by_branch
                        .get(rn)
                        .is_some_and(|wts| wts.iter().any(|wt| wt.owned_by_repo))
                })
                // The entrypoint is the subject of this view even when checked out elsewhere.
                .filter(|rn| Some(rn) != facts.entrypoint_ref())
                .filter(|rn| seen.insert(rn.clone()))
                .collect()
        };
        if !worktree_refs.is_empty() {
            let ws_ref = ws_info.as_ref().map(|(_, ri, _)| ri.ref_name.clone());
            fork_out_worktree_refs(
                &mut branches,
                &mut stacks,
                &worktree_refs,
                ws_ref.as_ref(),
                &ctx.worktree_by_branch,
            );
        }
    }

    // The lower-bound segment: the canonical run segment owning the bound commit. A workspace
    // metadata name on the bound passes to its materialized segment — ownership of the bound
    // commit moves to an unnamed segment — and an anonymous bound then takes a name by the same
    // single-ref/remote-scoped rules that name stack runs, since consumers select bases by
    // segment name.
    if let Some(lb) = lower_bound {
        lower_bound_segment_id = minted_of(&facts, &minted, lb);
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
            // segment-by-name lookup.
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
            // point), minus any already in the workspace. This replaces the old
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

    // A single ad-hoc stack whose head is the target's bound is fully integrated inline: show it
    // empty. Only a target *ref* prunes; a stored target commit alone merely flags integration.
    if stacks.len() == 1
        && has_target_ref
        && let Some(first) = stacks[0].segments.first()
        && lower_bound.is_some()
        && first.commits.first().map(|c| c.id) == lower_bound
    {
        stacks[0].segments.drain(1..);
        let first = stacks[0].segments.first_mut().expect("non-empty");
        let tip = first.commits.first().map(|c| c.id);
        first.commits.clear();
        first.commits_by_segment.clear();
        // An emptied branch rests on the commit it points to.
        first.base = tip;
        first.base_segment_id = Some(first.id);
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

    // Resolve commit-addressed values that consumers would otherwise navigate a segment graph
    // for: each stack segment's own tip (skip-empty, ref-info fallback), its remote tracking tip,
    // and its generation. build owns the graph, so it resolves these once; the projected output
    // then carries them directly. The skip-empty tip is derived from the BranchGraph the workspace
    // carries (navigate `outgoing` past empty branches; ambiguous = ≠1 outgoing → None), matching
    // `Graph::tip_skip_empty`.
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
                        .and_then(&skip_empty_tip)
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
                .and_then(&skip_empty_tip);
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
    // The workspace tip, navigating the BranchGraph: the branch leading
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
            .and_then(&skip_empty_tip)
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
    // segment tip (what segment_by_ref_name + tip_skip_empty did). Derived
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
        let represented = branches
            .iter()
            .filter_map(|b| b.worktree.as_ref())
            .chain(
                branches
                    .iter()
                    .flat_map(|b| b.commits.iter())
                    .flat_map(|c| c.refs.iter())
                    .filter_map(|ri| ri.worktree.as_ref()),
            )
            .chain(
                workspace_ref_info
                    .as_ref()
                    .and_then(|ri| ri.worktree.as_ref()),
            );
        for wt in represented {
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
    // The rebuild context: enough to re-run the traversal and to serve commit-level queries.
    let rebuild_commit_graph = Some(commit_graph);
    let rebuild_project_meta = state.project_meta.clone();
    let rebuild_options = state.options.clone();
    let rebuild_symbolic_remote_names = state.symbolic_remote_names.clone();
    let rebuild_worktree_tips = state.worktree_tips.clone();

    Ok(Workspace {
        commit_graph: rebuild_commit_graph,
        project_meta: rebuild_project_meta,
        options: rebuild_options,
        entrypoint_ref: rebuild_entrypoint_ref,
        symbolic_remote_names: rebuild_symbolic_remote_names,
        worktree_tips: rebuild_worktree_tips,
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
            // The remote tip: the segment named after the remote ref, or a commit carrying it.
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
    target_ref_commit: Option<gix::ObjectId>,
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
                      commits_by_segment_src: Vec<(usize, usize)>|
     -> StackSegment {
        let md = branch_md(meta, name);
        // The empty record sits above its anchor commit; the anchor lets `materialized_empties`
        // resolve its target without a graph edge.
        let rec = out.insert_segment(MintSeg {
            ref_info: Some(ref_info_adopting_worktree(facts, name, None)),
            metadata: md.clone().map(crate::SegmentMetadata::Branch),
            ..Default::default()
        });
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
                    // At the bound the commit itself is never shown. A member resting on the
                    // target's own first-parent line has no history of its own and materializes
                    // to stay visible; one integrated through a merge keeps its (pruned) history
                    // and stays consumed.
                    let at_bound = Some(at) == lower_bound
                        && !adhoc
                        && facts.entrypoint_ref() != Some(*name)
                        && target_ref_commit
                            .is_some_and(|t| facts.commits().first_parent_reaches(t, at));
                    let inside_a_stack = stacks.iter().any(|stack| {
                        stack
                            .segments
                            .iter()
                            .any(|seg| seg.commits.iter().any(|c| c.id == at))
                    });
                    // At the bound the commit itself is never shown, so the name must
                    // materialize to stay visible.
                    if owner_already_named
                        && !is_stack_tip_name
                        && !other_refs_remain
                        && !inside_a_stack
                        && !at_bound
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
                    let seg = mk_segment(out, minted, name, Vec::new(), Vec::new());
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
                            // TEST: drop the yielded empty segment, keep the name-clear.
                            out[canonical].ref_info = None;
                            out[canonical].metadata = None;
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
                        mk_segment(out, minted, name, commits, cbs)
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
                    let seg = mk_segment(out, minted, name, Vec::new(), Vec::new());
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
            // Empty stacks never get workspace-commit parent slots; metadata is their only
            // representation, so a base candidate suffices to anchor them.
            let _ = (restrict_to_ws_parents, ws_parents, ws_commit);
            let is_candidate = (Some(at) == lower_bound
                || stacks
                    .iter()
                    .any(|stack| stack.segments.last().is_some_and(|s| s.base == Some(at))))
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
                let seg = mk_segment(out, minted, name, Vec::new(), Vec::new());
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
                        Some(canonical) => canonical,
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
#[allow(clippy::too_many_arguments)]
fn branch_records(
    canonical_name_by_head: &BTreeMap<gix::ObjectId, gix::refs::FullName>,
    facts: &Facts<'_>,
    materialized_empties: &[(gix::refs::FullName, Option<gix::ObjectId>)],
    ws_ref: Option<&gix::refs::FullNameRef>,
    ws_md: Option<&ref_metadata::Workspace>,
    ws_commit: Option<gix::ObjectId>,
    target_commits: &[gix::ObjectId],
    lower_bound: Option<gix::ObjectId>,
    adhoc_branch_order: &[gix::refs::FullName],
    worktree_by_branch: &crate::init::WorktreeByBranch,
) -> Vec<crate::branch_graph::Branch> {
    use crate::branch_graph::Branch;
    let worktree_of = |name: &gix::refs::FullName| -> Option<crate::Worktree> {
        crate::RefInfo::from_ref(name.clone(), None, worktree_by_branch).worktree
    };
    // The metadata the walk recorded for the record named `name`, if any.
    let metadata_of = |name: &gix::refs::FullName| -> Option<crate::SegmentMetadata> {
        facts
            .named_segments()
            .find(|(_, ri)| ri.ref_name == *name)
            .and_then(|(rec, _)| facts.metadata_of(rec).cloned())
    };
    let entrypoint_rec = facts.entrypoint();
    // The managed workspace commit: its parent edges fan out through the stacks they carry.
    let ws_head = ws_commit;
    let mut meta_stacks: Vec<Vec<gix::refs::FullName>> = ws_md
        .map(|ws_md| {
            ws_md
                .stacks(but_core::ref_metadata::StackKind::Applied)
                .map(|ms| ms.branches.iter().map(|b| b.ref_name.clone()).collect())
                .collect()
        })
        .unwrap_or_default();
    // Workspace-metadata stacks split runs at their branches; the persisted ad-hoc order only
    // chains same-tip members, so it joins the head-naming below but never splits a run.
    let ws_meta_stacks: Vec<Vec<gix::refs::FullName>> = meta_stacks.clone();
    if !adhoc_branch_order.is_empty() {
        meta_stacks.push(adhoc_branch_order.to_vec());
    }
    // The walk's name for a run = the facts-derived canonical name (forced/disambiguated local,
    // else the picked remote-tracking ref). A remote-only run takes its remote name so it owns its
    // commits as a named, findable segment.
    let walk_name = |head: gix::ObjectId| -> Option<gix::refs::FullName> {
        canonical_name_by_head.get(&head).cloned()
    };
    // Each applied stack's tip commit with its ancestry. A stack tip resting on a workspace-commit
    // parent that another stack's history passes through is a sibling lane, not that commit's
    // owner: the workspace merge repeated an ancestor to keep the lane.
    let ws_parents: HashSet<gix::ObjectId> = ws_head
        .map(|id| {
            facts
                .commits()
                .node_data(id)
                .parent_ids
                .iter()
                .copied()
                .collect()
        })
        .unwrap_or_default();
    let stack_ancestries: Vec<(usize, std::collections::HashSet<gix::ObjectId>)> = meta_stacks
        .iter()
        .enumerate()
        .filter_map(|(i, names)| {
            let tip = names.iter().find_map(|n| {
                facts.head_of().values().copied().find(|&head| {
                    facts.commits().node(head).is_some_and(|nx| {
                        facts.commits().inner[nx]
                            .refs
                            .iter()
                            .any(|ri| ri.ref_name == *n)
                    })
                })
            })?;
            Some((i, facts.commits().ancestor_ids(tip)))
        })
        .collect();

    // Runs first, in record order; then empty named attached records.
    let mut list: Vec<Branch> = Vec::new();
    let mut index_of_run_head: BTreeMap<gix::ObjectId, usize> = BTreeMap::new();
    let mut target_of_run_head: BTreeMap<gix::ObjectId, usize> = BTreeMap::new();
    let mut index_of_record: BTreeMap<usize, usize> = BTreeMap::new();
    // Per run head: the empty chains stacks lift above it as `(stack, chain top, whole)`, `whole`
    // when the stack's top branch sits at the head (its lane hangs off the workspace), else the
    // chain continues that stack's own run above.
    let mut splice_route: BTreeMap<gix::ObjectId, Vec<(usize, usize, bool)>> = BTreeMap::new();
    // Per run head: the run's first part (entered directly) and the stack its chain above belongs to.
    let mut first_part_of_run_head: BTreeMap<gix::ObjectId, usize> = BTreeMap::new();
    let mut run_head_of_index: BTreeMap<usize, gix::ObjectId> = BTreeMap::new();
    let mut stack_of_run_head: BTreeMap<gix::ObjectId, Option<usize>> = BTreeMap::new();
    let stack_of_name = |name: &gix::refs::FullName| -> Option<usize> {
        meta_stacks.iter().position(|names| names.contains(name))
    };
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
        // A remote-named or anonymous run whose head carries workspace branches from a SINGLE
        // metadata stack gives the commit to the base-most (last in metadata order) of those
        // branches; the upper ones become empty records above it (built by `meta_chain` below) and
        // the remote keeps its own empty segment. When several stacks share the head the run stays
        // anonymous instead — the guard below handles that.
        let integrated_head = facts
            .commits()
            .node_data(head)
            .flags
            .contains(CommitFlags::Integrated);
        let below_other_stack = (ws_parents.contains(&head)
            && (integrated_head
                || stack_ancestries.iter().any(|(i, ancestors)| {
                    !meta_stacks[*i].iter().any(&at_head) && ancestors.contains(&head)
                })))
            || (integrated_head
                && lower_bound == Some(head)
                && meta_stacks.iter().any(|names| {
                    !names.first().is_some_and(&at_head) && names.iter().skip(1).any(&at_head)
                }));
        let claiming_ws_md_name_at_head = || {
            if below_other_stack {
                return None;
            }
            let mut stacks_with_head = meta_stacks
                .iter()
                .filter(|names| names.iter().any(&at_head));
            let only = stacks_with_head
                .next()
                .filter(|_| stacks_with_head.next().is_none())?;
            only.iter().rev().find(|name| at_head(name)).cloned()
        };
        let stacks_at_head = meta_stacks
            .iter()
            .filter(|names| names.iter().any(&at_head))
            .count();
        let mut owner_name = canonical_local_name
            .clone()
            .or_else(claiming_ws_md_name_at_head)
            .or(walk_name(head));
        // The workspace ref names its own workspace commit, or a commit no other local branch sits
        // on. Sharing a commit with another branch (its commit is missing), it keeps an empty
        // record above that branch's run, like an entrypoint that lost the name.
        let other_local_at_head = facts.commits().node_data(head).ref_name_iter().any(|rn| {
            rn.category() == Some(gix::refs::Category::LocalBranch) && !is_internal_ref(rn.as_ref())
        });
        let ws_ref_may_name = |name: &gix::refs::FullName| {
            Some(head) == ws_commit || Some(name.as_ref()) != ws_ref || !other_local_at_head
        };
        if owner_name
            .as_ref()
            .is_some_and(|name| !ws_ref_may_name(name))
        {
            owner_name = None;
        }
        // A sibling stack's branch resting inside another stack's history is an empty lane
        // spliced into the workspace edge, never that commit's owner.
        if below_other_stack
            && owner_name
                .as_ref()
                .is_some_and(|name| meta_stacks.iter().flatten().any(|n| n == name))
        {
            owner_name = None;
        }
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
                .filter(ws_ref_may_name)
                // A stack member is a lane at this head, not the run's name.
                .filter(|ep| stack_of_name(ep).is_none())
        });
        let mut chain_above: Vec<gix::refs::FullName> = meta_chain
            .into_iter()
            .filter(|name| Some(name) != run_name.as_ref())
            .collect();
        // The entrypoint ref sitting on the run head without naming the run (another branch won
        // the name) keeps its own empty record above it, so `HEAD` stays selectable and the walk
        // enters through it.
        // The entrypoint is this run when its record owns it, or when an unnamed entrypoint
        // record (a detached head, or one the walk disambiguated onto this commit) resolves to the
        // run head. A named entrypoint record keeps its own branch below.
        let entrypoint_here = entrypoint_rec == Some(owner)
            || entrypoint_rec.is_some_and(|rec| {
                facts.ref_info_of(rec).is_none() && facts.record_commit(rec) == Some(head)
            });
        let entrypoint_stack = facts.entrypoint_ref().and_then(stack_of_name);
        if let Some(ep_ref) = facts.entrypoint_ref()
            && entrypoint_here
            && run_name.as_ref() != Some(ep_ref)
            && at_head(ep_ref)
            && !chain_above.contains(ep_ref)
            && (entrypoint_stack.is_none()
                || entrypoint_stack == run_name.as_ref().and_then(stack_of_name))
        {
            chain_above.insert(0, ep_ref.clone());
        }
        let entrypoint_in_chain = facts.entrypoint_ref().is_some_and(|ep_ref| {
            entrypoint_here
                && (chain_above.contains(ep_ref)
                    || (Some(ep_ref) != run_name.as_ref()
                        && at_head(ep_ref)
                        && stack_of_name(ep_ref).is_some()))
        });
        // Meta stacks whose tips were deduplicated onto this head get an empty chain per stack,
        // spliced into the workspace-commit parent edge (ws -> tip -> run), fanning out when
        // several stacks share the head.
        let splice_chains: Vec<(usize, Vec<gix::refs::FullName>, bool)> = meta_stacks
            .iter()
            .enumerate()
            .filter(|(_, names)| !owner_name.as_ref().is_some_and(|o| names.contains(o)))
            .map(|(stack, names)| {
                let whole = names.first().is_some_and(&at_head);
                let mut names: Vec<_> = names
                    .iter()
                    .filter(|name| {
                        at_head(name)
                            && Some(*name) != run_name.as_ref()
                            && !chain_above.contains(name)
                    })
                    .cloned()
                    .collect();
                names.dedup();
                (stack, names, whole)
            })
            .filter(|(_, names, _)| !names.is_empty())
            .collect();
        let chain_top_in_list = list.len();
        let run_idx_in_list = list.len() + chain_above.len();
        for name in chain_above.iter() {
            list.push(Branch {
                ref_name: Some(name.clone()),
                commits: Vec::new(),
                outgoing: vec![(list.len() + 1, 0)],
                is_entrypoint: entrypoint_in_chain && Some(name) == facts.entrypoint_ref(),
                worktree: worktree_of(name),
                metadata: metadata_of(name),
            });
        }
        // Incoming connections route through the chain; the run's own connections leave from
        // the run element itself.
        target_of_run_head.insert(head, chain_top_in_list);
        first_part_of_run_head.insert(head, run_idx_in_list);
        run_head_of_index.insert(run_idx_in_list, head);
        index_of_run_head.insert(head, run_idx_in_list);
        index_of_record.insert(owner, run_idx_in_list);
        let run_stack = run_name.as_ref().and_then(stack_of_name);
        stack_of_run_head.insert(head, run_stack);
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
                let bytes: &[u8] = name.as_bstr().as_ref();
                let remote_head = bytes.ends_with(b"/HEAD");
                let internal_remote = name.category() == Some(gix::refs::Category::RemoteBranch)
                    && name.shorten().to_string().contains("/gitbutler/");
                Some(name) != run_name.as_ref()
                    && !remote_head
                    && !internal_remote
                    && !chain_above.contains(name)
                    && !splice_chains
                        .iter()
                        .flat_map(|(_, names, _)| names)
                        .any(|n| n == name)
                    && displaced_seen.insert(name.clone())
            })
            .collect();
        let strip_names: Vec<gix::refs::FullName> = chain_above
            .iter()
            .chain(splice_chains.iter().flat_map(|(_, names, _)| names))
            .chain(displaced_names.iter())
            .cloned()
            .chain(run_name.clone())
            .collect();
        // The run's commits, each keeping the refs the records don't lift: unlike the displayed
        // projection, the carrier keeps metadata-consumed refs on commits — except those lifted
        // into a chain, whose Reference steps the chain provides.
        let run_commits: Vec<gix::ObjectId> = facts.run(head);
        let carrier_commit = |id: gix::ObjectId, strip: &[gix::refs::FullName]| -> crate::Commit {
            let node = facts.commits().node_data(id);
            crate::Commit {
                id,
                parent_ids: node.parent_ids.clone(),
                flags: node.flags,
                refs: node
                    .refs
                    .iter()
                    .filter(|ri| {
                        !strip.contains(&ri.ref_name)
                            && !ri.ref_name.as_bstr().starts_with(b"refs/heads/gitbutler/")
                            && !facts
                                .consumed_local_refs
                                .contains(&(id, ri.ref_name.clone()))
                    })
                    .cloned()
                    .collect(),
            }
        };
        // Metadata-listed branches sitting on a commit inside the run split it there, like the
        // walk splits at unambiguous local branches: the names chain above the commit in
        // metadata order, the last of them owning the commits from there on.
        let mut split_points: Vec<(usize, Vec<gix::refs::FullName>)> = Vec::new();
        for (cidx, id) in run_commits.iter().enumerate().skip(1) {
            let at_commit = |name: &gix::refs::FullName| {
                facts.commits().node(*id).is_some_and(|nx| {
                    facts.commits().inner[nx]
                        .refs
                        .iter()
                        .any(|ri| ri.ref_name == *name)
                })
            };
            let mut names: Vec<gix::refs::FullName> = Vec::new();
            for stack in &ws_meta_stacks {
                for name in stack.iter().filter(|n| at_commit(n)) {
                    if !names.contains(name) {
                        names.push(name.clone());
                    }
                }
            }
            if !names.is_empty() {
                split_points.push((cidx, names));
            }
        }
        let first_split = split_points.first().map(|(cidx, _)| *cidx);
        let head_part: Vec<crate::Commit> = run_commits
            .iter()
            .take(first_split.unwrap_or(run_commits.len()))
            .map(|&id| carrier_commit(id, if id == head { &strip_names } else { &[] }))
            .collect();
        list.push(Branch {
            worktree: run_name.as_ref().and_then(&worktree_of),
            metadata: run_name
                .as_ref()
                .and_then(&metadata_of)
                .or_else(|| facts.metadata_of(owner).cloned()),
            ref_name: run_name,
            commits: head_part,
            outgoing: Vec::new(),
            is_entrypoint: entrypoint_here && !entrypoint_in_chain,
        });
        let mut last_part_idx = run_idx_in_list;
        for (i, (cidx, names)) in split_points.iter().enumerate() {
            let end = split_points
                .get(i + 1)
                .map(|(next, _)| *next)
                .unwrap_or(run_commits.len());
            let (owner_name, above) = names.split_last().expect("at least one name");
            // The previous part connects to the top of this chain.
            let chain_top = list.len();
            list[last_part_idx].outgoing.push((chain_top, 0));
            for name in above {
                list.push(Branch {
                    ref_name: Some(name.clone()),
                    commits: Vec::new(),
                    outgoing: vec![(list.len() + 1, 0)],
                    is_entrypoint: false,
                    worktree: worktree_of(name),
                    metadata: metadata_of(name),
                });
            }
            let commits: Vec<crate::Commit> = run_commits[*cidx..end]
                .iter()
                .map(|&id| carrier_commit(id, if id == run_commits[*cidx] { names } else { &[] }))
                .collect();
            list.push(Branch {
                worktree: worktree_of(owner_name),
                metadata: metadata_of(owner_name),
                ref_name: Some(owner_name.clone()),
                commits,
                outgoing: Vec::new(),
                is_entrypoint: false,
            });
            last_part_idx = list.len() - 1;
        }
        // Edges leaving the run leave from its last part.
        index_of_run_head.insert(head, last_part_idx);
        for (stack, names, whole) in &splice_chains {
            let top = list.len();
            for (i, name) in names.iter().enumerate() {
                // A stack's own chain continues into the run directly; the chain above belongs
                // to the run's stack.
                let next = if i + 1 == names.len() {
                    chain_top_in_list
                } else {
                    list.len() + 1
                };
                list.push(Branch {
                    ref_name: Some(name.clone()),
                    commits: Vec::new(),
                    outgoing: vec![(next, 0)],
                    is_entrypoint: entrypoint_here && Some(name) == facts.entrypoint_ref(),
                    worktree: worktree_of(name),
                    metadata: metadata_of(name),
                });
            }
            splice_route
                .entry(head)
                .or_default()
                .push((*stack, top, *whole));
        }
        for name in displaced_names {
            if list.iter().any(|s| s.ref_name.as_ref() == Some(&name)) {
                continue;
            }
            // A remote-tracking ref enters at the chain element of the branch it tracks when the
            // chain lifts that branch; every other displaced name enters the run directly.
            let name_bytes: &[u8] = name.as_bstr().as_ref();
            let tracked_in_chain = bstr::ByteSlice::rsplit_once_str(name_bytes, "/")
                .map(|(_, short)| short.to_vec())
                .and_then(|short| {
                    list[chain_top_in_list..run_idx_in_list]
                        .iter()
                        .position(|b| {
                            b.ref_name.as_ref().is_some_and(|rn| {
                                rn.category() == Some(gix::refs::Category::LocalBranch)
                                    && rn.shorten() == short.as_slice()
                            })
                        })
                        .map(|offset| chain_top_in_list + offset)
                });
            list.push(Branch {
                worktree: worktree_of(&name),
                metadata: metadata_of(&name),
                ref_name: Some(name),
                commits: Vec::new(),
                outgoing: vec![(tracked_in_chain.unwrap_or(run_idx_in_list), 0)],
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
        // Remote HEADs and GitButler's own remote refs never shape the branch graph.
        let bytes: &[u8] = ref_name.as_bstr().as_ref();
        if ref_name.category() == Some(gix::refs::Category::RemoteBranch)
            && (bytes.ends_with(b"/HEAD") || ref_name.shorten().to_string().contains("/gitbutler/"))
        {
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
            worktree: worktree_of(&ref_name),
            metadata: facts.metadata_of(rec).cloned(),
            ref_name: Some(ref_name),
            commits: Vec::new(),
            outgoing: vec![(target, 0)],
            is_entrypoint: entrypoint_rec == Some(rec),
        });
    }

    // Named empty segments the projection materialized without a record counterpart (e.g. a
    // deduplicated target tip) still need to be selectable. The caller supplies them as
    // (name, the commit they route down to), so this doesn't navigate a segment graph.
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
                worktree: worktree_of(name),
                metadata: metadata_of(name),
            });
        }
    }

    // Empty metadata stacks resting on a commit: their chain tops, in metadata order. The
    // workspace commit's edge to such a commit routes through them, one edge per stack, like the
    // workspace's parent slots would — an empty lane hangs off the workspace, not off the base.
    let empty_stack_tops_on = |list: &[Branch], commit: gix::ObjectId| -> Vec<usize> {
        let Some(&target) = target_of_run_head.get(&commit) else {
            return Vec::new();
        };
        let resolves_to = |mut idx: usize| -> bool {
            for _ in 0..list.len().max(1) {
                if idx == target {
                    return true;
                }
                match list[idx].outgoing.as_slice() {
                    [(next, _)] if list[idx].commits.is_empty() => idx = *next,
                    _ => return false,
                }
            }
            false
        };
        let mut tops = Vec::new();
        for names in &ws_meta_stacks {
            let Some(top_name) = names.first() else {
                continue;
            };
            let Some(idx) = list
                .iter()
                .position(|b| b.commits.is_empty() && b.ref_name.as_ref() == Some(top_name))
            else {
                continue;
            };
            if idx != target && resolves_to(idx) && !tops.contains(&idx) {
                tops.push(idx);
            }
        }
        tops
    };

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
        let source_stack = stack_of_run_head.get(&child_run_head).copied().flatten();
        if Some(child) == ws_head {
            // The workspace commit's edge routes through the whole-stack chains lifted at the
            // head when they exist, fanning out one per stack.
            let tops: Vec<usize> = splice_route
                .get(&parent)
                .map(|chains| {
                    chains
                        .iter()
                        .filter(|(_, _, whole)| *whole)
                        .map(|(_, top, _)| *top)
                        .collect()
                })
                .unwrap_or_default();
            if !tops.is_empty() {
                // Reversed to match the edge replay order, so stacks keep their metadata order.
                for &top in tops.iter().rev() {
                    list[source].outgoing.push((top, parent_order));
                }
                continue;
            }
            let tops = empty_stack_tops_on(&list, parent);
            if !tops.is_empty() {
                for top in tops {
                    list[source].outgoing.push((top, parent_order));
                }
                continue;
            }
            list[source].outgoing.push((target, parent_order));
            continue;
        }
        // A stack's own run continues into the chain it lifts at the head; other workspace
        // commits enter through the chain the run keeps above it, while the target's and the
        // remotes' lanes arrive below the chain, at the run itself.
        if let Some(stack) = source_stack
            && let Some((_, top, _)) = splice_route
                .get(&parent)
                .and_then(|chains| chains.iter().find(|(s, _, _)| *s == stack))
        {
            list[source].outgoing.push((*top, parent_order));
            continue;
        }
        let source_is_target_lane = target_commits
            .iter()
            .any(|t| facts.run(child_run_head).contains(t));
        let first_part = first_part_of_run_head[&parent];
        list[source].outgoing.push((
            if source_is_target_lane {
                first_part
            } else {
                target
            },
            parent_order,
        ));
    }
    // Every applied stack is a lane of the workspace: a stack top nothing reaches from the
    // workspace commit hangs off it after the real parents, in metadata order.
    if let Some(ws_head) = ws_head
        && let Some((_, ws_run_head)) = facts.run_of(ws_head)
        && let Some(&ws_idx) = index_of_run_head.get(&ws_run_head)
    {
        let mut reachable = BTreeSet::new();
        let mut pending = vec![ws_idx];
        while let Some(idx) = pending.pop() {
            if reachable.insert(idx) {
                pending.extend(list[idx].outgoing.iter().map(|(next, _)| *next));
            }
        }
        let mut appended = Vec::new();
        for names in &ws_meta_stacks {
            let Some(top_name) = names.first() else {
                continue;
            };
            let Some(idx) = list
                .iter()
                .position(|b| b.commits.is_empty() && b.ref_name.as_ref() == Some(top_name))
            else {
                continue;
            };
            // A stack checked out in a linked worktree is forked out of the workspace.
            if worktree_of(top_name).is_some_and(|wt| !wt.owned_by_repo) {
                continue;
            }
            if !reachable.contains(&idx) && !appended.contains(&idx) {
                appended.push(idx);
            }
        }
        // Order 0 like a real first parent: the rebase keeps real parents ahead by insertion.
        for idx in appended {
            list[ws_idx].outgoing.push((idx, 0));
        }
    }
    // A workspace ref without a workspace commit rests on its anchor directly; its lanes are the
    // applied stacks, wherever they rest. Fan the ref out to every stack's top record, in
    // metadata order, keeping the anchor edge only when no stack is represented.
    if ws_commit.is_none()
        && let Some(ws_md) = ws_md
        && let Some(ws_idx) = list
            .iter()
            .position(|b| b.commits.is_empty() && b.ref_name.as_ref().map(|n| n.as_ref()) == ws_ref)
    {
        let tops: Vec<usize> = ws_md
            .stacks(but_core::ref_metadata::StackKind::Applied)
            .filter_map(|ms| {
                let top = ms.branches.first()?;
                let idx = list
                    .iter()
                    .position(|b| b.ref_name.as_ref() == Some(&top.ref_name))?;
                // A stack top naming a run is entered through the chain above that run.
                Some(
                    run_head_of_index
                        .get(&idx)
                        .and_then(|head| target_of_run_head.get(head).copied())
                        .unwrap_or(idx),
                )
            })
            .filter(|&idx| idx != ws_idx)
            .collect();
        if !tops.is_empty() {
            let mut seen = BTreeSet::new();
            list[ws_idx].outgoing = tops
                .into_iter()
                .filter(|idx| seen.insert(*idx))
                .enumerate()
                .map(|(order, idx)| (idx, order as u32))
                .collect();
        }
    }

    // The entrypoint record may be an attached record whose name a run already carries (skipped
    // above), or a ref the records never name (like a tag). The branch carrying the entrypoint
    // ref, else the one owning the entrypoint commit, is the entrypoint then.
    if !list.iter().any(|b| b.is_entrypoint)
        && let Some(rec) = entrypoint_rec
    {
        let by_name = facts
            .entrypoint_ref()
            .and_then(|ep| list.iter().position(|b| b.ref_name.as_ref() == Some(ep)));
        let by_commit = facts.record_commit(rec).and_then(|id| {
            list.iter()
                .position(|b| b.commits.iter().any(|c| c.id == id))
        });
        if let Some(idx) = by_name.or(by_commit) {
            list[idx].is_entrypoint = true;
        }
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
    let cut_first_commit = stack.segments[cut_seg_idx].commits.first().map(|c| c.id);
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
    // A segment pruning emptied now rests on the commit its tip points to; one that keeps
    // commits rests on the first commit pruned below them.
    if let Some(last) = stack.segments.last_mut()
        && let Some(tip) = cut_first_commit
    {
        if last.commits.is_empty() {
            last.base = Some(tip);
            last.base_segment_id = Some(last.id);
        } else if !keep_if_fully_integrated {
            let first_pruned = last
                .commits
                .last()
                .and_then(|c| facts.commits().first_parent_id(c.id))
                .or(Some(tip));
            last.base = first_pruned;
            last.base_segment_id = None;
        }
    }
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

/// The persisted GitButler-created branch order of the chain containing the entrypoint ref, tip
/// to base, restricted to local branches that exist. Empty in managed workspaces and when nothing
/// was persisted.
fn adhoc_branch_order<T: RefMetadata>(
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    meta: &OverlayMetadata<'_, T>,
) -> anyhow::Result<(Vec<gix::refs::FullName>, bool)> {
    let Some(entrypoint_ref) = facts.entrypoint_ref() else {
        return Ok((Vec::new(), false));
    };
    if entrypoint_ref.category() != Some(gix::refs::Category::LocalBranch) {
        return Ok((Vec::new(), false));
    }
    // Without a persisted order, the applied workspace stack listing the checked-out branch
    // shapes the view in full: its branches above and below the checkout are its lanes. The
    // persisted single-branch order instead starts the view at the checked-out branch.
    let mut persisted = true;
    let order = match meta.branch_stack_order(entrypoint_ref.as_ref())? {
        Some(order) => order,
        None => {
            persisted = false;
            let ws_ref = gix::refs::FullName::try_from("refs/heads/gitbutler/workspace")?;
            let Some(stack) = meta.workspace_opt(ws_ref.as_ref())?.and_then(|ws| {
                ws.stacks(ref_metadata::StackKind::Applied)
                    .find(|ms| ms.branches.iter().any(|b| b.ref_name == *entrypoint_ref))
                    .map(|ms| {
                        ms.branches
                            .iter()
                            .map(|b| b.ref_name.clone())
                            .collect::<Vec<_>>()
                    })
            }) else {
                return Ok((Vec::new(), false));
            };
            stack
        }
    };
    let mut existing = Vec::new();
    for branch in order {
        if branch.category() != Some(gix::refs::Category::LocalBranch) {
            continue;
        }
        if ctx.repo.try_find_reference(branch.as_ref())?.is_none() {
            continue;
        }
        existing.push(branch);
    }
    Ok(if existing.len() < 2 {
        (Vec::new(), false)
    } else {
        (existing, persisted)
    })
}

/// Rebuild every same-tip run of `order` inside the ad-hoc stack: the last member keeps (or takes)
/// the commit-owning segment, the members above it become empty segments in order, and their refs
/// leave the commit's displayed refs. Members pointing at commits outside the stack, or alone at
/// their commit, are left as they are.
#[allow(clippy::too_many_arguments)]
fn apply_adhoc_branch_order<T: RefMetadata>(
    out: &mut NodeStore,
    facts: &Facts<'_>,
    ctx: &Context<'_>,
    meta: &OverlayMetadata<'_, T>,
    order: &[gix::refs::FullName],
    starts_at_checkout: bool,
    lower_bound: Option<gix::ObjectId>,
    stacks: &mut [Stack],
) {
    let branch_md = |name: &gix::refs::FullName| -> Option<ref_metadata::Branch> {
        meta.branch_opt(name.as_ref())
            .ok()
            .flatten()
            .map(|md| ref_metadata::Branch::clone(&md))
    };
    let Some(stack) = stacks.first_mut() else {
        return;
    };
    // Resolve each member freshly: the walked commits carry overlay refs the repository may not
    // know yet; anything else resolves through the repository.
    let mut resolved: Vec<(gix::refs::FullName, gix::ObjectId)> = Vec::new();
    for name in order {
        let walked = facts.state.commits.inner.node_indices().find_map(|nx| {
            let node = &facts.state.commits.inner[nx];
            node.refs
                .iter()
                .any(|ri| ri.ref_name == *name)
                .then_some(node.id)
        });
        let Some(id) = walked.or_else(|| {
            ctx.repo
                .try_find_reference(name.as_ref())
                .ok()
                .flatten()
                .and_then(|mut r| r.peel_to_id().ok())
                .map(|id| id.detach())
        }) else {
            continue;
        };
        resolved.push((name.clone(), id));
    }
    let mut groups: Vec<(gix::ObjectId, Vec<gix::refs::FullName>)> = Vec::new();
    for (name, id) in resolved {
        match groups.last_mut() {
            Some((gid, names)) if *gid == id => names.push(name),
            _ => groups.push((id, vec![name])),
        }
    }
    for (commit_id, names) in groups {
        if names.len() < 2 {
            continue;
        }
        // The segment holding the commit, or an empty one resting on it (an entry at the bound).
        let Some(seg_idx) = stack
            .segments
            .iter()
            .position(|seg| seg.commits.iter().any(|c| c.id == commit_id))
            .or_else(|| {
                stack.segments.iter().position(|seg| {
                    seg.commits.is_empty()
                        && (seg.base == Some(commit_id)
                            || seg.ref_info.as_ref().and_then(|ri| ri.commit_id) == Some(commit_id))
                })
            })
        else {
            // The group sits on the stack's base: the members above the last one are empty
            // dependents at the bottom, the last one is the base the stack rests on.
            if stack
                .segments
                .last()
                .is_some_and(|seg| seg.base == Some(commit_id))
            {
                let bottom_name = names.last().expect("at least two");
                for name in names.iter().filter(|n| *n != bottom_name) {
                    let ri = ref_info_adopting_worktree(facts, name, Some(commit_id));
                    let md = branch_md(name);
                    let rec = out.insert_segment(MintSeg {
                        ref_info: Some(ri.clone()),
                        metadata: md.clone().map(crate::SegmentMetadata::Branch),
                        ..Default::default()
                    });
                    if let Some(above) = stack.segments.last_mut() {
                        above.base = None;
                        above.base_segment_id = Some(rec);
                    }
                    stack.segments.push(StackSegment {
                        ref_info: Some(ri),
                        id: rec,
                        base: Some(commit_id),
                        base_segment_id: None,
                        metadata: md,
                        ..blank_stack_segment()
                    });
                }
            }
            continue;
        };
        let commit_idx = stack.segments[seg_idx]
            .commits
            .iter()
            .position(|c| c.id == commit_id)
            .unwrap_or(0);
        let bottom_name = names.last().expect("at least two");
        // The segment owning the commit: the one it starts, or a split of the one it sits in.
        let owner_idx = if commit_idx == 0 {
            seg_idx
        } else {
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
            let base = stack.segments[seg_idx].base;
            let base_segment_id = stack.segments[seg_idx].base_segment_id;
            let rec = out.insert_segment(MintSeg {
                ref_info: None,
                commits: Vec::new(),
                ..Default::default()
            });
            stack.segments[seg_idx].base = Some(commit_id);
            stack.segments[seg_idx].base_segment_id = Some(rec);
            stack.segments.insert(
                seg_idx + 1,
                StackSegment {
                    ref_info: None,
                    id: rec,
                    commits: tail,
                    commits_by_segment: tail_by_segment,
                    base,
                    base_segment_id,
                    ..blank_stack_segment()
                },
            );
            seg_idx + 1
        };
        // The last member always names the owner; a non-member name it carried stays on the commit.
        let previous_name = stack.segments[owner_idx]
            .ref_info
            .take()
            .filter(|ri| !names.contains(&ri.ref_name));
        let ri = ref_info_adopting_worktree(facts, bottom_name, Some(commit_id));
        stack.segments[owner_idx].ref_info = Some(ri.clone());
        stack.segments[owner_idx].metadata = branch_md(bottom_name);
        if let Some(canonical) = out.nodes.get_mut(&stack.segments[owner_idx].id) {
            canonical.ref_info = Some(ri);
            canonical.metadata = branch_md(bottom_name).map(crate::SegmentMetadata::Branch);
        }
        if let Some(previous) = previous_name
            && let Some(first) = stack.segments[owner_idx].commits.first_mut()
            && !first.refs.iter().any(|ri| ri.ref_name == previous.ref_name)
        {
            first.refs.push(previous);
        }
        // The members between the entrypoint (or the top) and the owner become empty segments
        // above it: the view starts at the checked-out branch.
        // The persisted single-branch order starts the view at the checked-out branch; a stack
        // taken from workspace metadata shows every member above the owner.
        let start = if starts_at_checkout {
            facts
                .entrypoint_ref()
                .and_then(|ep| names.iter().position(|n| n == ep))
                .unwrap_or(0)
        } else {
            0
        };
        let above: Vec<&gix::refs::FullName> = names[start..]
            .iter()
            .take_while(|n| *n != bottom_name)
            .collect();
        for (offset, name) in above.iter().enumerate() {
            let ri = ref_info_adopting_worktree(facts, name, Some(commit_id));
            let md = branch_md(name);
            let rec = out.insert_segment(MintSeg {
                ref_info: Some(ri.clone()),
                metadata: md.clone().map(crate::SegmentMetadata::Branch),
                ..Default::default()
            });
            let next_id = stack.segments[owner_idx + offset].id;
            stack.segments.insert(
                owner_idx + offset,
                StackSegment {
                    ref_info: Some(ri),
                    id: rec,
                    base: None,
                    base_segment_id: Some(next_id),
                    metadata: md,
                    ..blank_stack_segment()
                },
            );
        }
        let owner_idx = owner_idx + above.len();
        if let Some(first) = stack.segments[owner_idx].commits.first_mut() {
            first.refs.retain(|ri| !names.contains(&ri.ref_name));
        }
        if let Some(canonical) = out.nodes.get_mut(&stack.segments[owner_idx].id)
            && let Some(first) = canonical.commits.first_mut()
        {
            first.refs.retain(|ri| !names.contains(&ri.ref_name));
        }
        // The bottom member owning nothing visible below other members is the base they rest
        // on, not a lane of its own.
        let _ = lower_bound;
        if owner_idx > 0 && stack.segments[owner_idx].commits.is_empty() {
            let removed = stack.segments.remove(owner_idx);
            if let Some(above) = stack.segments.get_mut(owner_idx - 1) {
                above.base = removed.base.or(Some(commit_id));
                above.base_segment_id = removed.base_segment_id;
            }
        }
    }
}

/// Give each branch in `refs` - checked out in a linked worktree - an empty branch record that
/// forks directly onto the commit it points at, with nothing routing through it, and drop it from
/// the stack rows. A local branch naming the commit-owning record is hoisted into an empty record
/// that keeps its place in the lane; workspace refs and remote names stay put.
fn fork_out_worktree_refs(
    branches: &mut Vec<crate::branch_graph::Branch>,
    stacks: &mut Vec<Stack>,
    refs: &[gix::refs::FullName],
    ws_ref: Option<&gix::refs::FullName>,
    worktree_by_branch: &crate::init::WorktreeByBranch,
) {
    use crate::branch_graph::Branch;
    let worktree_of = |name: &gix::refs::FullName| -> Option<crate::Worktree> {
        crate::RefInfo::from_ref(name.clone(), None, worktree_by_branch).worktree
    };
    enum Location {
        Named(usize),
        OnCommit(usize, usize),
    }
    let retarget = |branches: &mut [Branch], from: usize, to: usize, except: Option<usize>| {
        for (i, b) in branches.iter_mut().enumerate() {
            if Some(i) == except {
                continue;
            }
            for (target, _) in b.outgoing.iter_mut() {
                if *target == from {
                    *target = to;
                }
            }
        }
    };
    let skip_empty = |branches: &[Branch], mut idx: usize| -> Option<gix::ObjectId> {
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
    for ref_name in refs {
        let location = branches.iter().enumerate().find_map(|(i, b)| {
            if b.ref_name.as_ref() == Some(ref_name) {
                return Some(Location::Named(i));
            }
            b.commits
                .iter()
                .position(|c| c.refs.iter().any(|ri| ri.ref_name == *ref_name))
                .map(|cidx| Location::OnCommit(i, cidx))
        });
        let Some(location) = location else {
            tracing::debug!(ref_name = %ref_name.as_bstr(), "worktree-checked-out ref not in graph, leaving it as is");
            continue;
        };
        let (fork, commit_id) = match location {
            Location::Named(_) if Some(ref_name) == ws_ref => continue,
            Location::Named(idx) if branches[idx].commits.is_empty() => {
                // Chained into a lane: splice it out and re-attach it as a fork below.
                let [(target, _)] = branches[idx].outgoing.as_slice() else {
                    continue;
                };
                let target = *target;
                let Some(commit_id) = skip_empty(branches, target) else {
                    continue;
                };
                retarget(branches, idx, target, Some(idx));
                branches[idx].outgoing.clear();
                (idx, commit_id)
            }
            Location::Named(idx) => {
                // The commits stay behind in the now anonymous record; the name moves to the fork.
                let commit_id = branches[idx].commits[0].id;
                let name = branches[idx].ref_name.take();
                let worktree = branches[idx].worktree.take();
                let metadata = branches[idx].metadata.take();
                branches.push(Branch {
                    ref_name: name,
                    commits: Vec::new(),
                    outgoing: Vec::new(),
                    is_entrypoint: false,
                    worktree,
                    metadata,
                });
                (branches.len() - 1, commit_id)
            }
            Location::OnCommit(idx, cidx) => {
                let commit_id = branches[idx].commits[cidx].id;
                branches[idx].commits[cidx]
                    .refs
                    .retain(|ri| ri.ref_name != *ref_name);
                branches.push(Branch {
                    ref_name: Some(ref_name.clone()),
                    commits: Vec::new(),
                    outgoing: Vec::new(),
                    is_entrypoint: false,
                    worktree: worktree_of(ref_name),
                    metadata: None,
                });
                (branches.len() - 1, commit_id)
            }
        };
        // Attach the fork to the record owning the commit, splitting it so the commit starts it.
        let Some((owner, cidx)) = branches.iter().enumerate().find_map(|(i, b)| {
            b.commits
                .iter()
                .position(|c| c.id == commit_id)
                .map(|cidx| (i, cidx))
        }) else {
            continue;
        };
        let owner = if cidx == 0 {
            owner
        } else {
            let tail: Vec<Commit> = branches[owner].commits.drain(cidx..).collect();
            let outgoing = std::mem::take(&mut branches[owner].outgoing);
            branches.push(Branch {
                ref_name: None,
                commits: tail,
                outgoing,
                is_entrypoint: false,
                worktree: None,
                metadata: None,
            });
            let tail_idx = branches.len() - 1;
            branches[owner].outgoing = vec![(tail_idx, 0)];
            tail_idx
        };
        // Only rewritable names couple the owner to a rewrite: hoist a local branch name into an
        // empty record that keeps the lane; the workspace ref and remote names stay.
        let owner_is_local = branches[owner]
            .ref_name
            .as_ref()
            .is_some_and(|rn| rn.category() == Some(gix::refs::Category::LocalBranch))
            && branches[owner].ref_name.as_ref() != ws_ref;
        if owner_is_local {
            let name = branches[owner].ref_name.take();
            let worktree = branches[owner].worktree.take();
            let metadata = branches[owner].metadata.take();
            let is_entrypoint = std::mem::take(&mut branches[owner].is_entrypoint);
            branches.push(Branch {
                ref_name: name,
                commits: Vec::new(),
                outgoing: vec![(owner, 0)],
                is_entrypoint,
                worktree,
                metadata,
            });
            let hoisted = branches.len() - 1;
            retarget(branches, owner, hoisted, Some(hoisted));
            // The fork must not follow the hoisted name.
            for (target, _) in branches[fork].outgoing.iter_mut() {
                if *target == hoisted {
                    *target = owner;
                }
            }
        }
        branches[fork].outgoing = vec![(owner, 0)];

        // The stack rows: the branch is no longer one of them, and its ref leaves the commits.
        for stack in stacks.iter_mut() {
            for seg in stack.segments.iter_mut() {
                for c in seg.commits.iter_mut() {
                    c.refs.retain(|ri| ri.ref_name != *ref_name);
                }
            }
            let Some(seg_idx) = stack
                .segments
                .iter()
                .position(|seg| seg.ref_name() == Some(ref_name.as_ref()))
            else {
                continue;
            };
            if stack.segments[seg_idx].commits.is_empty() {
                let removed = stack.segments.remove(seg_idx);
                if seg_idx > 0
                    && let Some(above) = stack.segments.get_mut(seg_idx - 1)
                {
                    above.base = removed.base;
                    above.base_segment_id = removed.base_segment_id;
                }
            } else if seg_idx > 0 {
                let removed = stack.segments.remove(seg_idx);
                let above = &mut stack.segments[seg_idx - 1];
                let offset = above.commits.len();
                above.commits.extend(removed.commits);
                above.commits_by_segment.extend(
                    removed
                        .commits_by_segment
                        .into_iter()
                        .map(|(sidx, ofs)| (sidx, ofs + offset)),
                );
                above.base = removed.base;
                above.base_segment_id = removed.base_segment_id;
                above.base_ref_name = removed.base_ref_name;
            } else {
                let seg = &mut stack.segments[seg_idx];
                seg.ref_info = None;
                seg.metadata = None;
                seg.remote_tracking_ref_name = None;
                seg.remote_tip_id = None;
                seg.is_entrypoint = false;
            }
        }
        stacks.retain(|stack| !stack.segments.is_empty());
        for stack in stacks.iter_mut() {
            if stack.segments.iter().all(|seg| seg.ref_info.is_none()) {
                stack.id = None;
            }
        }
    }
}
