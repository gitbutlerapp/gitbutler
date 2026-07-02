//! A changeset is everything that changed between two trees, and as such is nothing else than Vec<[`crate::TreeChange`]>.
//! Changesets can have IDs which uniquely identify a set of changes, independently of which trees it originated from.
//!
//! This property allows changeset IDs to be used to determine if two different commits, or sets of commits,
//! represent the same change.
//!
//! This module is the generic similarity engine: it works on anything implementing [`ChangesetCommit`](crate::changeset::ChangesetCommit), so both
//! the rebase Editor's commits and the richer `but-workspace` commits can reuse it without copying commit data.

use std::{
    borrow::Cow,
    collections::{HashMap, HashSet, VecDeque, hash_map::Entry},
    time::Duration,
};

use bstr::{BStr, BString, ByteSlice, ByteVec};
use gix::{
    diff::tree::{Visit, visit},
    object::tree::EntryKind,
    prelude::ObjectIdExt,
};

use crate::{ChangeId, ChangeState, Commit, commit, commit::TreeKind};

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

/// The commit data the changeset-similarity engine needs to identify a commit.
///
/// Implemented for [`crate::Commit`] (the by-id path used by the rebase Editor)
/// and, in `but-workspace`, for its richer `ref_info::Commit`, so neither side
/// has to copy commit data into an intermediate struct.
pub trait ChangesetCommit {
    /// The commit's own id.
    fn id(&self) -> gix::ObjectId;
    /// The id of the first parent, if any.
    fn first_parent_id(&self) -> Option<gix::ObjectId>;
    /// The GitButler change-id, if the commit carries one.
    fn change_id(&self) -> Option<ChangeId>;
    /// The commit's author signature.
    fn author(&self) -> &gix::actor::Signature;
    /// The commit message, with conflict markers stripped.
    fn message(&self) -> Cow<'_, BStr>;
}

impl ChangesetCommit for Commit<'_> {
    fn id(&self) -> gix::ObjectId {
        self.id.detach()
    }
    fn first_parent_id(&self) -> Option<gix::ObjectId> {
        self.inner.parents.first().copied()
    }
    fn change_id(&self) -> Option<ChangeId> {
        self.headers().and_then(|hdr| hdr.change_id)
    }
    fn author(&self) -> &gix::actor::Signature {
        &self.inner.author
    }
    fn message(&self) -> Cow<'_, BStr> {
        Cow::Owned(commit::strip_conflict_markers(self.inner.message.as_ref()))
    }
}

/// Lets the engine accept iterators of references (e.g. `Vec::iter`) as well as owned commits.
impl<T: ChangesetCommit + ?Sized> ChangesetCommit for &T {
    fn id(&self) -> gix::ObjectId {
        (**self).id()
    }
    fn first_parent_id(&self) -> Option<gix::ObjectId> {
        (**self).first_parent_id()
    }
    fn change_id(&self) -> Option<ChangeId> {
        (**self).change_id()
    }
    fn author(&self) -> &gix::actor::Signature {
        (**self).author()
    }
    fn message(&self) -> Cow<'_, BStr> {
        (**self).message()
    }
}

/// Similarity matches between workspace commits and upstream commits, computed from commit IDs.
pub struct SimilarityByCommitIds {
    /// Upstream commit IDs keyed by the workspace commit ID that matched them.
    pub matches_by_workspace_commit: HashMap<gix::ObjectId, gix::ObjectId>,
}

/// Compute upstream similarity for the provided workspace commits.
///
/// The returned matches use the same cheap and optional expensive checks as the per-segment
/// similarity used for upstream integration: change IDs are skipped, while commit data and
/// changeset IDs are considered.
pub fn compute_similarity_by_commit_ids(
    repo: &gix::Repository,
    upstream_commit_ids: &[gix::ObjectId],
    workspace_commit_ids: &[gix::ObjectId],
    expensive: bool,
) -> anyhow::Result<SimilarityByCommitIds> {
    let cost_info = (
        upstream_commit_ids.len(),
        repo.index_or_empty()?.entries().len(),
    );
    let upstream_lut = create_similarity_lut(
        repo,
        upstream_commit_ids
            .iter()
            .filter_map(|id| Commit::from_id(id.attach(repo)).ok()),
        cost_info,
        expensive,
    )?;

    let mut time_used = std::time::Duration::default();
    let mut matches_by_workspace_commit = HashMap::new();
    for workspace_commit_id in workspace_commit_ids {
        let commit = Commit::from_id(workspace_commit_id.attach(repo))?;
        let expensive = changeset_identifier(repo, expensive.then_some(&commit), &mut time_used)?;
        if let Some(upstream_commit_id) = lookup_similar(
            &upstream_lut,
            &commit,
            expensive.as_ref(),
            ChangeIdMode::Skip,
        ) {
            matches_by_workspace_commit.insert(*workspace_commit_id, *upstream_commit_id);
        }
    }

    Ok(SimilarityByCommitIds {
        matches_by_workspace_commit,
    })
}

