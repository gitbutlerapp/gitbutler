//! The diff column: every changed file as a card, built row by row so only what's on
//! screen is laid out. Uncommitted files carry review controls, and their hunks a Discard
//! button while under the cursor or the pointer, as in Zed's agent review.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Context, Entity, MouseButton, Pixels, Subscription, Toggled, WeakEntity,
    Window, div, list, prelude::*, px,
};

use super::{Cursor, Detail, DiscardHunk, LoadedDiff, TextPoint, ToggleReviewed, checkbox_box};
use crate::app::AppView;
use crate::backend::{self, ChangeKind, DiffBody, DiffLine, DiffSource, LineKind, Selected};
use crate::state::{Cut, ProjectId, Review};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow, HeaderDecor, LineDecor, LineSelect};
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{ButtonStyle, Icon, button, empty_state, h_flex, icon_button, shortcut};

/// The diff, drawn by a view of its own over the space [`Detail`] leaves for it. A scrolled
/// list notifies the view that drew it, and GPUI draws everything inside a notified view again,
/// so as part of `Detail` every scroll frame laid out the file list too.
pub struct DiffPane {
    detail: WeakEntity<Detail>,
    _observe: Subscription,
}

impl DiffPane {
    pub fn new(detail: &Entity<Detail>, cx: &mut Context<Self>) -> Self {
        Self {
            detail: detail.downgrade(),
            // Whatever changes the detail can change the diff; scrolling only notifies this.
            _observe: cx.observe(detail, |_, _, cx| cx.notify()),
        }
    }
}

impl Render for DiffPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let drawn = self.detail.update(cx, |detail, cx| {
            (
                detail.focus.clone(),
                detail.render_diff(&theme, cx),
                detail.render_find(window, &theme, cx),
            )
        });
        let Ok((focus, diff, find)) = drawn else {
            return h_flex();
        };
        h_flex()
            .size_full()
            .relative()
            // Keys go to the detail, as when the diff was drawn inside it.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx)
            })
            .child(diff)
            .children(find)
    }
}

impl Detail {
    /// The diff line at `row`, if that row is one.
    fn line_at(&self, row: usize) -> Option<&DiffLine> {
        let diff = self.diff.as_ref()?;
        match *diff.rows.get(row)? {
            DiffRow::Line {
                file, hunk, line, ..
            } => match &diff.files.get(file)?.body {
                DiffBody::Hunks { hunks, .. } => hunks.get(hunk)?.lines.get(line),
                _ => None,
            },
            // An unchanged line of a file shown whole, from its new side.
            DiffRow::Context { file, new, .. } => {
                let (_, lines) = self.whole.get(&diff.files.get(file)?.file.path)?.as_ref()?;
                lines.get((new as usize).checked_sub(1)?)
            }
            _ => None,
        }
    }

    /// The selection in order, unless it's empty.
    fn text_range(&self) -> Option<(TextPoint, TextPoint)> {
        let (anchor, head) = self.text_selection?;
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        (start != end).then_some((start, end))
    }

    /// The selected part of the line at `row`. Nothing when the line changed under the
    /// selection so its ends no longer fall in the text, as after an edit in place.
    fn selected_in_row(&self, row: usize) -> Option<Range<usize>> {
        let (start, end) = self.text_range()?;
        if row < start.0 || row > end.0 {
            return None;
        }
        let text = &self.line_at(row)?.text;
        let from = if row == start.0 { start.1 } else { 0 };
        let to = if row == end.0 { end.1 } else { text.len() };
        let fits = text.is_char_boundary(from) && text.is_char_boundary(to);
        (from < to && fits).then_some(from..to)
    }

    /// The selected text, one line per diff line, without the `+` and `-` signs.
    pub fn selected_text(&self) -> Option<String> {
        let (start, end) = self.text_range()?;
        let lines: Vec<&str> = (start.0..=end.0)
            .filter_map(|row| {
                let text = &self.line_at(row)?.text;
                Some(&text[self.selected_in_row(row).unwrap_or(0..0)])
            })
            .collect();
        Some(lines.join("\n"))
    }

