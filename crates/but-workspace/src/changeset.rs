//! A changeset is everything that changed between two trees, and as such is nothing else than Vec<[`TreeChange`]>.
//! Changesets can have IDs which uniquely identify a set of changes, independently of which trees it originated from.
//!
//! This property allows changeset IDs to be used to determine if two different commits, or sets of commits,
//! represent the same change.

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet, VecDeque, hash_map::Entry},
    ops::Deref,
    time::Duration,
};

use bstr::{BStr, BString, ByteSlice, ByteVec};
use but_core::{ChangeState, commit::TreeKind};
use gix::{
    diff::tree::{Visit, visit},
    object::tree::EntryKind,
    prelude::ObjectIdExt,
};

use crate::{
    RefInfo,
    ref_info::{LocalCommit, LocalCommitRelation},
    ui::PushStatus,
};

/// The ID of a changeset, calculated as Git hash for convenience.
type ChangesetID = gix::ObjectId;
/// A hash for select data in a commit to avoid copying it.
type CommitDataId = gix::ObjectId;

/// The version number for the changeset ID
enum ChangesetVersion {
    /// The initial version
    V1 = 1,
}

const CURRENT_VERSION: ChangesetVersion = ChangesetVersion::V1;

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
            .or(self.extra_target);
        let mut upstream_commits = Vec::new();
        let Some(target_tip) = topmost_target_sidx else {
            // Without any notion of 'target' we can't do anything here.
            self.compute_pushstatus();
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
            upstream_commits.iter().filter_map(|id| {
                but_core::Commit::from_id(id.attach(repo))
                    .map(crate::ref_info::Commit::from)
                    .ok()
            }),
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
                    .take_while(|c| {
                        matches!(
                            c.relation,
                            // This happens when the identity match with the remote didn't work.
                            LocalCommitRelation::LocalOnly |
                            // This would be expected to be a remote-match by identity (we don't check for this),
                            // something that is determined during graph traversal time. But we want to see
                            // if any of these is also integrated.
                            LocalCommitRelation::LocalAndRemote(_)
                        )
                    })
                {
                    let expensive =
                        changeset_identifier(repo, expensive.then_some(local), &mut time_used)?;
                    if let Some(upstream_commit_id) =
                        lookup_similar(&upstream_lut, local, expensive.as_ref(), ChangeId::Skip)
                    {
                        // Note that by keeping track of the upstream id, we can't abort early.
                        // Only expensive for expensive checks, so let's see.
                        local.relation = LocalCommitRelation::Integrated(*upstream_commit_id);
                    } else if let Some(remote_commit_id) =
                        lookup_similar(&remote_lut, local, expensive.as_ref(), ChangeId::Use)
                    {
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
                                          ChangeId::Skip).is_none()
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
                let Some(topmost_unintegrated_commit) = segment
                    .commits
                    .first()
                    .filter(|c| !matches!(c.relation, LocalCommitRelation::Integrated(_)))
                else {
                    continue;
                };
                let Some(changeset_id) =
                    id_for_tree_diff(repo, base_commit_id, topmost_unintegrated_commit.tree_id)?
                else {
                    continue;
                };

                let identity_of_tip_to_base = Identifier::ChangesetId(changeset_id);
                let Some(squashed_commit_id) = upstream_lut.get(&identity_of_tip_to_base).cloned()
                else {
                    continue;
                };

                for segment in Some(segment).into_iter().chain(segments) {
                    for commit in &mut segment.commits {
                        commit.relation = LocalCommitRelation::Integrated(squashed_commit_id)
                    }
                }
                break;
            }
        }
        self.compute_pushstatus();
        Ok(())
    }

    /// Recalculate everything that depends on these values and the exact set of remote commits.
    fn compute_pushstatus(&mut self) {
        for segment in self
            .stacks
            .iter_mut()
            .flat_map(|stack| stack.segments.iter_mut())
        {
            segment.push_status = PushStatus::derive_from_commits(
                segment.remote_tracking_ref_name.is_some(),
                &segment.commits,
                !segment.commits_on_remote.is_empty(),
            );
        }
    }
}

