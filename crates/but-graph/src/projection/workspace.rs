use crate::projection::{Stack, StackCommit, StackCommitFlags, StackSegment};
use crate::{CommitFlags, Graph, Segment, SegmentIndex};
use anyhow::Context;
use but_core::ref_metadata;
use gix::reference::Category;
use petgraph::Direction;
use petgraph::prelude::EdgeRef;
use petgraph::visit::NodeRef;
use std::collections::{BTreeSet, VecDeque};
use std::fmt::Formatter;
use tracing::instrument;

/// A workspace is a list of [Stacks](Stack).
#[derive(Clone)]
pub struct Workspace<'graph> {
    /// The underlying graph for providing simplified access to data.
    // TODO: remove this if this struct wants to be more than intermediate.
    pub graph: &'graph Graph,
    /// An ID which uniquely identifies the [graph segment](Segment) that represents this instance.
    pub id: SegmentIndex,
    /// Where `HEAD` is currently pointing to.
    pub head: HeadLocation,
    /// One or more stacks that live in the workspace.
    pub stacks: Vec<Stack>,
    /// The target to integrate workspace stacks into.
    ///
    /// If `None`, this is a local workspace that doesn't know when possibly pushed branches are considered integrated.
    /// This happens when there is a local branch checked out without a remote tracking branch.
    pub target: Option<Target>,
    /// Read-only workspace metadata with additional information, or `None` if nothing was present.
    pub metadata: Option<ref_metadata::Workspace>,
}

/// Learn where the current `HEAD` is pointing to.
#[derive(Debug, Clone)]
pub enum HeadLocation {
    /// The `HEAD` is pointing to the workspace reference, like `refs/heads/gitbutler/workspace`.
    Workspace {
        /// The name of the reference pointing to the workspace commit. Useful for deriving the workspace name.
        ref_name: gix::refs::FullName,
    },
    /// A segment is checked out directly.
    ///
    /// It can be inside or outside of a workspace.
    /// If the respective segment is not named, this means the `HEAD` id detached.
    /// The commit that the working tree is at is always implied to be the first commit of the [`StackSegment`].
    Segment {
        /// The id of the segment to be found in any stack of the [workspace stacks](Workspace::stacks).
        segment_index: SegmentIndex,
    },
}

/// Information about the target reference.
#[derive(Debug, Clone)]
pub struct Target {
    /// The name of the target branch, i.e. the branch that all [Stacks](Stack) want to get merged into.
    pub ref_name: gix::refs::FullName,
    /// The amount of commits that aren't reachable by any segment in the workspace, they are in its future.
    pub commits_ahead: usize,
}

impl Target {
    /// Return `None` if `ref_name` wasn't found as segment in `graph`.
    /// This can happen if a reference is configured, but not actually present as reference.
    fn from_ref_name(ref_name: &gix::refs::FullName, graph: &Graph) -> Option<Self> {
        let target_segment = graph
            .inner
            .node_indices()
            .find(|n| graph[*n].ref_name.as_ref() == Some(ref_name))?;
        Some(Target {
            ref_name: ref_name.to_owned(),
            commits_ahead: {
                // Find all remote commits but stop traversing when there is segments without remotes.
                let mut count = 0;
                graph.visit_all_segments_until(target_segment, Direction::Outgoing, |s| {
                    let remote_commits = s.commits.iter().filter(|c| c.flags.is_remote()).count();
                    count += remote_commits;
                    remote_commits != s.commits.len()
                });
                count
            },
        })
    }
}

