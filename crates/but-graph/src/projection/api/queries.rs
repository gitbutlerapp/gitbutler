//! Discoverable queries over [`Workspace`](crate::Workspace).
//!
//! These functions name the question being asked instead of exposing legacy
//! presentation shapes.

use anyhow::Context;

use crate::{Workspace, workspace::StackSegment};

/// Legacy query helpers kept for callers that still depend on compatibility
/// semantics.
#[cfg(feature = "legacy")]
#[path = "legacy.rs"]
pub mod legacy;

/// # Points of Interest
impl Workspace {
    /// The name of the segment owning the workspace's lower bound — the ref marking the
    /// common base all stacks converge on (e.g. the target's local `main`), if the bound
    /// exists and its segment is named.
    pub fn lower_bound_ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.lower_bound_ref_name.as_ref().map(|rn| rn.as_ref())
    }

    /// Return the id of the commit at the tip of the workspace, or that the tip
    /// reference was pointing to in Git.
    ///
    /// Empty virtual workspace tip segments may fan out to multiple stack
    /// branches, so the workspace segment has no unique graph path to a commit.
    /// This falls back to the peeled commit id stored in the workspace segment's
    /// [`crate::RefInfo`].
    ///
    /// Note that this commit could also be the base of the workspace,
    /// particularly if there are no commits in the workspace.
    pub fn tip_commit_id(&self) -> Option<gix::ObjectId> {
        self.tip_commit_id
    }

    /// Where the branch named `name` rests: its own tip commit, or — when it is empty —
    /// the first commit of a segment below it in the same stack, or the stack's base.
    /// `None` if `name` isn't a workspace stack branch.
    pub fn branch_resting_commit_id(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        let (stack_idx, seg_idx) = self.find_segment_owner_indexes_by_refname(name)?;
        let stack = &self.stacks[stack_idx];
        stack.segments[seg_idx..]
            .iter()
            .find_map(|segment| segment.commits.first().map(|commit| commit.id))
            .or_else(|| stack.base())
    }

    /// Like [`Self::branch_resting_commit_id()`], but with distinct errors for an unknown
    /// branch and for the impossible no-base case.
    pub fn try_branch_resting_commit_id(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> anyhow::Result<gix::ObjectId> {
        let (stack_idx, seg_idx) = self.try_find_segment_owner_indexes_by_refname(name)?;
        let stack = &self.stacks[stack_idx];
        stack.segments[seg_idx..]
            .iter()
            .find_map(|segment| segment.commits.first().map(|commit| commit.id))
            .or_else(|| stack.base())
            .context("BUG: we should always see through to the base or eligible commits")
    }

    /// The commit that `name` points to, directly or indirectly: the segment named `name` —
    /// or holding a commit that carries `name` in its refs — resolves it, seeing through
    /// empty segments. Tags may or may not be in the graph, depending on how it was built.
    pub fn commit_id_by_ref_name(&self, name: &gix::refs::FullNameRef) -> Option<gix::ObjectId> {
        // The workspace ref itself is excluded from the graph's ref mapping; the captured
        // tip ref info answers for it.
        self.commit_graph().commit_by_ref(name).or_else(|| {
            let tip = self.tip_ref_info.as_ref()?;
            (tip.ref_name.as_ref() == name).then_some(tip.commit_id)?
        })
    }

    /// Return the full [`RefInfo`](crate::RefInfo) of the workspace tip segment, if the tip
    /// is named. Unlike [`Self::ref_name()`](Self::ref_name) this includes worktree and
    /// pointed-to-commit information.
    pub fn tip_ref_info(&self) -> Option<&crate::RefInfo> {
        self.tip_ref_info.as_ref()
    }

    /// Return the id of the commit the traversal entrypoint sits on — usually where
    /// `HEAD` points — or `None` if the entrypoint segment carries no commit.
    pub fn entrypoint_commit_id(&self) -> anyhow::Result<Option<gix::ObjectId>> {
        Ok(self
            .entrypoint
            .as_ref()
            .context("BUG: entrypoint must always be set")?
            .commit_id)
    }

    /// Return the id of the entry-point commit if it is a
    /// managed workspace commit owned by GitButler (per its marker message),
    /// looked up and verified via `repo`.
    pub fn managed_entrypoint_commit_id(
        &self,
        repo: &gix::Repository,
    ) -> anyhow::Result<Option<gix::ObjectId>> {
        let Some(entrypoint_commit_id) = self.entrypoint_commit_id()? else {
            return Ok(None);
        };
        let commit = repo.find_commit(entrypoint_commit_id)?;
        let message = commit.message_raw()?;
        Ok(
            crate::workspace::commit::is_managed_workspace_by_message(message)
                .then_some(entrypoint_commit_id),
        )
    }

    /// Return `true` if the traversal entrypoint is the workspace tip segment itself.
    ///
    /// Note the difference to [`Self::is_entrypoint()`](Self::is_entrypoint): that one asks
    /// whether no stack segment holds the entrypoint, which is also `true` when the
    /// entrypoint sits outside the workspace entirely.
    pub fn tip_is_entrypoint(&self) -> anyhow::Result<bool> {
        Ok(self
            .entrypoint
            .as_ref()
            .context("BUG: entrypoint must always be set")?
            .is_ws_tip)
    }

    /// Return the stored target commit id.
    ///
    /// This is the previous target position remembered in workspace metadata.
    /// It is normally the base the workspace last integrated with, and
    /// intentionally differs from the current tip of the target reference.
    pub fn stored_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.target_commit.as_ref().map(|target| target.commit_id)
    }

    /// Return the configured target reference name if the workspace target was
    /// resolved to a branch during graph traversal.
    /// This is mere convenience and it should only be used for displaying the target ref.
    /// For everything else, use [`Self::target_ref`].
    pub fn target_ref_name(&self) -> Option<&gix::refs::FullNameRef> {
        self.target_ref
            .as_ref()
            .map(|target| target.ref_name.as_ref())
    }

    /// Return the commit id that currently acts as the workspace target.
    ///
    /// This follows the same precedence as operations that need a concrete
    /// target side: target ref tip, then stored target commit, then the first
    /// integrated traversal tip.
    pub fn effective_target_commit_id(&self) -> Option<gix::ObjectId> {
        self.effective_target_commit_id
    }
}

