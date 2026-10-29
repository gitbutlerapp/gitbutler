//! Utility for creating a tree with specific changes removed.

use anyhow::Context as _;
use bstr::ByteSlice as _;
use but_core::{ChangeState, DiffSpec, HunkHeader};
use gix::prelude::ObjectIdExt;

use crate::tree_manipulation::hunk::{HunkSubstraction, subtract_hunks};

/// Describes where the changes come from - either a commit or two trees.
pub enum ChangesSource {
    /// Changes from a commit (diff between commit's parent and the commit itself)
    Commit {
        /// The commit ID to get changes from
        id: gix::ObjectId,
    },
    /// Changes between two arbitrary trees
    Tree {
        /// The "after" tree ID
        after_id: gix::ObjectId,
        /// The "before" tree ID
        before_id: gix::ObjectId,
    },
}

impl ChangesSource {
    fn before<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Commit { id } => {
                let commit = repository.find_commit(*id)?;
                if let Some(parent_id) = commit.parent_ids().next() {
                    let parent = but_core::Commit::from_id(parent_id)?;
                    Ok(parent.tree_id_or_auto_resolution()?.object()?.into_tree())
                } else {
                    Ok(repository.empty_tree())
                }
            }
            ChangesSource::Tree { before_id, .. } => Ok(repository.find_tree(*before_id)?),
        }
    }

    fn after<'a>(&self, repository: &'a gix::Repository) -> anyhow::Result<gix::Tree<'a>> {
        match self {
            ChangesSource::Commit { id } => {
                let commit = but_core::Commit::from_id(id.attach(repository))?;
                if commit.is_conflicted() {
                    anyhow::bail!("The source of changes cannot have a conflicted 'after' side.");
                }
                Ok(repository.find_tree(commit.tree)?)
            }
            ChangesSource::Tree { after_id, .. } => Ok(repository.find_tree(*after_id)?),
        }
    }
}

