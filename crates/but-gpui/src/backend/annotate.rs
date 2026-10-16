//! What [deltoids](https://github.com/juanibiapina/deltoids) adds to a diff: the words
//! that changed within a line, and the function or type each hunk sits in.

use std::ops::Range;

use deltoids::{EmphKind, LineEmphasis};

use super::model::{DiffLine, Hunk, LineKind};

/// Change runs whose removed bytes × added bytes exceed this are left plain: pairing lines
/// up costs about that much time and memory, so one changed line of minified code could
/// otherwise take seconds and gigabytes.
const EMPHASIS_BUDGET: usize = 1 << 21;

/// Mark the changed words of each removed line that pairs up with an added one.
pub fn emphasize(hunk: &mut Hunk) {
    let mut start = 0;
    while start < hunk.lines.len() {
        let end = hunk.lines[start..]
            .iter()
            .position(|line| line.kind == LineKind::Context)
            .map_or(hunk.lines.len(), |len| start + len);
        if end > start {
            emphasize_run(&mut hunk.lines[start..end]);
            start = end;
        } else {
            start += 1;
        }
    }
}

fn emphasize_run(run: &mut [DiffLine]) {
    let (removed, added): (Vec<usize>, Vec<usize>) =
        (0..run.len()).partition(|&ix| run[ix].kind == LineKind::Removed);
    let bytes = |ixs: &[usize]| ixs.iter().map(|&ix| run[ix].text.len()).sum::<usize>();
    if removed.is_empty() || added.is_empty() || bytes(&removed) * bytes(&added) > EMPHASIS_BUDGET {
        return;
    }
    let text = |ixs: &[usize]| {
        ixs.iter()
            .map(|&ix| run[ix].text.as_ref())
            .collect::<Vec<_>>()
    };
    let (minus, plus) = deltoids::compute_subhunk_emphasis(&text(&removed), &text(&added));
    for (ix, emphasis) in removed
        .into_iter()
        .zip(minus)
        .chain(added.into_iter().zip(plus))
    {
        run[ix].emph = ranges(&emphasis, run[ix].text.len());
    }
}

/// The changed sections as byte ranges, or nothing when the whole line changed, since
/// that says no more than the line's own color.
fn ranges(emphasis: &LineEmphasis, len: usize) -> Vec<Range<usize>> {
    let LineEmphasis::Paired(sections) = emphasis else {
        return Vec::new();
    };
    let mut at = 0;
    let mut ranges = Vec::new();
    for section in sections {
        let range = at..at + section.text.len();
        at = range.end;
        if section.kind == EmphKind::Emph && !range.is_empty() {
            ranges.push(range);
        }
    }
    let whole_line = ranges.len() == 1 && ranges[0] == (0..len);
    if at != len || whole_line {
        return Vec::new();
    }
    ranges
}

/// Name the scopes each hunk changes, outermost first, by matching its first changed line
/// against deltoids' scope-aware diff of the same two texts.
pub fn label_scopes(hunks: &mut [Hunk], old: &str, new: &str, path: &str) {
    let scoped = deltoids::Diff::compute(old, new, path);
    if scoped.language().is_none() {
        return;
    }
    for hunk in hunks {
        let Some(first) = hunk
            .lines
            .iter()
            .find(|line| line.kind != LineKind::Context)
        else {
            continue;
        };
        let (side_is_new, number) = match (first.new, first.old) {
            (Some(new), _) if first.kind == LineKind::Added => (true, new as usize),
            (_, Some(old)) => (false, old as usize),
            _ => continue,
        };
        let containing = scoped.hunks().iter().find(|candidate| {
            let (start, len) = if side_is_new {
                let len = candidate
                    .lines
                    .iter()
                    .filter(|l| l.kind != deltoids::LineKind::Removed);
                (candidate.new_start, len.count())
            } else {
                let len = candidate
                    .lines
                    .iter()
                    .filter(|l| l.kind != deltoids::LineKind::Added);
                (candidate.old_start, len.count())
            };
            (start..start + len).contains(&number)
        });
        if let Some(containing) = containing {
            hunk.scope = containing
                .ancestors
                .iter()
                .map(|scope| {
                    scope
                        .text
                        .trim()
                        .trim_end_matches('{')
                        .trim_end()
                        .to_owned()
                })
                .collect();
        }
    }
}
