//! The diff column: every changed file as a card, built row by row so only what's on
//! screen is laid out. Uncommitted files carry review controls; every hunk gets a small
//! toolbar while it's under the cursor or the pointer, as in Zed's agent review.

use std::ops::Range;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Context, Entity, MouseButton, Subscription, WeakEntity, Window, div, list,
    prelude::*, px,
};

use super::{
    Check, Cursor, Detail, DiscardHunk, NextHunk, PreviousHunk, TextPoint, ToggleReviewed,
    checkbox_box,
};
use crate::app::{AppView, MenuItem};
use crate::backend::{DiffBody, DiffLine, DiffSource, Selected};
use crate::state::{ProjectId, Review};
use crate::theme::Theme;
use crate::ui::diff::{self, Decor, DiffRow, LineSelect};
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let drawn = self.detail.update(cx, |detail, cx| {
            (detail.focus.clone(), detail.render_diff(&theme, cx))
        });
        let Ok((focus, diff)) = drawn else {
            return h_flex();
        };
        h_flex()
            .size_full()
            // Keys go to the detail, as when the diff was drawn inside it.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx)
            })
            .child(diff)
    }
}

impl Detail {
    /// The diff line at `row`, if that row is one.
    fn line_at(&self, row: usize) -> Option<&DiffLine> {
        let diff = self.diff.as_ref()?;
        let DiffRow::Line {
            file, hunk, line, ..
        } = *diff.rows.get(row)?
        else {
            return None;
        };
        match &diff.files.get(file)?.body {
            DiffBody::Hunks { hunks, .. } => hunks.get(hunk)?.lines.get(line),
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
                list(self.list.clone(), move |ix, _window, cx| {
                    let Some(detail) = entity.upgrade() else {
                        return div().into_any_element();
                    };
                    detail.read(cx).render_diff_row(ix, detail.downgrade(), cx)
                })
                .size_full(),
            )
            .into_any_element()
    }