impl Graph {
    /// Analyse the current graph starting at its [entrypoint](Self::lookup_entrypoint()).
    ///
    /// No matter what, each location of `HEAD`, which corresponds to the entrypoint, can be represented as workspace.
    /// Further, the most expensive operations we perform to query additional commit information by reading it, but we
    /// only do so on the ones that the user can interact with.
    #[instrument(skip(self), err(Debug))]
    pub fn to_workspace(&self) -> anyhow::Result<Workspace<'_>> {
        #[rustfmt::skip]
        let (head, metadata, ws_tip_segment, entrypoint_sidx) = {
            let ep = self.lookup_entrypoint()?;
            match ep.segment.workspace_metadata() {
                None => {
                    // Skip over empty segments.
                    if ep.segment.non_empty_flags_of_first_commit()
                        .or_else(||self.find_map_downwards_along_first_parent(ep.segment_index, |s| s.commits.first().map(|c|c.flags)))
                        .is_some_and(|f| {
                            f.contains(CommitFlags::InWorkspace)
                                && !f.contains(CommitFlags::Integrated)
                        })
                    {
                        // search the (for now just one) workspace upstream and use it instead, mark this segment
                        // as entrypoint.
                        let ws_segment = self
                            .find_segment_upwards(ep.segment_index, |s| {
                                s.workspace_metadata().is_some()
                            })
                            .with_context(|| {
                                format!(
                                    "BUG: should have found upstream workspace segment from {:?}",
                                    ep.segment_index
                                )
                            })?;
                        (
                            HeadLocation::Workspace {
                                ref_name: ws_segment
                                    .ref_name
                                    .as_ref()
                                    .context(
                                        "BUG: cannot have workspace metadata but \
                                no ref name in segment",
                                    )?
                                    .clone(),
                            },
                            ws_segment.workspace_metadata().cloned(),
                            ws_segment,
                            Some(ep.segment_index)
                        )
                    } else {
                        (
                            HeadLocation::Segment {
                                segment_index: ep.segment_index,
                            },
                            None,
                            ep.segment,
                            None
                        )
                    }
                }
                Some(meta) => (
                    HeadLocation::Workspace {
                        ref_name: ep
                            .segment
                            .ref_name
                            .as_ref()
                            .context(
                                "BUG: cannot have workspace metadata but no ref name in segment",
                            )?
                            .clone(),
                    },
                    Some(meta.clone()),
                    ep.segment,
                    None
                ),
            }
        };

        let mut ws = Workspace {
            graph: self,
            id: ws_tip_segment.id,
            head,
            stacks: vec![],
            target: metadata
                .as_ref()
                .and_then(|md| Target::from_ref_name(md.target_ref.as_ref()?, self)),
            metadata,
        };

        if ws.is_managed() {
            for stack_top_sidx in self
                .inner
                .neighbors_directed(ws_tip_segment.id, Direction::Outgoing)
            {
                let stack_segment = &self[stack_top_sidx];
                ws.stacks.extend(
                    self.collect_stack_segments(
                        entrypoint_sidx,
                        stack_top_sidx,
                        |s| {
                            let stop = true;
                            // Assure entrypoints get their own segments
                            if s.id != stack_top_sidx && Some(s.id) == entrypoint_sidx {
                                return stop;
                            }
                            if s.commits
                                .first()
                                .is_some_and(|c| c.flags.contains(CommitFlags::Integrated))
                            {
                                return stop;
                            }
                            // TODO: test for that!
                            if s.workspace_metadata().is_some() {
                                return stop;
                            }
                            match (&stack_segment.ref_name, &s.ref_name) {
                                (Some(_), Some(_)) | (None, Some(_)) => stop,
                                (Some(_), None) | (None, None) => false,
                            }
                        },
                        |s| {
                            s.commits
                                .first()
                                .is_none_or(|c| !c.flags.contains(CommitFlags::Integrated))
                                && self
                                    .inner
                                    .neighbors_directed(s.id, Direction::Incoming)
                                    .all(|n| n.id() != ws_tip_segment.id)
                        },
                        |s| {
                            s.commits
                                .first()
                                .is_none_or(|c| c.flags.contains(StackCommitFlags::Integrated))
                        },
                    )?
                    // TODO: setup `base`
                    .map(Stack::from),
                );
            }
        } else {
            let start = ws_tip_segment;
            ws.stacks.extend(
                // TODO: This probably depends on more factors, could have relationship with remote tracking branch.
                self.collect_stack_segments(
                    entrypoint_sidx,
                    start.id,
                    |s| {
                        let stop = true;
                        // TODO: test for that!
                        if s.workspace_metadata().is_some() {
                            return stop;
                        }
                        match (&start.ref_name, &s.ref_name) {
                            (Some(_), Some(_)) | (None, Some(_)) => stop,
                            (Some(_), None) | (None, None) => false,
                        }
                    },
                    // We keep going until depletion
                    |_s| true,
                    // Never discard stacks
                    |_s| false,
                )?
                // TODO: setup `base`
                .map(Stack::from),
            );
        }

        ws.mark_remote_reachability()?;
        Ok(ws)
    }
}