/// # Carried Data
///
/// Data assembled during graph construction and carried into the projection.
impl Workspace {
    /// The commit graph the segment graph was assembled from — the commit-addressed
    /// substrate. Empty only for hand-assembled graphs that never went through the builders.
    pub fn commit_graph(&self) -> &crate::CommitGraph {
        &self.commit_graph
    }

    /// The project metadata as it was resolved when the graph was built.
    pub fn project_meta(&self) -> &but_core::ref_metadata::ProjectMeta {
        &self.ctx.project_meta
    }

    /// Mutable access to the carried project metadata — a simulation knob: change it, then
    /// [`redo`](Self::redo) to see the workspace under the altered resolution.
    pub fn project_meta_mut(&mut self) -> &mut but_core::ref_metadata::ProjectMeta {
        &mut self.ctx.project_meta
    }

    /// Mutable access to the carried traversal options — a simulation knob: change them, then
    /// [`redo`](Self::redo) to see the workspace under the altered traversal.
    pub fn options_mut(&mut self) -> &mut crate::walk::Options {
        &mut self.ctx.options
    }

    /// All remote names that aren't URLs, as retrieved during the traversal. Useful to
    /// extract remote names from remote-tracking refs like `refs/remotes/origin/master`,
    /// which may have slashes in them.
    pub fn symbolic_remote_names(&self) -> &[String] {
        &self.ctx.symbolic_remote_names
    }

    /// Return `true` if the graph traversal was stopped early due to a hard limit,
    /// meaning the graph — and thus this projection — may be incomplete.
    pub fn hard_limit_hit(&self) -> bool {
        self.commit_graph().hard_limit_hit
    }

    /// The LOCAL branch tracking `remote`, as the builder derived it from the repository
    /// (a git-configured binding, or name-deduction against the workspace's symbolic remotes).
    /// `None` when no local tracks `remote`, or for graphs not born from the builders.
    pub fn local_tracking_branch(
        &self,
        remote: &gix::refs::FullNameRef,
    ) -> Option<&gix::refs::FullName> {
        self.ctx
            .remote_tracking
            .iter()
            .find_map(|(local, r)| (r.as_ref() == remote).then_some(local))
    }

    /// The remote-tracking branch of `local`, the forward direction of
    /// [`Self::local_tracking_branch()`].
    pub fn remote_tracking_branch(
        &self,
        local: &gix::refs::FullNameRef,
    ) -> Option<&gix::refs::FullName> {
        self.ctx.remote_tracking.get(local)
    }
}

/// # Diagnostics
///
/// Debug-tooling views of the carried commit graph.
impl Workspace {
    /// Validate the carried commit graph's index and edge coherence, returning every
    /// violation.
    pub fn validation_errors(&self) -> Vec<anyhow::Error> {
        self.commit_graph().validation_errors()
    }

    /// Aggregate counts over the carried commit graph.
    pub fn statistics(&self) -> crate::CommitGraphStatistics {
        self.commit_graph().statistics()
    }

    /// The carried commit graph as a Graphviz `dot` document, very large graphs pruned
    /// from the bottom.
    pub fn dot_graph_pruned(&self) -> String {
        self.commit_graph().dot_graph()
    }

