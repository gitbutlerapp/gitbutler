use crate::commit_engine::tree::worktree_file_to_git_in_buf;
use crate::commit_engine::{HunkHeader, HunkRange, apply_hunks};
use anyhow::{Context, bail};
use bstr::ByteSlice;
use but_core::{ChangeState, TreeChange, UnifiedDiff};
use gix::filter::plumbing::driver::apply::{Delay, MaybeDelayed};
use gix::filter::plumbing::pipeline::convert::ToWorktreeOutcome;
use gix::prelude::ObjectIdExt;

/// Discard `hunks_to_discard` in the resource at `wt_change`, whose previous version is `previous_state` and is expected to
/// be tracked and readable from the object database. We will always read what's currently on disk as the current version
/// and overwrite it.
///
/// The general idea is to rebuild the file, but without the `hunks_to_discard`, write it into the worktree replacing
/// the previous file. `hunks_to_discard` are hunks based on a diff of what's in Git.
///
/// Note that an index update isn't necessary, as for the purpose of the application it might as well not exist due to the
/// way our worktree-changes function ignores the index entirely.
///
/// Note that `hunks_to_discard` that weren't present in the actual set of worktree changes will remain, so when everything
/// worked `hunks_to_discard` will be empty.
pub fn restore_state_to_worktree(
    wt_change: &TreeChange,
    previous_state: ChangeState,
    hunks_to_discard: &mut Vec<HunkHeader>,
    path_check: &mut gix::status::plumbing::SymlinkCheck,
    pipeline: &mut gix::filter::Pipeline<'_>,
    index: &gix::index::State,
    context_lines: u32,
) -> anyhow::Result<()> {
    let repo = pipeline.repo;
    let state_in_worktree = ChangeState {
        id: repo.object_hash().null(),
        kind: previous_state.kind,
    };
    let mut diff_filter = but_core::unified_diff::filter_from_state(
        repo,
        Some(state_in_worktree),
        UnifiedDiff::CONVERSION_MODE,
    )?;
    let UnifiedDiff::Patch {
        hunks: hunks_in_worktree,
        ..
    } = wt_change.unified_diff_with_filter(repo, context_lines, &mut diff_filter)?
    else {
        bail!("Couldn't obtain diff for worktree changes.")
    };

    let mut hunks_to_keep: Vec<HunkHeader> = hunks_in_worktree
        .into_iter()
        .map(Into::into)
        .filter(|hunk| {
            match hunks_to_discard
                .iter()
                .enumerate()
                .find_map(|(idx, hunk_to_discard)| (hunk_to_discard == hunk).then_some(idx))
            {
                None => true,
                Some(idx_to_remove) => {
                    hunks_to_discard.remove(idx_to_remove);
                    false
                }
            }
        })
        .collect();

    // Find associations with sub-hunks
    if !hunks_to_discard.is_empty() {
        // TODO(perf): instead of brute-force searching, assure hunks_to_discard are sorted and speed up the search that way.
        let mut hunks_to_keep_with_splits = Vec::new();
        for hunk_to_split in hunks_to_keep {
            let mut subtractions = Vec::new();
            hunks_to_discard.retain(|sub_hunk_to_discard| {
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
        hunks_to_keep = hunks_to_keep_with_splits;
    }

    let old = previous_state.id.attach(repo).object()?.detach().data;
    let mut new = repo.empty_reusable_buffer();
    let rela_path = wt_change.path.as_bstr();
    let worktree_path = path_check.verified_path_allow_nonexisting(rela_path)?;
    worktree_file_to_git_in_buf(&mut new, rela_path, &worktree_path, pipeline, index)?;

    let base_with_patches = apply_hunks(old.as_bstr(), new.as_bstr(), &hunks_to_keep)?;

    let to_worktree = pipeline.convert_to_worktree(&base_with_patches, rela_path, Delay::Forbid)?;
    match to_worktree {
        ToWorktreeOutcome::Unchanged(buf) => {
            std::fs::write(&worktree_path, buf)?;
        }
        ToWorktreeOutcome::Buffer(buf) => {
            std::fs::write(&worktree_path, buf)?;
        }
        ToWorktreeOutcome::Process(MaybeDelayed::Immediate(mut stream)) => {
            let mut file = std::fs::File::create(&worktree_path)?;
            std::io::copy(&mut stream, &mut file)?;
        }
        ToWorktreeOutcome::Process(MaybeDelayed::Delayed(_)) => unreachable!("disabled"),
    }
    Ok(())
}

#[derive(Debug, Copy, Clone)]
enum HunkSubstraction {
    /// Subtract the range from `old`.
    Old(HunkRange),
    /// Subtract the range from `new`.
    New(HunkRange),
}

/// Like a boolean subtraction, remove `subtractions` from `hunk`, and return the remaining pieces.
/// Note that the old and new ranges in `hunk` are split in lock-step, so that cutting out a piece from old will take
/// the respective amount of lines from new if these are available.
#[allow(clippy::indexing_slicing)]
fn subtract_hunks(
    hunk: HunkHeader,
    subtractions: impl IntoIterator<Item = HunkSubstraction>,
) -> anyhow::Result<Vec<HunkHeader>> {
    use HunkSubstraction::*;
    #[derive(Debug, Copy, Clone)]
    enum Source {
        Old,
        New,
    }
    #[derive(Debug)]
    struct Header {
        edit: HunkRange,
        keep: HunkRange,
        // Which hunk the `edit` range is coming from.
        edit_source: Source,
    }
    impl From<Header> for HunkHeader {
        fn from(v: Header) -> Self {
            match v.edit_source {
                Source::Old => HunkHeader {
                    old_start: v.edit.start,
                    old_lines: v.edit.lines,
                    new_start: v.keep.start,
                    new_lines: v.keep.lines,
                },
                Source::New => HunkHeader {
                    old_start: v.keep.start,
                    old_lines: v.keep.lines,
                    new_start: v.edit.start,
                    new_lines: v.edit.lines,
                },
            }
        }
    }
    impl Header {
        fn new(hdr: &HunkHeader, source: Source) -> Self {
            match source {
                Source::Old => Header {
                    edit: hdr.old_range(),
                    keep: hdr.new_range(),
                    edit_source: source,
                },
                Source::New => Header {
                    edit: hdr.new_range(),
                    keep: hdr.old_range(),
                    edit_source: source,
                },
            }
        }
        fn replaced(&self, edit: HunkRange, keep: HunkRange) -> Self {
            Header {
                edit,
                keep,
                edit_source: self.edit_source,
            }
        }
    }

    /// This works if `hdr` at `idx` in `out` fully contains `subtrahend`.
    fn adjust_boundary_or_split_equally(
        out: &mut Vec<HunkHeader>,
        idx: usize,
        mut hdr: Header,
        subtrahend: HunkRange,
    ) {
        if hdr.edit.start == subtrahend.start {
            hdr.edit.start += subtrahend.lines;
            hdr.edit.lines -= subtrahend.lines;
            out[idx] = hdr.into();
        } else if hdr.edit.end() == subtrahend.end() {
            hdr.edit.lines -= subtrahend.lines;
            out[idx] = hdr.into();
        } else {
            let before_split_edit = HunkRange {
                start: hdr.edit.start,
                lines: subtrahend.start - hdr.edit.start,
            };
            let before_split_keep = HunkRange {
                start: hdr.keep.start,
                lines: before_split_edit.lines.min(hdr.keep.lines),
            };
            let after_split_edit = HunkRange {
                start: subtrahend.end(),
                lines: hdr
                    .edit
                    .lines
                    .saturating_sub(before_split_edit.lines)
                    .saturating_sub(subtrahend.lines),
            };
            let after_split_keep = HunkRange {
                start: before_split_keep.end(),
                lines: hdr.keep.lines.saturating_sub(before_split_edit.lines),
            };

            out[idx] = hdr.replaced(after_split_edit, after_split_keep).into();
            out.insert(
                idx, /* insert before */
                hdr.replaced(before_split_edit, before_split_keep).into(),
            );
        }
    }

    let mut out = vec![hunk];
    let subtractions = {
        let mut v: Vec<_> = subtractions.into_iter().collect();
        v.sort_by_key(|s| match *s {
            Old(hr) => hr,
            New(hr) => hr,
        });
        v
    };
    for sub in subtractions {
        let (idx, hdr, subtrahend) = match sub {
            Old(subtrahend) => out.iter().enumerate().find_map(|(idx, hunk)| {
                hunk.old_range()
                    .contains(subtrahend)
                    .then(|| (idx, Header::new(hunk, Source::Old), subtrahend))
            }),
            New(subtrahend) => out.iter().enumerate().find_map(|(idx, hunk)| {
                hunk.new_range()
                    .contains(subtrahend)
                    .then(|| (idx, Header::new(hunk, Source::New), subtrahend))
            }),
        }
        .with_context(|| {
            format!(
                "BUG: provided hunk slices must always be \
            within their old and new hunk respectively: {sub:?} not in {hunk:?}"
            )
        })?;

        adjust_boundary_or_split_equally(&mut out, idx, hdr, subtrahend);
    }

    Ok(out)
}

#[cfg(test)]
mod tests;
