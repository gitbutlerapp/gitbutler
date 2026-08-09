//! A changeset is everything that changed between two trees, and as such is nothing else than Vec<[`TreeChange`]>.
//! Changesets can have IDs which uniquely identify a set of changes, independently of which trees it originated from.
//!
//! This property allows changeset IDs to be used to determine if two different commits, or sets of commits,
//! represent the same change.

use std::borrow::Cow;

use bstr::BStr;
use but_core::changeset::{
    ChangeIdMode, ChangesetCommit, changeset_identifier, create_similarity_lut, lookup_similar,
    range_changeset_identifier,
};
use gix::prelude::ObjectIdExt;

use crate::{
    RefInfo,
    ref_info::{Commit, LocalCommitRelation},
    ui::PushStatus,
};

/// Lets the `but-core` changeset engine read `ref_info::Commit` directly, without
/// copying it into an intermediate struct. The message is already conflict-stripped
/// and the change-id already derived, so these are cheap accessors.
impl ChangesetCommit for Commit {
    fn id(&self) -> gix::ObjectId {
        self.id
    }
    fn first_parent_id(&self) -> Option<gix::ObjectId> {
        self.parent_ids.first().copied()
    }
    fn change_id(&self) -> Option<but_core::ChangeId> {
        self.change_id.clone()
    }
    fn author(&self) -> &gix::actor::Signature {
        &self.author
    }
    fn message(&self) -> Cow<'_, BStr> {
        Cow::Borrowed(self.message.as_ref())
    }
}

