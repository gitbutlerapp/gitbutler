//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a [reference frame](ReferenceFrame).

use anyhow::{bail, Context};
use bstr::BString;
use but_core::unified_diff::DiffHunk;
use but_core::RepositoryExt;
use but_rebase::RebaseOutput;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::VirtualBranchesState;
use gix::prelude::ObjectIdExt as _;
use gix::refs::transaction::PreviousValue;
use serde::{Deserialize, Serialize};

mod tree;
use tree::{create_tree, CreateTreeOutcome};

mod index;
mod refs;

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
        /// Use `message` as commit message for the new commit.
        message: String,
    },
    /// Amend all changes to the given commit, leaving all other aspects of the commit unchanged.
    AmendCommit(gix::ObjectId),
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

/// A change that should be used to create a new commit or alter an existing one, along with enough information to know where to find it.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct DiffSpec {
    /// The previous location of the entry, the source of a rename if there was one.
    pub previous_path: Option<BString>,
    /// The worktree-relative path to the worktree file with the content to commit.
    ///
    /// If `hunks` is empty, this means the current content of the file should be committed.
    pub path: BString,
    /// If one or more hunks are specified, match them with actual changes currently in the worktree.
    /// Failure to match them will lead to the change being dropped.
    /// If empty, the whole file is taken as is.
    pub hunk_headers: Vec<HunkHeader>,
}

/// The header of a hunk that represents a change to a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HunkHeader {
    /// The 1-based line number at which the previous version of the file started.
    pub old_start: u32,
    /// The non-zero amount of lines included in the previous version of the file.
    pub old_lines: u32,
    /// The 1-based line number at which the new version of the file started.
    pub new_start: u32,
    /// The non-zero amount of lines included in the new version of the file.
    pub new_lines: u32,
}

impl From<but_core::unified_diff::DiffHunk> for HunkHeader {
    fn from(
        DiffHunk {
            old_start,
            old_lines,
            new_start,
            new_lines,
            // TODO(performance): if difflines are discarded, we could also just not compute them.
            diff: _,
        }: DiffHunk,
    ) -> Self {
        HunkHeader {
            old_start,
            old_lines,
            new_start,
            new_lines,
        }
    }
}

/// A type used in [`CreateCommitOutcome`] to indicate how a reference was changed so it keeps pointing
/// to the correct commit.
#[derive(Debug, PartialEq)]
pub struct UpdatedReference {
    /// The reference itself.
    // TODO: The virtual variant could contain stack-id as well, but it remains to be seen how useful this is.
    reference: but_core::Reference,
    /// The commit to which `reference` pointed before the update.
    old_commit_id: gix::ObjectId,
    /// The commit to which `reference` points now.
    new_commit_id: gix::ObjectId,
}