/// Traversals
impl Graph {
    /// Return the ancestry of `start` along the first parents, itself included, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    ///
    /// Note that the traversal assumes as well-segmented graph without cycles.
    fn collect_first_parent_segments_until<'a>(
        &'a self,
        start: &'a Segment,
        mut stop: impl FnMut(&Segment) -> bool,
    ) -> (Vec<&'a Segment>, Option<&'a Segment>) {
        let mut out = vec![start];
        let mut edge = self
            .inner
            .edges_directed(start.id, Direction::Outgoing)
            .last();
        let mut stopped_at = None;
        let mut seen = BTreeSet::new();
        while let Some(first_edge) = edge {
            let next = &self[first_edge.target()];
            if stop(next) {
                stopped_at = Some(next);
                break;
            }
            out.push(next);
            if seen.insert(next.id) {
                edge = self
                    .inner
                    .edges_directed(next.id, Direction::Outgoing)
                    .last();
            }
        }
        (out, stopped_at)
    }

    /// Visit the ancestry of `start` along the first parents, itself included, until `stop` returns `true`.
    /// Also return the segment that we stopped at.
    /// **Important**: `stop` is not called with `start`, this is a feature.
    ///
    /// Note that the traversal assumes as well-segmented graph without cycles.
    fn visit_segments_along_first_parent_until(
        &self,
        start: SegmentIndex,
        mut stop: impl FnMut(&Segment) -> bool,
    ) {
        let mut edge = self.inner.edges_directed(start, Direction::Outgoing).last();
        let mut seen = BTreeSet::new();
        while let Some(first_edge) = edge {
            let next = &self[first_edge.target()];
            if stop(next) {
                break;
            }
            if seen.insert(next.id) {
                edge = self
                    .inner
                    .edges_directed(next.id, Direction::Outgoing)
                    .last();
            }
        }
    }

    /// Visit all segments from `start`, excluding, and return once `find` returns something mapped from the
    /// first suitable segment it encountered.
    fn find_map_downwards_along_first_parent<T>(
        &self,
        start: SegmentIndex,
        mut find: impl FnMut(&Segment) -> Option<T>,
    ) -> Option<T> {
        let mut out = None;
        self.visit_segments_along_first_parent_until(start, |s| {
            if let Some(res) = find(s) {
                out = Some(res);
                true
            } else {
                false
            }
        });
        out
    }

    /// Return `OK(None)` if the post-process discarded this segment after collecting it in full as it was not
    /// local a local branch.
    ///
    /// `is_one_past_end_of_stack_segment(s)` returns `true` if the graph segment `s` should be considered past the
    /// currently collected stack segment. If `false` is returned, it will become part of the current stack segment.
    /// It's not called for the first segment, so you can use it to compare the first with other segments.
    ///
    /// `starts_next_stack_segment(s)` returns `true` if a new stack segment should be started with `s` as first member,
    /// or `false` if the stack segments are complete and with it all stack segments.
    ///
    /// `discard_stack(stack_segment)` returns `true` if after collecting everything, we'd still want to discard the
    /// whole stack due to custom rules, after assuring the stack segment is no entrypoint.
    /// It's also called to determine if a stack-segment (from the bottom of the stack upwards) should be discarded.
    /// If the stack is empty at the end, it will be discarded in full.
    fn collect_stack_segments(
        &self,
        entrypoint_sidx: Option<SegmentIndex>,
        from: SegmentIndex,
        mut is_one_past_end_of_stack_segment: impl FnMut(&Segment) -> bool,
        mut starts_next_stack_segment: impl FnMut(&Segment) -> bool,
        mut discard_stack: impl FnMut(&StackSegment) -> bool,
    ) -> anyhow::Result<Option<Vec<StackSegment>>> {
        // TODO: Test what happens if a workspace commit is pointed at by a different ref (which is the entrypoint).
        let mut out = Vec::new();
        let mut next = Some(from);
        while let Some(from) = next.take() {
            let start = &self[from];
            let (segments, stopped_at) = self
                .collect_first_parent_segments_until(start, &mut is_one_past_end_of_stack_segment);
            let mut segment = StackSegment::from_graph_segments(&segments, self)?;
            if entrypoint_sidx.is_some_and(|id| segment.id == id) {
                segment.is_entrypoint = true;
            }
            out.push(segment);
            next = stopped_at
                .filter(|s| starts_next_stack_segment(s))
                .map(|s| s.id);
        }

        fn is_entrypoint_or_local(s: &StackSegment) -> bool {
            if s.is_entrypoint {
                return true;
            }
            s.ref_name
                .as_ref()
                .and_then(|rn| rn.category())
                .is_none_or(|c| c == Category::LocalBranch)
        }

        // Prune empty invalid ones from the front as cleanup.
        // This isn't an issue for algorithms as they always see the full version.
        // TODO: remove this once we don't have remotes in a workspace because traversal logic can do it better.
        if let Some(end) = out
            .iter()
            .enumerate()
            .take_while(|(_idx, s)| s.commits.is_empty() && !is_entrypoint_or_local(s))
            .map(|(idx, _s)| idx + 1)
            .last()
        {
            out.drain(..end);
        }

        // Definitely remove non-local empties from behind.
        // TODO: revise this
        if let Some(new_len) = out
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_idx, s)| s.commits.is_empty() && !is_entrypoint_or_local(s))
            .last()
            .map(|(idx, _s)| idx)
        {
            out.truncate(new_len);
        }

        // TODO: remove the hack of avoiding empty segments as special case, remove .is_empty() condition
        let mut is_pruned = |s: &StackSegment| {
            !s.commits.is_empty() && (!is_entrypoint_or_local(s) || discard_stack(s))
        };
        // Prune the whole stack if we start with unwanted segments.
        if out.first().is_some_and(&mut is_pruned) {
            tracing::warn!(
                "Ignoring stack {:?} ({:?})",
                out.first().and_then(|s| s.ref_name.as_ref()),
                from,
            );
            return Ok(None);
        }

        // We may have picked up unwanted segments, if the graph isn't perfectly clean
        // TODO: remove this to rather assure that non-local branches aren't linked up that way.
        if let Some(new_len) = out
            .iter()
            .enumerate()
            .rev()
            .take_while(|(_idx, s)| is_pruned(s))
            .last()
            .map(|(idx, _s)| idx)
        {
            out.truncate(new_len);
        }
        Ok((!out.is_empty()).then_some(out))
    }

    /// Visit all segments, including `start`, unless `visit_and_prune(segment)` returns `true`.
    /// Pruned segments aren't returned and not traversed.
    pub(crate) fn visit_all_segments_until(
        &self,
        start: SegmentIndex,
        direction: Direction,
        mut visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        let mut next = VecDeque::new();
        next.push_back(start);
        let mut seen = BTreeSet::new();
        while let Some(next_sidx) = next.pop_front() {
            if !visit_and_prune(&self[next_sidx]) {
                next.extend(
                    self.inner
                        .neighbors_directed(next_sidx, direction)
                        .filter(|n| seen.insert(*n)),
                )
            }
        }
    }

    /// Visit all segments across all connections, including `start` and return the segment for which `f(segment)` returns `true`.
    /// There is no traversal pruning.
    pub(crate) fn find_segment_upwards(
        &self,
        start: SegmentIndex,
        mut f: impl FnMut(&Segment) -> bool,
    ) -> Option<&Segment> {
        let mut next = VecDeque::new();
        next.push_back(start);
        let mut seen = BTreeSet::new();
        while let Some(next_sidx) = next.pop_front() {
            let s = &self[next_sidx];
            if f(s) {
                return Some(s);
            }
            next.extend(
                self.inner
                    .neighbors_directed(next_sidx, Direction::Incoming)
                    .filter(|n| seen.insert(*n)),
            );
        }
        None
    }
}

