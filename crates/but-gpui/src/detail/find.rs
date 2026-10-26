//! Finding text in the diff (⌘F): every match marked in the lines, the current one more
//! strongly, and Enter and ⇧Enter stepping through them.

use std::collections::HashMap;
use std::ops::Range;

use gpui::{
    AnyElement, Context, Entity, Focusable as _, Pixels, ScrollHandle, Window, actions, point,
    prelude::*,
};

use super::Detail;
use crate::backend::{self, DiffLine, LineKind};
use crate::theme::Theme;
use crate::ui::diff::DiffRow;
use crate::ui::text_input::{InputEvent, TextInput};
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
}

impl Spot {
    fn of(row: DiffRow) -> Option<Spot> {
        match row {
            DiffRow::Line { file, hunk, line } => Some(Spot::Line((file, hunk, line))),
            DiffRow::Context { file, new, .. } => Some(Spot::Unchanged(file, new)),
            _ => None,
        }
    }

    fn row(self) -> DiffRow {
        match self {
            Spot::Line((file, hunk, line)) => DiffRow::Line { file, hunk, line },
            Spot::Unchanged(file, new) => DiffRow::Context { file, old: 0, new },
        }
    }
}

pub(super) struct Find {
    input: Entity<TextInput>,
    /// Every match, in the diff's order.
    matches: Vec<(Spot, Range<usize>)>,
    /// Each line's matches, by their place in `matches`.
    on: HashMap<Spot, Vec<usize>>,
    current: usize,
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

impl Detail {
    pub fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(find) = &self.find {
            let input = find.input.clone();
            input.update(cx, |input, cx| input.select_all_text(cx));
            window.focus(&input.focus_handle(cx), cx);
            return;
        }
        let input = cx.new(|cx| TextInput::new("Find in the diff", false, cx));
        cx.subscribe_in(&input, window, |this, _, event, window, cx| match event {
            InputEvent::Changed => this.run_find(true, cx),
            InputEvent::Submit => this.step_find(1, cx),
            InputEvent::Cancel => this.close_find(window, cx),
        })
        .detach();
        window.focus(&input.focus_handle(cx), cx);
        self.find = Some(Find {
            input,
            matches: Vec::new(),
            on: HashMap::new(),
            current: 0,
        });
        cx.notify();
    }

    /// Find `text` in the diff, as typing it in the find bar does.
    pub(super) fn find_text(&mut self, text: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.open_find(window, cx);
        if let Some(find) = &self.find {
            (find.input).update(cx, |input, cx| input.set_text(text, cx));
        }
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Search the diff again, as when the query or the diff changed; with `reveal`, go to
    /// the first match.
    pub(super) fn run_find(&mut self, reveal: bool, cx: &mut Context<Self>) {
        let (Some(find), Some(diff)) = (&self.find, &self.diff) else {
            return;
        };
        let query = find.input.read(cx).text().to_owned();
        // What the diff shows, unchanged lines and all, with folded files open: not what a
        // filter or an operation under way hides.
        let rows = self.build_rows(&diff.key, &diff.files, true, cx);
        let matches = search(&rows, |row| self.line_of(row), &query);
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
        let row = spot.row();
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

    /// The find bar, floating at the top right of the diff.
    pub(super) fn render_find(
        &self,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let find = self.find.as_ref()?;
        let focused = find.input.focus_handle(cx).is_focused(window);
        let empty = find.input.read(cx).text().is_empty();
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
                .child(field(&find.input, focused, theme).w(dp(220.)))
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
    use super::{Spot, search};
    use crate::backend::{DiffLine, LineKind};
    use crate::ui::diff::DiffRow;

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
