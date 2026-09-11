use std::collections::{BTreeSet, HashSet};

use anyhow::Context;
use bstr::ByteSlice;
use but_core::ref_metadata::{
    self, StackId,
    StackKind::{Applied, AppliedAndUnapplied},
};
use gix::{ObjectId, refs::Category};
use itertools::Itertools;
use petgraph::{Direction, prelude::EdgeRef, visit::NodeIndexable};
use tracing::instrument;

use crate::{
    CommitFlags, Graph, Segment, SegmentIndex, SegmentMetadata, Workspace,
    init::WorktreeTip,
    utils::SegmentTable,
    workspace::{
        Stack, StackCommit, StackCommitFlags, StackSegment, TargetCommit, TargetRef, WorkspaceKind,
        WorktreeBase, WorktreeStack, workspace::WorkspaceState,
    },
};

/// The segment a workspace is projected from, and the target it is framed against.
struct Frame<'graph> {
    kind: WorkspaceKind,
    metadata: Option<&'graph ref_metadata::Workspace>,
    ws: SegmentIndex,
    target_ref: Option<TargetRef>,
    target_commit: Option<TargetCommit>,
}

/// The first-parent walk from a stack tip down to the target.
struct Lane {
    /// The stack segments from the tip down. The first is headed by the tip itself, without
    /// members if the tip is at or below the target.
    groups: Vec<Group>,
    /// The first commit at or below the target, along with its owner.
    base: Option<(ObjectId, SegmentIndex)>,
    /// The walk ended because the traversal did, not because history did.
    early_end: bool,
}

/// Consecutive segments of a [`Lane`] that form one stack segment.
struct Group {
    /// The segments contributing commits, starting with `head`.
    /// Empty if `head` is at or below the target.
    members: Vec<SegmentIndex>,
    head: SegmentIndex,
}

/// Which segments the remote tracking branches of the projected segments reach.
struct RemoteReach(Vec<(gix::refs::FullName, SegmentTable<bool>)>);

impl RemoteReach {
    fn flags(
        &self,
        sidx: SegmentIndex,
        own_remote: Option<&gix::refs::FullName>,
    ) -> StackCommitFlags {
        self.0
            .iter()
            .filter(|(_, reachable)| reachable.get(sidx))
            .fold(StackCommitFlags::empty(), |flags, (remote, _)| {
                flags
                    | StackCommitFlags::ReachableByRemote
                    | if Some(remote) == own_remote {
                        StackCommitFlags::ReachableByMatchingRemote
                    } else {
                        StackCommitFlags::empty()
                    }
            })
    }
}

/// Local branches name stack segments; GitButler's own refs never do.
fn is_local_branch(name: Option<&gix::refs::FullNameRef>) -> bool {
    name.is_some_and(|rn| {
        rn.category() == Some(Category::LocalBranch)
            && !rn.as_bstr().starts_with_str("refs/heads/gitbutler/")
    })
}

fn is_remote_branch(segment: &Segment) -> bool {
    segment
        .ref_name()
        .is_some_and(|rn| rn.category() == Some(Category::RemoteBranch))
}

