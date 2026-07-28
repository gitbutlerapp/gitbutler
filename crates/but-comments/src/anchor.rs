//! Extracting anchorable lines from a file's unified diff, and re-locating an anchor in them.

use bstr::ByteSlice;

use crate::DiffSide;

/// How many diff rows before and after the anchored line the context excerpt includes.
const CONTEXT_ROWS: usize = 4;

/// One anchorable line of a file's diff. Context lines yield two instances, one per side.
#[derive(Debug, PartialEq)]
pub(crate) struct DiffLine {
    pub(crate) side: DiffSide,
    /// 1-based, in `side`'s coordinates.
    pub(crate) line_number: u32,
    /// The line content without the leading `+`/`-`/space marker and without the line terminator.
    pub(crate) content: String,
    /// Index into [`FileDiffLines::hunk_texts`] of the hunk this line belongs to.
    hunk_idx: usize,
    /// Row index of this line within the hunk's text (row 0 is the `@@` header).
    row_idx: usize,
}

/// All anchorable lines of one file's diff, plus the raw hunk texts for context excerpts.
#[derive(Debug, Default)]
pub(crate) struct FileDiffLines {
    /// The unified-diff text of each hunk (including the `@@` header), lossily decoded.
    hunk_texts: Vec<String>,
    lines: Vec<DiffLine>,
}

impl FileDiffLines {
    pub(crate) fn from_hunks(hunks: &[but_core::unified_diff::DiffHunk]) -> Self {
        let hunk_texts: Vec<String> = hunks
            .iter()
            .map(|hunk| hunk.diff.to_str_lossy().into_owned())
            .collect();
        let mut lines = Vec::new();
        for (hunk_idx, (hunk, text)) in hunks.iter().zip(&hunk_texts).enumerate() {
            let mut old_number = hunk.old_start;
            let mut new_number = hunk.new_start;
            for (row_idx, row) in text.lines().enumerate() {
                let mut push = |side, line_number, content: &str| {
                    lines.push(DiffLine {
                        side,
                        line_number,
                        content: content.to_string(),
                        hunk_idx,
                        row_idx,
                    });
                };
                match row.as_bytes().first() {
                    Some(b'@') => {}
                    Some(b'\\') => {} // "\ No newline at end of file"
                    Some(b'+') => {
                        push(DiffSide::New, new_number, &row[1..]);
                        new_number += 1;
                    }
                    Some(b'-') => {
                        push(DiffSide::Old, old_number, &row[1..]);
                        old_number += 1;
                    }
                    // A context line: a leading space, or a fully empty row which some diffs
                    // produce for empty context lines.
                    _ => {
                        let content = row.get(1..).unwrap_or("");
                        push(DiffSide::Old, old_number, content);
                        push(DiffSide::New, new_number, content);
                        old_number += 1;
                        new_number += 1;
                    }
                }
            }
        }
        FileDiffLines { hunk_texts, lines }
    }

    /// The line at exactly `(side, line_number)`, if the diff contains it.
    pub(crate) fn line_at(&self, side: DiffSide, line_number: u32) -> Option<&DiffLine> {
        self.lines
            .iter()
            .find(|line| line.side == side && line.line_number == line_number)
    }

    /// Re-locate an anchor among all same-side lines with equal content, or `None` when the
    /// content is gone from this side of the diff.
    ///
    /// Identical lines are common in code, so candidates are ranked by how many of the
    /// snapshotted neighbouring lines still sit next to them, and only then by distance to the
    /// stored position (the exact stored position has distance zero and wins any remaining tie).
    /// Neighbour agreement outranking position also un-sticks a stale anchor whose position
    /// still holds look-alike content while the real line moved elsewhere.
    pub(crate) fn locate(
        &self,
        side: DiffSide,
        line_number: u32,
        content: &str,
        line_before: Option<&str>,
        line_after: Option<&str>,
    ) -> Option<&DiffLine> {
        self.lines
            .iter()
            .filter(|line| line.side == side && line.content == content)
            .min_by_key(|line| {
                let neighbors = self.matching_neighbors(line, line_before, line_after);
                (
                    std::cmp::Reverse(neighbors),
                    line.line_number.abs_diff(line_number),
                )
            })
    }

    /// How many of the snapshotted neighbour lines match the lines currently adjacent to
    /// `line` on its side. Unknown snapshots (`None`) never count as a match.
    fn matching_neighbors(
        &self,
        line: &DiffLine,
        line_before: Option<&str>,
        line_after: Option<&str>,
    ) -> u32 {
        let neighbor_matches = |neighbor_number: Option<u32>, snapshot: Option<&str>| {
            let (Some(neighbor_number), Some(snapshot)) = (neighbor_number, snapshot) else {
                return false;
            };
            self.line_at(line.side, neighbor_number)
                .is_some_and(|neighbor| neighbor.content == snapshot)
        };
        u32::from(neighbor_matches(
            line.line_number.checked_sub(1),
            line_before,
        )) + u32::from(neighbor_matches(
            line.line_number.checked_add(1),
            line_after,
        ))
    }

