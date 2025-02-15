//! The machinery used to alter and mutate commits in various ways whilst adjusting descendant commits within a [reference frame](ReferenceFrame).

use anyhow::{bail, Context};
use bstr::{BStr, BString, ByteSlice};
use but_core::unified_diff::DiffHunk;
use but_core::{RepositoryExt, UnifiedDiff};
use but_rebase::RebaseOutput;
use gitbutler_oxidize::ObjectIdExt;
use gitbutler_project::access::WorktreeWritePermission;
use gitbutler_stack::{CommitOrChangeId, VirtualBranchesState};
use gix::filter::plumbing::driver::apply::{Delay, MaybeDelayed};
use gix::filter::plumbing::pipeline::convert::{ToGitOutcome, ToWorktreeOutcome};
use gix::merge::tree::TreatAsUnresolved;
use gix::objs::tree::EntryKind;
use gix::prelude::ObjectIdExt as _;
use gix::refs::transaction::PreviousValue;
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::Path;

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

/// Additional information about the outcome of a [`create_tree()`] call.
#[derive(Debug)]
pub struct CreateTreeOutcome {
    /// Changes that were removed from `new_tree` because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<DiffSpec>,
    /// The newly created seen from tree that acts as the destination of the changes, or `None` if no commit could be
    /// created as all changes-requests were rejected.
    pub destination_tree: Option<gix::ObjectId>,
    /// If `destination_tree` is `Some(_)`, this field is `Some(_)` as well and denotes the base-tree + all changes.
    /// If the applied changes were from the worktree, it's `HEAD^{tree}` + changes.
    /// Otherwise, it's `<commit>^{tree}` + changes.
    pub changed_tree_pre_cherry_pick: Option<gix::ObjectId>,
}

