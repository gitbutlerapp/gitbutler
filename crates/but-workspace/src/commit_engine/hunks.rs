use crate::{HunkHeader, commit_engine::HunkRange};
use anyhow::Context;
use bstr::{BStr, BString, ByteSlice};
use but_core::unified_diff::DiffHunk;

/// Given an `old_image` and a `new_image`, along with `hunks` that represent selections in `new_image`, apply these
/// hunks to `old_image` and return the newly constructed image.
/// This works like an overlay where selections from `new_image` are inserted into `new_image` with `hunks` as Windows,
/// and selections in `old_image` are discarded.
///
/// Note that we assume that both images are human-readable because we assume lines to be present,
/// either with Windows or Unix newlines, and we assume that the hunks match up with these lines.
/// This constraint means that the tokens used for diffing are the same lines.
pub fn apply_hunks(
    old_image: &BStr,
    new_image: &BStr,
    hunks: &[HunkHeader],
) -> anyhow::Result<BString> {
    let mut old_cursor = 1; /* 1-based counting */
    let mut old_iter = old_image.lines_with_terminator();
    let mut new_cursor = 1; /* 1-based counting */
    let mut new_iter = new_image.lines_with_terminator();
    let mut result_image: BString = Vec::with_capacity(old_image.len().max(new_image.len())).into();

    // To each selected hunk, put the old-lines into a buffer.
    // Skip over the old hunk in old hunk in old lines.
    // Skip all new lines till the beginning of the new hunk.
    // Write the new hunk.
    // Repeat for each hunk, and write all remaining old lines.
    for selected_hunk in hunks {
        let old_skips = (selected_hunk.old_start as usize)
            .checked_sub(old_cursor)
            .with_context(|| {
                format!(
                    "`old_skips = start({start}) - cursor({old_cursor})` mut be >= 0, hunk = {selected_hunk:?}",
                    start = selected_hunk.old_start
                )
            })?;
        let catchup_base_lines = old_iter.by_ref().take(old_skips);
        for old_line in catchup_base_lines {
            result_image.extend_from_slice(old_line);
        }
        let _consume_old_hunk_to_replace_with_new = old_iter
            .by_ref()
            .take(selected_hunk.old_lines as usize)
            .count();
        old_cursor += old_skips + selected_hunk.old_lines as usize;

        let new_skips = (selected_hunk.new_start as usize)
            .checked_sub(new_cursor)
            .context("hunks for new lines must be in order")?;
        if selected_hunk.new_lines == 0 {
            let _explicit_skips = new_iter.by_ref().take(new_skips).count();
        } else {
            let new_hunk_lines = new_iter
                .by_ref()
                .skip(new_skips)
                .take(selected_hunk.new_lines as usize);
            for new_line in new_hunk_lines {
                result_image.extend_from_slice(new_line);
            }
        }
        new_cursor += new_skips + selected_hunk.new_lines as usize;
    }

    for line in old_iter {
        result_image.extend_from_slice(line);
    }
    Ok(result_image)
}

// TODO: one day make `HunkHeader` use this type instead of loose fields.
impl HunkHeader {
    /// Return our old-range as self-contained structure.
    pub fn old_range(&self) -> HunkRange {
        HunkRange {
            start: self.old_start,
            lines: self.old_lines,
        }
    }

    /// Return our new-range as self-contained structure.
    pub fn new_range(&self) -> HunkRange {
        HunkRange {
            start: self.new_start,
            lines: self.new_lines,
        }
    }

    /// Return `true` if this hunk is fully contained in the other hunk.
    pub fn contains(self, other: HunkHeader) -> bool {
        self.old_range().contains(other.old_range()) && self.new_range().contains(other.new_range())
    }
}

impl std::fmt::Debug for HunkHeader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            r#"HunkHeader("-{},{}", "+{},{}")"#,
            self.old_start, self.old_lines, self.new_start, self.new_lines
        )
    }
}

impl HunkRange {
    /// Calculate the line number that is one past of what we include, i.e. the first excluded line number.
    pub fn end(&self) -> u32 {
        self.start + self.lines
    }
    /// Calculate line number of the last line.
    pub fn last_line(&self) -> u32 {
        if self.lines == 0 {
            return self.start;
        }
        self.start + self.lines - 1
    }
    /// Return `true` if a hunk with `start` and `lines` is fully contained in this hunk.
    pub fn contains(self, other: HunkRange) -> bool {
        other.start >= self.start && other.end() <= self.end()
    }

