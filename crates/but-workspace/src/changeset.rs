//! A changeset is everything that changed between two trees, and as such is nothing else than Vec<[`TreeChange`]>.
//! Changesets can have IDs which uniquely identify a set of changes, independently of which trees it originated from.
//!
//! This property allows changeset IDs to be used to determine if two different commits, or sets of commits,
//! represent the same change.

use std::borrow::Cow;

use bstr::BStr;
use but_core::changeset::{
    ChangeIdMode, ChangesetCommit, Identifier, changeset_identifier, create_similarity_lut,
    id_for_tree_diff, id_to_tree, lookup_similar,
};
use gix::prelude::ObjectIdExt;

use crate::{
    RefInfo,
    ref_info::{Commit, LocalCommit, LocalCommitRelation},
    ui::PushStatus,
};

// The by-ids similarity entry point now lives in `but-core`; re-exported so existing
// `crate::changeset::compute_similarity_by_commit_ids` callers stay unchanged.
pub(crate) use but_core::changeset::compute_similarity_by_commit_ids;

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
    ///     - the squash-merge between all stack-tips (ST) and the target branch to simulate squash merges
    ///       as they could have happened.
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

            // Another round from top to bottom where we take remote and local tips of non-integrated commits
            // and test-squash-merge them (cleanly), to see if that changeset ID is contained in upstream.
            // If so, the whole branch everything that follows is bluntly considered integrated, as it probably is
            // most of the time.
            let base_commit_id = stack.segments.last().and_then(|s| s.base);
            let mut segments = stack.segments.iter_mut();
            while let Some(segment) = segments.next() {
                // Find the topmost commit that isn't already integrated and carries changes of its
                // own; that is the integration boundary for the squash-merge trial. Commits above it
                // are either already integrated or introduce no changes of their own, so the trial
                // must not mark them: otherwise a no-change commit at the tip would be treated as
                // merged - because it borrows the cumulative content of the commits below it - and
                // its branch deleted.
                let Some((boundary, boundary_tree_id)) =
                    segment.commits.iter().enumerate().find_map(|(i, c)| {
                        let carries_changes =
                            !matches!(c.relation, LocalCommitRelation::Integrated(_))
                                && commit_introduces_changes(repo, c);
                        carries_changes.then_some((i, c.tree_id))
                    })
                else {
                    continue;
                };
                let Some(changeset_id) = id_for_tree_diff(repo, base_commit_id, boundary_tree_id)?
                else {
                    continue;
                };

                let identity_of_tip_to_base = Identifier::ChangesetId(changeset_id);
                let Some(squashed_commit_id) = upstream_lut.get(&identity_of_tip_to_base).cloned()
                else {
                    continue;
                };

                // Mark the boundary commit and everything below it (down to the base, across the
                // lower segments) as integrated; leave the commits above the boundary untouched.
                for (i, segment) in Some(segment).into_iter().chain(segments).enumerate() {
                    let skip = if i == 0 { boundary } else { 0 };
                    for commit in segment.commits.iter_mut().skip(skip) {
                        commit.relation = LocalCommitRelation::Integrated(squashed_commit_id)
                    }
                }
                break;
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

/// Whether `commit` introduces changes of its own, i.e. its tree differs from its first
/// parent's tree. This only compares tree ids and skips the full diff that [`id_for_tree_diff`]
/// computes, which matters when scanning many commits. On a lookup failure we assume the commit
/// carries changes, so a genuinely merged branch is never spared from squash-merge detection.
fn commit_introduces_changes(repo: &gix::Repository, commit: &LocalCommit) -> bool {
    match commit.parent_ids.first() {
        None => !commit.tree_id.is_empty_tree(),
        Some(parent) => id_to_tree(repo, *parent)
            .map(|parent_tree| parent_tree.id != commit.tree_id)
            .unwrap_or(true),
    }
}