impl RefInfo {
    /// This is a multi-stage process where we will find matching commits between
    /// the target branch and the workspace base (B)…
    /// * …and change-ids in stack commits then…
    /// * …and author and message (exact match) in stack commits…
    /// * …and (expensive) changeset-ids of
    ///     - stack commits
    ///     - content-equivalent stack prefixes, to simulate squash merges.
    ///
    /// Matches from the first two cheap stages will speed up the expensive stage, as fewer commits or combinations
    /// are left to test.
    ///
    /// If `expensive` is `true`, we will run checks that involve changeset-id computation and squash-merge trials.
    pub(crate) fn compute_similarity(
        &mut self,
        graph: &but_graph::Graph,
        repo: &gix::Repository,
        expensive: bool,
    ) -> anyhow::Result<()> {
        let topmost_target_sidx = self
            .target_ref
            .as_ref()
            .map(|t| t.segment_index)
            .or(self.target_commit.as_ref().map(|t| t.segment_index));
        let mut upstream_commits = Vec::new();
        let Some(target_tip) = topmost_target_sidx else {
            // Without any notion of 'target' we can't do anything here.
            self.compute_pushstatus(graph);
            return Ok(());
        };
        let lower_bound_generation = self.lower_bound.map(|sidx| graph[sidx].generation);
        graph.visit_all_segments_including_start_until(
            target_tip,
            but_graph::petgraph::Direction::Outgoing,
            |s| {
                let prune = true;
                if Some(s.id) == self.lower_bound
                    || lower_bound_generation.is_some_and(|generation| s.generation > generation)
                {
                    return prune;
                }
                for c in &s.commits {
                    upstream_commits.push(c.id);
                }
                !prune
            },
        );

        let cost_info = (
            upstream_commits.len(),
            repo.index_or_empty()?.entries().len(),
        );
        let upstream_lut = create_similarity_lut(
            repo,
            upstream_commits
                .iter()
                .filter_map(|id| but_core::Commit::from_id(id.attach(repo)).ok()),
            cost_info,
            expensive,
        )?;

        // Cheap checks to see which local commits belong to rebased remote or upstream commits.
        // We check by change-id and by author-signature + message combination.
        let mut time_used = std::time::Duration::default();
        'next_stack: for stack in &mut self.stacks {
            for segment in &mut stack.segments {
                // At first, these are all commits that aren't also available by identity as local commits.
                let remote_lut = create_similarity_lut(
                    repo,
                    segment.commits_on_remote.iter(),
                    cost_info,
                    expensive,
                )?;

                for local in segment
                    // top-to-bottom
                    .commits
                    .iter_mut()
                    .take_while(|c| is_similarity_candidate(c))
                {
                    let expensive = changeset_identifier(
                        repo,
                        expensive.then_some(&local.inner),
                        &mut time_used,
                    )?;
                    if let Some(upstream_commit_id) = lookup_similar(
                        &upstream_lut,
                        &local.inner,
                        expensive.as_ref(),
                        ChangeIdMode::Skip,
                    ) {
                        // Note that by keeping track of the upstream id, we can't abort early.
                        // Only expensive for expensive checks, so let's see.
                        local.relation = LocalCommitRelation::Integrated(*upstream_commit_id);
                    } else if let Some(remote_commit_id) = lookup_similar(
                        &remote_lut,
                        &local.inner,
                        expensive.as_ref(),
                        ChangeIdMode::Use,
                    ) {
                        local.relation = LocalCommitRelation::LocalAndRemote(*remote_commit_id);
                    }
                }

                segment.commits_on_remote.retain(|rc| {
                    let is_used_in_local_commits = segment.commits.iter().any(|c| {
                        matches!(c.relation,  LocalCommitRelation::LocalAndRemote(rid)| LocalCommitRelation::Integrated(rid)
                                              if rid == rc.id)
                    });
                    !is_used_in_local_commits
                        // It shouldn't be integrated (by rebase) either.
                        && lookup_similar(&upstream_lut, rc,
                                          changeset_identifier(repo, expensive.then_some(rc), &mut time_used).ok().flatten().as_ref(),
                                          ChangeIdMode::Skip).is_none()
                });
            }

            if !expensive {
                continue 'next_stack;
            }

            // Test prefixes from the base upwards, both within each named segment and across the
            // linear stack. The segment-local identity must be non-empty even for a stack-wide
            // match, so a live upper segment whose changes cancel out cannot borrow landed content
            // from below.
            let eligible = |segment: &crate::ref_info::Segment| {
                segment.commits_outside.is_none()
                    && segment
                        .ref_info
                        .as_ref()
                        .and_then(|info| info.ref_name.category())
                        == Some(gix::refs::Category::LocalBranch)
                    && segment
                        .commits
                        .iter()
                        .all(|commit| commit.parent_ids.len() == 1)
            };
            let stack_base = stack.segments.last().and_then(|segment| segment.base);
            for segment_idx in (0..stack.segments.len()).rev() {
                let (_, current_and_lower) = stack.segments.split_at_mut(segment_idx);
                let Some((segment, lower_segments)) = current_and_lower.split_first_mut() else {
                    continue;
                };
                let Some(base_commit_id) = segment.base else {
                    continue;
                };
                if !eligible(segment)
                    || segment.commits.last().is_some_and(|commit| {
                        matches!(commit.relation, LocalCommitRelation::Integrated(_))
                    })
                {
                    continue;
                }
                let stack_range_eligible = lower_segments.iter().all(&eligible);
                let mut matched = None;
                for (boundary, commit) in segment.commits.iter().enumerate().rev() {
                    let Some(segment_id) = range_changeset_identifier(
                        repo,
                        Some(base_commit_id),
                        commit.id,
                        &mut time_used,
                    )?
                    else {
                        continue;
                    };
                    let local_match = upstream_lut.get(&segment_id).copied();
                    let stack_match = if local_match.is_none()
                        && stack_base.is_some_and(|stack_base| stack_base != base_commit_id)
                        && stack_range_eligible
                    {
                        range_changeset_identifier(repo, stack_base, commit.id, &mut time_used)?
                            .and_then(|id| upstream_lut.get(&id).copied())
                    } else {
                        None
                    };
                    if let Some(squashed_commit_id) = local_match.or(stack_match) {
                        matched = Some((boundary, squashed_commit_id, stack_match.is_some()));
                        break;
                    }
                }

                let Some((boundary, squashed_commit_id, crosses_segments)) = matched else {
                    continue;
                };

                let (_, suffix) = segment.commits.split_at_mut(boundary);
                for commit in suffix {
                    commit.relation = LocalCommitRelation::Integrated(squashed_commit_id)
                }
                if crosses_segments {
                    for segment in lower_segments {
                        for commit in &mut segment.commits {
                            commit.relation = LocalCommitRelation::Integrated(squashed_commit_id)
                        }
                    }
                }
            }
        }
        self.compute_pushstatus(graph);
        Ok(())
    }

    /// Recalculate everything that depends on these values and the exact set of remote commits.
    fn compute_pushstatus(&mut self, graph: &but_graph::Graph) {
        for segment in self
            .stacks
            .iter_mut()
            .flat_map(|stack| stack.segments.iter_mut())
        {
            segment.push_status = derive_push_status_from_graph(graph, segment);
        }
    }
}

