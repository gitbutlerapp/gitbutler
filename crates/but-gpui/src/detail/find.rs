//! Finding text in the diff, or in the words and code of a brief (⌘F): every match marked, the
//! current one more strongly, and Enter and ⇧Enter stepping through them.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;

use gpui::{
    AnyElement, App, Context, Focusable as _, Pixels, ScrollHandle, Window, actions, point,
    prelude::*,
};

use super::Detail;
use crate::backend::briefs::{BriefBlock, BriefBody};
use crate::backend::{self, DiffLine, LineKind};
use crate::theme::Theme;
use crate::ui::diff::DiffRow;
use crate::ui::markdown;
use crate::ui::{Icon, dp, field, h_flex, icon_button, muted, popover};

actions!(find, [FindInDiff, FindNext, FindPrevious]);

/// A diff line, by file, hunk and line, which stays put when files fold.
pub(super) type LineId = (usize, usize, usize);

/// Where a match is, as the diff's rows name lines: one of a hunk's, or an unchanged one,
/// of the file shown whole or of a hunk's scope, by its file and new-side number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(super) enum Spot {
    Line(LineId),
    Unchanged(usize, u32),
    /// A paragraph of a brief's words, by its block and its place among the block's.
    BriefWords(usize, usize),
    /// A line of a brief's code card, by its block, its run of lines and its place in that.
    BriefCode(usize, usize, usize),
}

impl Spot {
    fn of(row: DiffRow) -> Option<Spot> {
        match row {
            DiffRow::Line { file, hunk, line } => Some(Spot::Line((file, hunk, line))),
            DiffRow::Context { file, new, .. } => Some(Spot::Unchanged(file, new)),
            _ => None,
        }
    }

    fn row(self) -> Option<DiffRow> {
        match self {
            Spot::Line((file, hunk, line)) => Some(DiffRow::Line { file, hunk, line }),
            Spot::Unchanged(file, new) => Some(DiffRow::Context { file, old: 0, new }),
            Spot::BriefWords(..) | Spot::BriefCode(..) => None,
        }
    }
}

pub(super) struct Find {
    /// Every match, in the diff's order.
    matches: Vec<(Spot, Range<usize>)>,
    /// Each line's matches, by their place in `matches`.
    on: HashMap<Spot, Vec<usize>>,
    current: usize,
}

impl Find {
    /// A find on match `current`, with its matches still to find.
    fn new(current: usize) -> Find {
        Find {
            matches: Vec::new(),
            on: HashMap::new(),
            current,
        }
    }
}

/// Where `query` appears in the lines `rows` show, ignoring case for ASCII.
fn search<'a>(
    rows: &[DiffRow],
    line: impl Fn(DiffRow) -> Option<&'a DiffLine>,
    query: &str,
) -> Vec<(Spot, Range<usize>)> {
    if query.is_empty() {
        return Vec::new();
    }
    let query = query.to_ascii_lowercase();
    let mut found = Vec::new();
    for &row in rows {
        let (Some(spot), Some(line)) = (Spot::of(row), line(row)) else {
            continue;
        };
        // ASCII lowering keeps byte offsets where they were.
        let text = line.text.to_ascii_lowercase();
        found.extend((text.match_indices(&query)).map(|(at, _)| (spot, at..at + query.len())));
    }
    found
}

/// Where `query` appears in a brief's words and code, ignoring case for ASCII.
fn search_brief(blocks: &[BriefBlock], query: &str) -> Vec<(Spot, Range<usize>)> {
    if query.is_empty() {
        return Vec::new();
    }
    let query = query.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut look = |spot, text: &str| {
        let text = text.to_ascii_lowercase();
        found.extend((text.match_indices(&query)).map(|(at, _)| (spot, at..at + query.len())));
    };
    for (at, block) in blocks.iter().enumerate() {
        match &block.body {
            BriefBody::Text(paragraphs) => {
                for (ix, paragraph) in paragraphs.iter().enumerate() {
                    look(Spot::BriefWords(at, ix), &paragraph.text);
                }
            }
            BriefBody::Code(code) => {
                for (run, lines) in code.parts.iter().enumerate() {
                    for (ix, line) in lines.iter().enumerate() {
                        look(Spot::BriefCode(at, run, ix), &line.text);
                    }
                }
            }
            BriefBody::Files { .. } | BriefBody::Pictures(_) | BriefBody::OnTheWeb(_) => {}
        }
    }
    found
}