    /// Render the carried commit graph as an SVG and open it in the default viewer.
    #[cfg(unix)]
    pub fn open_graph_as_svg(&self) {
        crate::debug::open_dot_as_svg(&self.commit_graph().dot_graph());
    }

    /// The carried graph's alternate Debug output.
    pub fn graph_debug_string(&self) -> String {
        self.commit_graph().debug_string()
    }

    /// Render `commit` exactly like the graph debug output does — entrypoint marker, stop
    /// condition, remote/local kind, flags, and refs — with worktree ownership context and
    /// traversal state (hard limit) read from this workspace.
    pub fn commit_debug_string(
        &self,
        commit: &crate::Commit,
        is_entrypoint: bool,
        stop_condition: Option<crate::StopCondition>,
    ) -> String {
        crate::debug::commit_debug_string_inner(
            commit,
            is_entrypoint,
            stop_condition,
            self.commit_graph().hard_limit_hit,
            self.has_multiple_worktrees,
        )
    }
}

/// # Sets of Interest
impl Workspace {
    /// All commit ids reachable from `included` but not from `excluded`, newest first
    /// (optionally first-parent only). Both ids must be in the graph.
    pub fn commit_ids_in_a_not_b(
        &self,
        included: gix::ObjectId,
        excluded: gix::ObjectId,
        first_parent: crate::FirstParent,
    ) -> anyhow::Result<Vec<gix::ObjectId>> {
        let cg = self.commit_graph();
        for id in [included, excluded] {
            anyhow::ensure!(cg.node(id).is_some(), "commit {id} is not in the graph");
        }
        let first_parent: bool = first_parent.into();
        let parents = |id: gix::ObjectId| -> Vec<gix::ObjectId> {
            if first_parent {
                cg.first_parent(id).into_iter().collect()
            } else {
                cg.all_parent_ids(id)
            }
        };
        let mut hidden = std::collections::HashSet::new();
        let mut stack = vec![excluded];
        while let Some(id) = stack.pop() {
            if hidden.insert(id) {
                stack.extend(parents(id));
            }
        }
        // Newest-first by generation, insertion order breaking ties — the commit-level
        // equivalent of the segment walk's generation order.
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut sequence = 0usize;
        let mut queue = std::collections::BinaryHeap::new();
        let push = |queue: &mut std::collections::BinaryHeap<_>, id, sequence: &mut usize| {
            let generation = cg.generation_of(id).unwrap_or_default();
            queue.push((generation, std::cmp::Reverse(*sequence), id));
            *sequence += 1;
        };
        push(&mut queue, included, &mut sequence);
        while let Some((_, _, id)) = queue.pop() {
            if !seen.insert(id) || hidden.contains(&id) || cg.node(id).is_none() {
                continue;
            }
            out.push(id);
            for parent in parents(id) {
                push(&mut queue, parent, &mut sequence);
            }
        }
        Ok(out)
    }

    /// Visit the commits in the ancestry of the workspace tip segment — excluding that
    /// segment's own commits — in traversal order, stopping segment-wise at the workspace
    /// [lower bound](Self::lower_bound). Return `true` from `visit` to stop the traversal.
    ///
    /// This is useful to search the workspace history, e.g. for a workspace commit buried
    /// beneath commits made by hand.
    pub fn visit_commits_below_tip(&self, mut visit: impl FnMut(&crate::Commit) -> bool) {
        let cg = self.commit_graph();
        for id in &self.commits_below_tip {
            let Some(node) = cg.node(*id) else { continue };
            if visit(node) {
                return;
            }
        }
    }

    /// Return all target-reference commits that are ahead of the workspace base,
    /// which is the commits counted with
    /// [workspace::TargetRef::commits_ahead](crate::workspace::TargetRef::commits_ahead)
    ///
    /// The traversal starts at the resolved target reference and stops at the
    /// workspace lower bound or at commits already marked as belonging to the
    /// workspace. The result is ordered in graph traversal order from newer
    /// commits toward older commits.
    pub fn incoming_target_commit_ids(&self) -> anyhow::Result<Vec<gix::ObjectId>> {
        self.incoming_target_commit_ids
            .clone()
            .context("incoming target commits require a workspace with a target ref")
    }