impl Graph {
    /// Project the graph into a workspace, seen from its [entrypoint](Self::entrypoint()).
    ///
    /// A managed workspace lists one stack per parent of its workspace commit, anything else
    /// is a single stack starting at the entrypoint. Each stack is the first-parent history of
    /// its tip down to the stored target commit, or all of it without one.
    #[instrument(
        name = "Graph::into_workspace",
        level = "trace",
        skip(self),
        err(Debug)
    )]
    pub fn into_workspace(self) -> anyhow::Result<Workspace> {
        let WorkspaceState {
            id,
            kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
            metadata,
            worktrees,
        } = self.to_workspace_state()?;
        Ok(Workspace {
            graph: self,
            id,
            kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit,
            metadata,
            worktrees,
        })
    }

    pub(crate) fn to_workspace_state(&self) -> anyhow::Result<WorkspaceState> {
        Ok(self.project(self.frame(self.entrypoint()?.segment.id)?))
    }

    fn frame(&self, ws: SegmentIndex) -> anyhow::Result<Frame<'_>> {
        let segment = &self[ws];
        let metadata = segment.workspace_metadata();
        let kind = match metadata {
            Some(_) => {
                let ref_info = segment
                    .ref_info
                    .clone()
                    .context("BUG: managed workspaces must always be on a named segment")?;
                if segment.commits.is_empty() {
                    WorkspaceKind::ManagedMissingWorkspaceCommit { ref_info }
                } else {
                    WorkspaceKind::Managed { ref_info }
                }
            }
            None => WorkspaceKind::AdHoc,
        };
        // A plain branch integrates with its own upstream.
        let own_upstream = || {
            segment
                .remote_tracking_ref_name
                .clone()
                .zip(segment.remote_tracking_branch_segment_id)
                .map(|(ref_name, segment_index)| TargetRef {
                    ref_name,
                    segment_index,
                    commits_ahead: 0,
                })
        };
        let target_ref = self
            .project_meta
            .target_ref
            .as_ref()
            .and_then(|rn| TargetRef::from_ref_name_without_commits_ahead(rn, self))
            .or_else(|| (!kind.has_managed_ref()).then(own_upstream).flatten());
        let target_commit = self
            .project_meta
            .target_commit_id
            .and_then(|id| TargetCommit::from_commit(id, self))
            .or_else(|| self.target_commit_from_ref());
        Ok(Frame {
            kind,
            metadata,
            ws,
            target_ref,
            target_commit,
        })
    }

    /// Without a usable stored target commit, the tip of the stored target ref stands in for it.
    fn target_commit_from_ref(&self) -> Option<TargetCommit> {
        let ref_name = self.project_meta.target_ref.as_ref()?;
        let segment = self.segment_by_ref_name(ref_name.as_ref())?;
        let commit_id = segment.commits.first()?.id;
        tracing::info!(
            %ref_name,
            %commit_id,
            "No usable stored target commit, using the tip of the target ref instead"
        );
        Some(TargetCommit {
            commit_id,
            segment_index: segment.id,
        })
    }

    fn project(&self, frame: Frame<'_>) -> WorkspaceState {
        let tips = self.stack_tips(&frame);
        let stop = self.stop_set(&frame);
        let lanes = tips.iter().map(|&tip| self.lane(tip, &stop)).collect_vec();
        let worktree_lanes = self.worktree_lanes(&frame, &lanes);
        let ids = self.stack_ids(&frame, &lanes);
        // The first commit below the target shared by all lanes.
        let (lower_bound, lower_bound_segment_id) = lanes
            .iter()
            .filter_map(|lane| lane.base.map(|(_, sidx)| sidx))
            .reduce(|base, other| self.find_merge_base(base, other).unwrap_or(base))
            .and_then(|sidx| self.resolve_to_unambiguously_pointed_to_commit(sidx))
            .map(|(commit, sidx)| (commit.id, sidx))
            .unzip();
        let remotes = self.remote_reachability(
            lanes
                .iter()
                .chain(worktree_lanes.iter().map(|(_, lane)| lane))
                .flat_map(|lane| lane.groups.iter()),
        );
        let stacks = lanes
            .into_iter()
            .zip(ids)
            .filter_map(|(lane, id)| self.stack(lane, id, &frame, &remotes))
            .collect();
        let worktrees = worktree_lanes
            .into_iter()
            .map(|(tip, lane)| self.worktree_stack(tip, lane, &stop, &frame, &remotes))
            .collect();
        let target_ref = frame.target_ref.map(|target| TargetRef {
            commits_ahead: TargetRef::commits_ahead(
                self,
                target.segment_index,
                lower_bound_segment_id,
            ),
            ..target
        });
        WorkspaceState {
            id: frame.ws,
            kind: frame.kind,
            stacks,
            lower_bound,
            lower_bound_segment_id,
            target_ref,
            target_commit: frame.target_commit,
            metadata: frame.metadata.cloned(),
            worktrees,
        }
    }

    /// One lane per [worktree tip](Graph::worktree_tips), in tip order, each owning what
    /// neither the workspace lanes, the target, nor an earlier worktree lane does.
    fn worktree_lanes<'a>(
        &'a self,
        frame: &Frame<'_>,
        stack_lanes: &[Lane],
    ) -> Vec<(&'a WorktreeTip, Lane)> {
        fn claim(stop: &mut SegmentTable<bool>, lane: &Lane) {
            for &sidx in lane.groups.iter().flat_map(|group| group.members.iter()) {
                stop.set(sidx, true);
            }
        }
        let mut stop = self.stop_set(frame);
        stop.set(frame.ws, true);
        for lane in stack_lanes {
            claim(&mut stop, lane);
        }
        let mut lanes = Vec::new();
        for tip in &self.worktree_tips {
            let Some(sidx) = self.worktree_tip_segment(tip) else {
                tracing::warn!(
                    worktree = %tip.name,
                    head = %tip.id,
                    "Worktree tip is not part of the graph, skipping it"
                );
                continue;
            };
            let lane = self.lane(sidx, &stop);
            claim(&mut stop, &lane);
            lanes.push((tip, lane));
        }
        lanes
    }

    /// The segment named by the checked-out branch, or the one owning a detached `HEAD`.
    fn worktree_tip_segment(&self, tip: &WorktreeTip) -> Option<SegmentIndex> {
        match &tip.ref_name {
            Some(name) => self.segment_by_ref_name(name.as_ref()).map(|s| s.id),
            None => self.segment_id_by_commit_id(tip.id).ok(),
        }
    }

    /// A detached `HEAD` names no segment: the first segment is anonymous and the name it sits
    /// on moves onto its first commit, as it does for a detached entrypoint.
    fn worktree_stack(
        &self,
        tip: &WorktreeTip,
        lane: Lane,
        target_stop: &SegmentTable<bool>,
        frame: &Frame<'_>,
        remotes: &RemoteReach,
    ) -> WorktreeStack {
        let head = lane
            .groups
            .first()
            .and_then(|group| self.tip_skip_empty(group.head))
            .map_or(tip.id, |commit| commit.id);
        let base = lane.base.map(|(id, sidx)| {
            if target_stop.get(sidx) {
                WorktreeBase::Outside(id)
            } else {
                WorktreeBase::InWorkspace(id)
            }
        });
        let mut segments = self.lane_segments(lane, None, true, frame, remotes);
        if tip.ref_name.is_none()
            && let Some(first) = segments.first_mut()
            && let Some(ref_info) = first.ref_info.take()
            && let Some(commit) = first.commits.first_mut()
        {
            commit.refs.push(ref_info);
        }
        WorktreeStack {
            name: tip.name.clone(),
            ref_name: tip.ref_name.clone(),
            head,
            base,
            segments,
        }
    }

    fn stack_tips(&self, frame: &Frame<'_>) -> Vec<SegmentIndex> {
        if !frame.kind.has_managed_ref() {
            return vec![frame.ws];
        }
        self.inner
            .neighbors_directed(frame.ws, Direction::Outgoing)
            .collect()
    }

    /// Mark every segment at or below the target commit.
    fn stop_set(&self, frame: &Frame<'_>) -> SegmentTable<bool> {
        let mut stop = SegmentTable::new(self.inner.node_bound(), false);
        if let Some(target) = &frame.target_commit {
            self.visit_all_segments_including_start_until(
                target.segment_index,
                Direction::Outgoing,
                |s| {
                    stop.set(s.id, true);
                    false
                },
            );
        }
        stop
    }

    fn lane(&self, tip: SegmentIndex, stop: &SegmentTable<bool>) -> Lane {
        let mut seen = self.seen_table();
        let mut walked = Vec::new();
        let mut cursor = Some(tip);
        while let Some(sidx) = cursor {
            if stop.get(sidx) || !seen.insert_unseen(sidx) {
                break;
            }
            walked.push(sidx);
            cursor = self
                .inner
                .edges_directed(sidx, Direction::Outgoing)
                .min_by_key(|edge| edge.weight().parent_order)
                .map(|e| e.target());
        }
        let base = cursor
            .filter(|&sidx| stop.get(sidx))
            .and_then(|sidx| self.resolve_to_unambiguously_pointed_to_commit(sidx))
            .map(|(commit, owner)| (commit.id, owner));
        let early_end = cursor.is_none()
            && walked.last().is_some_and(|&last| {
                self.stop_condition(last)
                    .is_some_and(|condition| condition.at_limit())
            });
        Lane {
            groups: self.groups(tip, &walked),
            base,
            early_end,
        }
    }

    /// Split a walk at each local branch.
    fn groups(&self, tip: SegmentIndex, walked: &[SegmentIndex]) -> Vec<Group> {
        let group = |sidx: SegmentIndex, members: Vec<SegmentIndex>| Group {
            members,
            head: sidx,
        };
        if walked.is_empty() {
            return vec![group(tip, Vec::new())];
        }
        walked
            .iter()
            .fold(Vec::<Group>::new(), |mut groups, &sidx| {
                let name = self[sidx].ref_name();
                let starts_group = is_local_branch(name)
                    && groups
                        .last()
                        .is_some_and(|last| self[last.head].ref_name() != name);
                match groups.last_mut().filter(|_| !starts_group) {
                    Some(last) => last.members.push(sidx),
                    None => groups.push(group(sidx, vec![sidx])),
                }
                groups
            })
    }

    /// The id of the first metadata stack naming one of the segments of each lane, preferring
    /// segment names over refs on commits and applied stacks over unapplied ones. Ids are handed
    /// out once, in lane order.
    fn stack_ids(&self, frame: &Frame<'_>, lanes: &[Lane]) -> Vec<Option<StackId>> {
        if !frame.kind.has_managed_ref() {
            return vec![Some(StackId::single_branch_id()); lanes.len()];
        }
        let Some(metadata) = frame.metadata else {
            return vec![None; lanes.len()];
        };
        lanes
            .iter()
            .scan(BTreeSet::new(), |used, lane| {
                let segment_names = lane
                    .groups
                    .iter()
                    .filter_map(|g| self[g.head].ref_name())
                    .filter(|name| is_local_branch(Some(name)))
                    .collect_vec();
                let commit_refs = lane
                    .groups
                    .iter()
                    .flat_map(|g| g.members.iter())
                    .flat_map(|&sidx| self[sidx].commits.iter())
                    .flat_map(|c| c.refs.iter().map(|ri| ri.ref_name.as_ref()))
                    .collect_vec();
                let id = [segment_names, commit_refs]
                    .iter()
                    .flatten()
                    .find_map(|&name| {
                        [Applied, AppliedAndUnapplied].into_iter().find_map(|kind| {
                            let stack = metadata.stacks(kind).find(|stack| {
                                stack.branches.iter().any(|b| b.ref_name.as_ref() == name)
                            })?;
                            used.insert(stack.id).then_some(stack.id)
                        })
                    });
                Some(id)
            })
            .collect()
    }

    /// Reachability from each remote tracking branch shown in `groups`.
    fn remote_reachability<'a>(&self, groups: impl Iterator<Item = &'a Group>) -> RemoteReach {
        RemoteReach(
            groups
                .filter_map(|g| {
                    let head = &self[g.head];
                    head.remote_tracking_ref_name
                        .clone()
                        .zip(head.remote_tracking_branch_segment_id)
                })
                .unique()
                .map(|(remote, remote_sidx)| {
                    let mut reachable = SegmentTable::new(self.inner.node_bound(), false);
                    self.visit_all_segments_including_start_until(
                        remote_sidx,
                        Direction::Outgoing,
                        |s| {
                            reachable.set(s.id, true);
                            false
                        },
                    );
                    (remote, reachable)
                })
                .collect(),
        )
    }

    fn stack(
        &self,
        lane: Lane,
        id: Option<StackId>,
        frame: &Frame<'_>,
        remotes: &RemoteReach,
    ) -> Option<Stack> {
        let keep_first = !frame.kind.has_managed_ref();
        let segments = self.lane_segments(lane, id, keep_first, frame, remotes);
        (!segments.is_empty()).then_some(Stack { id, segments })
    }

    fn lane_segments(
        &self,
        lane: Lane,
        id: Option<StackId>,
        keep_first: bool,
        frame: &Frame<'_>,
        remotes: &RemoteReach,
    ) -> Vec<StackSegment> {
        let groups = self.retained_groups(lane.groups, frame, id, keep_first);
        groups
            .iter()
            .enumerate()
            .map(|(idx, group)| {
                let above = groups[..idx]
                    .iter()
                    .flat_map(|g| g.members.iter())
                    .flat_map(|&sidx| self[sidx].commits.iter().map(|c| c.id))
                    .collect();
                let base = match groups.get(idx + 1) {
                    Some(next) => (
                        next.members
                            .first()
                            .and_then(|&sidx| self[sidx].commits.first())
                            .map(|c| c.id),
                        Some(next.head),
                    ),
                    None => lane.base.unzip(),
                };
                let early_end = lane.early_end && idx + 1 == groups.len();
                self.stack_segment(group, base, early_end, frame, remotes, &above)
            })
            .collect_vec()
    }

    /// Empty segments are shown only if metadata asks for them, or as the first if `keep_first`.
    fn retained_groups(
        &self,
        groups: Vec<Group>,
        frame: &Frame<'_>,
        id: Option<StackId>,
        keep_first: bool,
    ) -> Vec<Group> {
        let own_metadata = id.and_then(|id| {
            frame
                .metadata
                .as_ref()?
                .stacks(Applied)
                .find(|stack| stack.id == id)
        });
        let ad_hoc_names: BTreeSet<_> = if frame.kind.has_managed_ref() {
            BTreeSet::new()
        } else {
            self.ad_hoc_branch_stack_orders
                .iter()
                .flatten()
                .map(|rn| rn.as_ref())
                .collect()
        };
        let wanted_by_metadata = |name: &gix::refs::FullNameRef| {
            own_metadata.is_some_and(|stack| {
                stack
                    .branches
                    .iter()
                    .any(|b| b.ref_name.as_ref() == name && !b.archived)
            }) || ad_hoc_names.contains(name)
        };
        groups
            .into_iter()
            .enumerate()
            .filter(|(idx, group)| {
                group
                    .members
                    .iter()
                    .any(|&sidx| !self[sidx].commits.is_empty())
                    || (*idx == 0 && keep_first)
                    || self[group.head].ref_name().is_some_and(wanted_by_metadata)
            })
            .map(|(_, group)| group)
            .collect()
    }

    fn stack_segment(
        &self,
        group: &Group,
        (base, base_segment_id): (Option<ObjectId>, Option<SegmentIndex>),
        early_end: bool,
        frame: &Frame<'_>,
        remotes: &RemoteReach,
        above: &HashSet<ObjectId>,
    ) -> StackSegment {
        let head = &self[group.head];
        let remote = head.remote_tracking_ref_name.as_ref();
        let keep_any_name = !frame.kind.has_managed_ref();
        let mut commits: Vec<_> = group
            .members
            .iter()
            .flat_map(|&sidx| self[sidx].commits.iter().map(move |c| (sidx, c)))
            .map(|(sidx, commit)| StackCommit {
                flags: StackCommitFlags::from(commit.flags) | remotes.flags(sidx, remote),
                ..StackCommit::from_graph_commit(commit)
            })
            .collect();
        if let Some(last) = commits.last_mut().filter(|_| early_end) {
            last.flags |= StackCommitFlags::EarlyEnd;
        }
        StackSegment {
            ref_info: head
                .ref_info
                .clone()
                .filter(|ri| keep_any_name || is_local_branch(Some(ri.ref_name.as_ref()))),
            remote_tracking_ref_name: remote.cloned(),
            remote_tracking_branch_segment_id: head.remote_tracking_branch_segment_id,
            id: group.head,
            commits,
            base,
            base_segment_id,
            commits_by_segment: group
                .members
                .iter()
                .scan(0, |offset, &sidx| {
                    let entry = (sidx, *offset);
                    *offset += self[sidx].commits.len();
                    Some(entry)
                })
                .collect(),
            commits_on_remote: head
                .remote_tracking_branch_segment_id
                .map(|remote_sidx| self.commits_on_remote(remote_sidx, above))
                .unwrap_or_default(),
            metadata: match &head.metadata {
                Some(SegmentMetadata::Branch(md)) => Some(md.clone()),
                Some(SegmentMetadata::Workspace(_)) | None => None,
            },
        }
    }

    /// The commits only the remote tracking branch at `remote_sidx` has: its remote-only history
    /// up to any other remote's, and commits of the segments `above` that it still points at
    /// after a branch was split.
    fn commits_on_remote(
        &self,
        remote_sidx: SegmentIndex,
        above: &HashSet<ObjectId>,
    ) -> Vec<StackCommit> {
        // An empty remote segment shares its tip commit with the segment owning it.
        let start = self
            .resolve_to_unambiguously_pointed_to_commit(remote_sidx)
            .map_or(remote_sidx, |(_, owner)| owner);
        let mut commits = Vec::new();
        self.visit_all_segments_including_start_until(start, Direction::Outgoing, |s| {
            let prune = true;
            if !s.commits.iter().all(|c| c.flags.is_remote())
                || (s.id != start && is_remote_branch(s))
            {
                return prune;
            }
            commits.extend(s.commits.iter().map(StackCommit::from_graph_commit));
            !prune
        });
        let mut seen: HashSet<_> = commits.iter().map(|c| c.id).collect();
        self.visit_segments_downward_along_first_parent_exclude_start(remote_sidx, |s| {
            if is_remote_branch(s) {
                return true;
            }
            commits.extend(
                s.commits
                    .iter()
                    .filter(|c| {
                        above.contains(&c.id)
                            && !c.flags.contains(CommitFlags::Integrated)
                            && seen.insert(c.id)
                    })
                    .map(StackCommit::from_graph_commit),
            );
            false
        });
        commits
    }
}
