//! Finding text in the diff (⌘F): every match marked in the lines, the current one more
//! strongly, and Enter and ⇧Enter stepping through them.

use std::collections::HashMap;
use std::ops::Range;

use gpui::{
    AnyElement, App, Context, Entity, Focusable as _, KeyBinding, Window, actions, div, prelude::*,
    px,
};

use super::Detail;
use crate::backend::{DiffBody, FileDiff};
use crate::theme::Theme;
use crate::ui::diff::DiffRow;
use crate::ui::text_input::{InputEvent, TextInput};
use crate::ui::{Icon, field, h_flex, icon_button, popover};

actions!(find, [FindInDiff, FindNext, FindPrevious]);

pub fn bind_keys(cx: &mut App) {
    // After the text field's own keys, so these win in the find field.
    let field = Some("FindBar > TextInput");
    cx.bind_keys([
        KeyBinding::new("cmd-f", FindInDiff, Some("Detail")),
        KeyBinding::new("enter", FindNext, field),
        KeyBinding::new("shift-enter", FindPrevious, field),
        KeyBinding::new("cmd-g", FindNext, Some("Detail")),
        KeyBinding::new("cmd-shift-g", FindPrevious, Some("Detail")),
    ]);
}

/// A diff line, by file, hunk and line, which stays put when files fold.
type LineId = (usize, usize, usize);

pub(super) struct Find {
    input: Entity<TextInput>,
    /// Every match, in the diff's order.
    matches: Vec<(LineId, Range<usize>)>,
    /// The matches on each line, by their index in `matches`.
    by_line: HashMap<LineId, Vec<usize>>,
    current: usize,
}

/// Where `query` appears in `files`' lines, ignoring case for ASCII.
fn search(files: &[FileDiff], query: &str) -> Vec<(LineId, Range<usize>)> {
    if query.is_empty() {
        return Vec::new();
    }
    let query = query.to_ascii_lowercase();
    let mut found = Vec::new();
    for (file_ix, file) in files.iter().enumerate() {
        let DiffBody::Hunks { hunks, .. } = &file.body else {
            continue;
        };
        for (hunk_ix, hunk) in hunks.iter().enumerate() {
            for (line_ix, line) in hunk.lines.iter().enumerate() {
                // ASCII lowering keeps byte offsets where they were.
                let text = line.text.to_ascii_lowercase();
                found.extend(
                    text.match_indices(&query)
                        .map(|(at, _)| ((file_ix, hunk_ix, line_ix), at..at + query.len())),
                );
            }
        }
    }
    found
}

impl Detail {
    pub(super) fn open_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
            by_line: HashMap::new(),
            current: 0,
        });
        cx.notify();
    }

    pub(super) fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Search the diff again, as when the query or the diff changed; with `reveal`, go to
    /// the first match.
    pub(super) fn run_find(&mut self, reveal: bool, cx: &mut Context<Self>) {
        let (Some(find), Some(diff)) = (&mut self.find, &self.diff) else {
            return;
        };
        let query = find.input.read(cx).text().to_owned();
        find.matches = search(&diff.files, &query);
        find.by_line.clear();
        for (ix, (line, _)) in find.matches.iter().enumerate() {
            find.by_line.entry(*line).or_default().push(ix);
        }
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

    /// Scroll the current match into view, unfolding its file if need be.
    fn reveal_match(&mut self, cx: &mut Context<Self>) {
        let Some((file, hunk, line)) = self
            .find
            .as_ref()
            .and_then(|find| find.matches.get(find.current))
            .map(|(line, _)| *line)
        else {
            return;
        };
        let row = DiffRow::Line {
            file,
            hunk,
            line,
            last: false,
        };
        let same_line = |r: &DiffRow| match (r, row) {
            (
                DiffRow::Line {
                    file, hunk, line, ..
                },
                DiffRow::Line {
                    file: f,
                    hunk: h,
                    line: l,
                    ..
                },
            ) => (*file, *hunk, *line) == (f, h, l),
            _ => false,
        };
        if self.row_of(same_line).is_none() && self.is_folded(file) {
            self.toggle_fold(file, cx);
        }
        if let Some(ix) = self.row_of(same_line) {
            self.list.scroll_to_reveal_item(ix);
        }
    }

    /// The matches on `line` to mark, each with whether it's the current one.
    pub(super) fn found_on(&self, line: LineId) -> Vec<(Range<usize>, bool)> {
        let Some(find) = &self.find else {
            return Vec::new();
        };
        find.by_line
            .get(&line)
            .into_iter()
            .flatten()
            .map(|ix| (find.matches[*ix].1.clone(), *ix == find.current))
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
                .on_action(cx.listener(|this, _: &FindNext, _, cx| this.step_find(1, cx)))
                .on_action(cx.listener(|this, _: &FindPrevious, _, cx| this.step_find(-1, cx)))
                // Clicks on the bar stay off the diff under it.
                .occlude()
                .absolute()
                .top(px(8.))
                .right(px(24.))
                .flex_row()
                .items_center()
                .gap(px(4.))
                .p(px(6.))
                .child(field(&find.input, focused, theme).w(px(220.)))
                .child(
                    div()
                        .min_w(px(64.))
                        .text_size(px(12.))
                        .text_color(theme.text_2)
                        .child(count),
                )
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
