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

/// The most unchanged lines a hunk's scope adds around it. A bigger scope, as a long
/// function, leaves the hunk its few lines of context and its name in the hunk's header.
const SCOPE_CONTEXT: usize = 20;

/// Name the scopes each hunk changes, outermost first, by matching its first changed line
/// against deltoids' scope-aware diff of the same two texts, and show the rest of the
/// innermost one around the hunk when that's at most [`SCOPE_CONTEXT`] more lines, as
/// deltoids grows a hunk's context to the scope it changes.
pub fn label_scopes(hunks: &mut [Hunk], old: &str, new: &str, path: &str) {
    let scoped = deltoids::Diff::compute(old, new, path);
    if scoped.language().is_none() {
        return;
    }
    let mut spans = Vec::with_capacity(hunks.len());
    for hunk in hunks.iter_mut() {
        let Some(first) = hunk
            .lines
            .iter()
            .find(|line| line.kind != LineKind::Context)
        else {
            spans.push(None);
            continue;
        };
        let (side_is_new, number) = match (first.new, first.old) {
            (Some(new), _) if first.kind == LineKind::Added => (true, new as usize),
            (_, Some(old)) => (false, old as usize),
            _ => {
                spans.push(None);
                continue;
            }
        };
        let side = |candidate: &deltoids::Hunk, new: bool| {
            let (start, skipped) = match new {
                true => (candidate.new_start, deltoids::LineKind::Removed),
                false => (candidate.old_start, deltoids::LineKind::Added),
            };
            let len = candidate.lines.iter().filter(|l| l.kind != skipped).count();
            start..start + len
        };
        let containing = (scoped.hunks().iter())
            .find(|candidate| side(candidate, side_is_new).contains(&number));
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
        spans.push(containing.map(|containing| side(containing, true)));
    }
    widen(hunks, &spans, new);
}