    fn render_diff_row(&self, ix: usize, entity: WeakEntity<Detail>, cx: &App) -> AnyElement {
        let theme = Theme::get(cx);
        let Some(diff) = self.diff.as_ref().filter(|diff| ix < diff.rows.len()) else {
            return div().into_any_element();
        };
        let row = diff.rows[ix];
        let worktree = diff.key.source == DiffSource::Worktree;
        let store = self.store.read(cx);
        let hunk_key = |file: usize, hunk: usize| match &diff.files[file].body {
            DiffBody::Hunks { hunks, .. } => {
                Some((diff.files[file].file.path.clone(), hunks[hunk].key))
            }
            _ => None,
        };
        let excluded = |file: usize, hunk: usize| {
            self.composing
                && worktree
                && hunk_key(file, hunk).is_some_and(|(path, key)| !self.hunk_checked(&path, key))
        };
        match row {
            DiffRow::File(file) => {
                let folded = matches!(diff.rows.get(ix + 1), Some(DiffRow::Gap));
                let review =
                    worktree.then(|| store.review(&diff.key.project, &diff.files[file].file));
                let toggle = entity.clone();
                let header = diff::render_row(
                    row,
                    &diff.files,
                    theme,
                    Decor {
                        trailing: review.map(|review| review_button(ix, file, review, entity, cx)),
                        current: self.cursor == Cursor { file, hunk: None },
                        folded,
                        ..Default::default()
                    },
                );
                div()
                    .id(("file-header", ix))
                    .child(header)
                    .on_click(move |_, _, cx| {
                        toggle
                            .update(cx, |detail, cx| detail.toggle_fold(file, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Hunk { file, hunk } => {
                let current = self.cursor
                    == Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                let leading = (self.composing && worktree)
                    .then(|| hunk_key(file, hunk))
                    .flatten()
                    .map(|(path, key)| {
                        let check = if self.hunk_checked(&path, key) {
                            Check::All
                        } else {
                            Check::None
                        };
                        let entity = entity.clone();
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
                let discard = hunk_key(file, hunk).filter(|_| worktree).map(|(_, key)| {
                    let change = Selected {
                        file: diff.files[file].file.clone(),
                        hunks: Some(vec![key]),
                    };
                    Rc::new((diff.key.project.clone(), change))
                });
                let toolbar = hunk_toolbar(
                    ix,
                    (file, hunk),
                    discard.clone(),
                    current,
                    (entity.clone(), self.app.clone()),
                    cx,
                );
                let element = diff::render_row(
                    row,
                    &diff.files,
                    theme,
                    Decor {
                        leading,
                        trailing: Some(toolbar),
                        excluded: excluded(file, hunk),
                        current,
                        ..Default::default()
                    },
                );
                // Right-click offers the same discard, with its key.
                div()
                    .id(("hunk", ix))
                    .group("hunk-row")
                    .child(element)
                    .when_some(discard, |el, discard| {
                        let app = self.app.clone();
                        el.on_mouse_down(MouseButton::Right, move |event, _, cx| {
                            let discard = discard.clone();
                            let item = MenuItem::new("Discard hunk", move |app, window, cx| {
                                let (project, change) = (*discard).clone();
                                app.discard(project, vec![change], window, cx)
                            })
                            .key(shortcut(&DiscardHunk, cx))
                            .danger();
                            app.update(cx, |app, cx| app.open_menu(event.position, vec![item], cx))
                                .ok();
                        })
                    })
                    .into_any_element()
            }
            DiffRow::Line { file, hunk, .. } => {
                let (down, drag) = (entity.clone(), entity);
                let select = LineSelect {
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
                };
                diff::render_row(
                    row,
                    &diff.files,
                    theme,
                    Decor {
                        excluded: excluded(file, hunk),
                        select: Some(select),
                        selected: self.selected_in_row(ix),
                        ..Default::default()
                    },
                )
            }
            DiffRow::Note(_) | DiffRow::Gap => {
                diff::render_row(row, &diff.files, theme, Decor::default())
            }
        }
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
        Review::Reviewed => (Check::All, "Reviewed"),
        Review::Changed => (Check::Some, "Needs review"),
        Review::NotReviewed => (Check::None, "Not reviewed"),
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

/// Previous, next and (for uncommitted changes) discard, shown on the hunk under the
/// cursor and on the one under the pointer.
fn hunk_toolbar(
    ix: usize,
    (file, hunk): (usize, usize),
    discard: Option<Rc<(ProjectId, Selected)>>,
    current: bool,
    (entity, app): (WeakEntity<Detail>, WeakEntity<AppView>),
    cx: &App,
) -> AnyElement {
    let step = |forward: bool| {
        let entity = entity.clone();
        move |_: &gpui::ClickEvent, _: &mut gpui::Window, cx: &mut App| {
            entity
                .update(cx, |detail, cx| {
                    detail.cursor = Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                    detail.move_hunk(forward, cx);
                })
                .ok();
        }
    };
    h_flex()
        .flex_none()
        .gap(px(2.))
        .font_family(Theme::get(cx).ui_font.clone())
        .when(!current, |el| {
            el.invisible().group_hover("hunk-row", |el| el.visible())
        })
        .child(
            icon_button(("hunk-up", ix), Icon::ArrowUp)
                .small()
                .tooltip_with_key("Previous hunk", shortcut(&PreviousHunk, cx))
                .on_click(step(false)),
        )
        .child(
            icon_button(("hunk-down", ix), Icon::ArrowDown)
                .small()
                .tooltip_with_key("Next hunk", shortcut(&NextHunk, cx))
                .on_click(step(true)),
        )
        .when_some(discard, |el, discard| {
            el.child(
                button(("hunk-discard", ix), "Discard")
                    .small()
                    .style(ButtonStyle::Ghost)
                    .tooltip_with_key("Throw this hunk away", shortcut(&DiscardHunk, cx))
                    .on_click(move |_, window, cx| {
                        let (project, change) = (*discard).clone();
                        app.update(cx, |app, cx| app.discard(project, vec![change], window, cx))
                            .ok();
                    }),
            )
        })
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
