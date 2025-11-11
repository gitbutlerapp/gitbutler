//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a workspace.

use crate::DiffSpec;
use anyhow::bail;
use but_core::RepositoryExt;
use but_core::ref_metadata::StackId;
use but_rebase::{RebaseOutput, commit::DateMode};
use gix::prelude::ObjectIdExt as _;

pub(crate) mod tree;
use tree::{CreateTreeOutcome, create_tree};

pub(crate) mod index;

mod hunks;
pub use hunks::apply_hunks;
pub use tree::apply_worktree_changes;

/// Types for use in the frontend with serialization support.
pub mod ui;

/// The place to apply the [change-specifications](DiffSpec) to.
///
/// Note that any commit this instance points to will be the basis to apply all changes to.
#[derive(Debug, Clone)]
pub enum Destination {
    /// Create a new commit on top of the given `parent_commit_id`, so it will be the sole parent
    /// of the newly created commit, making it its ancestor.
    NewCommit {
        /// If `None`, the base-state for the new commit will be an empty tree and the new commit will be the first one
        /// (i.e. have no parent). This is the case when `HEAD` is unborn. If `HEAD` is detached, this is a failure.
        ///
        /// To create a commit at the position of the first commit of a branch, the parent has to be the merge-base with the *target branch*.
        parent_commit_id: Option<gix::ObjectId>,
        /// The stack and reference the commit is supposed to go into. It is necessary to disambiguate the reference update.
        stack_segment: Option<StackSegmentId>,
        /// Use `message` as a commit message for the new commit.
        message: String,
    },
    /// Amend all changes to the given commit, leaving all other aspects of the commit unchanged.
    AmendCommit {
        /// The commit to use as a base to amend to. It will be rewritten, retaining its parents.
        commit_id: gix::ObjectId,
        /// If `Some()`, set the commit message as well.
        new_message: Option<String>,
    },
}

/// The stack and the branch the commit is supposed to go into.
#[derive(Debug, Clone)]
pub struct StackSegmentId {
    /// Identifies the stack the commit is destined to (without it, ambiguity is still possible, e.g. when all stacks have no commits)
    pub stack_id: StackId,
    /// The name of the ref pointing to the tip of the stack segment the commit is supposed to go into. It is necessary to disambiguate the reference update.
    pub segment_ref: gix::refs::FullName,
}

/// Identify the commit that contains the patches to be moved, along with the branch that should be rewritten.
#[derive(Debug, Clone, Copy)]
pub struct MoveSourceCommit {
    /// The commit that acts as the source of all changes. Note that these changes will be *removed* from the
    /// commit, which gets rewritten in the process.
    pub commit_id: gix::ObjectId,
    /// The commit at the *very top* of the branch which has the commit that acts as source of changes in its ancestry.
    pub branch_tip: gix::ObjectId,
}

/// The range of a hunk as denoted by a 1-based starting line, and the amount of lines from there.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HunkRange {
    /// The number of the first line in the hunk, 1 based.
    pub start: u32,
    /// The amount of lines in the range.
    ///
    /// If `0`, this is an empty hunk.
    pub lines: u32,
}

/// A type used in [`CreateCommitOutcome`] to indicate how a reference was changed so it keeps pointing
/// to the correct commit.
#[derive(Debug, PartialEq)]
pub struct UpdatedReference {
    /// The reference itself.
    // TODO: The virtual variant could contain stack-id as well, but it remains to be seen how useful this is.
    pub reference: but_core::Reference,
    /// The commit to which `reference` pointed before the update.
    pub old_commit_id: gix::ObjectId,
    /// The commit to which `reference` points now.
    pub new_commit_id: gix::ObjectId,
}