/// Derive the push-status from the first-parent relationship between a local
/// segment and its remote-tracking branch segment.
///
/// We intentionally reason in terms of the branch line, not arbitrary
/// all-parents reachability:
///
/// - stack segments are themselves built from a first-parent walk
/// - fast-forward vs force-push depends on whether one tip is contained in
///   the other's branch line
/// - merge-side ancestry is too permissive here, as it would make a remote
///   tip merged into target look "behind" instead of "rewritten"
///
/// The cases handled below are:
///
/// - no remote configured: `CompletelyUnpushed`
/// - top local commit already known integrated by similarity checks:
///   `Integrated`
/// - local and remote tips are identical: `NothingToPush`
/// - remote tip is on the local first-parent line: usually
///   `UnpushedCommits`, unless this segment already contains an integrated
///   commit below a local tip, which indicates that advancing the remote
///   would rewrite a branch state that was already merged
/// - otherwise, either the remote is ahead of us on its branch line or the
///   two tips diverged; both cases require force-push
fn derive_push_status_from_graph(
    graph: &but_graph::Graph,
    segment: &crate::ref_info::Segment,
) -> PushStatus {
    let Some(remote_segment_id) = segment.remote_tracking_branch_segment_id else {
        // Generally, don't do anything if no remote relationship is set up (anymore).
        // There may be better ways to deal with this.
        return PushStatus::CompletelyUnpushed;
    };

    if segment
        .commits
        .first()
        .is_some_and(|commit| matches!(commit.relation, LocalCommitRelation::Integrated(_)))
    {
        return PushStatus::Integrated;
    }

    let local_segment_id = segment.id;
    let Some(local_tip_id) = graph
        .tip_skip_empty(local_segment_id)
        .map(|commit| commit.id)
    else {
        return PushStatus::NothingToPush;
    };
    let Some(remote_tip_id) = graph
        .tip_skip_empty(remote_segment_id)
        .map(|commit| commit.id)
    else {
        // A missing remote tip acts like an unpushed branch: there is a
        // remote configured, but nothing reachable on that side that could
        // block a normal push.
        return PushStatus::UnpushedCommits;
    };

    let first_commit_is_local = segment
        .commits
        .first()
        .is_some_and(|commit| matches!(commit.relation, LocalCommitRelation::LocalOnly));
    let has_integrated_commit_in_segment = segment
        .commits
        .iter()
        .any(|commit| matches!(commit.relation, LocalCommitRelation::Integrated(_)));

    if local_tip_id == remote_tip_id {
        // Same tip, regardless of how the graph was segmented.
        PushStatus::NothingToPush
    } else if first_parent_contains_commit(graph, local_segment_id, remote_tip_id) {
        // Local is a straightforward first-parent extension of remote.
        // However, if this segment already contains an integrated commit
        // below a local tip, we preserve the previous behavior and treat it
        // as a force-push case. This covers the "remote behind after a
        // no-ff merge into target" scenario, while avoiding false
        // positives for integrated ancestors that live in lower segments of
        // the stack.
        if first_commit_is_local && has_integrated_commit_in_segment {
            PushStatus::UnpushedCommitsRequiringForce
        } else {
            PushStatus::UnpushedCommits
        }
    } else {
        // If the remote tip isn't on our first-parent line, then a normal
        // push cannot advance it. That covers both "remote is ahead" and
        // "local/remote diverged", and both require force-push.
        PushStatus::UnpushedCommitsRequiringForce
    }
}

/// Return `true` if `sought_commit_id` occurs on the first-parent branch line
/// of `start_segment_id`.
///
/// This is stricter than an all-parents reachability test on purpose:
///
/// - a merge can make a commit reachable without making it part of the branch's
///   own line
/// - pushability is about whether one branch tip can advance another branch tip
///   without rewriting that line
/// - therefore "reachable somewhere in history" is not the right predicate for
///   `ahead/behind` here
fn first_parent_contains_commit(
    graph: &but_graph::Graph,
    start_segment_id: but_graph::SegmentIndex,
    sought_commit_id: gix::ObjectId,
) -> bool {
    let mut found = false;
    if graph[start_segment_id]
        .commits
        .iter()
        .any(|commit| commit.id == sought_commit_id)
    {
        return true;
    }
    graph.visit_segments_downward_along_first_parent_exclude_start(start_segment_id, |segment| {
        found = segment
            .commits
            .iter()
            .any(|commit| commit.id == sought_commit_id);
        found
    });
    found
}

fn is_similarity_candidate(commit: &crate::ref_info::LocalCommit) -> bool {
    matches!(
        commit.relation,
        // This happens when the identity match with the remote didn't work.
        LocalCommitRelation::LocalOnly |
        // This would be expected to be a remote-match by identity (we don't check for this),
        // something that is determined during graph traversal time. But we want to see
        // if any of these is also integrated.
        LocalCommitRelation::LocalAndRemote(_)
    )
}
