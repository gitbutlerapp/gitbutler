use std::{borrow::Cow, collections::BTreeMap};

use anyhow::bail;
use bstr::ByteSlice;
use gix::{merge::tree::TreatAsUnresolved, object::tree::EntryKind, prelude::ObjectIdExt};

use crate::{DiffSpec, HunkHeader, HunkRange, RepositoryExt, UnifiedPatch, apply_hunks};

/// Utility types for the [`create_tree()`] function
pub mod create_tree {

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
}
use create_tree::RejectionReason;

use crate::worktree::worktree_file_to_git_in_buf;

/// Additional information about the outcome of a [`super::create_tree()`] call.
#[derive(Debug)]
pub struct CreateTreeOutcome {
    /// Changes that were removed from `new_tree` because they caused conflicts when rebasing dependent commits,
    /// when merging the workspace commit, or because the specified hunks didn't match exactly due to changes
    /// that happened in the meantime, or if a file without a change was specified.
    pub rejected_specs: Vec<(RejectionReason, DiffSpec)>,
    /// The newly created seen from tree that acts as the destination of the changes, or `None` if no tree could be
    /// created as all changes-requests were rejected (or there was no change).
    pub destination_tree: Option<gix::ObjectId>,
    /// If `destination_tree` is `Some(_)`, this field is `Some(_)` as well and denotes the base-tree + all changes.
    /// If the applied changes were from the worktree, it's `HEAD^{tree}` + changes.
    /// Otherwise, it's `<commit>^{tree}` + changes.
    pub changed_tree_pre_cherry_pick: Option<gix::ObjectId>,
}

