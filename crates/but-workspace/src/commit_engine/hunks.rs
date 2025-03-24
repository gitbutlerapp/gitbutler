use crate::commit_engine::HunkHeader;
use anyhow::Context;
use bstr::{BStr, BString, ByteSlice};

/// Given an `old_image` and a `new_image`, along with `hunks` that represent selections in `new_image`, apply these
/// hunks to `old_image` and return the newly constructed image.
/// This works like an overlay where selections from `new_image` are inserted into `new_image` with `hunks` as Windows.
///
/// Note that we assume that both images are human-readable because we assume lines to be present,
/// either with Windows or Unix newlines, and we assume that the hunks match up with these lines.
/// This constraint means that the tokens used for diffing are the same lines.
pub fn apply_hunks(
    old_image: &BStr,
    new_image: &BStr,
    hunks: &[HunkHeader],
) -> anyhow::Result<BString> {
    let mut worktree_base_cursor = 1; /* 1-based counting */
    let mut old_iter = old_image.lines_with_terminator();
    let mut worktree_actual_cursor = 1; /* 1-based counting */
    let mut new_iter = new_image.lines_with_terminator();
    let mut result_image: BString = Vec::with_capacity(old_image.len().max(new_image.len())).into();

    // To each selected hunk, put the old-lines into a buffer.
    // Skip over the old hunk in old hunk in old lines.
    // Skip all new lines till the beginning of the new hunk.
    // Write the new hunk.
    // Repeat for each hunk, and write all remaining old lines.
    for selected_hunk in hunks {
        let catchup_base_lines = old_iter.by_ref().take(
            (selected_hunk.old_start as usize)
                .checked_sub(worktree_base_cursor)
                .context("hunks must be in order from top to bottom of the file")?,
        );
        for line in catchup_base_lines {
            result_image.extend_from_slice(line);
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
            result_image.extend_from_slice(line);
        }
        worktree_actual_cursor += selected_hunk.new_lines as usize;
    }

    for line in old_iter {
        result_image.extend_from_slice(line);
    }
    Ok(result_image)
}
