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
    changes_base_tree: Option<gix::ObjectId>,
    repo: &'repo gix::Repository,
    changes: &mut [PossibleChange],
    context_lines: u32,
) -> anyhow::Result<(Option<gix::Id<'repo>>, gix::ObjectId)> {
    fn has_zero_based_line_numbers(hunk_header: &HunkHeader) -> bool {
        hunk_header.new_start == 0 || hunk_header.old_start == 0
    }
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
        if change_request.hunk_headers.is_empty() {
            // NOTE: See copy below!
            if let Some(previous_path) = change_request.previous_path.as_ref().map(|p| p.as_bstr())
            {
                base_tree_editor.remove(previous_path)?;
            }
            let rela_path = change_request.path.as_bstr();
            // TODO: this is wrong, we know that the diffs were created from the version stored in Git,
            //       useful for git-lfs I suppose. Fix this conversion so hunks line up.
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
            let previous_path = worktree_change.previous_path();
            if let Some(previous_path) = previous_path {
                base_tree_editor.remove(previous_path)?;
            }
            let base_rela_path = previous_path.unwrap_or(change_request.path.as_bstr());
            let Some(entry) = base_tree.lookup_entry(base_rela_path.split(|b| *b == b'/'))? else {
                // Assume the file is untracked, so no entry exists yet. Handle it as if there were no hunks,
                // assuming there is only one.
                if change_request.hunk_headers.len() == 1 {
                    // NOTE: See copy above!
                    if let Some(previous_path) =
                        change_request.previous_path.as_ref().map(|p| p.as_bstr())
                    {
                        base_tree_editor.remove(previous_path)?;
                    }
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
                } else {
                    into_err_spec(possible_change, RejectionReason::PathNotFoundInBaseTree);
                }
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
                into_err_spec(possible_change, RejectionReason::UnsupportedDirectoryEntry);
                continue;
            };

            let worktree_base = if entry.mode().is_link() || entry.mode().is_blob() {
                entry.object()?.detach().data
            } else {
                // defensive: assure file wasn't swapped with something we can't handle
                into_err_spec(possible_change, RejectionReason::UnsupportedTreeEntry);
                continue;
            };

            worktree_file_to_git_in_buf(
                &mut current_worktree,
                base_rela_path,
                &path,
                &mut pipeline,
                &index,
            )?;
            let worktree_hunks: Vec<HunkHeader> = hunks.into_iter().map(Into::into).collect();
            for selected_hunk in &change_request.hunk_headers {
                if !worktree_hunks.contains(selected_hunk)
                    || has_zero_based_line_numbers(selected_hunk)
                {
                    into_err_spec(possible_change, RejectionReason::MissingDiffSpecAssociation);
                    // TODO: only skip this one hunk, but collect skipped hunks into a new err-spec.
                    continue 'each_change;
                }
            }
            let base_with_patches = apply_hunks(
                worktree_base.as_bstr(),
                current_worktree.as_bstr(),
                &change_request.hunk_headers,
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
