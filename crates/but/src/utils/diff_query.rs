//! Query filtering shared by human and JSON diff output.
use std::collections::HashMap;

use bstr::{BString, ByteSlice as _};
use but_core::{SingleHunk, TreeStatusKind, UnifiedPatch, unified_diff::DiffHunk};
use but_lisp::{ChangedLine, File, FileStatus, Query, Side};

pub fn parse(source: &str) -> crate::CliResult<Query> {
    Query::parse(source).map_err(|error| {
        let mut rendered = String::new();
        miette::GraphicalReportHandler::new_themed(miette::GraphicalTheme::unicode_nocolor())
            .render_report(&mut rendered, &error)
            .expect("rendering a diagnostic into a string cannot fail");
        crate::bad_input(rendered).into()
    })
}

fn file(path: &[u8], status: TreeStatusKind) -> File<'_> {
    File {
        path,
        status: match status {
            TreeStatusKind::Addition => FileStatus::Added,
            TreeStatusKind::Deletion => FileStatus::Deleted,
            TreeStatusKind::Modification => FileStatus::Modified,
            TreeStatusKind::Rename => FileStatus::Renamed,
        },
    }
}

/// Hunks must be supplied in file order. Offsets describe the selected patch's
/// new image, while query ranges always use the original diff's coordinates.
pub struct Filter<'a> {
    query: Option<&'a Query>,
    offsets: HashMap<BString, i64>,
}