/// Give each hunk the unchanged lines out to the edges of its scope's new-side lines in
/// `spans`, short of what the hunks around it show, when its scope adds few enough in all,
/// across every hunk in it.
fn widen(hunks: &mut [Hunk], spans: &[Option<Range<usize>>], new: &str) {
    let text: Vec<&str> = new.lines().collect();
    // The first new-side line nothing shows yet. Scopes don't overlap, so whether the one
    // before shows its lines doesn't move where this one's start.
    let mut free = 1;
    let mut ranges = Vec::with_capacity(hunks.len());
    for (ix, hunk) in hunks.iter().enumerate() {
        let key = hunk.key.0;
        let (start, end) = (
            key.new_start as usize,
            (key.new_start + key.new_lines) as usize,
        );
        let next = (hunks.get(ix + 1)).map_or(text.len() + 1, |next| next.key.0.new_start as usize);
        let (lead, tail) = match &spans[ix] {
            // A hunk with no new-side lines has nowhere to stand among them.
            Some(span) if key.new_lines > 0 => {
                (span.start.max(free)..start, end..span.end.min(next))
            }
            _ => (start..start, end..end),
        };
        free = end.max(tail.end);
        ranges.push((lead, tail));
    }
    let added = |span: &Range<usize>| -> usize {
        (spans.iter().zip(&ranges))
            .filter(|(other, _)| other.as_ref() == Some(span))
            .map(|(_, (lead, tail))| lead.len() + tail.len())
            .sum()
    };
    for (ix, (lead, tail)) in ranges.iter().enumerate() {
        if spans[ix]
            .as_ref()
            .is_none_or(|span| added(span) > SCOPE_CONTEXT)
        {
            continue;
        }
        let key = hunks[ix].key.0;
        // Old-side numbers run behind or ahead of the new side's by what the hunks before
        // removed or added.
        let unchanged = |new: usize, ahead: i64| {
            Some(DiffLine {
                kind: LineKind::Context,
                old: u32::try_from(new as i64 - ahead).ok(),
                new: Some(new as u32),
                text: (*text.get(new.checked_sub(1)?)?).into(),
                spans: Vec::new(),
                emph: Vec::new(),
            })
        };
        let before = i64::from(key.new_start) - i64::from(key.old_start);
        let after =
            i64::from(key.new_start + key.new_lines) - i64::from(key.old_start + key.old_lines);
        hunks[ix].lead = lead
            .clone()
            .filter_map(|new| unchanged(new, before))
            .collect();
        hunks[ix].tail = tail
            .clone()
            .filter_map(|new| unchanged(new, after))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::HunkKey;

    /// The hunk Git makes, with 3 lines of context, for line `at` of `old` changed to
    /// `new`'s, the two having as many lines.
    fn changed_line(old: &str, new: &str, at: u32) -> Hunk {
        let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
        let (first, last) = (at.saturating_sub(3).max(1), (at + 3).min(old.len() as u32));
        let line = |kind, old: Option<u32>, new: Option<u32>, text: &str| DiffLine {
            kind,
            old,
            new,
            text: text.into(),
            spans: Vec::new(),
            emph: Vec::new(),
        };
        let unchanged = |n: u32| line(LineKind::Context, Some(n), Some(n), old[n as usize - 1]);
        let lines = (first..at)
            .map(unchanged)
            .chain([
                line(LineKind::Removed, Some(at), None, old[at as usize - 1]),
                line(LineKind::Added, None, Some(at), new[at as usize - 1]),
            ])
            .chain((at + 1..=last).map(unchanged))
            .collect();
        let count = last - first + 1;
        Hunk {
            key: HunkKey(but_core::HunkHeader {
                old_start: first,
                old_lines: count,
                new_start: first,
                new_lines: count,
            }),
            header: String::new(),
            lines,
            scope: Vec::new(),
            lead: Vec::new(),
            tail: Vec::new(),
        }
    }

    /// A Rust file with a function of `body` lines, each `let` changing on line `changed`
    /// of the file when `edit`.
    fn source(body: u32, changed: u32, edit: bool) -> String {
        let mut lines = vec!["fn work() {".to_owned()];
        for n in 1..=body {
            let value = if edit && n + 1 == changed { 100 + n } else { n };
            lines.push(format!("    let v{n} = {value};"));
        }
        lines.push("}".to_owned());
        lines.join("\n") + "\n"
    }

    fn numbers(lines: &[DiffLine]) -> Vec<u32> {
        lines.iter().filter_map(|line| line.new).collect()
    }

    #[test]
    fn a_hunk_in_a_short_scope_shows_all_of_it() {
        let (old, new) = (source(10, 7, false), source(10, 7, true));
        let mut hunks = [changed_line(&old, &new, 7)];
        label_scopes(&mut hunks, &old, &new, "work.rs");
        assert_eq!(
            numbers(&hunks[0].lead),
            [1, 2, 3],
            "up to the function's first line"
        );
        assert_eq!(
            numbers(&hunks[0].tail),
            [11, 12],
            "down to its closing brace"
        );
        assert_eq!(
            hunks[0].lead[0].text.as_ref(),
            "fn work() {",
            "with the file's text"
        );
    }

    #[test]
    fn hunks_in_one_scope_show_each_line_of_it_once() {
        let old = source(16, 0, false);
        let new = (old.lines().enumerate())
            .map(|(ix, line)| match ix + 1 {
                4 | 14 => line.replace(" = ", " = 1"),
                _ => line.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut hunks = [changed_line(&old, &new, 4), changed_line(&old, &new, 14)];
        label_scopes(&mut hunks, &old, &new, "work.rs");
        assert_eq!(
            numbers(&hunks[0].tail),
            [8, 9, 10],
            "the first shows the lines up to the second"
        );
        assert!(
            hunks[1].lead.is_empty(),
            "which then has none to add before it"
        );
        assert_eq!(numbers(&hunks[1].tail), [18], "and the rest after it");
        assert_eq!(
            crate::ui::diff::unmodified_before(&hunks, 1),
            0,
            "nothing between them is left out"
        );
    }

    #[test]
    fn a_long_scope_split_among_hunks_counts_as_one() {
        let old = source(40, 0, false);
        let new = (old.lines().enumerate())
            .map(|(ix, line)| match ix + 1 {
                11 | 31 => line.replace(" = ", " = 1"),
                _ => line.to_owned(),
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let mut hunks = [changed_line(&old, &new, 11), changed_line(&old, &new, 31)];
        label_scopes(&mut hunks, &old, &new, "work.rs");
        assert!(
            hunks
                .iter()
                .all(|hunk| hunk.lead.is_empty() && hunk.tail.is_empty()),
            "each would add few lines, but the function adds more than {SCOPE_CONTEXT} in all"
        );
    }

    #[test]
    fn a_hunk_in_a_long_scope_keeps_its_context() {
        let (old, new) = (source(40, 21, false), source(40, 21, true));
        let mut hunks = [changed_line(&old, &new, 21)];
        label_scopes(&mut hunks, &old, &new, "work.rs");
        assert!(
            hunks[0].lead.is_empty() && hunks[0].tail.is_empty(),
            "the function would add more than {SCOPE_CONTEXT} lines"
        );
        assert!(!hunks[0].scope.is_empty(), "it's still named");
    }
}
