//! The machinery used to alter and mutate commits in various ways.

use anyhow::{bail, Context};
use bstr::{BString, ByteSlice};
use but_core::unified_diff::DiffHunk;
use but_core::{RepositoryExt, UnifiedDiff};
use gix::filter::plumbing::driver::apply::{Delay, MaybeDelayed};
use gix::filter::plumbing::pipeline::convert::{ToGitOutcome, ToWorktreeOutcome};
use gix::merge::tree::TreatAsUnresolved;
use gix::objs::tree::EntryKind;
use gix::prelude::ObjectIdExt;
use serde::{Deserialize, Serialize};
use std::io::Read;

/// Types for use in the frontend with serialization support.
pub mod ui;

mod plumbing;

/// The place to apply the [change-specifications](DiffSpec) to.
///
/// Note that any commit this instance points to will be the basis to apply all changes to.
#[derive(Debug, Clone, Copy)]
pub enum Destination {
    /// Create a new commit on top of the given `Some(commit)`, so it will be the sole parent
    /// of the newly created commit, making it its ancestor.
    /// To create a commit at the position of the first commit of a branch, the parent has to be the merge-base with the *target branch*.
    ///
    /// If the commit is `None`, the base-state for the new commit will be an empty tree and the new commit will be the first one
    /// (i.e. have no parent). This is the case when `HEAD` is unborn. If `HEAD` is detached, this is a failure.
    ParentForNewCommit(Option<gix::ObjectId>),
    /// Amend the given commit.
    AmendCommit(gix::ObjectId),
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

/// Additional information about the outcome of a [`create_commit()`] call.
#[derive(Debug, PartialEq)]
pub struct CreateCommitOutcome {
    /// Changes that were removed from a commit because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<DiffSpec>,
    /// The newly created commit, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_commit: Option<gix::ObjectId>,
    /// Only set when `HEAD` was updated as it was unborn, and we created the first commit.
    pub ref_edit: Option<gix::refs::transaction::RefEdit>,
}

/// Additional information about the outcome of a [`create_tree()`] call.
#[derive(Debug, PartialEq)]
pub struct CreateTreeOutcome {
    /// Changes that were removed from `new_tree` because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<DiffSpec>,
    /// The newly created tree, or `None` if no commit could be created as all changes-requests were rejected.
    pub new_tree: Option<gix::ObjectId>,
}

/// Like [`create_commit()`], but lower-level and only returns a new tree, without finally associating it with a commit.
pub fn create_tree(
    repo: &gix::Repository,
    destination: Destination,
    origin_commit: Option<gix::ObjectId>,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateTreeOutcome> {
    if changes.is_empty() {
        bail!("Have to provide at least one change in order to mutate a commit");
    }

    let target_tree = match destination {
        Destination::ParentForNewCommit(None) => gix::ObjectId::empty_tree(repo.object_hash()),
        Destination::ParentForNewCommit(Some(base_commit))
        | Destination::AmendCommit(base_commit) => {
            but_core::Commit::from_id(base_commit.attach(repo))?
                .tree_id()?
                .detach()
        }
    };

    let mut changes: Vec<_> = changes.into_iter().map(Ok).collect();
    let new_tree = 'retry: loop {
        let (maybe_new_tree, actual_base_tree) = if let Some(_origin_commit) = origin_commit {
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
            break 'retry None;
        };
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
        break 'retry Some(tree_with_changes);
    };
    Ok(CreateTreeOutcome {
        rejected_specs: changes.into_iter().filter_map(Result::err).collect(),
        new_tree,
    })
}

/// Control how [`create_commit()`] alters references after creating the new commit.
#[derive(Debug, Copy, Clone)]
pub enum RefHandling {
    /// If the commit is created on top of a commit that the `HEAD` ref is currently pointing to, then update it to point to the new commit.
    UpdateHEADRefForTipCommits,
    /// Do not touch any ref, only create a commit.
    None,
}

/// Alter the single `destination` in a given `frame` with as many `changes` as possible and write new objects into `repo`,
/// but only if the commit succeeds.
/// If `origin_commit` is `Some(commit)`, all changes are considered to originate from the given commit, otherwise they originate from the worktree.
/// Use `message` as commit message.
/// `context_lines` is the amount of lines of context included in each [`HunkHeader`], and the value that will be used to recover the existing hunks,
/// so that the hunks can be matched.
///
/// Return additional information that helps to understand to what extent the commit was created, as the commit might not contain all the [`DiffSpecs`](DiffSpec)
/// that were requested if they failed to apply.
///
/// Note that the ref pointed to by `HEAD` will be updated with the new commit if the new commits parent was pointed to by `HEAD` before. Detached heads will cause failure.
/// If `allow_ref_change` is false, `HEAD` will never be adjusted to prevent additional side-effects.
pub fn create_commit(
    repo: &gix::Repository,
    destination: Destination,
    origin_commit: Option<gix::ObjectId>,
    changes: Vec<DiffSpec>,
    message: &str,
    context_lines: u32,
    ref_handling: RefHandling,
) -> anyhow::Result<CreateCommitOutcome> {
    let parents = match destination {
        Destination::ParentForNewCommit(None) => Vec::new(),
        Destination::ParentForNewCommit(Some(parent)) => vec![parent],
        Destination::AmendCommit(_) => {
            todo!("get parents of the given commit ")
        }
    };

    if parents.len() > 1 {
        bail!("cannot currently handle more than 1 parent")
    }

    let ref_name_to_update = repo
        .head_name()?
        .context("Refusing to commit into a detached HEAD")?;
    let ref_name_to_update = match ref_handling {
        RefHandling::UpdateHEADRefForTipCommits => {
            if let &[parent] = &parents[..] {
                new_commit_is_on_top_of_tip(repo, ref_name_to_update.as_ref(), parent)?
                    .then_some(ref_name_to_update)
            } else if repo.head()?.is_unborn() {
                Some(ref_name_to_update)
            } else {
                None
            }
        }
        RefHandling::None => None,
    };

    let CreateTreeOutcome {
        rejected_specs,
        new_tree,
    } = create_tree(repo, destination, origin_commit, changes, context_lines)?;
    let (new_commit, ref_edit) = if let Some(new_tree) = new_tree {
        let (author, committer) = repo.commit_signatures()?;
        let (new_commit, ref_edit) = plumbing::create_commit(
            repo,
            ref_name_to_update,
            author,
            committer,
            message,
            new_tree,
            parents,
            None,
        )?;
        (Some(new_commit), ref_edit)
    } else {
        (None, None)
    };
    Ok(CreateCommitOutcome {
        rejected_specs,
        new_commit,
        ref_edit,
    })
}

fn into_err_spec(input: &mut PossibleChange) {
    *input = match std::mem::replace(input, Ok(Default::default())) {
        // What we thought was a good change turned out to be a no-op, rejected.
        Ok(inner) => Err(inner),
        Err(inner) => Err(inner),
    };
}

fn new_commit_is_on_top_of_tip(
    repo: &gix::Repository,
    name: &gix::refs::FullNameRef,
    parent: gix::ObjectId,
) -> anyhow::Result<bool> {
    let Some(head_ref) = repo.try_find_reference(name)? else {
        return Ok(true);
    };
    Ok(head_ref.id() == *parent)
}

type PossibleChange = Result<DiffSpec, DiffSpec>;

/// Apply `changes` to `changes_base_tree` and return the newly written tree.
/// All `changes` are expected to originate from `changes_base_tree`.
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
        let md = match std::fs::symlink_metadata(&path) {
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
                if gix::fs::is_executable(&md) {
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
    Ok((
        (actual_base_tree != altered_base_tree_id).then_some(altered_base_tree_id),
        actual_base_tree,
    ))
}

fn has_zero_based_line_numbers(hunk_header: &HunkHeader) -> bool {
    hunk_header.new_start == 0 || hunk_header.old_start == 0
}
