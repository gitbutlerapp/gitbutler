//! A changeset is everything that changed between two trees, and as such is nothing else than Vec<[`TreeChange`]>.
//! Changesets can have IDs which uniquely identify a set of changes, independently of which trees it originated from.
//!
//! This property allows changeset IDs to be used to determine if two different commits, or sets of commits,
//! represent the same change.

use std::{
    borrow::Borrow,
    collections::{HashMap, HashSet, hash_map::Entry},
};

use crate::{
    RefInfo,
    ref_info::{
        ui,
        ui::{LocalCommit, LocalCommitRelation},
    },
    ui::PushStatus,
};
use bstr::BString;
use but_core::{ChangeState, TreeChange, TreeStatus, commit::TreeKind};
use gix::{ObjectId, Repository, object::tree::EntryKind, prelude::ObjectIdExt};
use itertools::Itertools;

/// The ID of a changeset, calculated as Git hash for convenience.
type ChangesetID = gix::ObjectId;

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
    ///     - the squash-merge between all remote stack tips and the target branch
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
            .target
            .as_ref()
            .map(|t| t.segment_index)
            .into_iter()
            .chain(self.extra_target)
            .sorted_by_key(|sidx| graph[*sidx].generation)
            .next();
        let mut upstream_commits = Vec::new();
        let Some(target_tip) = topmost_target_sidx else {
            // Without any notion of 'target' we can't do anything here.
            self.compute_pushstatus();
            return Ok(());
        };
        graph.visit_all_segments_until(target_tip, but_graph::petgraph::Direction::Outgoing, |s| {
            let prune = true;
            if Some(s.id) == self.lower_bound {
                return prune;
            }
            for c in &s.commits {
                upstream_commits.push(c.id);
            }
            !prune
        });

        let upstream_lut = create_similarity_lut(
            repo,
            upstream_commits.iter().filter_map(|id| {
                but_core::Commit::from_id(id.attach(repo))
                    .map(ui::Commit::from)
                    .ok()
            }),
            expensive,
        )?;

        // Cheap checks to see which local commits belong to rebased remote or upstream commits.
        // We check by change-id and by author-signature + message combination.
        'next_stack: for stack in &mut self.stacks {
            for segment in &mut stack.segments {
                // At first, these are all commits that aren't also available by identity as local commits.
                let remote_lut =
                    create_similarity_lut(repo, segment.commits_on_remote.iter(), expensive)?;

                for local in segment
                    // top-to-bottom
                    .commits
                    .iter_mut()
                    .take_while(|c| c.relation == LocalCommitRelation::LocalOnly)
                {
                    let expensive = changeset_identifier(repo, expensive.then_some(local))?;
                    if let Some(upstream_commit_id) =
                        lookup_similar(&upstream_lut, local, expensive.as_ref())
                    {
                        // Note that by keeping track of the upstream id, we can't abort early.
                        // Only expensive for expensive checks, so let's see.
                        local.relation = LocalCommitRelation::Integrated(*upstream_commit_id);
                    } else if let Some(remote_commit_id) =
                        lookup_similar(&remote_lut, local, expensive.as_ref())
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
                        // TODO: test, also: what about simple merges?
                        //       This would make them integrated by flag, do we pick that up?
                        && lookup_similar(&upstream_lut, rc, None).is_none()
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
        } else if first_commit_is_local {
            if remote_has_commits {
                PushStatus::UnpushedCommitsRequiringForce
            } else {
                PushStatus::UnpushedCommits
            }
        } else {
            PushStatus::NothingToPush
        }
    }
}

fn changeset_identifier(
    repo: &gix::Repository,
    commit: Option<&ui::Commit>,
) -> anyhow::Result<Option<Identifier>> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    Ok(
        id_for_tree_diff(repo, commit.parent_ids.first().cloned(), commit.id)?
            .map(Identifier::ChangesetId),
    )
}