/// Like [`create_commit()`], but lower-level and only returns a new tree, without finally associating it with a commit.
pub fn create_tree(
    repo: &gix::Repository,
    target_tree: gix::ObjectId,
    changes: Vec<DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<CreateTreeOutcome> {
    let mut changes: Vec<_> = changes.into_iter().map(Ok).collect();
    let (new_tree, changed_tree_pre_cherry_pick) = if changes.is_empty() {
        (Some(target_tree), None)
    } else {
        'retry: loop {
            let (new_tree, actual_base_tree) = {
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

            let tree_with_changes = if new_tree == actual_base_tree
                && changes.iter().all(|c| {
                    c.is_ok()
                        // Some rejections are OK, and we want to create a commit anyway.
                        || !matches!(
                            c,
                            Err((RejectionReason::CherryPickMergeConflict,_))
                        )
                }) {
                changes
                    .iter_mut()
                    .for_each(|c| into_err_spec(c, RejectionReason::NoEffectiveChanges));
                break 'retry (None, None);
            } else {
                new_tree
            };
            let tree_with_changes_without_cherry_pick = tree_with_changes.detach();
            let mut tree_with_changes = tree_with_changes.detach();
            let needs_cherry_pick = actual_base_tree
                != gix::ObjectId::empty_tree(repo.object_hash())
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
                        c.as_ref().ok().is_some_and(|change| {
                            unresolved_conflicts.contains(&change.path.as_bstr())
                        })
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
        }
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

/// A utility type to keep track of `Ok` specs to use, or `Err` specs that have been rejected.
pub type PossibleChange = Result<DiffSpec, (RejectionReason, DiffSpec)>;

/// Apply `changes` to `changes_base_tree` and return the newly written tree as `(new_tree, actual_base_tree)`.
/// All `changes` are expected to originate from `changes_base_tree`, and will be applied `changes_base_tree`.
///
/// Note that the returned `new_tree` may be the same as `actual_base_tree`, if no change was successfully applied
/// as recorded in `changes`.
pub fn apply_worktree_changes<'repo>(
    actual_base_tree: gix::ObjectId,
    repo: &'repo gix::Repository,
    changes: &mut [PossibleChange],
    context_lines: u32,
) -> anyhow::Result<(gix::Id<'repo>, gix::ObjectId)> {
    let base_tree = actual_base_tree.attach(repo).object()?.peel_to_tree()?;
    let mut base_tree_editor = base_tree.edit()?;
    let (mut pipeline, index) = repo.filter_pipeline(None)?;
    let has_changes_with_hunks = changes
        .iter()
        .filter_map(|c| c.as_ref().ok())
        .any(|c| !c.hunk_headers.is_empty());
    let worktree_changes = has_changes_with_hunks
        .then(|| crate::diff::worktree_changes(repo).map(|wtc| wtc.changes))
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
            let mut diff_filter = crate::unified_diff::filter_from_state(
                repo,
                worktree_change.status.state(),
                UnifiedPatch::CONVERSION_MODE,
            )?;
            debug_assert_eq!(
                UnifiedPatch::CONVERSION_MODE,
                gix::diff::blob::pipeline::Mode::ToGitUnlessBinaryToTextIsPresent,
                "BUG: if this changes, the uses of worktree filters need a review"
            );
            // TODO(perf): avoid computing the unified diff here, we only need hunks with, usually with zero context.
            let Some(UnifiedPatch::Patch { hunks, .. }) =
                worktree_change.unified_patch_with_filter(repo, context_lines, &mut diff_filter)?
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
                let Some(UnifiedPatch::Patch {
                    hunks: hunks_no_context,
                    ..
                }) = worktree_change.unified_patch_with_filter(repo, 0, &mut diff_filter)?
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
                to_additive_hunks(selected_hunks, &worktree_hunks, &worktree_hunks_no_context)?;

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
                &md,
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
    Ok((altered_base_tree_id, actual_base_tree))
}

/// Given `hunks_to_keep` (ascending hunks by starting line) and the set of `worktree_hunks_no_context`
/// (worktree hunks without context), return `(hunks_to_commit, rejected_hunks)`.
/// `hunks_to_commit` is the headers to drive the additive operation to create the buffer to commit, and `rejected_hunks` is the list of
/// hunks from `hunks_to_keep` that couldn't be associated with `worktree_hunks_no_context` because they weren't included.
///
/// `worktree_hunks` is the hunks with a given amount of context, usually 3, and it's used to quickly select original hunks
/// without sub-selection, which is needed when no sub-selections are specified for all hunks. Those with sub-selections and without
/// can be mixed freely though.
///
/// `hunks_to_keep` indicate that they are a selection of either old or new by marking the other side with `0,0`, i.e. `-1,2 +0,0` selects *old* `1,2`,
/// and `-0,0 +2,3` selects *new* `2,3`. Our job here is to rebuild the original hunk selections from that, as if the user had
/// selected them in the UI, and we want to recreate the hunks that would match their selection.
///
/// The idea here is that `worktree_hunks_no_context` is the smallest-possible hunks that still contain the designated
/// selections in the old or new image respectively. This is necessary to maintain the right order in the face of context lines.
/// Note that the order of changes is still affected by what which selection comes first, i.e. old and new, or vice versa, if these
/// selections are in the same hunk.
fn to_additive_hunks(
    hunks_to_keep: impl IntoIterator<Item = HunkHeader>,
    worktree_hunks: &[HunkHeader],
    worktree_hunks_no_context: &[HunkHeader],
) -> anyhow::Result<(Vec<HunkHeader>, Vec<HunkHeader>)> {
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

    fn in_order(hunks: &[HunkHeader]) -> bool {
        hunks
            .iter()
            .zip(hunks.iter().skip(1))
            .all(|(prev, next)| prev < next)
    }

    let res = if !in_order(&hunks_to_commit) {
        tracing::info!(
            "Using alternative hunk algorithm for {hunks_to_commit:?}, wth: {worktree_hunks:?}, wth0: {worktree_hunks_no_context:?}"
        );
        let hunks: Vec<_> = hunks_to_commit
            .into_iter()
            .map(
                |HunkHeader {
                     old_start,
                     old_lines,
                     new_start,
                     new_lines,
                 }| {
                    if old_lines == 0 {
                        Ok(HunkHeader {
                            old_start: 0,
                            old_lines: 0,
                            new_start,
                            new_lines,
                        })
                    } else if new_lines == 0 {
                        Ok(HunkHeader {
                            old_start,
                            old_lines,
                            new_start: 0,
                            new_lines: 0,
                        })
                    } else {
                        bail!("Unexpected hunk with neither newlines or oldlines being 0");
                    }
                },
            )
            .collect::<Result<_, _>>()?;
        let (hunks_to_commit, rejected) =
            to_additive_hunks_fallback(hunks, worktree_hunks, worktree_hunks_no_context);
        if !in_order(&hunks_to_commit) {
            bail!(
                "Alternative hunks algorithms still didn't produce properly ordered hunks or saw duplicate inputs: {hunks_to_commit:?}"
            );
        }
        (hunks_to_commit, rejected)
    } else {
        (hunks_to_commit, rejected)
    };
    Ok(res)
}

/// This algorithm is better when the basic one fails, but both have their merit.
/// Right now this is a brute-force one or the other approach, but we could also apply them selectively.
/// Note that what we really want to simulate is what the UI shows. But since the UI also doesn't really know hunks,
/// we have to fiddle it together here, at least we know the hunks themselves.
///
/// Note that this algorithm is kind of the opposite of what people would expect if it's run where `to_additive_hunks()` works.
/// But here we are… just making this work.
#[allow(clippy::indexing_slicing)]
fn to_additive_hunks_fallback(
    hunks_to_keep: impl IntoIterator<Item = HunkHeader>,
    worktree_hunks: &[HunkHeader],
    worktree_hunks_no_context: &[HunkHeader],
) -> (Vec<HunkHeader>, Vec<HunkHeader>) {
    let mut rejected = Vec::new();

    let mut map = BTreeMap::<HunkHeader, Vec<HunkHeader>>::new();
    for selected_hunk in hunks_to_keep {
        let sh = selected_hunk;
        if sh.new_range().is_null() {
            if let Some(wh) = worktree_hunks_no_context
                .iter()
                .find(|wh| wh.old_range().contains(sh.old_range()))
            {
                let hunks = map.entry(*wh).or_default();
                if let Some(existing_wh_pos) = hunks.iter_mut().position(|wh| wh.old_lines == 0) {
                    let wh = &mut hunks[existing_wh_pos];
                    wh.old_start = sh.old_start;
                    wh.old_lines = sh.old_lines;

                    let iter = existing_wh_pos..hunks.len();
                    for (prev, next) in iter.clone().zip(iter.skip(1)) {
                        hunks[next].old_start = hunks[prev].old_range().end();
                    }
                } else {
                    hunks.push(HunkHeader {
                        old_start: sh.old_start,
                        old_lines: sh.old_lines,
                        new_start: hunks
                            .last()
                            .and_then(|wh| {
                                wh.new_range()
                                    .contains(HunkRange {
                                        start: wh.new_start,
                                        lines: 0,
                                    })
                                    .then_some(wh.new_range().end())
                            })
                            .unwrap_or(wh.new_start),
                        new_lines: 0,
                    });
                }
                continue;
            }
        } else if sh.old_range().is_null() {
            if let Some(wh) = worktree_hunks_no_context
                .iter()
                .find(|wh| wh.new_range().contains(sh.new_range()))
            {
                let hunks = map.entry(*wh).or_default();

                if let Some(existing_wh_pos) = hunks.iter_mut().position(|wh| wh.new_lines == 0) {
                    let wh = &mut hunks[existing_wh_pos];
                    wh.new_start = sh.new_start;
                    wh.new_lines = sh.new_lines;

                    let iter = existing_wh_pos..hunks.len();
                    for (prev, next) in iter.clone().zip(iter.skip(1)) {
                        hunks[next].new_start = hunks[prev].new_range().end();
                    }
                } else {
                    hunks.push(HunkHeader {
                        old_start: hunks
                            .last()
                            .and_then(|wh| {
                                wh.old_range()
                                    .contains(HunkRange {
                                        start: wh.old_start,
                                        lines: 0,
                                    })
                                    .then_some(wh.old_range().end())
                            })
                            .unwrap_or(wh.old_start),
                        old_lines: 0,
                        new_start: sh.new_start,
                        new_lines: sh.new_lines,
                    });
                }
                continue;
            }
        } else if worktree_hunks.contains(&sh) {
            let hunks = map.entry(sh).or_default();
            hunks.push(sh);
            continue;
        }
        rejected.push(sh);
    }

    (map.into_values().flatten().collect(), rejected)
}

#[cfg(test)]
mod tests;