impl Detail {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.find_input.clone();
        if self.find.is_some() {
            input.update(cx, |input, cx| input.select_all_text(cx));
            window.focus(&input.focus_handle(cx), cx);
            return;
        }
        input.update(cx, |input, cx| input.restore("", cx));
        window.focus(&input.focus_handle(cx), cx);
        self.find = Some(Find::new(0));
        cx.notify();
    }

    /// The find bar as the page shown now left it, `left` being what it was finding and which
    /// match it was on, or closed. The diff finds the matches as it lands.
    pub(super) fn restore_find(&mut self, left: Option<(String, usize)>, cx: &mut Context<Self>) {
        let Some((query, current)) = left else {
            self.find = None;
            return;
        };
        (self.find_input).update(cx, |input, cx| input.restore(query, cx));
        self.find = Some(Find::new(current));
    }

    /// What the find bar is finding and which match it's on, while it's open.
    pub(super) fn find_left(&self, cx: &App) -> Option<(String, usize)> {
        let find = self.find.as_ref()?;
        Some((self.find_input.read(cx).text().to_owned(), find.current))
    }

    /// Find `text` in the diff, as typing it in the find bar does.
    pub(super) fn find_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_find(window, cx);
        (self.find_input).update(cx, |input, cx| input.set_text(text, cx));
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Search the diff again, as when the query or the diff changed; with `reveal`, go to
    /// the first match.
    pub(super) fn run_find(&mut self, reveal: bool, cx: &mut Context<Self>) {
        if self.find.is_none() {
            return;
        }
        let query = self.find_input.read(cx).text().to_owned();
        let matches = match self.showing_brief() {
            true => (self.page.brief())
                .and_then(|brief| brief.page())
                .map_or_else(Vec::new, |page| search_brief(&page.blocks, &query)),
            false => {
                let Some(diff) = &self.diff else {
                    return;
                };
                // What the diff shows, unchanged lines and all, with folded files open: not what
                // a filter or an operation under way hides.
                let (rows, ..) = self.build_rows(&diff.key, &diff.files, true, cx);
                search(&rows, |row| self.line_of(row), &query)
            }
        };
        let mut on: HashMap<Spot, Vec<usize>> = HashMap::new();
        for (ix, (spot, _)) in matches.iter().enumerate() {
            on.entry(*spot).or_default().push(ix);
        }
        let Some(find) = &mut self.find else {
            return;
        };
        (find.matches, find.on) = (matches, on);
        find.current = find.current.min(find.matches.len().saturating_sub(1));
        if reveal {
            find.current = 0;
            self.reveal_match(cx);
        }
        cx.notify();
    }

    pub(super) fn step_find(&mut self, by: isize, cx: &mut Context<Self>) {
        let Some(find) = &mut self.find else {
            return;
        };
        let count = find.matches.len() as isize;
        if count == 0 {
            return;
        }
        find.current = (find.current as isize + by).rem_euclid(count) as usize;
        self.reveal_match(cx);
        cx.notify();
    }

    /// Scroll the current match into view, unfolding its file if need be, and sideways when
    /// it's past the edge of a long line.
    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        let Some((spot, range)) = self
            .find
            .as_ref()
            .and_then(|find| find.matches.get(find.current))
            .cloned()
        else {
            return;
        };
        let Some(row) = spot.row() else {
            return self.reveal_match_in_brief(spot, cx);
        };
        let Some(file) = row.file() else {
            return;
        };
        let same_line = |r: &DiffRow| r.same_place(row);
        if self.row_of(same_line).is_none() && self.is_folded(file) {
            self.toggle_fold(file, cx);
        }
        if let Some(ix) = self.row_of(same_line) {
            self.reveal_row(ix);
        }
        let prefs = &self.store.read(cx).prefs;
        let diff_file = self.diff.as_ref().and_then(|diff| diff.files.get(file));
        let (Some(diff_file), false) = (diff_file, prefs.wrap) else {
            return;
        };
        let (Some(text), Some(scroll)) =
            (self.line_of(row), self.sideways.get(&diff_file.file.path))
        else {
            return;
        };
        let (Some(before), Some(found)) = (text.text.get(..range.start), text.text.get(range))
        else {
            return;
        };
        // A unified diff scrolls both sides as the new one; split, a line on both sides
        // scrolls both.
        let [old, new] = scroll;
        let sides: &[&ScrollHandle] = match (prefs.split, text.kind) {
            (true, LineKind::Removed) => &[old],
            (true, LineKind::Context) => &[old, new],
            _ => &[new],
        };
        let theme = Theme::get(cx);
        let digit = crate::ui::diff::digit_width(theme);
        let start = theme.scaled(backend::columns(before) as f32 * digit);
        let end = start + theme.scaled(backend::columns(found) as f32 * digit);
        for handle in sides {
            let width = handle.bounds().size.width - theme.scaled(8.);
            let offset = -handle.offset().x;
            if start < offset || end > offset + width {
                let left = (start - theme.scaled(4. * digit)).max(Pixels::ZERO);
                handle.set_offset(point(-left, Pixels::ZERO));
            }
        }
    }

    /// The matches on the line at `spot` to mark, each with whether it's the current one.
    pub(super) fn found_on(&self, spot: Spot) -> Vec<(Range<usize>, bool)> {
        let Some(find) = &self.find else {
            return Vec::new();
        };
        (find.on.get(&spot).into_iter().flatten())
            .filter_map(|&ix| Some((find.matches.get(ix)?.1.clone(), ix == find.current)))
            .collect()
    }

    /// Bring a match in the brief into view, by its place among what its block has.
    fn reveal_match_in_brief(&mut self, spot: Spot, cx: &mut Context<Self>) {
        let place = match spot {
            Spot::BriefWords(at, paragraph) => Some((at, paragraph, 0)),
            Spot::BriefCode(at, run, line) => (self.page.brief())
                .and_then(|brief| brief.page())
                .and_then(|page| page.blocks.get(at))
                .and_then(|block| match &block.body {
                    BriefBody::Code(code) => {
                        let lines = |runs: &[Vec<DiffLine>]| runs.iter().map(Vec::len).sum();
                        let before = code.parts.get(..run).map_or(0, lines);
                        Some((at, before + line, lines(&code.parts)))
                    }
                    _ => None,
                }),
            Spot::Line(_) | Spot::Unchanged(..) => None,
        };
        if let Some((at, part, parts)) = place {
            self.reveal_in_brief(at, part, parts, cx);
        }
    }

    /// `words`, the block `at`'s from paragraph `from`, with what's found in them marked.
    pub(super) fn found_in_words<'a>(
        &self,
        at: usize,
        from: usize,
        words: &'a [markdown::Block],
    ) -> Cow<'a, [markdown::Block]> {
        if self.find.is_none() {
            return Cow::Borrowed(words);
        }
        let mut marked = words.to_vec();
        for (ix, paragraph) in marked.iter_mut().enumerate() {
            let found = self.found_on(Spot::BriefWords(at, from + ix));
            paragraph
                .styles
                .extend(found.into_iter().map(|(range, current)| {
                    let style = markdown::Style {
                        mark: true,
                        current,
                        ..Default::default()
                    };
                    (range, style)
                }));
        }
        Cow::Owned(marked)
    }

    /// The find bar, floating at the top right of what it finds in.
    pub(super) fn render_find(
        &self,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let find = self.find.as_ref()?;
        let focused = self.find_input.focus_handle(cx).is_focused(window);
        let empty = self.find_input.read(cx).text().is_empty();
        let count = match (empty, find.matches.len()) {
            (true, _) => String::new(),
            (false, 0) => "No matches".into(),
            (false, n) => format!("{} of {n}", find.current + 1),
        };
        Some(
            popover(theme)
                .key_context("FindBar")
                .on_action(
                    cx.listener(|this, _: &FindInDiff, window, cx| this.open_find(window, cx)),
                )
                .on_action(cx.listener(|this, _: &FindNext, _, cx| this.step_find(1, cx)))
                .on_action(cx.listener(|this, _: &FindPrevious, _, cx| this.step_find(-1, cx)))
                // Clicks on the bar stay off the diff under it.
                .occlude()
                .absolute()
                .top(dp(8.))
                .right(dp(24.))
                .flex_row()
                .items_center()
                .gap(dp(4.))
                .p(dp(6.))
                .child(field(&self.find_input, focused, theme).w(dp(220.)))
                .child(muted(count, theme).min_w(dp(64.)))
                .child(
                    h_flex()
                        .child(
                            icon_button("find-previous", Icon::ArrowUp)
                                .small()
                                .tooltip_with_key("Previous match", Some("⇧↩".into()))
                                .on_click(cx.listener(|this, _, _, cx| this.step_find(-1, cx))),
                        )
                        .child(
                            icon_button("find-next", Icon::ArrowDown)
                                .small()
                                .tooltip_with_key("Next match", Some("↩".into()))
                                .on_click(cx.listener(|this, _, _, cx| this.step_find(1, cx))),
                        )
                        .child(
                            icon_button("find-close", Icon::Cross)
                                .small()
                                .tooltip_with_key("Close", Some("esc".into()))
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.close_find(window, cx)),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{Spot, search, search_brief};
    use crate::backend::briefs::{BriefBlock, BriefBody, BriefCode, CodeSource};
    use crate::backend::{DiffLine, LineKind};
    use crate::ui::diff::DiffRow;
    use crate::ui::markdown;

    #[test]
    fn a_brief_is_searched_in_its_words_and_code_in_order() {
        let block = |body| BriefBlock {
            positions: 1..=1,
            fresh: false,
            only_new: None,
            detail: None,
            body,
        };
        let line = |text: &str| DiffLine::new(LineKind::Added, None, Some(1), text);
        let blocks = [
            block(BriefBody::Text(markdown::parse(
                "Nothing here\n\nA Purge runs",
            ))),
            block(BriefBody::Code(BriefCode {
                path: "a.rs".into(),
                source: CodeSource::Committed,
                status: None,
                stats: None,
                parts: vec![vec![line("other")], vec![line("no"), line("fn purge()")]],
                omitted: Vec::new(),
            })),
        ];
        assert_eq!(
            search_brief(&blocks, "purge"),
            [
                (Spot::BriefWords(0, 1), 2..7),
                (Spot::BriefCode(1, 1, 1), 3..8)
            ],
            "the paragraph with it, then the code line with it, ignoring case"
        );
        assert!(
            search_brief(&blocks, "").is_empty(),
            "no query finds nothing"
        );
    }

    #[test]
    fn unchanged_lines_are_searched_in_the_order_shown() {
        let line = |kind, text: &str| DiffLine::new(kind, None, None, text);
        let (scope, changed) = (
            line(LineKind::Context, "fn work() {"),
            line(LineKind::Added, "    work();"),
        );
        let rows = [
            DiffRow::File(0),
            DiffRow::Context {
                file: 0,
                old: 1,
                new: 1,
            },
            DiffRow::Hunk { file: 0, hunk: 0 },
            DiffRow::Line {
                file: 0,
                hunk: 0,
                line: 0,
            },
        ];
        let found = search(
            &rows,
            |row| match row {
                DiffRow::Context { .. } => Some(&scope),
                DiffRow::Line { .. } => Some(&changed),
                _ => None,
            },
            "WORK",
        );
        assert_eq!(
            found,
            [(Spot::Unchanged(0, 1), 3..7), (Spot::Line((0, 0, 0)), 4..8)],
            "the scope's line above the hunk, then the hunk's, ignoring case"
        );
    }
}
