use anyhow::Context as _;
use bstr::{BStr, BString, ByteSlice};
use serde::{Deserialize, Serialize};

use crate::{DiffSpec, HunkHeader, TreeChange, UnifiedPatch};

/// A single hunk of an uncommitted change, identified by its `path` and `hunk_header`.
///
/// Unlike `but_hunk_assignment::HunkAssignment` this carries no assignment state. It is a
/// pure function of the diff, so it needs no database and is meaningful in any checkout.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "export-schema", derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SingleHunk {
    /// The hunk within `path`.
    ///
    /// `None` for binary files, files too large to diff, and whole-file changes, where
    /// `path` is the only identity.
    pub hunk_header: Option<HunkHeader>,
    /// The worktree-relative path of the file this hunk belongs to.
    #[serde(rename = "pathBytes")]
    #[cfg_attr(
        feature = "export-schema",
        schemars(schema_with = "but_schemars::bstring_bytes")
    )]
    pub path: BString,
    /// The hunk's unified-diff text, for internal use only.
    ///
    /// Not serialized: no API or SDK consumer reads it, and carrying every hunk's patch
    /// text would bloat the payloads this type round-trips through.
    #[serde(skip)]
    pub diff: Option<BString>,
}
#[cfg(feature = "export-schema")]
but_schemars::register_sdk_type!(SingleHunk);

impl SingleHunk {
    /// Split `change` into the hunks of `patch`.
    ///
    /// Changes without sub-file identity — binaries, files too large to diff, patches
    /// produced by a binary-to-text filter, and changes whose patch couldn't be computed
    /// at all — yield a single whole-file hunk rather than disappearing.
    pub fn from_tree_change(change: &TreeChange, patch: Option<UnifiedPatch>) -> Vec<Self> {
        let hunks = match &patch {
            Some(UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion,
                ..
            }) if !is_result_of_binary_to_text_conversion => hunks.as_slice(),
            Some(
                UnifiedPatch::Binary | UnifiedPatch::TooLarge { .. } | UnifiedPatch::Patch { .. },
            )
            | None => &[],
        };
        if hunks.is_empty() {
            return vec![SingleHunk {
                hunk_header: None,
                path: change.path.clone(),
                diff: None,
            }];
        }
        hunks
            .iter()
            .map(|hunk| SingleHunk {
                hunk_header: Some(hunk.into()),
                path: change.path.clone(),
                diff: Some(hunk.diff.clone()),
            })
            .collect()
    }

    /// Return the `(added, removed)` line numbers of this hunk, or `None` if it has no
    /// hunk header or no diff text to derive them from.
    ///
    /// Added line numbers refer to the new version of the file, removed ones to the old
    /// version, both 1-based and inclusive of the start.
    pub fn line_nums(&self) -> Option<(Vec<usize>, Vec<usize>)> {
        let diff = self.diff.as_ref()?;
        let header = self.hunk_header?;
        Some(line_nums_from_hunk(
            diff,
            header.old_start,
            header.new_start,
        ))
    }

    /// Return `true` if `other` addresses the same hunk, i.e. the same range in the same file.
    ///
    /// The diff content is deliberately not compared: hunks read from the worktree at two
    /// different times must still match even if the file changed in between.
    pub fn identifies_same_hunk(&self, other: &Self) -> bool {
        self.hunk_header == other.hunk_header && self.path == other.path
    }
}

impl From<SingleHunk> for DiffSpec {
    fn from(value: SingleHunk) -> Self {
        DiffSpec {
            previous_path: None, // TODO
            path: value.path,
            hunk_headers: value.hunk_header.into_iter().collect(),
        }
    }
}

/// Given a `diff`, extract the line numbers that were added and removed in the hunk, returned
/// as `(added, removed)`.
///
/// The line numbers are relative to the start of the hunk (`new_start` and `old_start`
/// respectively). The start is inclusive.
fn line_nums_from_hunk(diff: &BString, old_start: u32, new_start: u32) -> (Vec<usize>, Vec<usize>) {
    let mut line_nums_removed_old = vec![];
    let mut line_nums_added_new = vec![];
    let mut old_line_num = old_start as usize;
    let mut new_line_num = new_start as usize;
    for line in diff.lines() {
        let Some(first_char) = line.first() else {
            continue;
        };
        match *first_char {
            b'+' => {
                // Line added in new version
                line_nums_added_new.push(new_line_num);
                new_line_num += 1;
            }
            b'-' => {
                // Line removed from old version
                line_nums_removed_old.push(old_line_num);
                old_line_num += 1;
            }
            b'@' => {
                // Header line, skip
                continue;
            }
            // Context lines (' ') and anything else are treated as unchanged.
            _ => {
                old_line_num += 1;
                new_line_num += 1;
            }
        }
    }
    (line_nums_added_new, line_nums_removed_old)
}

/// Split every uncommitted change in `repo` into the hunks that address it, using
/// `context_lines` of context in each patch.
///
/// Hunks are a pure function of the diff, so this needs no database and works for any
/// checkout, not just the one a workspace is on. Callers that already hold the worktree
/// changes should use [`hunks_from_changes()`] to avoid diffing the worktree twice.
pub fn worktree_hunks(
    repo: &gix::Repository,
    context_lines: u32,
) -> anyhow::Result<Vec<SingleHunk>> {
    Ok(hunks_from_changes(
        repo,
        crate::diff::worktree_changes(repo)?.changes,
        context_lines,
    ))
}

/// Split each of `changes` into the hunks that address it, using `context_lines` of context
/// in each patch.
pub fn hunks_from_changes(
    repo: &gix::Repository,
    changes: impl IntoIterator<Item = impl Into<TreeChange>>,
    context_lines: u32,
) -> Vec<SingleHunk> {
    let mut hunks = Vec::new();
    for change in changes {
        let change = change.into();
        let patch = change.unified_patch(repo, context_lines).ok().flatten();
        hunks.extend(SingleHunk::from_tree_change(&change, patch));
    }
    hunks
}

/// Convert `hunk` into a diff spec, completing it with the rename, addition and deletion
/// metadata of the matching entry in `changes`.
pub fn diff_spec_with_changes(hunk: SingleHunk, changes: &[crate::ui::TreeChange]) -> DiffSpec {
    let path = hunk.path.clone();
    let mut spec = DiffSpec::from(hunk);
    for change in changes {
        if change.path_bytes != path {
            continue;
        }
        match &change.status {
            crate::ui::TreeStatus::Rename {
                previous_path_bytes,
                ..
            } => {
                spec.previous_path = Some(previous_path_bytes.clone());
            }
            crate::ui::TreeStatus::Addition { .. } | crate::ui::TreeStatus::Deletion { .. } => {
                spec.hunk_headers.clear();
            }
            crate::ui::TreeStatus::Modification { .. } => {}
        }
        break;
    }
    spec
}

/// Like [`diff_spec_with_changes()`], but for many `hunks` at once.
pub fn diff_specs_with_changes(
    hunks: impl IntoIterator<Item = SingleHunk>,
    changes: &[crate::ui::TreeChange],
) -> Vec<DiffSpec> {
    hunks
        .into_iter()
        .map(|hunk| diff_spec_with_changes(hunk, changes))
        .collect()
}

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

/// The range of a hunk as denoted by a 1-based starting line, and the amount of lines from there.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct HunkRange {
    /// The number of the first line in the hunk, 1 based.
    pub start: u32,
    /// The amount of lines in the range.
    ///
    /// If `0`, this is an empty hunk.
    pub lines: u32,
}

// TODO: one day make this struct use `HunkRange` instead of loose fields.
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