    /// Return `true` if this range is equal to or intersects with the other
    /// range.
    pub fn intersects(self, other: HunkRange) -> bool {
        if self.start <= other.start && other.start <= self.last_line() {
            return true;
        }

        if self.start <= other.last_line() && other.last_line() <= self.last_line() {
            return true;
        }

        if other.start <= self.start && self.start <= other.last_line() {
            return true;
        }

        if other.start <= self.last_line() && self.last_line() <= other.last_line() {
            return true;
        }

        false
    }

    /// Return `true` if this range is a null-range, a marker value that doesn't happen.
    pub fn is_null(&self) -> bool {
        self.start == 0 && self.lines == 0
    }
}

impl From<DiffHunk> for HunkHeader {
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

#[cfg(test)]
mod test {
    use super::*;

    mod contains {
        use super::*;

        #[test]
        fn contains_returns_true_if_a_smaller_range_is_inside_a_larger_range() {
            let larger = HunkRange {
                start: 1,
                lines: 10,
            };
            let smaller = HunkRange { start: 2, lines: 5 };
            assert!(larger.contains(smaller));
            assert!(!smaller.contains(larger));
        }

        #[test]
        fn contains_returns_true_if_two_equal_ranges() {
            let range = HunkRange {
                start: 1,
                lines: 10,
            };
            assert!(range.contains(range));

            let zero_range = HunkRange { start: 1, lines: 0 };
            assert!(zero_range.contains(zero_range));
        }

        #[test]
        fn a_zero_range_does_not_contain_zero_range_next_to_it() {
            let zero_range = HunkRange { start: 1, lines: 0 };
            let next_to_zero_range = HunkRange { start: 2, lines: 0 };
            assert!(!zero_range.contains(next_to_zero_range));
            assert!(!next_to_zero_range.contains(zero_range));
        }

        #[test]
        fn a_one_range_contains_a_zero_range() {
            let one_range = HunkRange { start: 1, lines: 1 };
            let zero_range = HunkRange { start: 1, lines: 0 };
            assert!(one_range.contains(zero_range));
            assert!(!zero_range.contains(one_range));
        }
    }

    mod intersects {
        use super::*;

        #[test]
        fn intersects_returns_true_if_a_smaller_range_is_inside_a_larger_range() {
            let larger = HunkRange {
                start: 1,
                lines: 10,
            };
            let smaller = HunkRange { start: 2, lines: 5 };
            assert!(larger.intersects(smaller));
            assert!(smaller.intersects(larger));
        }

        #[test]
        fn intersects_returns_true_if_two_equal_ranges() {
            let range = HunkRange {
                start: 1,
                lines: 10,
            };
            assert!(range.intersects(range));

            let zero_range = HunkRange { start: 1, lines: 0 };
            assert!(zero_range.intersects(zero_range));
        }

        #[test]
        fn a_zero_range_does_not_intersects_zero_range_next_to_it() {
            let zero_range = HunkRange { start: 1, lines: 0 };
            let next_to_zero_range = HunkRange { start: 2, lines: 0 };
            assert!(!zero_range.intersects(next_to_zero_range));
            assert!(!next_to_zero_range.intersects(zero_range));
        }

        #[test]
        fn a_one_range_intersects_a_zero_range() {
            let one_range = HunkRange { start: 1, lines: 1 }; // Line 1
            let zero_range = HunkRange { start: 1, lines: 0 }; // No lines
            assert!(one_range.intersects(zero_range));
            assert!(zero_range.intersects(one_range));
        }

        #[test]
        fn a_one_range_intersects_a_zero_range_next_to_it() {
            let one_range = HunkRange { start: 1, lines: 1 }; // Line 1
            let zero_range = HunkRange { start: 2, lines: 0 }; // No lines
            assert!(!one_range.intersects(zero_range));
            assert!(!zero_range.intersects(one_range));
        }

        #[test]
        fn a_one_range_intersects_a_zero_range_before_it() {
            let one_range = HunkRange { start: 1, lines: 1 }; // Line 1
            let zero_range = HunkRange { start: 0, lines: 0 }; // No lines
            assert!(!one_range.intersects(zero_range));
            assert!(!zero_range.intersects(one_range));
        }

        #[test]
        fn ranges_that_are_not_fully_contained_in_each_other_intersects() {
            let left = HunkRange {
                start: 1,
                lines: 10,
            };
            let right = HunkRange {
                start: 10,
                lines: 10,
            };
            assert!(left.intersects(right));
            assert!(right.intersects(left));
        }

        #[test]
        fn ranges_that_are_next_to_each_other_but_not_intersecting() {
            let left = HunkRange {
                start: 1,
                lines: 10,
            };
            let right = HunkRange {
                start: 11,
                lines: 10,
            };
            assert!(!left.intersects(right));
            assert!(!right.intersects(left));
        }
    }
}
