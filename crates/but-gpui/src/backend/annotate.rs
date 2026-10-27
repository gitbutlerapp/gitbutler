//! What [deltoids](https://github.com/juanibiapina/deltoids) adds to a diff: the words
//! that changed within a line, and the function or type each hunk sits in.

use std::ops::Range;

use deltoids::{EmphKind, LineEmphasis};

use super::model::{DiffLine, Hunk, LineKind, scope_name};

/// Change runs whose removed bytes × added bytes exceed this are left plain: pairing lines
/// up costs about that much time and memory, so a rewrite of hundreds of lines could
/// otherwise take seconds and gigabytes. A pair of lines at the diff's 2000-byte cut, as a
/// paragraph written on one line is, still fits, at about 15ms at worst.
const EMPHASIS_BUDGET: usize = 1 << 22;

/// Mark the changed words of each removed line that pairs up with an added one.
pub fn emphasize(hunk: &mut Hunk) {
    for run in hunk.lines.split_mut(|line| line.kind == LineKind::Context) {
        emphasize_run(run);
    }
}

/// Mark the changed words of each removed line in `run` that pairs up with an added one.
/// `run` has no unchanged lines.
pub(super) fn emphasize_run(run: &mut [DiffLine]) {
    let (removed, added): (Vec<&mut DiffLine>, Vec<&mut DiffLine>) =
        (run.iter_mut()).partition(|line| line.kind == LineKind::Removed);
    let bytes = |lines: &[&mut DiffLine]| lines.iter().map(|line| line.text.len()).sum::<usize>();
    if removed.is_empty() || added.is_empty() || bytes(&removed) * bytes(&added) > EMPHASIS_BUDGET {
        return;
    }
    fn text<'a>(lines: &'a [&mut DiffLine]) -> Vec<&'a str> {
        lines.iter().map(|line| line.text.as_ref()).collect()
    }
    let (minus, plus) = deltoids::compute_subhunk_emphasis(&text(&removed), &text(&added));
    for (line, emphasis) in removed
        .into_iter()
        .zip(minus)
        .chain(added.into_iter().zip(plus))
    {
        line.emph = ranges(&emphasis, line.text.len());
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
    let whole_line = ranges.len() == 1 && ranges.first() == Some(&(0..len));
    if at != len || whole_line {
        return Vec::new();
    }
    ranges
}

/// Name the scopes each hunk changes, outermost first, by matching its first changed line
/// against deltoids' scope-aware diff of the same two texts, and show the rest of the
/// innermost one around the hunk when that's at most `limit` more lines, as deltoids grows
/// a hunk's context to the scope it changes. A bigger scope, as a long function, leaves the
/// hunk its few lines of context and its name in the hunk's header.
pub fn label_scopes(hunks: &mut [Hunk], old: &str, new: &str, path: &str, limit: usize) {
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
            // A one-line item, as `mod foo;`, is the changed line itself, not around it.
            hunk.scope = (containing.ancestors.iter())
                .filter(|scope| scope.end_line > scope.start_line)
                .map(|scope| scope_name(&scope.text).to_owned())
                .collect();
        }
        spans.push(containing.map(|containing| side(containing, true)));
    }
    widen(hunks, &spans, new, limit);
}