/// More processing
impl Workspace<'_> {
    // NOTE: it's a disadvantage to not do this on graph level - then all we'd need is
    //       - a remote_sidx to know which segment belongs to our remote tracking ref (for ease of use)
    //       - an identity set for each remote ref
    //       - a field that tells us the identity bit on the remote segment, so we can check if it's set.
    //       Now we basically re-do the remote tracking in the workspace projection, which is always a bit
    //       awkward to do.
    fn mark_remote_reachability(&mut self) -> anyhow::Result<()> {
        let remote_refs: Vec<_> = self
            .stacks
            .iter()
            .flat_map(|s| {
                s.segments.iter().filter_map(|s| {
                    s.remote_tracking_ref_name
                        .as_ref()
                        .cloned()
                        .zip(s.sibling_segment_id)
                })
            })
            .collect();
        let graph = self.graph;
        for (remote_tracking_ref_name, remote_sidx) in remote_refs {
            let mut remote_commits = Vec::new();
            graph.visit_all_segments_until(remote_sidx, Direction::Outgoing, |s| {
                let prune = !s.commits.iter().all(|c| c.flags.is_remote())
                    // Do not 'steal' commits from other known remote segments while they are officially connected.
                    || (s.id != remote_sidx
                    && s.ref_name
                    .as_ref()
                    .is_some_and(|orn| orn.category() == Some(Category::RemoteBranch)));
                if prune {
                    // See if this segment links to a commit we know as local, and mark it accordingly,
                    // along with all segments in that stack.
                    for stack in &mut self.stacks {
                        let Some((first_segment, first_commit_index)) =
                            stack.segments.iter().enumerate().find_map(|(os_idx, os)| {
                                os.commits_by_segment
                                    .iter()
                                    .find_map(|(sidx, commit_ofs)| {
                                        (*sidx == s.id).then_some(commit_ofs)
                                    })
                                    .map(|commit_ofs| (os_idx, *commit_ofs))
                            })
                        else {
                            continue;
                        };

                        let mut first_commit_index = Some(first_commit_index);
                        for segment in &mut stack.segments[first_segment..] {
                            let remote_reachable = StackCommitFlags::ReachableByRemote
                                | if segment.remote_tracking_ref_name.as_ref()
                                    == Some(&remote_tracking_ref_name)
                                {
                                    StackCommitFlags::ReachableByMatchingRemote
                                } else {
                                    StackCommitFlags::empty()
                                };
                            for commit in &mut segment.commits
                                [first_commit_index.take().unwrap_or_default()..]
                            {
                                commit.flags |= remote_reachable;
                            }
                        }
                        // keep looking - other stacks can repeat the segment!
                        continue;
                    }
                } else {
                    for commit in &s.commits {
                        remote_commits.push(StackCommit::from_graph_commit(commit));
                    }
                }
                prune
            });

            // Have to keep looking for matching segments, they can be mentioned multiple times.
            let mut found_segment = false;
            let remote_commits: Vec<_> = remote_commits.into_iter().collect::<Result<_, _>>()?;
            for local_segment_with_this_remote in self.stacks.iter_mut().flat_map(|stack| {
                stack.segments.iter_mut().filter_map(|s| {
                    (s.remote_tracking_ref_name.as_ref() == Some(&remote_tracking_ref_name))
                        .then_some(s)
                })
            }) {
                found_segment = true;
                local_segment_with_this_remote.commits_on_remote = remote_commits.clone();
            }
            if found_segment {
                tracing::error!(
                    "BUG: Couldn't find local segment with remote tracking ref '{rn}' - remote commits for it seem to be missing",
                    rn = remote_tracking_ref_name.as_bstr()
                );
            }
        }
        Ok(())
    }
}