/// Compute the changeset identifier of `commit` (the changes it introduces over its first parent),
/// bounded by the wall-clock budget tracked in `elapsed`. Returns `None` if there is no commit, the
/// budget is exhausted, or the commit introduces no changes.
pub fn changeset_identifier<C: ChangesetCommit>(
    repo: &gix::Repository,
    commit: Option<&C>,
    elapsed: &mut Duration,
) -> anyhow::Result<Option<Identifier>> {
    let Some(commit) = commit else {
        return Ok(None);
    };
    if *elapsed > MAX_COMPUTE_WALLCLOCK_DURATION {
        return Ok(None);
    }

    let start = std::time::Instant::now();
    let res = id_for_tree_diff(repo, commit.first_parent_id(), commit.id())?;
    *elapsed += start.elapsed();

    if *elapsed > MAX_COMPUTE_WALLCLOCK_DURATION {
        tracing::warn!(
            "Stopping expensive main-thread changeset computation after {}s - integration checks may not be correct",
            elapsed.as_secs()
        );
    }
    Ok(res.map(Identifier::ChangesetId))
}

/// Whether change-ids should be considered when looking up a similar commit.
pub enum ChangeIdMode {
    /// ChangeIDs should be used for remotes, where we can always
    /// push changes back and see commits as containers
    Use,
    /// We'd want to skip the change-ids for integrated commits,
    /// where we go with changeset ids instead (computed).
    Skip,
}

/// Look up the commit in `map` most similar to `commit`, trying change-id, commit data, and
/// (if provided) the `expensive` changeset id, in that order.
pub fn lookup_similar<'a, C: ChangesetCommit>(
    map: &'a Identity,
    commit: &C,
    expensive: Option<&Identifier>,
    change_id: ChangeIdMode,
) -> Option<&'a gix::ObjectId> {
    commit
        .change_id()
        .filter(|_| matches!(change_id, ChangeIdMode::Use))
        .and_then(|cid| map.get(&Identifier::ChangeId(cid)))
        .or_else(|| commit_data_id(commit).ok().and_then(|id| map.get(&id)))
        .or_else(|| map.get(expensive?))
}

/// Build a lookup table from the changes represented by `commits` to the commit id that introduced
/// them, used to detect when two commits represent the same change.
pub fn create_similarity_lut<C: ChangesetCommit>(
    repo: &gix::Repository,
    commits: impl Iterator<Item = C>,
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
                ambiguous_commits.insert(ambiguous.key().clone());
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
            if let Some(change_id) = commit.change_id() {
                insert_or_expell_ambiguous(Identifier::ChangeId(change_id), commit.id());
            }
            insert_or_expell_ambiguous(commit_data_id(&commit)?, commit.id());

            if let Some(start) = expensive {
                let Some(changeset_id) =
                    id_for_tree_diff(repo, commit.first_parent_id(), commit.id())?
                else {
                    continue;
                };
                insert_or_expell_ambiguous(Identifier::ChangesetId(changeset_id), commit.id());

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
            if let Some(change_id) = commit.change_id() {
                insert_or_expell_ambiguous(Identifier::ChangeId(change_id), commit.id());
            }
            insert_or_expell_ambiguous(commit_data_id(&commit)?, commit.id());

            in_tx
                .send((idx, commit.first_parent_id(), commit.id()))
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
/// We use the current `CURRENT_VERSION`, which must be considered when handling persisted values.
pub fn id_for_tree_diff(
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
                return std::ops::ControlFlow::Continue(());
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
            std::ops::ControlFlow::Continue(())
        }
    }

    let empty_tree = repo.empty_tree();
    let mut state = Default::default();
    let mut delegate = Delegate::default();
    gix::diff::tree(
        gix::objs::TreeRefIter::from_bytes(
            &lhs_tree.unwrap_or(empty_tree).data,
            repo.object_hash(),
        ),
        gix::objs::TreeRefIter::from_bytes(&rhs_tree.data, repo.object_hash()),
        &mut state,
        &repo.objects,
        &mut delegate,
    )?;
    let Some(hasher) = delegate.hash.take() else {
        return Ok(None);
    };
    Ok(Some(hasher.try_finalize()?))
}

// TODO: use `peel_to_tree()` once special conflict commits aren't needed anymore.
/// Resolve `id` to its tree, transparently handling GitButler conflict commits.
pub fn id_to_tree(repo: &gix::Repository, id: gix::ObjectId) -> anyhow::Result<gix::Tree<'_>> {
    let object = repo.find_object(id)?;
    if object.kind == gix::object::Kind::Commit {
        let commit = Commit::from_id(object.peel_to_commit()?.id())?;
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

/// One of the identities a commit can be matched by, from cheapest to most expensive.
#[derive(Debug, Hash, Clone, Eq, PartialEq)]
pub enum Identifier {
    /// The GitButler change-id.
    ChangeId(ChangeId),
    /// A hash over select commit data (author + message).
    CommitData(CommitDataId),
    /// The changeset id, i.e. the content of the changes the commit introduces.
    ChangesetId(ChangesetID),
}

fn commit_data_id<C: ChangesetCommit>(c: &C) -> anyhow::Result<Identifier> {
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
    } = c.author();
    hasher.update(b"N");
    hasher.update(name.as_slice());
    hasher.update(b"E");
    hasher.update(email.as_slice());
    hasher.update(b"T");
    hasher.update(&seconds.to_le_bytes());
    hasher.update(b"TO");
    hasher.update(&offset.to_le_bytes());

    hasher.update(b"M");
    // Trim trailing line endings before hashing so that messages differing
    // only in trailing newlines still match.  This is needed because
    // `strip_conflict_markers` may consume trailing newlines that the
    // original message had; the remote copy retains them.
    let message = c.message();
    let msg = message.trim_end_with(|c| c == '\n' || c == '\r');
    hasher.update(msg);

    Ok(Identifier::CommitData(hasher.try_finalize()?))
}

/// A lookup from a commit identity to the commit id that introduced it.
pub type Identity = HashMap<Identifier, gix::ObjectId>;