/// Additional information about the outcome of a [`create_commit()`] call.
#[derive(Debug)]
pub struct CreateCommitOutcome {
    /// Changes that were removed from a commit because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// The newly created commit, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// If `new_commit` is `Some(_)`, this field is `Some(_)` as well and denotes the base-tree + all changes.
    /// If the applied changes were from the worktree, it's `HEAD^{tree}` + changes.
    /// Otherwise, it's `<commit>^{tree}` + changes.
    pub changed_tree_pre_cherry_pick: Option<gix::ObjectId>,
    /// The rewritten references `(old, new, reference)`, along with their `old` and `new` commit location, along
    /// with the reference itself.
    /// If `new_commit` is `None`, this array will be an empty.
    pub references: Vec<UpdatedReference>,
    /// `Some(_)` if a rebase was performed.
    pub rebase_output: Option<RebaseOutput>,
    /// An index based on the existing index on disk that matches *the tree at `HEAD`*.
    /// Note that a couple of extensions that relate to paths will have been dropped to assure consistency - we don't have
    /// `unpack_trees` just yet.
    /// The index wasn't written yet, but could be to match `HEAD^{commit}`.
    pub index: Option<gix::index::File>,
}

/// Provide a description of why a [`DiffSpec`] was rejected for application to the tree of a commit.
#[derive(Default, Debug, Copy, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RejectionReason {
    /// All changes were applied, but they didn't end up effectively change the tree to something differing from the target tree.
    /// This means the changes were a no-op.
    /// Is that even possible? The code says so, for good measure.
    // We don't really have a default, this is just for convenience
    #[default]
    NoEffectiveChanges,
    /// The final cherry-pick to bring the new tree down onto the target tree (merge it in) failed with a conflict.
    CherryPickMergeConflict,
    /// The final merge of the workspace commit failed with a conflict.
    WorkspaceMergeConflict,
    /// The final merge of the workspace commit failed with a conflict,
    /// but the involved file wasn't anything the user provided as diff-spec.
    WorkspaceMergeConflictOfUnrelatedFile,
    /// This is just a theoretical possibility that *could* happen if somebody deletes a file that was there before *right after* we checked its
    /// metadata and found that it still exists.
    /// So if you see this, you could also have won the lottery.
    WorktreeFileMissingForObjectConversion,
    /// When performing a unified diff, it had to refused as the file was too large or turned out to be binary.
    /// Note that this only happens for binary files if there is no `diff.<name>.textconv` filters configured.
    FileToLargeOrBinary,
    /// A change with multiple hunks to be applied wasn't present in the base-tree.
    /// Previously this was possible when untracked files were added with their single hunk specified, but now this shouldn't be happening anymore.
    PathNotFoundInBaseTree,
    /// There was a change, but the path pointed to something that wasn't a file or a link.
    /// You would see this if also in case of submodules or repositories to be added with hunks, which shouldn't be easy to do accidentally even.
    UnsupportedDirectoryEntry,
    /// The base version of a file to apply worktree changes to as present in a Git tree had an undiffable entry type.
    /// This can happen if the target tree has an entry that isn't of the same type as the source worktree changes.
    UnsupportedTreeEntry,
    /// The DiffSpec points to an actual change, or a subset of that change using a file path and optionally hunks into that file.
    /// However, at least one hunk was not fully contained.
    MissingDiffSpecAssociation,
}

