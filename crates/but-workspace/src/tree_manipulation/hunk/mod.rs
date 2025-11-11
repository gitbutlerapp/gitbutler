use anyhow::Context;
use but_core::{HunkHeader, HunkRange};

#[derive(Debug, Copy, Clone)]
pub(crate) enum HunkSubstraction {
    /// Subtract the range from `old`.
    Old(HunkRange),
    /// Subtract the range from `new`.
    New(HunkRange),
}

/// Like a boolean subtraction, remove `subtractions` from `hunk`, and return the remaining pieces.
/// Note that the old and new ranges in `hunk` are split in lock-step, so that cutting out a piece from old will take
/// the respective amount of lines from new if these are available.
#[expect(clippy::indexing_slicing)]
pub(crate) fn subtract_hunks(
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