impl PushStatus {
    /// Derive the push-status by looking at commits in the local and remote tracking branches.
    /// TODO: tests
    ///       * generally this doesn't currently handle advanced (and possibly fast-forwardable)
    ///         remotes very well. It doesn't feel like it can be expressed.
    ///       * It doesn't deal with diverged local/remote branches.
    ///       * Special cases of remote is merged, and remote tracking branch is deleted after fetch
    ///         if it was deleted on the remote?
    fn derive_from_commits(
        has_remote_tracking_ref: bool,
        commits: &[LocalCommit],
        remote_has_commits: bool,
    ) -> Self {
        if !has_remote_tracking_ref {
            // Generally, don't do anything if no remote relationship is set up (anymore).
            // There may be better ways to deal with this.
            return PushStatus::CompletelyUnpushed;
        }

        let first_commit = commits.first();
        let everything_integrated_locally =
            first_commit.is_some_and(|c| matches!(c.relation, LocalCommitRelation::Integrated(_)));
        let first_commit_is_local =
            first_commit.is_some_and(|c| matches!(c.relation, LocalCommitRelation::LocalOnly));
        if everything_integrated_locally {
            PushStatus::Integrated
        } else if commits.iter().any(|c| {
            matches!(c.relation, LocalCommitRelation::LocalAndRemote(id) if c.id != id)
                || (first_commit_is_local
                    && matches!(c.relation, LocalCommitRelation::Integrated(_)))
        }) {
            PushStatus::UnpushedCommitsRequiringForce
        } else if remote_has_commits {
            // If there are remote commits, pushing would require a force push, as the remote-only
            // commits would be overwritten.
            PushStatus::UnpushedCommitsRequiringForce
        } else if first_commit_is_local {
            PushStatus::UnpushedCommits
        } else {
            PushStatus::NothingToPush
        }
    }
}

fn changeset_identifier(
    repo: &gix::Repository,
    commit: Option<&crate::ref_info::Commit>,
    elapsed: &mut Duration,
) -> anyhow::Result<Option<Identifier>> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    if *elapsed > MAX_COMPUTE_WALLCLOCK_DURATION {
        return Ok(None);
    }

    let start = std::time::Instant::now();
    let res = id_for_tree_diff(repo, commit.parent_ids.first().cloned(), commit.id)?;
    *elapsed += start.elapsed();

    if *elapsed > MAX_COMPUTE_WALLCLOCK_DURATION {
        tracing::warn!(
            "Stopping expensive main-thread changeset computation after {}s - integration checks may not be correct",
            elapsed.as_secs()
        );
    }
    Ok(res.map(Identifier::ChangesetId))
}

enum ChangeId {
    /// ChangeIDs should be used for remotes, where we can always
    /// push changes back and see commits as containers
    Use,
    /// We'd want to skip the change-ids for integrated commits,
    /// where we go with changeset ids instead (computed).
    Skip,
}

fn lookup_similar<'a>(
    map: &'a Identity,
    commit: &crate::ref_info::Commit,
    expensive: Option<&Identifier>,
    change_id: ChangeId,
) -> Option<&'a gix::ObjectId> {
    commit
        .change_id
        .as_ref()
        .filter(|_| matches!(change_id, ChangeId::Use))
        .and_then(|cid| map.get(&Identifier::ChangeId(*cid)))
        .or_else(|| commit_data_id(commit).ok().and_then(|id| map.get(&id)))
        .or_else(|| map.get(expensive?))
}

