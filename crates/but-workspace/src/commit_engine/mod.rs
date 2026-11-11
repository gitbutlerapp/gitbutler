//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a workspace.

use anyhow::bail;
use but_core::{
    DiffSpec, RepositoryExt,
    commit::{Headers, SignCommit},
    ref_metadata::StackId,
    tree::{CreateTreeOutcome, create_tree, create_tree::RejectionReason},
};

use crate::commit::Conflicts;
use but_rebase::{RebaseOutput, commit::DateMode};
use gix::prelude::ObjectIdExt as _;

/// Types for use in the frontend with serialization support.
pub mod ui;

mod merge;
use merge::Merge;

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
    /// Changes, or the hunks of them, that were left out of the commit because they conflicted
    /// with it while conflicts were rejected, or because the specified hunks didn't match exactly
    /// due to changes that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// The changes, or the hunks of them, that went into the commit.
    pub consumed_specs: Vec<DiffSpec>,
    /// The newly created commit, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_commit: Option<gix::ObjectId>,
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

/// Alter the single `destination` in a given `frame` with as many `changes` as possible and write new objects into `repo`,
/// but only if the commit succeeds.
///
/// `context_lines` is the amount of lines of context included in each [`HunkHeader`](but_core::HunkHeader), and the value that will be used to recover the existing hunks,
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
    changes: Vec<DiffSpec>,
    context_lines: u32,
    conflicts: Conflicts,
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

    let onto = match &destination {
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

    let asked = !changes.is_empty();
    let CreateTreeOutcome {
        mut rejected_specs,
        changed_tree,
    } = create_tree(repo, changes.clone(), context_lines)?;
    let Some(changed) = changed_tree else {
        return Ok(CreateCommitOutcome::unmade(rejected_specs));
    };
    let applied = applied(changes, &rejected_specs);
    let merge = Merge {
        head: repo.head_tree_id_or_empty()?.detach(),
        changed,
        onto,
    };
    let mut merged = merge.merged_tree(repo)?;
    let consumed_specs = match (conflicts, merged.conflict.take()) {
        (Conflicts::Reject, Some(conflict)) => {
            let split = merge.split(repo, merged.tree_id, &conflict.conflict_entries, applied)?;
            rejected_specs.extend(split.left_out);
            split.taken
        }
        (Conflicts::Keep, conflict) => {
            merged.conflict = conflict;
            applied
        }
        (Conflicts::Reject, None) => applied,
    };
    if asked && consumed_specs.is_empty() {
        return Ok(CreateCommitOutcome::unmade(rejected_specs));
    }

    let (mut commit, date_mode, change_id) = match destination {
        Destination::NewCommit { message, .. } => (
            unwritten_commit(repo, &message, parents)?,
            DateMode::CommitterKeepAuthorKeep,
            None,
        ),
        Destination::AmendCommit {
            commit_id,
            new_message,
        } => {
            let amended = but_core::Commit::from_id(commit_id.attach(repo))?;
            let change_id = amended.change_id();
            let mut commit = amended.detach().inner;
            if let Some(message) = new_message {
                commit.message = message.into();
            }
            (commit, DateMode::CommitterUpdateAuthorKeep, Some(change_id))
        }
    };
    merged.apply_to(repo, &mut commit)?;
    let new_commit = but_rebase::commit::create(
        repo,
        commit,
        date_mode,
        SignCommit::IfSignCommitsEnabled,
        change_id,
    )?;
    Ok(CreateCommitOutcome {
        rejected_specs,
        consumed_specs,
        new_commit: Some(new_commit),
        references: Vec::new(),
        rebase_output: None,
        index: None,
    })
}

impl CreateCommitOutcome {
    /// No commit made, as every change was left out, for the reasons `rejected_specs` gives.
    fn unmade(rejected_specs: Vec<(RejectionReason, DiffSpec)>) -> Self {
        CreateCommitOutcome {
            rejected_specs,
            consumed_specs: Vec::new(),
            new_commit: None,
            references: Vec::new(),
            rebase_output: None,
            index: None,
        }
    }
}

/// The specs of `changes` that `rejected` doesn't name the file of.
pub(crate) fn applied(
    changes: Vec<DiffSpec>,
    rejected: &[(RejectionReason, DiffSpec)],
) -> Vec<DiffSpec> {
    let rejected: std::collections::BTreeSet<_> =
        rejected.iter().map(|(_, spec)| &spec.path).collect();
    changes
        .into_iter()
        .filter(|spec| !rejected.contains(&spec.path))
        .collect()
}

/// A commit with `message` on `parents`, signed by the repository's identities and with the
/// headers its configuration asks for, to be given a tree and written.
fn unwritten_commit(
    repo: &gix::Repository,
    message: &str,
    parents: Vec<gix::ObjectId>,
) -> anyhow::Result<gix::objs::Commit> {
    let (author, committer) = repo.commit_signatures()?;
    let headers = Headers::from_config(&repo.config_snapshot());
    Ok(gix::objs::Commit {
        message: message.into(),
        tree: gix::ObjectId::empty_tree(repo.object_hash()),
        author,
        committer,
        encoding: None,
        parents: parents.into(),
        extra_headers: (&headers).into(),
    })
}