/// Query
impl Workspace<'_> {
    /// Return `true` if this workspace is managed, meaning we control certain aspects of it.
    /// If `false`, we are more conservative and may not support all features.
    pub fn is_managed(&self) -> bool {
        self.metadata.is_some()
    }
}

/// Debugging
impl Workspace<'_> {
    /// Produce a distinct and compressed debug string to show at a glance what the workspace is about.
    pub fn debug_string(&self) -> String {
        let graph = self.graph;
        let (name, sign) = match &self.head {
            HeadLocation::Workspace { ref_name } => (Graph::ref_debug_string(ref_name), "🏘️"),
            HeadLocation::Segment { segment_index } => (
                graph[*segment_index]
                    .ref_name
                    .as_ref()
                    .map_or("DETACHED".into(), Graph::ref_debug_string),
                "⌂",
            ),
        };
        let target = self.target.as_ref().map_or_else(
            || "!".to_string(),
            |t| {
                format!(
                    "{target}{ahead}",
                    target = t.ref_name,
                    ahead = if t.commits_ahead == 0 {
                        "".to_string()
                    } else {
                        format!("⇣{}", t.commits_ahead)
                    }
                )
            },
        );
        format!(
            "{meta}{sign}:{id}:{name} <> ✓{target}",
            meta = if self.metadata.is_some() { "📕" } else { "" },
            id = self.id.index(),
        )
    }
}

impl std::fmt::Debug for Workspace<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct(&format!("Workspace({})", self.debug_string()))
            .field("id", &self.id.index())
            .field("stacks", &self.stacks)
            .field("metadata", &self.metadata)
            .finish()
    }
}