/// Returns the fully-loaded commits suitable to be passed to UI, to have better reuse.
fn create_similarity_lut(
    repo: &gix::Repository,
    commits: impl Iterator<Item = impl Borrow<crate::ref_info::Commit>>,
    (max_commits, num_tracked_files): (usize, usize),
    expensive: bool,
) -> anyhow::Result<Identity> {
    // experimental modern CPU perf, based on 120 diffs/s at 90k entries
    // Make this smaller to get more threads even with lower amounts of work.
    const CPU_PERF: usize = 10_000_000 / 5 /* start parallelizing earlier */;
    let aproximate_cpu_seconds = (max_commits * num_tracked_files) / CPU_PERF;
    let num_threads = aproximate_cpu_seconds
        .max(1)
        .min(std::thread::available_parallelism()?.get());

    let mut similarity_lut = HashMap::<Identifier, gix::ObjectId>::new();
    let mut ambiguous_commits = HashSet::<Identifier>::new();

    let mut insert_or_expell_ambiguous = |k: Identifier, v: gix::ObjectId| {
        if ambiguous_commits.contains(&k) {
            return;
        }
        match similarity_lut.entry(k) {
            Entry::Occupied(ambiguous) => {
                if matches!(ambiguous.key(), Identifier::ChangesetId(_)) {
                    // the most expensive option should never be ambiguous (which can happen with merges),
                    // so just keep the (typically top-most/first) commit with a changeset ID instead.
                    return;
                }
                ambiguous_commits.insert(*ambiguous.key());
                ambiguous.remove();
            }
            Entry::Vacant(entry) => {
                entry.insert(v);
            }
        }
    };

    if num_threads <= 1 || !expensive {
        let mut expensive = expensive.then(std::time::Instant::now);
        for (idx, commit) in commits.enumerate() {
            let commit = commit.borrow();
            if let Some(change_id) = &commit.change_id {
                insert_or_expell_ambiguous(Identifier::ChangeId(*change_id), commit.id);
            }
            insert_or_expell_ambiguous(commit_data_id(commit)?, commit.id);

            if let Some(start) = expensive {
                let Some(changeset_id) =
                    id_for_tree_diff(repo, commit.parent_ids.first().cloned(), commit.id)?
                else {
                    continue;
                };
                insert_or_expell_ambiguous(Identifier::ChangesetId(changeset_id), commit.id);

                if should_stop(start, idx) {
                    expensive = None;
                }
            }
        }
    } else {
        let (in_tx, out_rx) = {
            let (in_tx, in_rx) = flume::unbounded();
            let (out_tx, out_rx) = flume::unbounded();
            for tid in 0..num_threads {
                std::thread::Builder::new()
                    .name(format!("GitButler::compute-changeset({tid})"))
                    .spawn({
                        let in_rx = in_rx.clone();
                        let out_tx = out_tx.clone();
                        let repo = repo.clone().into_sync();
                        move || -> anyhow::Result<()> {
                            let mut repo = repo.to_thread_local();
                            repo.object_cache_size_if_unset(
                                repo.compute_object_cache_size_for_tree_diffs(
                                    &*repo.index_or_empty()?,
                                ),
                            );
                            for (idx, lhs, rhs) in in_rx {
                                let res = id_for_tree_diff(&repo, lhs, rhs)
                                    .map(|opt| opt.map(|cs_id| (idx, cs_id, rhs)));
                                if out_tx.send(res).is_err() {
                                    break;
                                }
                            }
                            Ok(())
                        }
                    })?;
            }
            (in_tx, out_rx)
        };

        assert!(
            expensive,
            "BUG: multi-threading is only for expensive checks"
        );
        for (idx, commit) in commits.enumerate() {
            let commit = commit.borrow();
            if let Some(change_id) = &commit.change_id {
                insert_or_expell_ambiguous(Identifier::ChangeId(*change_id), commit.id);
            }
            insert_or_expell_ambiguous(commit_data_id(commit)?, commit.id);

            in_tx
                .send((idx, commit.parent_ids.first().cloned(), commit.id))
                .ok();
        }
        drop(in_tx);

        let start = std::time::Instant::now();
        let mut max_idx = 0;
        for res in out_rx {
            let Some((idx, changeset_id, commit_id)) = res? else {
                continue;
            };

            insert_or_expell_ambiguous(Identifier::ChangesetId(changeset_id), commit_id);

            max_idx = max_idx.max(idx);
            if should_stop(start, max_idx) {
                break;
            }
        }
    }

    Ok(similarity_lut)
}

/// The total amount of time we should be able to use for expensive changeset computation.
const MAX_COMPUTE_WALLCLOCK_DURATION: std::time::Duration = std::time::Duration::from_secs(1);

fn should_stop(start: std::time::Instant, commit_idx: usize) -> bool {
    let out_of_time = start.elapsed() > MAX_COMPUTE_WALLCLOCK_DURATION;
    if out_of_time {
        tracing::warn!(
            "Stopping expensive changeset computation after {}s and {commit_idx} diffs computed ({throughput:02} diffs/s)",
            MAX_COMPUTE_WALLCLOCK_DURATION.as_secs(),
            throughput = commit_idx as f32 / start.elapsed().as_secs_f32(),
        );
    }
    out_of_time
}