    /// The pointer went down on a line: start selecting there, take the word on a double
    /// click and the line on a triple, or reach to it with shift held.
    fn start_text_selection(
        &mut self,
        at: TextPoint,
        clicks: usize,
        extend: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (row, ix) = at;
        let text = self.line_at(row).map_or("", |line| line.text.as_ref());
        // The text may have changed since the line was drawn.
        let ix = (0..=ix.min(text.len()))
            .rev()
            .find(|ix| text.is_char_boundary(*ix))
            .unwrap_or(0);
        let at = (row, ix);
        self.text_selection = match clicks {
            2 => {
                let word = word_at(text, ix);
                Some(((row, word.start), (row, word.end)))
            }
            3.. => Some(((row, 0), (row, text.len()))),
            _ => match self.text_selection {
                Some((anchor, _)) if extend => Some((anchor, at)),
                _ => Some((at, at)),
            },
        };
        self.selecting = true;
        if let Some(DiffRow::Line { file, hunk, .. }) = self
            .diff
            .as_ref()
            .and_then(|diff| diff.rows.get(row))
            .copied()
        {
            self.cursor = Cursor {
                file,
                hunk: Some(hunk),
            };
        }
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn drag_text_selection(&mut self, at: TextPoint, cx: &mut Context<Self>) {
        if let (true, Some((_, head))) = (self.selecting, &mut self.text_selection)
            && *head != at
        {
            *head = at;
            cx.notify();
        }
    }

    /// Drop the text selection, saying whether there was one.
    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had = self.text_range().is_some();
        self.text_selection = None;
        cx.notify();
        had
    }