fn lookup_similar<'a>(
    map: &'a Identity,
    commit: &ui::Commit,
    expensive: Option<&Identifier>,
) -> Option<&'a gix::ObjectId> {
    commit
        .change_id
        .as_ref()
        .and_then(|cid| map.get(&Identifier::ChangeId(cid.clone())))
        .or_else(|| {
            map.get(&Identifier::CommitData {
                author: commit.author.clone().into(),
                message: commit.message.clone(),
            })
        })
        .or_else(|| map.get(expensive?))
}

/// Returns the fully-loaded commits suitable to be passed to UI, to have better re-use.
fn create_similarity_lut(
    repo: &Repository,
    commits: impl Iterator<Item = impl Borrow<ui::Commit>>,
    expensive: bool,
) -> anyhow::Result<Identity> {
    let mut similarity_lut = HashMap::<Identifier, gix::ObjectId>::new();
    {
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
                    ambiguous_commits.insert(ambiguous.key().clone());
                    ambiguous.remove();
                }
                Entry::Vacant(entry) => {
                    entry.insert(v);
                }
            }
        };
        for commit in commits {
            let commit = commit.borrow();
            if let Some(change_id) = &commit.change_id {
                insert_or_expell_ambiguous(Identifier::ChangeId(change_id.clone()), commit.id);
            }
            insert_or_expell_ambiguous(
                Identifier::CommitData {
                    author: commit.author.clone().into(),
                    message: commit.message.clone(),
                },
                commit.id,
            );
            if expensive {
                let Some(changeset_id) =
                    id_for_tree_diff(repo, commit.parent_ids.first().cloned(), commit.id)?
                else {
                    continue;
                };
                insert_or_expell_ambiguous(Identifier::ChangesetId(changeset_id), commit.id);
            }
        }
    }
    Ok(similarity_lut)
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
    // TODO(perf): use plumbing directly to avoid resource-cache overhead.
    //             consider parallelization
    //             really needs caching to be practical, in-memory might suffice for now.
    let changes = repo.diff_tree_to_tree(
        lhs_tree.as_ref(),
        &rhs_tree,
        *gix::diff::Options::default()
            .track_path()
            // Rewrite tracking isn't needed for unique IDs and doesn't alter the validity,
            // but would cost time, making it useless.
            .track_rewrites(None),
    )?;
    if changes.is_empty() {
        return Ok(None);
    }

    let mut hash = gix::hash::hasher(gix::hash::Kind::Sha1);
    hash.update(&[CURRENT_VERSION as u8]);

    // We rely on the diff order, it's consistent as rewrites are disabled.
    for c in changes {
        if c.entry_mode().is_tree() {
            continue;
        }
        // For simplicity, use this type.
        let c = TreeChange::from(c);
        // must hash all fields, even if None for unambiguous hashes.
        hash.update(&c.path);
        match c.status {
            TreeStatus::Addition {
                state,
                // Ignore as untracked files can't happen with tree/tree diffs
                is_untracked: _,
            } => {
                hash.update(b"A");
                hash_change_state(&mut hash, state)
            }
            TreeStatus::Deletion { previous_state } => {
                hash.update(b"D");
                hash_change_state(&mut hash, previous_state)
            }
            TreeStatus::Modification {
                previous_state,
                state,
                // Ignore as it's derived
                flags: _,
            } => {
                hash.update(b"M");
                hash_change_state(&mut hash, previous_state);
                hash_change_state(&mut hash, state);
            }
            TreeStatus::Rename { .. } => {
                unreachable!("disabled in prior configuration")
            }
        }
    }
    Ok(Some(hash.try_finalize()?))
}

// TODO: use `peel_to_tree()` once special conflict markers aren't needed anymore.
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

#[derive(Debug, Hash, Clone, Eq, PartialEq)]
enum Identifier {
    ChangeId(String),
    CommitData {
        author: gix::actor::Identity,
        message: BString,
    },
    ChangesetId(ChangesetID),
}

type Identity = HashMap<Identifier, ObjectId>;