    /// A unified-diff excerpt of up to [`CONTEXT_ROWS`] rows around `line` within its hunk.
    pub(crate) fn context_excerpt(&self, line: &DiffLine) -> String {
        let rows: Vec<&str> = self.hunk_texts[line.hunk_idx].lines().collect();
        // Row 0 is the `@@` header, which the excerpt does not include.
        let first = line.row_idx.saturating_sub(CONTEXT_ROWS).max(1);
        let last = (line.row_idx + CONTEXT_ROWS).min(rows.len().saturating_sub(1));
        rows[first..=last].join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use but_core::unified_diff::DiffHunk;

    fn hunk(old_start: u32, new_start: u32, diff: &str) -> DiffHunk {
        DiffHunk {
            old_start,
            old_lines: diff.lines().filter(|l| !l.starts_with(['+', '@'])).count() as u32,
            new_start,
            new_lines: diff.lines().filter(|l| !l.starts_with(['-', '@'])).count() as u32,
            diff: diff.into(),
        }
    }

    fn file() -> FileDiffLines {
        FileDiffLines::from_hunks(&[
            hunk(
                1,
                1,
                "@@ -1,3 +1,4 @@\n context one\n-removed line\n+added line\n+another added\n context two\n",
            ),
            hunk(
                10,
                11,
                "@@ -10,2 +11,2 @@\n context ten\n-old tail\n+new tail\n",
            ),
        ])
    }

    #[test]
    fn extracts_lines_with_correct_numbers_and_sides() {
        let file = file();
        // Hunk 1: old side is context one (1), removed line (2), context two (3);
        // new side is context one (1), added line (2), another added (3), context two (4).
        assert_eq!(
            file.line_at(DiffSide::Old, 2).unwrap().content,
            "removed line"
        );
        assert_eq!(
            file.line_at(DiffSide::New, 2).unwrap().content,
            "added line"
        );
        assert_eq!(
            file.line_at(DiffSide::New, 3).unwrap().content,
            "another added"
        );
        assert_eq!(
            file.line_at(DiffSide::Old, 3).unwrap().content,
            "context two"
        );
        assert_eq!(
            file.line_at(DiffSide::New, 4).unwrap().content,
            "context two"
        );
        // Hunk 2 continues in its own coordinates.
        assert_eq!(file.line_at(DiffSide::Old, 11).unwrap().content, "old tail");
        assert_eq!(file.line_at(DiffSide::New, 12).unwrap().content, "new tail");
        // Lines outside any hunk do not exist.
        assert_eq!(file.line_at(DiffSide::New, 7), None);
    }

    #[test]
    fn locate_prefers_exact_position() {
        let file =
            FileDiffLines::from_hunks(&[hunk(1, 1, "@@ -1,2 +1,3 @@\n+dup\n context\n+dup\n")]);
        // Both line 1 and line 3 on the new side say "dup"; the stored position wins.
        assert_eq!(
            file.locate(DiffSide::New, 3, "dup", None, None)
                .unwrap()
                .line_number,
            3
        );
    }

    #[test]
    fn locate_rematches_drifted_line_by_content() {
        let file = file();
        // The comment was on "added line" when it was at line 5; an edit above moved it to line 2.
        let line = file
            .locate(DiffSide::New, 5, "added line", None, None)
            .unwrap();
        assert_eq!(line.line_number, 2);
        // Content that is gone from the diff no longer anchors.
        assert_eq!(
            file.locate(DiffSide::New, 2, "deleted content", None, None),
            None
        );
        // Same content on the wrong side does not anchor.
        assert_eq!(
            file.locate(DiffSide::Old, 2, "added line", None, None),
            None
        );
    }

    #[test]
    fn locate_picks_nearest_of_multiple_matches() {
        let file = FileDiffLines::from_hunks(&[hunk(
            1,
            1,
            "@@ -1,3 +1,6 @@\n+dup\n a\n+dup\n b\n+dup\n c\n",
        )]);
        // New side: dup(1) a(2) dup(3) b(4) dup(5) c(6). Anchor stored at 4 → nearest dup is 3 or 5;
        // min_by_key returns the first minimum, which is the earlier line.
        assert_eq!(
            file.locate(DiffSide::New, 4, "dup", None, None)
                .unwrap()
                .line_number,
            3
        );
        assert_eq!(
            file.locate(DiffSide::New, 6, "dup", None, None)
                .unwrap()
                .line_number,
            5
        );
    }

    #[test]
    fn locate_disambiguates_identical_lines_by_neighbors() {
        let file = FileDiffLines::from_hunks(&[hunk(
            1,
            1,
            "@@ -1,0 +1,6 @@\n+fn a() {\n+dup\n+x\n+fn b() {\n+dup\n+y\n",
        )]);
        // The stored position (2) holds look-alike content, but the snapshotted neighbours
        // identify the second twin at line 5 — neighbour agreement outranks distance.
        assert_eq!(
            file.locate(DiffSide::New, 2, "dup", Some("fn b() {"), Some("y"))
                .unwrap()
                .line_number,
            5
        );
        // With one agreeing neighbour on each candidate the tie falls back to distance.
        assert_eq!(
            file.locate(DiffSide::New, 2, "dup", Some("fn a() {"), Some("y"))
                .unwrap()
                .line_number,
            2
        );
        // Without neighbour snapshots the nearest twin wins, as before.
        assert_eq!(
            file.locate(DiffSide::New, 2, "dup", None, None)
                .unwrap()
                .line_number,
            2
        );
    }

    #[test]
    fn context_excerpt_is_a_window_within_the_hunk() {
        let file = file();
        let line = file.line_at(DiffSide::New, 2).unwrap();
        // The whole hunk is smaller than the window, so everything but the header is included.
        assert_eq!(
            file.context_excerpt(line),
            " context one\n-removed line\n+added line\n+another added\n context two"
        );
        let tail = file.line_at(DiffSide::New, 12).unwrap();
        assert_eq!(
            file.context_excerpt(tail),
            " context ten\n-old tail\n+new tail"
        );
    }

    #[test]
    fn crlf_terminators_do_not_leak_into_content() {
        let file = FileDiffLines::from_hunks(&[hunk(1, 1, "@@ -1,1 +1,1 @@\r\n-a\r\n+b\r\n")]);
        assert_eq!(file.line_at(DiffSide::New, 1).unwrap().content, "b");
    }
}