/// Give each hunk the unchanged lines out to the edges of its scope's new-side lines in
/// `spans`, short of what the hunks around it show, when its scope adds at most `limit` in
/// all, across every hunk in it.
fn widen(hunks: &mut [Hunk], spans: &[Option<Range<usize>>], new: &str, limit: usize) {
    // As the diff's own lines are shown, tabs as spaces and long lines cut.
    let text: Vec<String> = new.lines().map(super::diff::display_text).collect();
    // The first new-side line nothing shows yet. Scopes don't overlap, so whether the one
    // before shows its lines doesn't move where this one's start.
    let mut free = 1;
    let mut ranges = Vec::with_capacity(hunks.len());
    for (ix, (hunk, span)) in hunks.iter().zip(spans).enumerate() {
        let key = hunk.key.0;
        let (start, end) = (
            key.new_start as usize,
            (key.new_start + key.new_lines) as usize,
        );
        let next = (hunks.get(ix + 1)).map_or(text.len() + 1, |next| next.key.0.new_start as usize);
        let (lead, tail) = match span {
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
    for ((hunk, span), (lead, tail)) in hunks.iter_mut().zip(spans).zip(&ranges) {
        if span.as_ref().is_none_or(|span| added(span) > limit) {
            continue;
        }
        let key = hunk.key.0;
        // Old-side numbers run behind or ahead of the new side's by what the hunks before
        // removed or added.
        let unchanged = |new: usize, ahead: i64| {
            Some(DiffLine {
                old: u32::try_from(new as i64 - ahead).ok(),
                ..super::diff::context_line(new as u32, text.get(new.checked_sub(1)?)?)
            })
        };
        let before = i64::from(key.new_start) - i64::from(key.old_start);
        let after =
            i64::from(key.new_start + key.new_lines) - i64::from(key.old_start + key.old_lines);
        hunk.lead = lead
            .clone()
            .filter_map(|new| unchanged(new, before))
            .collect();
        hunk.tail = tail
            .clone()
            .filter_map(|new| unchanged(new, after))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::HunkKey;
    use crate::ui::diff::{named_scopes, unmodified_before};

    const LIMIT: usize = 20;

    /// The scopes hunk `ix`'s header names with only the hunks shown, the lines a scope adds
    /// around them drawn while `around`.
    fn named(hunks: &[Hunk], ix: usize, around: bool) -> &[String] {
        named_scopes(hunks, ix, unmodified_before(hunks, ix), around)
    }

    #[test]
    fn a_paragraph_on_one_line_gets_its_changed_word_marked() {
        let paragraph = "word ".repeat(400);
        let (old, new) = (format!("START {paragraph}"), format!("CHANGED {paragraph}"));
        let mut hunk = changed_line(&old, &new, 1);
        emphasize(&mut hunk);
        assert_eq!(
            hunk.lines[1].emph.first(),
            Some(&(0..7)),
            "a 2000-byte line, as the diff cuts one, marks the word that changed"
        );
    }

    /// The hunk Git makes, with 3 lines of context, for line `at` of `old` changed to
    /// `new`'s, the two having as many lines.
    fn changed_line(old: &str, new: &str, at: u32) -> Hunk {
        let (old, new): (Vec<&str>, Vec<&str>) = (old.lines().collect(), new.lines().collect());
        let (first, last) = (at.saturating_sub(3).max(1), (at + 3).min(old.len() as u32));
        let line = DiffLine::new;
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
        label_scopes(&mut hunks, &old, &new, "work.rs", LIMIT);
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
        assert!(
            named(&hunks, 0, true).is_empty(),
            "so its header doesn't name the function again"
        );
        assert_eq!(
            named(&hunks, 0, false),
            hunks[0].scope,
            "unless conflicts in the file leave those lines out"
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
        label_scopes(&mut hunks, &old, &new, "work.rs", LIMIT);
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
            unmodified_before(&hunks, 1),
            0,
            "nothing between them is left out"
        );
        assert!(
            named(&hunks, 1, true).is_empty(),
            "so the second's header doesn't name the function the first is in"
        );
        assert_eq!(
            named(&hunks, 1, false),
            hunks[1].scope,
            "unless conflicts leave out the lines between them"
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
        label_scopes(&mut hunks, &old, &new, "work.rs", LIMIT);
        assert!(
            hunks
                .iter()
                .all(|hunk| hunk.lead.is_empty() && hunk.tail.is_empty()),
            "each would add few lines, but the function adds more than {LIMIT} in all"
        );
        assert_eq!(
            named(&hunks, 1, true),
            hunks[1].scope,
            "the lines between them are left out, so the second names the function again"
        );
        assert!(
            named_scopes(&hunks, 1, 0, true).is_empty(),
            "but not in the file shown whole, which draws every line between them"
        );
    }

    #[test]
    fn a_one_line_item_isnt_named_as_its_own_scope() {
        let old = "mod a;\nmod b;\nmod c;\n";
        let new = old.replace("mod b;", "mod bb;");
        let mut hunks = [changed_line(old, &new, 2)];
        label_scopes(&mut hunks, old, &new, "lib.rs", LIMIT);
        assert!(hunks[0].scope.is_empty(), "{:?}", hunks[0].scope);
    }

    #[test]
    fn a_hunk_in_a_long_scope_keeps_its_context() {
        let (old, new) = (source(40, 21, false), source(40, 21, true));
        let mut hunks = [changed_line(&old, &new, 21)];
        label_scopes(&mut hunks, &old, &new, "work.rs", LIMIT);
        assert!(
            hunks[0].lead.is_empty() && hunks[0].tail.is_empty(),
            "the function would add more than {LIMIT} lines"
        );
        assert!(
            !hunks[0].scope.is_empty() && named(&hunks, 0, true) == hunks[0].scope,
            "its header still names the function, whose first line isn't shown"
        );
    }

    #[test]
    fn an_outer_scope_whose_first_line_isnt_shown_is_still_named() {
        let consts: String = (1..=40)
            .map(|n| format!("    const C{n}: i32 = {n};\n"))
            .collect();
        let module = |edit| format!("mod outer {{\n{consts}{}}}\n", source(6, 4, edit));
        let (old, new) = (module(false), module(true));
        // The function starts on line 42, in the hunk's context before its change.
        let mut hunks = [changed_line(&old, &new, 45)];
        label_scopes(&mut hunks, &old, &new, "lib.rs", LIMIT);
        assert_eq!(hunks[0].scope, ["mod outer", "fn work()"]);
        assert_eq!(
            named(&hunks, 0, true),
            ["mod outer"],
            "the function's first line shows above the change, the module's doesn't"
        );
    }
}
