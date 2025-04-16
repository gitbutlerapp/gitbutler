use crate::commit_engine::{
    Destination, DiffSpec, HunkHeader, MoveSourceCommit, RejectionReason, apply_hunks,
};
use anyhow::bail;
use bstr::{BStr, ByteSlice};
use but_core::{RepositoryExt, UnifiedDiff};
use gix::filter::plumbing::pipeline::convert::ToGitOutcome;
use gix::merge::tree::TreatAsUnresolved;
use gix::object::tree::EntryKind;
use gix::prelude::ObjectIdExt;
use std::borrow::Cow;
use std::io::Read;
use std::path::Path;

/// Additional information about the outcome of a [`create_tree()`] call.
#[derive(Debug)]
pub struct CreateTreeOutcome {
    /// Changes that were removed from `new_tree` because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
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
            todo!(
                "get base tree and apply changes by cherry-picking, probably can all be done by one function, but optimizations are different"
            )
        } else {
            let changes_base_tree = repo
                .head()?
                .id()
                .and_then(|id| {
                    id.object()
                        .ok()?
                        .peel_to_commit()
                        .ok()?
                        .tree_id()
                        .ok()?
                        .detach()
                        .into()
                })
                .unwrap_or(target_tree);
            apply_worktree_changes(changes_base_tree, repo, &mut changes, context_lines)?
        };

        let Some(tree_with_changes) =
            maybe_new_tree.filter(|tree_with_changes| *tree_with_changes != target_tree)
        else {
            changes
                .iter_mut()
                .for_each(|c| into_err_spec(c, RejectionReason::NoEffectiveChanges));
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
                    into_err_spec(change, RejectionReason::CherryPickMergeConflict);
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

fn into_err_spec(input: &mut PossibleChange, reason: RejectionReason) {
    *input = match std::mem::replace(input, Ok(Default::default())) {
        // What we thought was a good change turned out to be a no-op, rejected.
        Ok(inner) => Err((reason, inner)),
        Err(inner) => Err(inner),
    };
}

type PossibleChange = Result<DiffSpec, (RejectionReason, DiffSpec)>;

/// Apply `changes` to `changes_base_tree` and return the newly written tree as `(maybe_new_tree, actual_base_tree, maybe_new_index)`.
/// All `changes` are expected to originate from `changes_base_tree`, and will be applied `changes_base_tree`.
///
/// `head_index`, is expected to match `changes_base_tree` initially
/// and will be adjusted to contain all the `changes`, thus matching the output tree.
/// Since we read the latest stats, we will also update these accordingly.
/// It is treated as if it lived on disk and may contain initial values, as a way to
/// avoid destroying indexed information like stats which would slow down the next status.
fn apply_worktree_changes<'repo>(
    actual_base_tree: gix::ObjectId,
    repo: &'repo gix::Repository,
    changes: &mut [PossibleChange],
    context_lines: u32,
) -> anyhow::Result<(Option<gix::Id<'repo>>, gix::ObjectId)> {
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

    let work_dir = repo.workdir().expect("non-bare repo");
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
        // NOTE: See copy below!
        if let Some(previous_path) = change_request.previous_path.as_ref().map(|p| p.as_bstr()) {
            base_tree_editor.remove(previous_path)?;
        }
        if change_request.hunk_headers.is_empty() {
            let rela_path = change_request.path.as_bstr();
            match pipeline.worktree_file_to_object(rela_path, &index)? {
                Some((id, kind, _fs_metadata)) => {
                    base_tree_editor.upsert(rela_path, kind, id)?;
                }
                None => into_err_spec(
                    possible_change,
                    RejectionReason::WorktreeFileMissingForObjectConversion,
                ),
            }
        } else if let Some(worktree_changes) = &worktree_changes {
            let Some(worktree_change) = worktree_changes.iter().find(|c| {
                c.path == change_request.path
                    && c.previous_path()
                        == change_request.previous_path.as_ref().map(|p| p.as_bstr())
            }) else {
                into_err_spec(possible_change, RejectionReason::NoEffectiveChanges);
                continue;
            };
            let mut diff_filter = but_core::unified_diff::filter_from_state(
                repo,
                worktree_change.status.state(),
                UnifiedDiff::CONVERSION_MODE,
            )?;
            debug_assert_eq!(
                UnifiedDiff::CONVERSION_MODE,
                gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
                "BUG: if this changes, the uses of worktree filters need a review"
            );
            // TODO(perf): avoid computing the unified diff here, we only need hunks with, usually with zero context.
            let UnifiedDiff::Patch { hunks, .. } =
                worktree_change.unified_diff_with_filter(repo, context_lines, &mut diff_filter)?
            else {
                into_err_spec(possible_change, RejectionReason::FileToLargeOrBinary);
                continue;
            };

            let has_hunk_selections = change_request
                .hunk_headers
                .iter()
                .any(|h| h.old_range().is_null() || h.new_range().is_null());
            let worktree_hunks: Vec<HunkHeader> = hunks.into_iter().map(Into::into).collect();
            let worktree_hunks_no_context = if has_hunk_selections {
                let UnifiedDiff::Patch {
                    hunks: hunks_no_context,
                    ..
                } = worktree_change.unified_diff_with_filter(repo, 0, &mut diff_filter)?
                else {
                    into_err_spec(possible_change, RejectionReason::FileToLargeOrBinary);
                    continue;
                };
                Cow::Owned(hunks_no_context.into_iter().map(Into::into).collect())
            } else {
                Cow::Borrowed(worktree_hunks.as_slice())
            };

            let selected_hunks = change_request.hunk_headers.drain(..);
            let (hunks_to_commit, rejected) =
                to_additive_hunks(selected_hunks, &worktree_hunks, &worktree_hunks_no_context);

            change_request.hunk_headers = rejected;
            if hunks_to_commit.is_empty() && !change_request.hunk_headers.is_empty() {
                into_err_spec(possible_change, RejectionReason::MissingDiffSpecAssociation);
                continue 'each_change;
            }
            let (previous_state, previous_path) = worktree_change
                .status
                .previous_state_and_path()
                .map(|(state, maybe_path)| (Some(state), maybe_path))
                .unwrap_or_default();
            let base_rela_path = previous_path.unwrap_or(change_request.path.as_bstr());
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
                into_err_spec(possible_change, RejectionReason::UnsupportedDirectoryEntry);
                continue;
            };

            let worktree_base = match previous_state {
                None => Vec::new(),
                Some(previous_state) => {
                    match previous_state.kind {
                        EntryKind::Tree | EntryKind::Commit => {
                            // defensive: assure file wasn't swapped with something we can't handle
                            into_err_spec(possible_change, RejectionReason::UnsupportedTreeEntry);
                            continue;
                        }
                        EntryKind::Blob | EntryKind::BlobExecutable | EntryKind::Link => {
                            repo.find_blob(previous_state.id)?.detach().data
                        }
                    }
                }
            };

            worktree_file_to_git_in_buf(
                &mut current_worktree,
                base_rela_path,
                &path,
                &mut pipeline,
                &index,
            )?;
            let base_with_patches = apply_hunks(
                worktree_base.as_bstr(),
                current_worktree.as_bstr(),
                &hunks_to_commit,
            )?;
            let blob_with_selected_patches = repo.write_blob(base_with_patches.as_slice())?;
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

pub(crate) fn worktree_file_to_git_in_buf(
    buf: &mut Vec<u8>,
    rela_path: &BStr,
    path: &Path,
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
) -> anyhow::Result<()> {
    buf.clear();
    let to_git = pipeline.convert_to_git(
        std::fs::File::open(path)?,
        &gix::path::from_bstr(rela_path),
        index,
    )?;
    match to_git {
        ToGitOutcome::Unchanged(mut file) => {
            file.read_to_end(buf)?;
        }
        ToGitOutcome::Process(mut stream) => {
            stream.read_to_end(buf)?;
        }
        ToGitOutcome::Buffer(buf2) => buf.extend_from_slice(buf2),
    };
    Ok(())
}

/// Given `hunks_to_keep` (ascending hunks by starting line) and the set of `worktree_hunks_no_context`
/// (worktree hunks without context), return `(hunks_to_commit, rejected_hunks)` where `hunks_to_commit` is the
/// headers to drive the additive operation to create the buffer to commit, and `rejected_hunks` is the list of
/// hunks from `hunks_to_keep` that couldn't be associated with `worktree_hunks_no_context` because they weren't actually included.
///
/// `worktree_hunks` is the hunks with a given amount of context, usually 3, and it's used to quickly select original hunks
/// without selection.
/// `hunks_to_keep` indicate that they are a selection by marking the other side with `0,0`, i.e. `-1,2 +0,0` selects old `1,2`,
/// and `-0,0 +2,3` selects new `2,3`.
///
/// The idea here is that `worktree_hunks_no_context` is the smallest-possible hunks that still contain the designated
/// selections in the old or new image respectively. This is necessary to maintain the right order in the face of context lines.
/// Note that the order of changes is still affected by what which selection comes first, i.e. old and new, or vice versa, if these
/// selections are in the same hunk.
fn to_additive_hunks(
    hunks_to_keep: impl IntoIterator<Item = HunkHeader>,
    worktree_hunks: &[HunkHeader],
    worktree_hunks_no_context: &[HunkHeader],
) -> (Vec<HunkHeader>, Vec<HunkHeader>) {
    let mut hunks_to_commit = Vec::new();
    let mut rejected = Vec::new();
    let mut previous = HunkHeader {
        old_start: 1,
        old_lines: 0,
        new_start: 1,
        new_lines: 0,
    };
    let mut last_wh = None;
    for selected_hunk in hunks_to_keep {
        let sh = selected_hunk;
        if sh.new_range().is_null() {
            if let Some(wh) = worktree_hunks_no_context
                .iter()
                .find(|wh| wh.old_range().contains(sh.old_range()))
            {
                if last_wh != Some(*wh) {
                    last_wh = Some(*wh);
                    previous.new_start = wh.new_start;
                }
                hunks_to_commit.push(HunkHeader {
                    old_start: sh.old_start,
                    old_lines: sh.old_lines,
                    new_start: previous.new_start,
                    new_lines: 0,
                });
                previous.old_start = sh.old_range().end();
                continue;
            }
        } else if sh.old_range().is_null() {
            if let Some(wh) = worktree_hunks_no_context
                .iter()
                .find(|wh| wh.new_range().contains(sh.new_range()))
            {
                if last_wh != Some(*wh) {
                    last_wh = Some(*wh);
                    previous.old_start = wh.old_start;
                }
                hunks_to_commit.push(HunkHeader {
                    old_start: previous.old_start,
                    old_lines: 0,
                    new_start: sh.new_start,
                    new_lines: sh.new_lines,
                });
                previous.new_start = sh.new_range().end();
                continue;
            }
        } else if worktree_hunks.contains(&sh) {
            previous.old_start = sh.old_range().end();
            previous.new_start = sh.new_range().end();
            last_wh = Some(sh);
            hunks_to_commit.push(sh);
            continue;
        }
        rejected.push(sh);
    }
    (hunks_to_commit, rejected)
}

#[cfg(test)]
mod tests;