/// Like [`create_commit()`], but lower-level and only returns a new tree, without finally associating it with a commit.
pub fn create_tree(
    repo: &gix::Repository,
    destination: &Destination,
    move_source: Option<MoveSourceCommit>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateTreeOutcome> {
    if changes.is_empty() {
        bail!("Have to provide at least one change in order to mutate a commit");
    }

    let target_tree = match destination {
        Destination::NewCommit {
            parent_commit_id: None,
            ..
        } => gix::ObjectId::empty_tree(repo.object_hash()),
        Destination::NewCommit {
            parent_commit_id: Some(base_commit),
            ..
        }
        | Destination::AmendCommit(base_commit) => {
            but_core::Commit::from_id(base_commit.attach(repo))?
                .tree_id()?
                .detach()
        }
    };

    let mut changes: Vec<_> = changes.into_iter().map(Ok).collect();
    let (new_tree, changed_tree_pre_cherry_pick) = 'retry: loop {
        let (maybe_new_tree, actual_base_tree) = if let Some(_source) = move_source {
            todo!("get base tree and apply changes by cherry-picking, probably can all be done by one function, but optimizations are different")
        } else {
            let changes_base_tree = repo.head()?.id().and_then(|id| {
                id.object()
                    .ok()?
                    .peel_to_commit()
                    .ok()?
                    .tree_id()
                    .ok()?
                    .detach()
                    .into()
            });
            apply_worktree_changes(changes_base_tree, repo, &mut changes, context_lines)?
        };

        let Some(tree_with_changes) =
            maybe_new_tree.filter(|tree_with_changes| *tree_with_changes != target_tree)
        else {
            changes.iter_mut().for_each(into_err_spec);
            break 'retry (None, None);
        };
        let tree_with_changes_without_cherry_pick = tree_with_changes.detach();
        let mut tree_with_changes = tree_with_changes.detach();
        let needs_cherry_pick = actual_base_tree != gix::ObjectId::empty_tree(repo.object_hash())
            && actual_base_tree != target_tree;
        if needs_cherry_pick {
            let base = actual_base_tree;
            let ours = target_tree;
            let theirs = tree_with_changes;
            let mut merge_result = repo.merge_trees(
                base,
                ours,
                theirs,
                repo.default_merge_labels(),
                repo.tree_merge_options()?,
            )?;
            let unresolved_conflicts: Vec<_> = merge_result
                .conflicts
                .iter()
                .filter_map(|c| {
                    c.is_unresolved(TreatAsUnresolved::git())
                        .then_some(c.theirs.location())
                })
                .collect();
            if !unresolved_conflicts.is_empty() {
                for change in changes.iter_mut().filter(|c| {
                    c.as_ref()
                        .ok()
                        .is_some_and(|change| unresolved_conflicts.contains(&change.path.as_bstr()))
                }) {
                    into_err_spec(change);
                }
                continue 'retry;
            }
            tree_with_changes = merge_result.tree.write()?.detach();
        }
        break 'retry (
            Some(tree_with_changes),
            Some(tree_with_changes_without_cherry_pick),
        );
    };
    Ok(CreateTreeOutcome {
        rejected_specs: changes.into_iter().filter_map(Result::err).collect(),
        destination_tree: new_tree,
        changed_tree_pre_cherry_pick,
    })
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
            // Theory: `refs/gitbutler` isn't relevant as for now it's only used to dump unapplied branches.
            .prefixed("refs/heads/")?
            .filter_map(Result::ok)
            .filter_map(|r| {
                r.try_id()
                    .map(|id| (id.detach(), but_core::Reference::Git(r.inner.name)))
            })
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
            rewrite_references(
                repo,
                frame.vb,
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

            let rebase = {
                // Set commits leading up to the tip on top of the new commit, serving as base.
                let mut builder =
                    but_rebase::RebaseBuilder::new(repo, new_commit, Some(commit_in_graph))?;
                let workspace_tip = frame
                    .workspace_tip
                    .filter(|tip| !commits_to_rebase.contains(tip));
                builder.steps_unvalidated(commits_to_rebase.into_iter().rev().map(|commit_id| {
                    but_rebase::RebaseStep::Pick {
                        commit_id,
                        new_message: None,
                    }
                }));
                if let Some(workspace_tip) = workspace_tip {
                    // We can assume the workspace tip is connected to a pick (or else the rebase will fail)
                    builder.step(but_rebase::RebaseStep::Pick {
                        commit_id: workspace_tip,
                        new_message: None,
                    })?;
                }
                builder.rebase()?
            };

            rewrite_references(
                repo,
                frame.vb,
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
        apply_index(
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

fn rewrite_references(
    repo: &gix::Repository,
    state: &mut VirtualBranchesState,
    mut refs_by_commit_id: gix::hashtable::HashMap<gix::ObjectId, Vec<but_core::Reference>>,
    changed_commits: impl IntoIterator<Item = (gix::ObjectId, gix::ObjectId)>,
    updated_refs: &mut Vec<UpdatedReference>,
) -> anyhow::Result<()> {
    let mut ref_edits = Vec::new();
    let mut branches_ordered: Vec<_> = state.branches.values_mut().collect();
    branches_ordered.sort_by(|a, b| a.name.cmp(&b.name));
    for (old, new) in changed_commits {
        let old_git2 = old.to_git2();
        for stack in &mut branches_ordered {
            if stack.head == old_git2 {
                stack.head = new.to_git2();
                stack.tree = new
                    .attach(repo)
                    .object()?
                    .into_commit()
                    .tree_id()?
                    .to_git2();
                updated_refs.push(UpdatedReference {
                    old_commit_id: old,
                    new_commit_id: new,
                    reference: but_core::Reference::Virtual(stack.name.clone()),
                });
            }
            for (id, branch_target_id_hex, name) in
                stack.heads.iter_mut().filter_map(|b| match &mut b.head {
                    CommitOrChangeId::CommitId(id_hex) => {
                        gix::ObjectId::from_hex(id_hex.as_bytes())
                            .ok()
                            .map(|id| (id, id_hex, &b.name))
                    }
                    CommitOrChangeId::ChangeId(_) => None,
                })
            {
                if id == old {
                    *branch_target_id_hex = new.to_string();
                    updated_refs.push(UpdatedReference {
                        old_commit_id: old,
                        new_commit_id: new,
                        reference: but_core::Reference::Virtual(name.clone()),
                    });
                }
            }
        }

        let Some(refs_to_rewite) = refs_by_commit_id.remove(&old) else {
            continue;
        };
        // No rebase required, change affects the tip of these heads.
        for r in refs_to_rewite {
            match r {
                but_core::Reference::Git(name) => {
                    use gix::refs::{
                        transaction::{Change, LogChange, RefEdit, RefLog},
                        Target,
                    };
                    updated_refs.push(UpdatedReference {
                        old_commit_id: old,
                        new_commit_id: new,
                        reference: but_core::Reference::Git(name.clone()),
                    });
                    ref_edits.push(RefEdit {
                        change: Change::Update {
                            log: LogChange {
                                mode: RefLog::AndReference,
                                force_create_reflog: false,
                                message: "Created or amended commit".into(),
                            },
                            expected: PreviousValue::ExistingMustMatch(Target::Object(old)),
                            new: Target::Object(new),
                        },
                        name,
                        deref: false,
                    });
                }
                but_core::Reference::Virtual(_name) => {
                    todo!("change dta in `branches`")
                }
            }
        }
    }
    repo.edit_references(ref_edits)?;
    Ok(())
}

fn into_err_spec(input: &mut PossibleChange) {
    *input = match std::mem::replace(input, Ok(Default::default())) {
        // What we thought was a good change turned out to be a no-op, rejected.
        Ok(inner) => Err(inner),
        Err(inner) => Err(inner),
    };
}

type PossibleChange = Result<DiffSpec, DiffSpec>;

/// Apply `changes` to `changes_base_tree` and return the newly written tree as `(maybe_new_tree, actual_base_tree, maybe_new_index)`.
/// All `changes` are expected to originate from `changes_base_tree`, and will be applied `changes_base_tree`.
///
/// `head_index`, is expected to match `changes_base_tree` initially
/// and will be adjusted to contain all the `changes`, thus matching the output tree.
/// Since we read the latest stats, we will also update these accordingly.
/// It is treated as if it lived on disk and may contain initial values, as a way to
/// avoid destroying indexed information like stats which would slow down the next status.
fn apply_worktree_changes<'repo>(
    changes_base_tree: Option<gix::ObjectId>,
    repo: &'repo gix::Repository,
    changes: &mut [PossibleChange],
    context_lines: u32,
) -> anyhow::Result<(Option<gix::Id<'repo>>, gix::ObjectId)> {
    let actual_base_tree =
        changes_base_tree.unwrap_or_else(|| gix::ObjectId::empty_tree(repo.object_hash()));
    let base_tree = actual_base_tree.attach(repo).object()?.peel_to_tree()?;
    let mut base_tree_editor = base_tree.edit()?;
    let (mut pipeline, index) = repo.filter_pipeline(None)?;
    let changes_with_hunks = changes
        .iter()
        .filter_map(|c| c.as_ref().ok())
        .any(|c| !c.hunk_headers.is_empty());
    let worktree_changes = changes_with_hunks
        .then(|| but_core::diff::worktree_changes(repo).map(|wtc| wtc.changes))
        .transpose()?;
    let mut current_worktree = Vec::new();

    let work_dir = repo.work_dir().expect("non-bare repo");
    'each_change: for possible_change in changes.iter_mut() {
        let change_request = match possible_change {
            Ok(change) => change,
            Err(_) => continue,
        };
        let path = work_dir.join(gix::path::from_bstr(change_request.path.as_bstr()));
        let md = match gix::index::fs::Metadata::from_path_no_follow(&path) {
            Ok(md) => md,
            Err(err) if gix::fs::io_err::is_not_found(err.kind(), err.raw_os_error()) => {
                base_tree_editor.remove(change_request.path.as_bstr())?;
                continue;
            }
            Err(err) => return Err(err.into()),
        };
        if change_request.hunk_headers.is_empty() {
            if let Some(previous_path) = change_request.previous_path.as_ref().map(|p| p.as_bstr())
            {
                base_tree_editor.remove(previous_path)?;
            }
            let rela_path = change_request.path.as_bstr();
            match pipeline.worktree_file_to_object(rela_path, &index)? {
                Some((id, kind, _fs_metadata)) => {
                    base_tree_editor.upsert(rela_path, kind, id)?;
                }
                None => into_err_spec(possible_change),
            }
        } else if let Some(worktree_changes) = &worktree_changes {
            let Some(worktree_change) = worktree_changes.iter().find(|c| {
                c.path == change_request.path
                    && c.previous_path()
                        == change_request.previous_path.as_ref().map(|p| p.as_bstr())
            }) else {
                into_err_spec(possible_change);
                continue;
            };
            let UnifiedDiff::Patch { hunks } = worktree_change.unified_diff(repo, context_lines)?
            else {
                into_err_spec(possible_change);
                continue;
            };
            let previous_path = worktree_change.previous_path();
            if let Some(previous_path) = previous_path {
                base_tree_editor.remove(previous_path)?;
            }
            let base_rela_path = previous_path.unwrap_or(change_request.path.as_bstr());
            let Some(entry) = base_tree.lookup_entry(base_rela_path.split(|b| *b == b'/'))? else {
                into_err_spec(possible_change);
                continue;
            };

            let current_entry_kind = if md.is_symlink() {
                EntryKind::Link
            } else if md.is_file() {
                if md.is_executable() {
                    EntryKind::BlobExecutable
                } else {
                    EntryKind::Blob
                }
            } else {
                // This could be a fifo (skip) or a repository. But that wouldn't have hunks.
                into_err_spec(possible_change);
                continue;
            };

            let worktree_base = if entry.mode().is_link() {
                entry.object()?.detach().data
            } else if entry.mode().is_blob() {
                let mut obj_in_git = entry.object()?;
                match pipeline.convert_to_worktree(
                    &obj_in_git.data,
                    base_rela_path,
                    Delay::Forbid,
                )? {
                    ToWorktreeOutcome::Unchanged(_) => obj_in_git.detach().data,
                    ToWorktreeOutcome::Buffer(buf) => buf.to_owned(),
                    ToWorktreeOutcome::Process(MaybeDelayed::Immediate(mut stream)) => {
                        obj_in_git.data.clear();
                        stream.read_to_end(&mut obj_in_git.data)?;
                        obj_in_git.detach().data
                    }
                    ToWorktreeOutcome::Process(MaybeDelayed::Delayed(_)) => {
                        unreachable!("We forbade that")
                    }
                }
            } else {
                // defensive: assure file wasn't swapped with something we can't handle
                into_err_spec(possible_change);
                continue;
            };

            // TODO(performance): find a byte-line buffered reader so it doesn't have to be all in memory.
            current_worktree.clear();
            std::fs::File::open(path)?.read_to_end(&mut current_worktree)?;

            let worktree_hunks: Vec<HunkHeader> = hunks.into_iter().map(Into::into).collect();
            let mut worktree_base_cursor = 1; /* 1-based counting */
            let mut old_iter = worktree_base.lines_with_terminator();
            let mut worktree_actual_cursor = 1; /* 1-based counting */
            let mut new_iter = current_worktree.lines_with_terminator();
            let mut base_with_patches: BString = Vec::with_capacity(md.len().try_into()?).into();

            // To each selected hunk, put the old-lines into a buffer.
            // Skip over the old hunk in old hunk in old lines.
            // Skip all new lines till the beginning of the new hunk.
            // Write the new hunk.
            // Repeat for each hunk, and write all remaining old lines.
            for selected_hunk in &change_request.hunk_headers {
                if !worktree_hunks.contains(selected_hunk)
                    || has_zero_based_line_numbers(selected_hunk)
                {
                    into_err_spec(possible_change);
                    // TODO: only skip this one hunk, but collect skipped hunks into a new err-spec.
                    continue 'each_change;
                }
                let catchup_base_lines = old_iter.by_ref().take(
                    (selected_hunk.old_start as usize)
                        .checked_sub(worktree_base_cursor)
                        .context("hunks must be in order from top to bottom of the file")?,
                );
                for line in catchup_base_lines {
                    base_with_patches.extend_from_slice(line);
                }
                let _consume_old_hunk_to_replace_with_new = old_iter
                    .by_ref()
                    .take(selected_hunk.old_lines as usize)
                    .count();
                worktree_base_cursor += selected_hunk.old_lines as usize;

                let new_hunk_lines = new_iter
                    .by_ref()
                    .skip(
                        (selected_hunk.new_start as usize)
                            .checked_sub(worktree_actual_cursor)
                            .context("hunks for new lines must be in order")?,
                    )
                    .take(selected_hunk.new_lines as usize);

                for line in new_hunk_lines {
                    base_with_patches.extend_from_slice(line);
                }
                worktree_actual_cursor += selected_hunk.new_lines as usize;
            }

            for line in old_iter {
                base_with_patches.extend_from_slice(line);
            }

            let slice_read = &mut base_with_patches.as_slice();
            let to_git = pipeline.convert_to_git(
                slice_read,
                gix::path::from_bstr(&change_request.path).as_ref(),
                &index,
            )?;

            let blob_with_selected_patches = match to_git {
                ToGitOutcome::Unchanged(slice) => repo.write_blob(slice)?,
                ToGitOutcome::Process(stream) => repo.write_blob_stream(stream)?,
                ToGitOutcome::Buffer(buf) => repo.write_blob(buf)?,
            };
            base_tree_editor.upsert(
                change_request.path.as_bstr(),
                current_entry_kind,
                blob_with_selected_patches,
            )?;
        } else {
            unreachable!("worktree-changes are always set if there are hunks")
        }
    }

    let altered_base_tree_id = base_tree_editor.write()?;
    let maybe_new_tree = (actual_base_tree != altered_base_tree_id).then_some(altered_base_tree_id);
    Ok((maybe_new_tree, actual_base_tree))
}

/// Turn `rhs` into `lhs` by modifying `rhs`. This will leave `rhs` intact as much as possible, but will remove
/// Note that conflicting entries will be replaced by an addition or edit automatically.
/// extensions that might be affected by these changes, for a lack of finesse with our edits.
fn apply_index(
    workdir: &Path,
    lhs: &gix::index::State,
    rhs: &mut gix::index::State,
) -> anyhow::Result<()> {
    let mut num_sorted_entries = rhs.entries().len();
    let mut needs_sorting = false;

    let mut changes = Vec::new();
    gix::diff::index(
        lhs,
        rhs,
        |change| -> Result<_, std::convert::Infallible> {
            changes.push(change.into_owned());
            Ok(gix::diff::index::Action::Continue)
        },
        None::<gix::diff::index::RewriteOptions<'_, gix::Repository>>,
        &mut gix::pathspec::Search::from_specs(None, None, workdir)?,
        &mut |_, _, _, _| unreachable!("no pathspec is used"),
    )?;

    use gix::diff::index::Change;
    for change in changes {
        match change {
            Change::Addition { location, .. } => {
                delete_entry_by_path_bounded(rhs, location.as_bstr(), &mut num_sorted_entries);
            }
            Change::Deletion {
                location,
                entry_mode,
                id,
                ..
            }
            | Change::Modification {
                location,
                previous_entry_mode: entry_mode,
                previous_id: id,
                ..
            } => {
                let md = gix::index::fs::Metadata::from_path_no_follow(
                    &workdir.join(gix::path::from_bstr(location.as_bstr())),
                )?;
                needs_sorting |= upsert_index_entry(
                    rhs,
                    location.as_bstr(),
                    &md,
                    id.into_owned(),
                    entry_mode,
                    &mut num_sorted_entries,
                )?;
            }
            Change::Rewrite { .. } => {
                unreachable!("rewrites tracking was disabled")
            }
        }
    }

    if needs_sorting {
        rhs.sort_entries();
    }
    rhs.remove_tree();
    rhs.remove_resolve_undo();
    Ok(())
}

// TODO(gix): this could be a platform in Gix which supports these kinds of edits while assuring
//       consistency. It could use some tricks to not have worst-case performance like this has.
//       It really is index-add that we need.
fn upsert_index_entry(
    index: &mut gix::index::State,
    rela_path: &BStr,
    md: &gix::index::fs::Metadata,
    id: gix::ObjectId,
    mode: gix::index::entry::Mode,
    num_sorted_entries: &mut usize,
) -> anyhow::Result<bool> {
    use gix::index::entry::Stage;
    delete_entry_by_path_bounded_stages(
        index,
        rela_path,
        num_sorted_entries,
        &[Stage::Base, Stage::Ours, Stage::Theirs],
    );

    let needs_sort = if let Some(pos) = index.entry_index_by_path_and_stage_bounded(
        rela_path,
        Stage::Unconflicted,
        *num_sorted_entries,
    ) {
        #[allow(clippy::indexing_slicing)]
        let entry = &mut index.entries_mut()[pos];
        entry.stat = gix::index::entry::Stat::from_fs(md)?;
        entry.id = id;
        entry.mode = mode;
        false
    } else {
        index.dangerously_push_entry(
            gix::index::entry::Stat::from_fs(md)?,
            id,
            gix::index::entry::Flags::empty(),
            mode,
            rela_path,
        );
        true
    };
    Ok(needs_sort)
}

fn delete_entry_by_path_bounded(
    index: &mut gix::index::State,
    rela_path: &BStr,
    num_sorted_entries: &mut usize,
) {
    use gix::index::entry::Stage;
    delete_entry_by_path_bounded_stages(
        index,
        rela_path,
        num_sorted_entries,
        &[Stage::Unconflicted, Stage::Base, Stage::Ours, Stage::Theirs],
    );
}

// TODO(gix)
// TODO(performance): make an efficient version of this available in `gix`,
//                    right now we need 4 lookups for each deletion, and possibly 4 rewrites of the vec
fn delete_entry_by_path_bounded_stages(
    index: &mut gix::index::State,
    rela_path: &BStr,
    num_sorted_entries: &mut usize,
    stages: &[gix::index::entry::Stage],
) {
    for stage in stages {
        if let Some(pos) =
            index.entry_index_by_path_and_stage_bounded(rela_path, *stage, *num_sorted_entries)
        {
            index.remove_entry_at_index(pos);
            *num_sorted_entries -= 1;
        }
    }
}

fn has_zero_based_line_numbers(hunk_header: &HunkHeader) -> bool {
    hunk_header.new_start == 0 || hunk_header.old_start == 0
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