/// Discard the given `changes` in either the work tree or an arbitrary commit or tree. If a change could not be matched with an
/// actual worktree change, for instance due to a race, that's not an error, instead it will be returned in the result Vec, along
/// with all hunks that couldn't be matched.
///
/// The returned Vec is typically empty, meaning that all `changes` could be discarded.
///
/// `context_lines` is the amount of context lines we should assume when obtaining hunks of worktree changes to match against
/// the ones we have specified in the hunks contained within `changes`.
///
/// Discarding a change is really more of an 'undo' of a change as it will restore the previous state to the desired extent - Git
/// doesn't have a notion of this on a whole-file basis.
///
/// Each of the `changes` will be matched against actual worktree changes to make this operation as safe as possible, after all, it
/// discards changes without recovery.
///
/// In practice, this is like a selective 'inverse-checkout', as such it must have a lot of the capabilities of checkout, but focussed
/// on just a couple of paths, and with special handling for renamed files, something that `checkout` can't naturally handle
/// as it's only dealing with single file-paths.
///
/// ### Hunk-based discarding
///
/// When an instance in `changes` contains hunks, these are the hunks to be discarded. If they match a whole hunk in the worktree changes,
/// it will be discarded entirely, simply by not applying it.
///
/// ### Sub-Hunk discarding
///
/// It's possible to specify ranges of hunks to discard. To do that, they need an *anchor*. The *anchor* is the pair of
/// `(line_number, line_count)` that should not be changed, paired with the *other* pair with the new `(line_number, line_count)`
/// to discard.
///
/// For instance, when there is a single patch `-1,10 +1,10` and we want to bring back the removed 5th line *and* the added 5th line,
/// we'd specify *just* two selections, one in the old via `-5,1 +1,10` and one in the new via `-1,10 +5,1`.
/// This works because internally, it will always match the hunks (and sub-hunks) with their respective pairs obtained through a
/// worktree status.
pub fn create_tree_without_diff(
    repository: &gix::Repository,
    changes_source: ChangesSource,
    changes_to_discard: impl IntoIterator<Item = DiffSpec>,
    context_lines: u32,
) -> anyhow::Result<(gix::ObjectId, Vec<DiffSpec>)> {
    let mut dropped = Vec::new();

    let before = changes_source.before(repository)?;
    let after = changes_source.after(repository)?;

    let mut builder = repository.edit_tree(after.id())?;

    for change in changes_to_discard {
        let before_path = change
            .previous_path
            .clone()
            .unwrap_or_else(|| change.path.clone());
        let before_entry = before.lookup_entry(before_path.clone().split_str("/"))?;

        let Some(after_entry) = after.lookup_entry(change.path.clone().split_str("/"))? else {
            let Some(before_entry) = before_entry else {
                // If there is no before entry and no after entry, then
                // something has gone wrong.
                dropped.push(change);
                continue;
            };

            if change.hunk_headers.is_empty() {
                // If there is no after_change, then it must have been deleted.
                // Therefore, we can just add it again.
                builder.upsert(
                    change.path.as_bstr(),
                    before_entry.mode().kind(),
                    before_entry.object_id(),
                )?;
                continue;
            }

            if !is_blob(before_entry.mode().kind()) {
                // Only blobs have hunks to speak of, so anything else stays the
                // caller's problem, exactly as it was before.
                anyhow::bail!(
                    "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
                );
            }

            // A file the change deletes has no 'after' state to diff against, so it gets
            // an empty one. Removing all of the deletion's hunks then brings the file
            // back in full, and removing only some of them brings back just those lines.
            let before_blob = before_entry.object()?.into_blob();
            let removal = contents_without_hunks(
                repository,
                &change,
                before_path.as_bstr(),
                ChangeState {
                    id: before_entry.id().detach(),
                    kind: before_entry.mode().kind(),
                },
                &before_blob.data,
                ChangeState {
                    id: repository.write_blob(b"")?.detach(),
                    kind: before_entry.mode().kind(),
                },
                &[],
                context_lines,
            )?;

            if !removal.unmatched.is_empty() {
                dropped.push(DiffSpec {
                    previous_path: change.previous_path.clone(),
                    path: change.path.clone(),
                    hunk_headers: removal.unmatched,
                });
            }

            if removal.removed_any {
                // Leave the deletion in place when nothing matched, or the file would
                // come back empty rather than staying deleted.
                let restored = repository.write_blob(&removal.contents)?;
                builder.upsert(change.path.as_bstr(), before_entry.mode().kind(), restored)?;
            }
            continue;
        };

        match after_entry.mode().kind() {
            gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable => {
                let after_blob = after_entry.object()?.into_blob();
                if change.hunk_headers.is_empty() {
                    revert_file_to_before_state(&before_entry, &mut builder, &change)?;
                } else {
                    // A file the change adds has no 'before' state to diff against, so
                    // it gets an empty one. That keeps its hunks matchable, and removing
                    // all of them then simply means the addition is gone.
                    // Anything that isn't a blob has no contents to work with, and is
                    // left to the diff below to reject with its own message.
                    let before_blob = match &before_entry {
                        Some(before_entry) if is_blob(before_entry.mode().kind()) => {
                            Some(before_entry.object()?.into_blob())
                        }
                        _ => None,
                    };
                    let before_data: &[u8] = before_blob
                        .as_ref()
                        .map_or(&[], |blob| blob.data.as_slice());
                    let before_state = match &before_entry {
                        Some(before_entry) => ChangeState {
                            id: before_entry.id().detach(),
                            kind: before_entry.mode().kind(),
                        },
                        None => ChangeState {
                            id: repository.write_blob(b"")?.detach(),
                            kind: after_entry.mode().kind(),
                        },
                    };

                    let removal = contents_without_hunks(
                        repository,
                        &change,
                        before_path.as_bstr(),
                        before_state,
                        before_data,
                        ChangeState {
                            id: after_entry.id().detach(),
                            kind: after_entry.mode().kind(),
                        },
                        &after_blob.data,
                        context_lines,
                    )?;

                    if !removal.unmatched.is_empty() {
                        dropped.push(DiffSpec {
                            previous_path: change.previous_path.clone(),
                            path: change.path.clone(),
                            hunk_headers: removal.unmatched,
                        });
                    }

                    let new_after_contents = removal.contents;
                    if before_entry.is_none()
                        && removal.removed_any
                        && new_after_contents.is_empty()
                    {
                        // Every hunk of an added file was removed, so there is no
                        // addition left to keep - an empty blob would be a different
                        // file rather than none at all. Nothing matching at all leaves
                        // the addition alone instead, including for an empty file.
                        builder.remove(change.path.as_bstr())?;
                    } else {
                        let mode = match &before_entry {
                            Some(before_entry) if new_after_contents == before_data => {
                                before_entry.mode().kind()
                            }
                            _ => after_entry.mode().kind(),
                        };
                        let new_after_contents = repository.write_blob(&new_after_contents)?;

                        // Keep the mode of the after state. We _should_ at some
                        // point introduce the mode specifically as part of the
                        // DiscardSpec, but for now, we can just use the after state.
                        builder.upsert(change.path.as_bstr(), mode, new_after_contents)?;
                    }
                }
            }
            _ => {
                revert_file_to_before_state(&before_entry, &mut builder, &change)?;
            }
        }
    }

    let final_tree = builder.write()?;
    Ok((final_tree.detach(), dropped))
}