impl<'a> Filter<'a> {
    pub fn new(query: Option<&'a Query>) -> Filter<'a> {
        Filter {
            query,
            offsets: HashMap::new(),
        }
    }

    pub fn non_text(&self, path: &[u8], status: TreeStatusKind) -> bool {
        self.query
            .is_none_or(|query| query.selects_non_text(file(path, status)))
    }

    pub fn single_hunk(
        &mut self,
        status: TreeStatusKind,
        mut hunk: SingleHunk,
    ) -> Option<SingleHunk> {
        if self.query.is_none() {
            return Some(hunk);
        }
        let (Some(header), Some(diff)) = (hunk.hunk_header, hunk.diff.as_ref()) else {
            return self.non_text(&hunk.path, status).then_some(hunk);
        };
        let filtered = self.hunk(
            file(&hunk.path, status),
            DiffHunk {
                old_start: header.old_start,
                old_lines: header.old_lines,
                new_start: header.new_start,
                new_lines: header.new_lines,
                diff: diff.clone(),
            },
        )?;
        hunk.hunk_header = Some(but_core::HunkHeader {
            old_start: filtered.old_start,
            old_lines: filtered.old_lines,
            new_start: filtered.new_start,
            new_lines: filtered.new_lines,
        });
        hunk.diff = Some(filtered.diff);
        Some(hunk)
    }

    pub fn patch(
        &mut self,
        path: &[u8],
        status: TreeStatusKind,
        patch: UnifiedPatch,
    ) -> Option<UnifiedPatch> {
        if self.query.is_none() {
            return Some(patch);
        }
        match patch {
            UnifiedPatch::Patch {
                hunks,
                is_result_of_binary_to_text_conversion,
                ..
            } if !hunks.is_empty() => {
                let hunks: Vec<_> = hunks
                    .into_iter()
                    .filter_map(|hunk| self.hunk(file(path, status), hunk))
                    .collect();
                if hunks.is_empty() {
                    return None;
                }
                let mut lines_added = 0;
                let mut lines_removed = 0;
                for hunk in &hunks {
                    for line in hunk.diff.lines().skip(1) {
                        match line.first() {
                            Some(b'+') => lines_added += 1,
                            Some(b'-') => lines_removed += 1,
                            _ => {}
                        }
                    }
                }
                Some(UnifiedPatch::Patch {
                    hunks,
                    lines_added,
                    lines_removed,
                    is_result_of_binary_to_text_conversion,
                })
            }
            patch => self.non_text(path, status).then_some(patch),
        }
    }

    fn hunk(&mut self, file: File<'_>, hunk: DiffHunk) -> Option<DiffHunk> {
        let Some(query) = self.query else {
            return Some(hunk);
        };
        let mut old = hunk.old_start;
        let mut new = hunk.new_start;
        let mut lines = Vec::new();
        for line in hunk.diff.lines().skip(1) {
            match line.first() {
                Some(b'+') => {
                    lines.push(ChangedLine {
                        side: Side::Added,
                        number: new,
                        content: &line[1..],
                    });
                    new += 1;
                }
                Some(b'-') => {
                    lines.push(ChangedLine {
                        side: Side::Removed,
                        number: old,
                        content: &line[1..],
                    });
                    old += 1;
                }
                Some(b' ') => {
                    old += 1;
                    new += 1;
                }
                _ => {}
            }
        }
        let selected = query.select(file, &lines);
        if !selected.iter().any(|selected| *selected) {
            return None;
        }
        let all_selected = selected.iter().all(|selected| *selected);
        let mut selected = selected.into_iter();
        let mut body = Vec::new();
        let mut new_lines = 0;
        let mut kept_previous = false;
        for line in hunk.diff.lines_with_terminator().skip(1) {
            match line.first() {
                Some(b'+') => {
                    kept_previous = selected.next().expect("one selection per changed line");
                    if kept_previous {
                        body.extend_from_slice(line);
                        new_lines += 1;
                    }
                }
                Some(b'-') => {
                    let keep = selected.next().expect("one selection per changed line");
                    body.push(if keep { b'-' } else { b' ' });
                    body.extend_from_slice(&line[1..]);
                    if !keep {
                        new_lines += 1;
                    }
                    kept_previous = true;
                }
                Some(b' ') => {
                    body.extend_from_slice(line);
                    new_lines += 1;
                    kept_previous = true;
                }
                Some(b'\\') if kept_previous => body.extend_from_slice(line),
                _ => {}
            }
        }
        let offset = self.offsets.entry(file.path.into()).or_default();
        // Core hunk starts are one-based, including empty sides.
        let start = i64::from(hunk.old_start) + *offset;
        let new_start =
            u32::try_from(start).expect("selected patch coordinates fit the original file");
        *offset += i64::from(new_lines) - i64::from(hunk.old_lines);
        if all_selected && new_start == hunk.new_start {
            return Some(hunk);
        }
        // Preserve any function context following the hunk header.
        let header = hunk.diff.lines().next().unwrap_or_default();
        let suffix = header
            .get(2..)
            .and_then(|rest| rest.find(b"@@").map(|pos| &rest[pos + 2..]))
            .unwrap_or_default();
        let mut diff = format!(
            "@@ -{},{} +{},{} @@",
            hunk.old_start, hunk.old_lines, new_start, new_lines
        )
        .into_bytes();
        diff.extend_from_slice(suffix);
        diff.push(b'\n');
        diff.extend_from_slice(&body);
        Some(DiffHunk {
            new_start,
            new_lines,
            diff: diff.into(),
            ..hunk
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Filter, file};
    use but_core::{TreeStatusKind, UnifiedPatch, unified_diff::DiffHunk};
    use but_lisp::Query;
    use snapbox::IntoData as _;

    #[test]
    fn omitted_hunks_do_not_shift_the_selected_image() {
        let query = Query::parse("(line-added :range '(12 12))").unwrap();
        let patch = UnifiedPatch::Patch {
            hunks: Vec::from([
                DiffHunk {
                    old_start: 1,
                    old_lines: 1,
                    new_start: 1,
                    new_lines: 2,
                    diff: "@@ -1,1 +1,2 @@\n a\n+skip\n".into(),
                },
                DiffHunk {
                    old_start: 10,
                    old_lines: 2,
                    new_start: 11,
                    new_lines: 3,
                    diff: "@@ -10,2 +11,3 @@ function\n b\n+keep\n c\n".into(),
                },
            ]),
            lines_added: 2,
            lines_removed: 0,
            is_result_of_binary_to_text_conversion: false,
        };
        let patch = Filter::new(Some(&query))
            .patch(b"file", TreeStatusKind::Modification, patch)
            .unwrap();
        let UnifiedPatch::Patch {
            hunks,
            lines_added,
            lines_removed,
            ..
        } = patch
        else {
            panic!("text remains a patch")
        };
        assert_eq!(
            hunks.len(),
            1,
            "only the second hunk matches the original new-file coordinates"
        );
        assert_eq!(
            (lines_added, lines_removed),
            (1, 0),
            "statistics describe selected changes"
        );
        assert_eq!(
            (hunks[0].new_start, hunks[0].new_lines),
            (10, 3),
            "excluded additions do not affect preview coordinates"
        );
        snapbox::assert_data_eq!(
            hunks[0].diff.to_string(),
            "@@ -10,2 +10,3 @@ function\n b\n+keep\n c\n"
        );
    }

    #[test]
    fn retained_removals_shift_later_selected_hunks() {
        let query = Query::parse(r#"(line-added :contains "keep")"#).unwrap();
        let mut filter = Filter::new(Some(&query));
        let first = DiffHunk {
            old_start: 1,
            old_lines: 1,
            new_start: 1,
            new_lines: 1,
            diff: "@@ -1,1 +1,1 @@\n-old\n+keep\n".into(),
        };
        let second = DiffHunk {
            old_start: 10,
            old_lines: 1,
            new_start: 10,
            new_lines: 1,
            diff: "@@ -10,1 +10,1 @@\n-later old\n+keep later\n".into(),
        };
        let first = filter
            .hunk(file(b"file", TreeStatusKind::Modification), first)
            .unwrap();
        let second = filter
            .hunk(file(b"file", TreeStatusKind::Modification), second)
            .unwrap();
        assert_eq!(
            (first.new_start, first.new_lines),
            (1, 2),
            "retaining the removal adds a line to the preview"
        );
        assert_eq!(
            (second.new_start, second.new_lines),
            (11, 2),
            "later hunks account for earlier selected changes"
        );
        snapbox::assert_data_eq!(
            second.diff.to_string(),
            "@@ -10,1 +11,2 @@\n later old\n+keep later\n"
        );
    }

    #[test]
    fn removed_lines_use_old_ranges_and_preserve_eof_markers() {
        let query = Query::parse("(line-removed :range '(5 5))").unwrap();
        let hunk = DiffHunk { old_start: 5, old_lines: 1, new_start: 9, new_lines: 1, diff: "@@ -5,1 +9,1 @@\n-old\n\\ No newline at end of file\n+new\n\\ No newline at end of file\n".into() };
        let hunk = Filter::new(Some(&query))
            .hunk(file(b"file", TreeStatusKind::Modification), hunk)
            .unwrap();
        assert_eq!(
            (hunk.old_lines, hunk.new_lines),
            (1, 0),
            "only the deletion contributes to the header"
        );
        snapbox::assert_data_eq!(
            hunk.diff.to_string(),
            snapbox::str![[r#"
@@ -5,1 +5,0 @@
-old
\ No newline at end of file

"#]]
            .raw()
        );
    }

    #[test]
    fn excluded_removals_become_context() {
        let query = Query::parse(r#"(line-added :contains "new")"#).unwrap();
        let hunk = DiffHunk {
            old_start: 1,
            old_lines: 2,
            new_start: 1,
            new_lines: 2,
            diff: "@@ -1,2 +1,2 @@\n context\n-old\n+new\n".into(),
        };
        let hunk = Filter::new(Some(&query))
            .hunk(file(b"file", TreeStatusKind::Modification), hunk)
            .unwrap();
        assert_eq!(
            (hunk.old_lines, hunk.new_lines),
            (2, 3),
            "keeping the original line grows the selected image"
        );
        snapbox::assert_data_eq!(
            hunk.diff.to_string(),
            "@@ -1,2 +1,3 @@\n context\n old\n+new\n"
        );
    }

    #[test]
    fn filtering_preserves_non_utf8_bytes() {
        let query = Query::parse(r#"(line :regex "(?-u:\\xFF)")"#).unwrap();
        let hunk = DiffHunk {
            old_start: 1,
            old_lines: 0,
            new_start: 1,
            new_lines: 2,
            diff: b"@@ -1,0 +1,2 @@\n+\xff\n+skip\n".to_vec().into(),
        };
        let hunk = Filter::new(Some(&query))
            .hunk(file(b"file", TreeStatusKind::Addition), hunk)
            .unwrap();
        assert_eq!(
            hunk.diff.as_slice(),
            b"@@ -1,0 +1,1 @@\n+\xff\n",
            "query matching and patch construction preserve raw file bytes"
        );
    }
}