/// Additional information about the outcome of a [`create_commit()`] call.
#[derive(Debug)]
pub struct CreateCommitOutcome {
    /// Changes that were removed from a commit because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<DiffSpec>,
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
        Destination::AmendCommit(commit_id) => commit_id
            .attach(repo)
            .object()?
            .peel_to_commit()?
            .parent_ids()
            .map(|id| id.detach())
            .collect(),
    };

    if !matches!(destination, Destination::AmendCommit(_)) && parents.len() > 1 {
        bail!("cannot currently handle more than 1 parent")
    }

    let CreateTreeOutcome {
        rejected_specs,
        destination_tree,
        changed_tree_pre_cherry_pick,
    } = create_tree(repo, &destination, move_source, changes, context_lines)?;
    let new_commit = if let Some(new_tree) = destination_tree {
        match destination {
            Destination::NewCommit {
                message,
                parent_commit_id: _,
            } => {
                let (author, committer) = repo.commit_signatures()?;
                let new_commit = create_possibly_signed_commit(
                    repo, author, committer, &message, new_tree, parents, None,
                )?;
                Some(new_commit)
            }
            Destination::AmendCommit(commit_id) => {
                let mut commit = commit_id
                    .attach(repo)
                    .object()?
                    .peel_to_commit()?
                    .decode()?
                    .to_owned();
                commit.tree = new_tree;
                Some(but_rebase::commit::create(repo, commit)?)
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

/// All information to know where in the commit-graph the rewritten commit is located to figure out
/// which descendant commits to rewrite.
#[derive(Debug)]
pub struct ReferenceFrame<'a> {
    /// The commit that merges all stacks together for a unified view.
    pub workspace_tip: Option<gix::ObjectId>,
    /// If given, and if rebases are necessary, this is the tip (commit) whose ancestry contains
    /// the commit which is committed onto, or amended to.
    ///
    /// Note that in case of moves, right now the source *and* destination need to be contained.
    pub branch_tip: Option<gix::ObjectId>,
    /// A snapshot of all virtual branches we have to possibly rewrite.
    /// They also inform about the stacks active in the workspace, if any.
    pub vb: &'a mut VirtualBranchesState,
}

/// Like [`create_commit()`], but allows to also update virtual branches and git references pointing to commits
/// after rebasing all descendants, along with re-merging possible workspace merge commits.
///
/// `frame` contains the virtual branches to be modified to point to the rebased versions of commits, but also to inform
/// about the available stacks in the workspace and helps to find the stack that contains the affects commits.
///
/// Note that conflicts that occur during the rebase will be swallowed, putting the commit into a conflicted state.
/// Finally, the index will be written so it matches the `HEAD^{commit}` *if* there were worktree changes.
///
/// An index is produced so it matches the tree at `HEAD^{tree}` while keeping all content already present a possibly
/// existing `.git/index`. Updated or added files will have their `stat` forcefully updated.
///
/// ### Performance Note
///
/// As commit traversals will be performed for better performance, an
/// [object cache](gix::Repository::object_cache_size_if_unset()) should be configured.
pub fn create_commit_and_update_refs(
    repo: &gix::Repository,
    frame: ReferenceFrame<'_>,
    destination: Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let mut out = create_commit(
        repo,
        destination.clone(),
        move_source,
        changes,
        context_lines,
    )?;

    let Some(new_commit) = out.new_commit else {
        return Ok(out);
    };

    let commit_to_find = match destination {
        Destination::NewCommit {
            parent_commit_id, ..
        } => parent_commit_id,
        Destination::AmendCommit(commit) => Some(commit),
    };

    if let Some(commit_in_graph) = commit_to_find {
        let mut all_refs_by_id = gix::hashtable::HashMap::<_, Vec<_>>::default();
        for (commit_id, git_reference) in repo
            .references()?
            .prefixed("refs/heads/")?
            .chain(repo.references()?.prefixed("refs/gitbutler/")?)
            .filter_map(Result::ok)
            .filter_map(|r| r.try_id().map(|id| (id.detach(), r.inner.name)))
        {
            all_refs_by_id
                .entry(commit_id)
                .or_default()
                .push(git_reference);
        }

        // Special case: commit/amend on top of `HEAD` and no merge above: no rebase necessary
        if frame.workspace_tip.is_none()
            && repo.head_id().ok().map(|id| id.detach()) == Some(commit_in_graph)
            && all_refs_by_id.contains_key(&commit_in_graph)
        {
            refs::rewrite(
                repo,
                frame.vb,
                None,
                all_refs_by_id,
                [(commit_in_graph, new_commit)],
                &mut out.references,
            )?;
        } else {
            let Some(branch_tip) = frame.branch_tip else {
                bail!(
                    "HEAD isn't connected to the affected commit and a rebase is necessary, but no branch tip was provided"
                );
            };
            // Use the branch tip to find all commits leading up to the one that was affected
            // - these are the commits to rebase.
            let mut found_marker = false;
            let commits_to_rebase: Vec<_> = branch_tip
                .attach(repo)
                .ancestors()
                .first_parent_only()
                .all()?
                .filter_map(Result::ok)
                .take_while(|info| {
                    if info.id == commit_in_graph {
                        found_marker = true;
                        false
                    } else {
                        true
                    }
                })
                .map(|info| info.id)
                .collect();
            if !found_marker {
                bail!(
                    "Branch tip at {branch_tip} didn't contain the affected commit - cannot rebase"
                );
            }

            let workspace_tip = frame
                .workspace_tip
                .filter(|tip| !commits_to_rebase.contains(tip));
            let rebase = {
                // Set commits leading up to the tip on top of the new commit, serving as base.
                let mut builder = but_rebase::Rebase::new(repo, new_commit, Some(commit_in_graph))?;
                builder.steps(commits_to_rebase.into_iter().rev().map(|commit_id| {
                    but_rebase::RebaseStep::Pick {
                        commit_id,
                        new_message: None,
                    }
                }))?;
                if let Some(workspace_tip) = workspace_tip {
                    // We can assume the workspace tip is connected to a pick (or else the rebase will fail)
                    builder.steps([but_rebase::RebaseStep::Pick {
                        commit_id: workspace_tip,
                        new_message: None,
                    }])?;
                }
                builder.rebase()?
            };

            refs::rewrite(
                repo,
                frame.vb,
                workspace_tip,
                all_refs_by_id,
                rebase
                    .commit_mapping
                    .iter()
                    .map(|(_base, old, new)| (*old, *new))
                    .chain(Some((commit_in_graph, new_commit))),
                &mut out.references,
            )?;
            out.rebase_output = Some(rebase);
        }
        // Assume an index to be present and adjust it to match the new tree.

        let tree_index = repo.index_from_tree(&repo.head_tree_id()?)?;
        let mut disk_index = repo.open_index()?;
        index::apply_lhs_to_rhs(
            repo.work_dir().expect("non-bare"),
            &tree_index,
            &mut disk_index,
        )?;
        out.index = disk_index.into();
    } else {
        // unborn branch special case.
        repo.reference(
            repo.head_name()?
                .context("unborn HEAD must contain a ref-name")?,
            new_commit,
            PreviousValue::Any,
            format!(
                "commit (initial): {title}",
                title = new_commit
                    .attach(repo)
                    .object()?
                    .into_commit()
                    .message()?
                    .title
            ),
        )?;
        let new_tree = new_commit.attach(repo).object()?.into_commit().tree_id()?;
        out.index = repo.index_from_tree(&new_tree)?.into();
    }

    if let Some(index) = out.index.as_mut() {
        index.write(Default::default())?;
    }
    Ok(out)
}

/// Like [`create_commit_and_update_refs()`], but integrates with an existing GitButler `project`
/// if present. Alternatively it uses the current `HEAD` as only reference point.
/// Note that virtual branches will not be written back after this call.
pub fn create_commit_and_update_refs_with_project(
    repo: &gix::Repository,
    project: Option<(ReferenceFrame<'_>, &mut WorktreeWritePermission)>,
    destination: Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateCommitOutcome> {
    let mut vbs_storage = VirtualBranchesState::default();
    let frame = if let Some((frame, _perm)) = project {
        frame
    } else {
        ReferenceFrame {
            workspace_tip: None,
            branch_tip: repo.head_id()?.detach().into(),
            vb: &mut vbs_storage,
        }
    };

    create_commit_and_update_refs(
        repo,
        frame,
        destination,
        move_source,
        changes,
        context_lines,
    )
}

/// Create a commit exactly as specified, and sign it depending on Git and GitButler specific Git configuration.
#[allow(clippy::too_many_arguments)]
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
        extra_headers: commit_headers.unwrap_or_default().into(),
    };
    but_rebase::commit::create(repo, commit)
}