/// Produce a changeset ID to represent the changes between `lhs` and `rhs`, where `lhs` is
/// the previous version of the treeish, and `rhs` is the current version of that treeish.
/// We use the [`CURRENT_VERSION`], which must be considered when handling persisted values.
fn id_for_tree_diff(
    repo: &gix::Repository,
    lhs: Option<gix::ObjectId>,
    rhs: gix::ObjectId,
) -> anyhow::Result<Option<ChangesetID>> {
    let lhs_tree = lhs.map(|id| id_to_tree(repo, id)).transpose()?;
    let rhs_tree = id_to_tree(repo, rhs)?;

    let no_changes = lhs_tree
        .as_ref()
        .map_or(rhs_tree.id.is_empty_tree(), |lhs_tree| {
            lhs_tree.id == rhs_tree.id
        });
    if no_changes {
        return Ok(None);
    }

    #[derive(Default)]
    struct Delegate {
        path_deque: VecDeque<BString>,
        path: BString,
        hash: Option<gix::hash::Hasher>,
    }

    impl Delegate {
        fn pop_element(&mut self) {
            if let Some(pos) = self.path.rfind_byte(b'/') {
                self.path.resize(pos, 0);
            } else {
                self.path.clear();
            }
        }

        fn push_element(&mut self, name: &BStr) {
            if name.is_empty() {
                return;
            }
            if !self.path.is_empty() {
                self.path.push(b'/');
            }
            self.path.push_str(name);
        }
    }

    impl Visit for Delegate {
        fn pop_front_tracked_path_and_set_current(&mut self) {
            self.path = self
                .path_deque
                .pop_front()
                .expect("every parent is set only once");
        }

        fn push_back_tracked_path_component(&mut self, component: &BStr) {
            self.push_element(component);
            self.path_deque.push_back(self.path.clone());
        }

        fn push_path_component(&mut self, component: &BStr) {
            self.push_element(component);
        }

        fn pop_path_component(&mut self) {
            self.pop_element();
        }

        fn visit(&mut self, change: visit::Change) -> visit::Action {
            let hash = self.hash.get_or_insert_with(|| {
                let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
                hash.update(&[CURRENT_VERSION as u8]);
                hash
            });

            if change.entry_mode().is_tree() {
                return visit::Action::Continue;
            }

            // must hash all fields, even if None for unambiguous hashes.
            hash.update(self.path.as_ref());
            match change {
                visit::Change::Addition {
                    entry_mode, oid, ..
                } => {
                    hash.update(b"A");
                    hash_change_state(
                        hash,
                        ChangeState {
                            id: oid,
                            kind: entry_mode.kind(),
                        },
                    )
                }
                visit::Change::Deletion {
                    entry_mode, oid, ..
                } => {
                    hash.update(b"D");
                    hash_change_state(
                        hash,
                        ChangeState {
                            id: oid,
                            kind: entry_mode.kind(),
                        },
                    );
                }
                visit::Change::Modification {
                    previous_entry_mode,
                    previous_oid,
                    entry_mode,
                    oid,
                    ..
                } => {
                    hash.update(b"M");
                    hash_change_state(
                        hash,
                        ChangeState {
                            id: previous_oid,
                            kind: previous_entry_mode.kind(),
                        },
                    );
                    hash_change_state(
                        hash,
                        ChangeState {
                            id: oid,
                            kind: entry_mode.kind(),
                        },
                    );
                }
            }
            visit::Action::Continue
        }
    }

    let empty_tree = repo.empty_tree();
    let mut state = Default::default();
    let mut delegate = Delegate::default();
    gix::diff::tree(
        gix::objs::TreeRefIter::from_bytes(&lhs_tree.unwrap_or(empty_tree).data),
        gix::objs::TreeRefIter::from_bytes(&rhs_tree.data),
        &mut state,
        repo.objects.deref(),
        &mut delegate,
    )?;
    let Some(hasher) = delegate.hash.take() else {
        return Ok(None);
    };
    Ok(Some(hasher.try_finalize()?))
}

// TODO: use `peel_to_tree()` once special conflict commits aren't needed anymore.
fn id_to_tree(repo: &gix::Repository, id: gix::ObjectId) -> anyhow::Result<gix::Tree<'_>> {
    let object = repo.find_object(id)?;
    if object.kind == gix::object::Kind::Commit {
        let commit = but_core::Commit::from_id(object.peel_to_commit()?.id())?;
        let tree = commit.tree_id_or_kind(TreeKind::AutoResolution)?;
        let tree = repo.find_tree(tree)?;
        Ok(tree)
    } else {
        Ok(object.peel_to_tree()?)
    }
}

fn hash_change_state(h: &mut gix::hash::Hasher, ChangeState { id, kind }: ChangeState) {
    h.update(id.as_slice());
    h.update(&[match kind {
        EntryKind::Tree => b'T',
        EntryKind::Blob => b'B',
        EntryKind::BlobExecutable => b'E',
        EntryKind::Link => b'L',
        EntryKind::Commit => b'C',
    }]);
}

#[derive(Debug, Hash, Clone, Copy, Eq, PartialEq)]
enum Identifier {
    ChangeId(but_core::commit::ChangeId),
    CommitData(CommitDataId),
    ChangesetId(ChangesetID),
}

fn commit_data_id(c: &crate::ref_info::Commit) -> anyhow::Result<Identifier> {
    let mut hasher = gix::hash::hasher(gix::hash::Kind::Sha1);

    let gix::actor::Signature {
        name,
        email,
        time:
            gix::date::Time {
                seconds,
                // The offset doesn't change the time in absolute terms,
                // for we consider it for completeness.
                // Real rebases wouldn't touch it.
                offset,
            },
    } = &c.author;
    hasher.update(b"N");
    hasher.update(name.as_slice());
    hasher.update(b"E");
    hasher.update(email.as_slice());
    hasher.update(b"T");
    hasher.update(&seconds.to_le_bytes());
    hasher.update(b"TO");
    hasher.update(&offset.to_le_bytes());

    hasher.update(b"M");
    hasher.update(c.message.as_slice());

    Ok(Identifier::CommitData(hasher.try_finalize()?))
}

type Identity = HashMap<Identifier, gix::ObjectId>;