    pub(super) fn render_diff(&self, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let ground = |child: AnyElement| {
            div()
                .flex_1()
                .min_w(px(0.))
                .h_full()
                .bg(theme.bg_2)
                .child(child)
                .into_any_element()
        };
        if let Some(err) = &self.diff_error {
            return ground(empty_state("Couldn't load the diff", err.clone(), theme));
        }
        let Some(diff) = self.shown_diff() else {
            let title = if self.loading.is_some() {
                "Loading diff…"
            } else {
                ""
            };
            return ground(empty_state(title, "", theme));
        };
        if diff.files.is_empty() {
            let (title, detail) = match diff.key.source {
                DiffSource::Worktree => (
                    "No uncommitted changes",
                    "Edits show up here as soon as they're saved",
                ),
                _ => ("No changes", ""),
            };
            return ground(empty_state(title, detail, theme));
        }
        let entity = cx.entity().downgrade();
        // A copy of the header of the file at the top, while the file scrolls under it. It's
        // clipped to the list, so the next file can push it out the top.
        let sticky = self.sticky_header(diff).map(|(ix, lift)| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(px(diff::FILE_HEADER_HEIGHT))
                .overflow_hidden()
                .child(
                    div()
                        .id("sticky-header")
                        .absolute()
                        .top(lift)
                        .left_0()
                        .right_0()
                        // The pane's ground, so the lines under it don't show past the header's
                        // rounded corners.
                        .bg(theme.bg_2)
                        .block_mouse_except_scroll()
                        .child(self.render_diff_row(ix, entity.clone(), cx)),
                )
        });
        let scrollbar = scrollbar("diff-scrollbar", Scroll::List(self.list.clone()), theme);
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .bg(theme.bg_2)
            .pt(px(12.))
            // A press anywhere ends the last drag; one on a line starts a new one after this.
            .capture_any_mouse_down(cx.listener(|this, _, _, _| this.selecting = false))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, _| this.selecting = false),
            )
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(
                        list(self.list.clone(), move |ix, _window, cx| {
                            let Some(detail) = entity.upgrade() else {
                                return div().into_any_element();
                            };
                            detail.read(cx).render_diff_row(ix, detail.downgrade(), cx)
                        })
                        .size_full(),
                    )
                    .children(sticky)
                    .children(scrollbar),
            )
            .into_any_element()
    }

    /// The header to keep at the top of the diff while its file scrolls under it, by row, and
    /// how far the file's end has pushed it up. Nothing while a header is at the top itself.
    fn sticky_header(&self, diff: &LoadedDiff) -> Option<(usize, Pixels)> {
        let top = self.list.logical_scroll_top();
        let row = *diff.rows.get(top.item_ix)?;
        let file = row.file()?;
        if row == DiffRow::File(file) && top.offset_in_item <= px(0.) {
            return None;
        }
        let header = (0..=top.item_ix)
            .rev()
            .find(|&ix| diff.rows[ix] == DiffRow::File(file))?;
        // The file's last row pushes it, so that row and the card's end below it show whole
        // rather than a sliver of them under the header.
        let last = (top.item_ix..diff.rows.len())
            .find(|&ix| diff.rows[ix].file() != Some(file))
            .unwrap_or(diff.rows.len())
            - 1;
        let lift = self.list.bounds_for_item(last).map_or(px(0.), |last| {
            let room = last.top() - self.list.viewport_bounds().top();
            (room - px(diff::FILE_HEADER_HEIGHT)).min(px(0.))
        });
        Some((header, lift))
    }

    fn render_diff_row(&self, ix: usize, entity: WeakEntity<Detail>, cx: &App) -> AnyElement {
        let theme = Theme::get(cx);
        let Some(diff) = self.diff.as_ref().filter(|diff| ix < diff.rows.len()) else {
            return div().into_any_element();
        };
        let row = diff.rows[ix];
        let worktree = diff.key.source == DiffSource::Worktree;
        let store = self.store.read(cx);
        // What dragging or right-clicking a file or some of its hunks carries: changes of the
        // worktree or of one commit. A branch's diff spans commits, so it carries nothing.
        let payload = |file: usize, hunks| {
            let from = match diff.key.source {
                DiffSource::Worktree => None,
                // Not a commit of the target's history, on the project page: it isn't the
                // workspace's to change.
                DiffSource::Commit(id)
                    if store
                        .workspace(&diff.key.project)
                        .is_some_and(|ws| ws.find_commit(id).is_some()) =>
                {
                    Some(id)
                }
                _ => return None,
            };
            Some(Cut::Files {
                project: diff.key.project.clone(),
                from,
                files: vec![Selected {
                    file: diff.files[file].file.clone(),
                    hunks,
                }]
                .into(),
            })
        };
        let menu = |payload: Option<Cut>, line: Option<u32>, entity: WeakEntity<Detail>| {
            move |event: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
                if let Some(payload) = payload.clone() {
                    entity
                        .update(cx, |detail, cx| {
                            detail.changes_menu(payload, Some(line), event.position, cx)
                        })
                        .ok();
                }
            }
        };
        // The margin between the cards and the sides of the pane.
        let padded = div().w_full().px(px(16.));
        match row {
            DiffRow::File(file) => {
                let folded = matches!(diff.rows.get(ix + 1), Some(DiffRow::Gap));
                let review = (diff.key.review_scope())
                    .map(|scope| store.review(&scope, &diff.files[file].file));
                let whole = self.whole_button(ix, file, entity.clone());
                let toggle = entity.clone();
                let trailing = h_flex()
                    .flex_none()
                    .gap(px(4.))
                    .children(whole)
                    .children(review.map(|review| review_button(ix, file, review, entity, cx)));
                let header = diff::file_header(
                    file,
                    &diff.files[file],
                    theme,
                    HeaderDecor {
                        trailing: Some(trailing.into_any_element()),
                        current: self.cursor == Cursor { file, hunk: None },
                        folded,
                        ..Default::default()
                    },
                );
                padded
                    .id(("file-header", ix))
                    .child(header)
                    .on_mouse_down(
                        MouseButton::Right,
                        menu(payload(file, None), None, toggle.clone()),
                    )
                    .on_click(move |_, _, cx| {
                        toggle
                            .update(cx, |detail, cx| detail.toggle_fold(file, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Hunk { file, hunk } => {
                let DiffBody::Hunks { hunks, .. } = &diff.files[file].body else {
                    return padded.into_any_element();
                };
                let (path, key) = (&diff.files[file].file.path, hunks[hunk].key);
                let current = self.cursor
                    == Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                let composing = self.composing && worktree;
                let leading = composing.then(|| {
                    let check = self.hunk_check(path, key);
                    let (path, entity) = (path.clone(), entity.clone());
                    checkbox_box(("hunk-check", ix), check, theme)
                        .on_click(move |_, _, cx| {
                            entity
                                .update(cx, |detail, cx| detail.toggle_hunk(&path, key, cx))
                                .ok();
                        })
                        .into_any_element()
                });
                // What discarding this hunk means, fixed now: by the time a click lands, a
                // reload may have moved other hunks into this row's indices.
                // A conflicted file goes whole or not at all; see `backend::discard`.
                let discard = (worktree && !diff.files[file].file.conflicted).then(|| {
                    let change = Selected {
                        file: diff.files[file].file.clone(),
                        hunks: Some(vec![key]),
                    };
                    let project = diff.key.project.clone();
                    discard_button(ix, project, change, current, self.app.clone(), cx)
                });
                let header = diff::hunk_header(
                    &hunks[hunk],
                    theme,
                    HeaderDecor {
                        leading,
                        trailing: discard,
                        excluded: composing && !self.hunk_checked(path, key),
                        current,
                        ..Default::default()
                    },
                );
                let line = key.0.new_start;
                let carried = payload(file, Some(vec![key]));
                padded
                    .id(("hunk", ix))
                    .group("hunk-row")
                    .child(header)
                    .on_mouse_down(
                        MouseButton::Right,
                        menu(carried.clone(), Some(line), entity),
                    )
                    .when_some(carried, |el, carried| {
                        let label = format!("{path} · line {line}");
                        el.on_drag(
                            carried,
                            crate::navigator::drag_preview(self.store.clone(), label.into()),
                        )
                    })
                    .into_any_element()
            }
            DiffRow::Line {
                file,
                hunk,
                line,
                last,
            } => {
                let Some(text) = self.line_at(ix) else {
                    return padded.into_any_element();
                };
                let place = (file, hunk, line);
                let (excluded, toggle) = self.line_check(diff, place, &entity);
                let decor = LineDecor {
                    excluded,
                    toggle,
                    select: Some(line_select(ix, entity)),
                    selected: self.selected_in_row(ix),
                    found: self.found_on(place),
                    wrap: store.prefs.wrap,
                };
                let numbers = (text.old, text.new);
                let element = diff::line_row(text, numbers, place, last, theme, decor);
                padded.child(element).into_any_element()
            }
            DiffRow::Pair {
                file,
                hunk,
                left,
                right,
                last,
            } => {
                let DiffBody::Hunks { hunks, .. } = &diff.files[file].body else {
                    return padded.into_any_element();
                };
                let lines = &hunks[hunk].lines;
                let half = |line: Option<usize>| {
                    let Some(line) = line else {
                        return diff::Half::default();
                    };
                    let (excluded, toggle) = self.line_check(diff, (file, hunk, line), &entity);
                    diff::Half {
                        excluded,
                        toggle,
                        found: self.found_on((file, hunk, line)),
                    }
                };
                let sides = [
                    left.map(|l| (&lines[l], lines[l].old)),
                    right.map(|r| (&lines[r], lines[r].new)),
                ];
                // Each line is on one row only, so the right one, or the left where there's
                // none, tells rows apart.
                let place = (file, hunk, right.or(left).unwrap_or_default());
                let halves = [half(left), half(right)];
                let pair = diff::pair_row(sides, halves, place, last, theme, store.prefs.wrap);
                // A click puts the cursor on the hunk, so its keys act on what was clicked.
                let entity = entity.clone();
                padded
                    .child(pair)
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity
                            .update(cx, |detail, cx| {
                                detail.cursor = Cursor {
                                    file,
                                    hunk: Some(hunk),
                                };
                                cx.notify();
                            })
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Context {
                file,
                old,
                new,
                last,
            } => {
                let Some(line) = self.line_at(ix) else {
                    return padded.into_any_element();
                };
                let decor = LineDecor {
                    select: Some(line_select(ix, entity)),
                    selected: self.selected_in_row(ix),
                    wrap: store.prefs.wrap,
                    ..Default::default()
                };
                let split = store.prefs.split;
                let element = diff::context_row(line, file, (old, new), last, theme, decor, split);
                padded.child(element).into_any_element()
            }
            DiffRow::Note(file) => padded
                .child(diff::note(&diff.files[file], theme))
                .into_any_element(),
            DiffRow::Gap => padded.h(px(12.)).into_any_element(),
        }
    }
}

impl Detail {
    /// On a changed file's header: show the unchanged lines around its hunks, or hide them.
    fn whole_button(
        &self,
        ix: usize,
        file: usize,
        entity: WeakEntity<Detail>,
    ) -> Option<AnyElement> {
        let diff = self.diff.as_ref()?;
        let file_diff = &diff.files[file];
        let has_context = matches!(file_diff.body, DiffBody::Hunks { .. })
            && matches!(
                file_diff.file.kind,
                ChangeKind::Modified | ChangeKind::Renamed
            );
        if !has_context {
            return None;
        }
        let whole = self.whole.contains_key(&file_diff.file.path);
        let (icon, tip) = if whole {
            (Icon::CollapseVertical, "Show only the changes")
        } else {
            (Icon::ExpandVertical, "Show the whole file")
        };
        Some(
            icon_button(("whole", ix), icon)
                .small()
                .tooltip(tip)
                .on_click(move |_, _, cx| {
                    entity
                        .update(cx, |detail, cx| detail.toggle_whole(file, cx))
                        .ok();
                })
                .into_any_element(),
        )
    }

    /// Show every line of `file`, or only its hunks again.
    pub(super) fn toggle_whole(&mut self, file: usize, cx: &mut Context<Self>) {
        let Some(path) = (self.diff.as_ref())
            .and_then(|diff| diff.files.get(file))
            .map(|f| f.file.path.clone())
        else {
            return;
        };
        if self.whole.remove(&path).is_none() {
            self.whole.insert(path, None);
            self.load_whole(file, cx);
        }
        self.rebuild_rows(cx);
    }

    /// Read the lines of `file`, shown whole, as its current patch has them.
    pub(super) fn load_whole(&mut self, file: usize, cx: &mut Context<Self>) {
        let Some(diff) = &self.diff else {
            return;
        };
        let Some(root) = self.store.read(cx).path(&diff.key.project) else {
            return;
        };
        let Some(file) = diff.files.get(file) else {
            return;
        };
        let (change, fingerprint) = (file.file.clone(), file.fingerprint);
        let worktree = match &diff.key.source {
            DiffSource::Linked(name) => Some(name.clone()),
            _ => None,
        };
        let path = change.path.clone();
        // One read at a time: the one under way reads again when it lands stale.
        if !self.whole_reading.insert(path.clone()) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let lines =
                backend::run(move || backend::file_lines(&root, worktree.as_deref(), &change))
                    .await;
            this.update(cx, |this, cx| {
                this.whole_reading.remove(&path);
                if !this.whole.contains_key(&path) {
                    return;
                }
                // The file changed while it was read: read it again as it is now.
                let now = (this.diff.as_ref())
                    .and_then(|diff| diff.files.iter().position(|f| f.file.path == path));
                let current = now.is_some_and(|ix| {
                    this.diff
                        .as_ref()
                        .is_some_and(|d| d.files[ix].fingerprint == fingerprint)
                });
                if !current {
                    if let Some(ix) = now {
                        this.load_whole(ix, cx);
                    }
                    return;
                }
                match lines {
                    Ok(Some(lines)) => {
                        this.whole
                            .insert(path, Some((fingerprint, Arc::new(lines))));
                    }
                    Ok(None) => {
                        this.whole.remove(&path);
                    }
                    Err(err) => {
                        this.whole.remove(&path);
                        this.store.update(cx, |store, cx| {
                            store.error("Couldn't read the file", format!("{err:#}"), cx)
                        });
                    }
                }
                this.rebuild_rows(cx);
            })
            .ok();
        })
        .detach();
    }
}

impl Detail {
    /// Whether a hunk's line is left out of the next commit, and while composing, for a
    /// changed line, what clicking its numbers does.
    fn line_check(
        &self,
        diff: &LoadedDiff,
        (file, hunk, line): (usize, usize, usize),
        entity: &WeakEntity<Detail>,
    ) -> (bool, Option<diff::OnToggle>) {
        let DiffBody::Hunks { hunks, .. } = &diff.files[file].body else {
            return (false, None);
        };
        if !self.composing || diff.key.source != DiffSource::Worktree {
            return (false, None);
        }
        let (path, key) = (diff.files[file].file.path.clone(), hunks[hunk].key);
        if hunks[hunk].lines[line].kind == LineKind::Context {
            return (!self.hunk_checked(&path, key), None);
        }
        let excluded = !self.line_checked(&path, key, line);
        let entity = entity.clone();
        let toggle = Rc::new(move |_: &mut Window, cx: &mut App| {
            entity
                .update(cx, |detail, cx| detail.toggle_line(&path, key, line, cx))
                .ok();
        });
        (excluded, Some(toggle))
    }
}

/// What the pointer does to the text of the line at row `ix`: select from where it's
/// pressed, and extend as it drags.
fn line_select(ix: usize, entity: WeakEntity<Detail>) -> LineSelect {
    let (down, drag) = (entity.clone(), entity);
    LineSelect {
        down: Rc::new(move |at, event, window, cx| {
            let (count, shift) = (event.click_count, event.modifiers.shift);
            down.update(cx, |detail, cx| {
                detail.start_text_selection((ix, at), count, shift, window, cx)
            })
            .ok();
        }),
        drag: Rc::new(move |at, _, cx| {
            drag.update(cx, |detail, cx| detail.drag_text_selection((ix, at), cx))
                .ok();
        }),
    }
}

/// Lite's three-state review toggle on a file header.
fn review_button(
    ix: usize,
    file: usize,
    review: Review,
    entity: WeakEntity<Detail>,
    cx: &App,
) -> AnyElement {
    let theme = Theme::get(cx);
    let (check, label) = match review {
        Review::Reviewed => (Toggled::True, "Reviewed"),
        Review::Changed => (Toggled::Mixed, "Needs review"),
        Review::NotReviewed => (Toggled::False, "Not reviewed"),
    };
    let key = shortcut(&ToggleReviewed, cx);
    h_flex()
        .id(("review", ix))
        .flex_none()
        .h(px(22.))
        .px(px(6.))
        .gap(px(6.))
        .rounded(px(4.))
        .text_size(px(12.))
        .text_color(if review == Review::Changed {
            theme.warn
        } else {
            theme.text_2
        })
        .hover(|el| el.bg(theme.hover()))
        .child(checkbox_box(("review-check", ix), check, theme))
        .child(label)
        .tooltip(move |_, cx| {
            crate::ui::Tooltip::with_key(
                "Mark reviewed and go to the next file".into(),
                key.clone(),
                cx,
            )
        })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            entity
                .update(cx, |detail, cx| detail.toggle_reviewed_file(file, cx))
                .ok();
        })
        .into_any_element()
}

/// Discard for an uncommitted hunk, shown on the hunk under the cursor and on the one under
/// the pointer.
fn discard_button(
    ix: usize,
    project: ProjectId,
    change: Selected,
    current: bool,
    app: WeakEntity<AppView>,
    cx: &App,
) -> AnyElement {
    div()
        .flex_none()
        .font_family(Theme::get(cx).ui_font.clone())
        .when(!current, |el| {
            el.invisible().group_hover("hunk-row", |el| el.visible())
        })
        .child(
            button(("hunk-discard", ix), "Discard")
                .small()
                .style(ButtonStyle::Ghost)
                .tooltip_with_key("Throw this hunk away", shortcut(&DiscardHunk, cx))
                .on_click(move |_, window, cx| {
                    let changes = vec![change.clone()];
                    app.update(cx, |app, cx| {
                        app.discard(project.clone(), changes, window, cx)
                    })
                    .ok();
                }),
        )
        .into_any_element()
}

/// The word around byte `ix` of `text`, or an empty range when `ix` isn't in one.
fn word_at(text: &str, ix: usize) -> Range<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let start = text[..ix]
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(at, _)| at);
    let end = text[ix..]
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(at, _)| ix + at);
    start..end
}