    /// Return the ids of all target-side commits that are not part of the workspace's
    /// shared history, in traversal order from the target tip downward.
    /// Return `None` if the workspace has no notion of a target at all.
    ///
    /// The target side is the target ref if resolved, otherwise the stored target commit.
    ///
    /// RULING (2026-07-04): a DISJOINT target contributes no upstream commits. One pass
    /// decides everything on the carried commit graph: paint the lower bound's ancestor set
    /// (= shared history, following connected edges only), then walk down from the target
    /// tip collecting commits until the walk TOUCHES shared history. Diverged commits are
    /// never in that set, so a rewritten remote is collected at any depth; if the walk never
    /// touches it, target and workspace share no history — as far as the graph can see —
    /// and nothing is upstream.
    pub fn upstream_commit_ids_outside_shared_history(&self) -> Option<Vec<gix::ObjectId>> {
        let cg = self.commit_graph();
        let target_tip = self
            .target_ref
            .as_ref()
            .and_then(|t| cg.commit_by_ref(t.ref_name.as_ref()))
            .or_else(|| self.target_commit.as_ref().map(|t| t.commit_id))?;
        let shared_history = self.lower_bound.and_then(|lb| {
            // An empty (default) commit graph — hand-assembled graphs — knows no ancestry.
            cg.node(lb).is_some().then(|| cg.ancestor_set(lb))
        });
        let mut upstream_commits = Vec::new();
        let mut touched_shared_history = false;
        let mut seen = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::from([target_tip]);
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            let in_shared_history = shared_history.as_ref().is_some_and(|s| s.contains(&id));
            touched_shared_history |= in_shared_history;
            if in_shared_history || Some(id) == self.lower_bound || cg.node(id).is_none() {
                continue;
            }
            upstream_commits.push(id);
            queue.extend(cg.all_parent_ids(id));
        }
        if shared_history.is_some() && !touched_shared_history {
            // Never touching shared history means either a truly DISJOINT target (nothing is
            // upstream) or a target whose connection point lies beyond the traversal window
            // (a diverged remote cut short by limits — its commits ARE upstream). The graph
            // records where the traversal cut: if every collected commit's ancestry is fully
            // present and roots out without meeting shared history, it is genuinely disjoint.
            let genuinely_disjoint = !upstream_commits.iter().any(|id| cg.has_cut_parents(*id))
                && !shared_history
                    .iter()
                    .flatten()
                    .any(|id| cg.has_cut_parents(*id));
            if genuinely_disjoint {
                upstream_commits.clear();
            }
        }
        Some(upstream_commits)
    }

    /// Return a [`StackTip`] for each segment strictly below the
    /// reference `name` that coincides with a workspace stack tip — matched by segment id,
    /// or by first commit id to also catch the same commit in a differently segmented spot.
    /// The traversal prunes below each match.
    ///
    /// Return `None` if `name` isn't in the graph at all, which callers may not consider fatal.
    ///
    /// This tells which stacks a to-be-applied branch already flows into, i.e. which stacks
    /// it would supersede.
    pub fn stack_tip_segments_below_ref(
        &self,
        name: &gix::refs::FullNameRef,
    ) -> Option<Vec<StackTip>> {
        let cg = self.commit_graph();
        // Only refs that NAME a segment participate — a passive (e.g. ambiguous) ref does
        // not reach into the workspace on its own. Empty stacks have no tip commit and are
        // never superseded.
        let layout = cg.layout.as_ref()?;
        if !layout.facts_of(name)?.names_segment {
            return None;
        }
        let on = layout.positioned_on(name)?;
        let stack_tips: Vec<(gix::ObjectId, &StackSegment)> = self
            .stacks
            .iter()
            .filter_map(|stack| {
                let tip = stack.segments.first()?;
                tip.commits.first().map(|c| (c.id, tip))
            })
            .collect();

        let mut matches = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let names_empty = layout
            .facts_of(name)
            .is_some_and(|facts| facts.names_empty_segment);
        let mut queue: std::collections::VecDeque<gix::ObjectId> = if names_empty {
            // An empty named segment rests ON its anchor: the anchor's owner is the
            // first thing below it.
            std::collections::VecDeque::from([on])
        } else {
            // An owning segment: everything below starts at its tip's parent edges.
            cg.all_parent_ids(on).into()
        };
        while let Some(id) = queue.pop_front() {
            if !seen.insert(id) {
                continue;
            }
            if let Some((_, segment)) = stack_tips.iter().find(|(cid, _)| *cid == id) {
                // Prune below a match, like the segment traversal did.
                matches.push(StackTip {
                    ref_name: segment.ref_name().map(|rn| rn.to_owned()),
                    commit_id: segment.commits.first().map(|c| c.id),
                });
                continue;
            }
            queue.extend(cg.all_parent_ids(id));
        }
        Some(matches)
    }
}

/// A workspace stack tip in plain data, as returned by
/// [`Workspace::stack_tip_segments_below_ref()`].
#[derive(Debug, Clone)]
pub struct StackTip {
    /// The name of the tip segment, if it is named.
    pub ref_name: Option<gix::refs::FullName>,
    /// The tip segment's first commit, absent for empty segments.
    pub commit_id: Option<gix::ObjectId>,
}