/// Alter the single `destination` in a given `frame` with as many `changes` as possible and write new objects into `repo`,
/// but only if the commit succeeds.
///
/// If `move_source` is `Some(source)`, all changes are considered to originate from the given commit to move out of, otherwise they originate from the worktree.
/// `context_lines` is the amount of lines of context included in each [`HunkHeader`], and the value that will be used to recover the existing hunks,
/// so that the hunks can be matched.
///
/// Return additional information that helps to understand to what extent the commit was created, as the commit might not contain all the [`DiffSpecs`](DiffSpec)
/// that were requested if they failed to apply.
///
/// Note that no [`index`](CreateCommitOutcome::index) is produced here as the `HEAD` isn't queried and doesn't play a role.
///
/// No reference is touched in the process.
///
/// ### Hunk-based discarding
///
/// When an instance in `changes` contains hunks, these are the hunks to be committed. If they match a whole hunk in the worktree changes,
/// it will be committed in its entirety.
///
/// ### Sub-Hunk discarding
///
/// It's possible to specify ranges of hunks to discard. To do that, they need an *anchor*. The *anchor* is the pair of
/// `(line_number, line_count)` that should not be changed, paired with the *other* pair with the new `(line_number, line_count)`
/// to discard.
///
/// For instance, when there is a single patch `-1,10 +1,10` and we want to commit the removed 5th line *and* the added 5th line,
/// we'd specify *just* two selections, one in the old via `-5,1 +1,10` and one in the new via `-1,10 +5,1`.
/// This works because internally, it will always match the hunks (and sub-hunks) with their respective pairs obtained through a
/// worktree status, using the anchor, and apply an additional processing step to get the actual old/new hunk pairs to use when
/// building the buffer to commit.
pub fn create_commit(
    repo: &gix::Repository,
    destination: Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let parents = match &destination {
        Destination::NewCommit {
            parent_commit_id: None,
            ..
        } => Vec::new(),
        Destination::NewCommit {
            parent_commit_id: Some(parent),
            ..
        } => vec![*parent],
        Destination::AmendCommit { commit_id, .. } => commit_id
            .attach(repo)
            .object()?
            .peel_to_commit()?
            .parent_ids()
            .map(|id| id.detach())
            .collect(),
    };

    if !matches!(destination, Destination::AmendCommit { .. }) && parents.len() > 1 {
        bail!("cannot currently handle more than 1 parent")
    }

    let target_tree = match &destination {
        Destination::NewCommit {
            parent_commit_id: None,
            ..
        } => gix::ObjectId::empty_tree(repo.object_hash()),
        Destination::NewCommit {
            parent_commit_id: Some(base_commit),
            ..
        }
        | Destination::AmendCommit {
            commit_id: base_commit,
            ..
        } => but_core::Commit::from_id(base_commit.attach(repo))?
            .tree_id_or_auto_resolution()?
            .detach(),
    };

    let CreateTreeOutcome {
        rejected_specs,
        destination_tree,
        changed_tree_pre_cherry_pick,
    } = create_tree(repo, target_tree, move_source, changes, context_lines)?;
    let new_commit = if let Some(new_tree) = destination_tree {
        match destination {
            Destination::NewCommit {
                message,
                parent_commit_id: _,
                stack_segment: _,
            } => {
                let (author, committer) = repo.commit_signatures()?;
                let new_commit = create_possibly_signed_commit(
                    repo, author, committer, &message, new_tree, parents, None,
                )?;
                Some(new_commit)
            }
            Destination::AmendCommit {
                commit_id,
                new_message,
            } => {
                let mut commit = but_core::Commit::from_id(commit_id.attach(repo))?;
                commit.tree = new_tree;
                if let Some(message) = new_message {
                    commit.message = message.into();
                }
                Some(but_rebase::commit::create(
                    repo,
                    commit.inner,
                    DateMode::CommitterUpdateAuthorUpdate,
                )?)
            }
        }
    } else {
        None
    };
    Ok(CreateCommitOutcome {
        rejected_specs,
        new_commit,
        changed_tree_pre_cherry_pick,
        references: Vec::new(),
        rebase_output: None,
        index: None,
    })
}

/// Create a commit exactly as specified, and sign it depending on Git and GitButler specific Git configuration.
fn create_possibly_signed_commit(
    repo: &gix::Repository,
    author: gix::actor::Signature,
    committer: gix::actor::Signature,
    message: &str,
    tree: gix::ObjectId,
    parents: impl IntoIterator<Item = impl Into<gix::ObjectId>>,
    commit_headers: Option<but_core::commit::HeadersV2>,
) -> anyhow::Result<gix::ObjectId> {
    let commit = gix::objs::Commit {
        message: message.into(),
        tree,
        author,
        committer,
        encoding: None,
        parents: parents.into_iter().map(Into::into).collect(),
        extra_headers: (&commit_headers.unwrap_or_default()).into(),
    };
    but_rebase::commit::create(repo, commit, DateMode::CommitterKeepAuthorKeep)
}