/// Remove the hunks named in `change` from the diff between `before` and `after`,
/// returning the resulting file contents along with the hunks that couldn't be matched.
///
/// A side that doesn't exist - an addition has no `before`, a deletion has no `after` -
/// is passed in as an empty blob. That keeps its hunks matchable, so removing all of them
/// resolves to the whole-file outcome instead of being rejected as undefined.
#[allow(clippy::too_many_arguments)]
fn contents_without_hunks(
    repository: &gix::Repository,
    change: &DiffSpec,
    before_path: &bstr::BStr,
    before_state: ChangeState,
    before_data: &[u8],
    after_state: ChangeState,
    after_data: &[u8],
    context_lines: u32,
) -> anyhow::Result<HunkRemoval> {
    let diff = but_core::UnifiedPatch::compute(
        repository,
        change.path.as_bstr(),
        Some(before_path),
        after_state,
        before_state,
        context_lines,
    )?
    .context("Cannot diff submodules - if this is encountered we should look into it")?;

    let but_core::UnifiedPatch::Patch {
        hunks: diff_hunks, ..
    } = diff
    else {
        anyhow::bail!("expected a patch");
    };

    let mut good_hunk_headers = vec![];
    let mut bad_hunk_headers = vec![];
    for hunk in &change.hunk_headers {
        if diff_hunks
            .iter()
            .any(|diff_hunk| HunkHeader::from(diff_hunk).contains(*hunk))
        {
            good_hunk_headers.push(*hunk);
        } else {
            bad_hunk_headers.push(*hunk);
        }
    }

    // TODO: Validate that the hunks correspond with actual changes?
    let removed_any = !good_hunk_headers.is_empty();
    let new_hunks = new_hunks_after_removals(
        diff_hunks.into_iter().map(Into::into).collect(),
        good_hunk_headers,
    )?;
    let contents = but_core::apply_hunks(before_data.as_bstr(), after_data.as_bstr(), &new_hunks)?;
    Ok(HunkRemoval {
        contents,
        unmatched: bad_hunk_headers,
        removed_any,
    })
}

/// Whether `kind` is a file whose contents can be diffed into hunks.
fn is_blob(kind: gix::objs::tree::EntryKind) -> bool {
    matches!(
        kind,
        gix::objs::tree::EntryKind::Blob | gix::objs::tree::EntryKind::BlobExecutable
    )
}

/// The outcome of removing hunks from a file's diff.
struct HunkRemoval {
    /// The file's contents with the matched hunks removed.
    contents: bstr::BString,
    /// Hunks that matched nothing in the diff, and so were not removed.
    unmatched: Vec<HunkHeader>,
    /// Whether any hunk matched at all.
    ///
    /// When nothing matched, the entry must be left exactly as it is: reporting a
    /// spec as unmatched and still rewriting the entry would contradict each other.
    removed_any: bool,
}

fn new_hunks_after_removals(
    change_hunks: Vec<HunkHeader>,
    mut removal_hunks: Vec<HunkHeader>,
) -> anyhow::Result<Vec<HunkHeader>> {
    // If a removal hunk matches completely then we can drop it entirely.
    let hunks_to_keep: Vec<HunkHeader> = change_hunks
        .into_iter()
        .filter(|hunk| {
            match removal_hunks
                .iter()
                .enumerate()
                .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
            {
                None => true,
                Some(idx_to_remove) => {
                    removal_hunks.remove(idx_to_remove);
                    false
                }
            }
        })
        .collect();

    // TODO(perf): instead of brute-force searching, assure hunks_to_discard are sorted and speed up the search that way.
    let mut hunks_to_keep_with_splits = Vec::new();
    for hunk_to_split in hunks_to_keep {
        let mut subtractions = Vec::new();
        removal_hunks.retain(|sub_hunk_to_discard| {
            if sub_hunk_to_discard.old_range() == hunk_to_split.old_range() {
                subtractions.push(HunkSubstraction::New(sub_hunk_to_discard.new_range()));
                false
            } else if sub_hunk_to_discard.new_range() == hunk_to_split.new_range() {
                subtractions.push(HunkSubstraction::Old(sub_hunk_to_discard.old_range()));
                false
            } else {
                true
            }
        });
        if subtractions.is_empty() {
            hunks_to_keep_with_splits.push(hunk_to_split);
        } else {
            let hunk_with_subtractions = subtract_hunks(hunk_to_split, subtractions)?;
            hunks_to_keep_with_splits.extend(hunk_with_subtractions);
        }
    }
    Ok(hunks_to_keep_with_splits)
}

fn revert_file_to_before_state(
    before_entry: &Option<gix::object::tree::Entry<'_>>,
    builder: &mut gix::object::tree::Editor<'_>,
    change: &DiffSpec,
) -> Result<(), anyhow::Error> {
    // If there are no hunk headers, then we want to revert the
    // whole file to the state it was in before tree.
    if let Some(before_entry) = before_entry {
        builder.remove(change.path.as_bstr())?;
        builder.upsert(
            change
                .previous_path
                .clone()
                .unwrap_or(change.path.clone())
                .as_bstr(),
            before_entry.mode().kind(),
            before_entry.object_id(),
        )?;
    } else {
        builder.remove(change.path.as_bstr())?;
    }
    Ok(())
}
