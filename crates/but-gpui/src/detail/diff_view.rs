//! The diff column: every changed file as a card, built row by row so only what's on
//! screen is laid out. Uncommitted hunks carry a Discard button while under the cursor or
//! the pointer, as in Zed's agent review, a linked worktree's too.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Action, AnyElement, App, ClickEvent, Context, CursorStyle, Entity, MouseButton, MouseDownEvent,
    Pixels, Point, SharedString, Subscription, WeakEntity, Window, div, list, prelude::*, px,
};

use super::lines::Pick;
use super::{
    Cursor, Detail, DiscardHunk, LoadedDiff, TextPoint, ToggleFiles, TogglePlainLines, ToggleSplit,
    ToggleWrap,
};
use crate::app::AppView;
use crate::backend::{
    self, Branch, ChangeKind, DiffBody, DiffLine, DiffSource, Drawing, FileDiff, LineKind,
    PushState, Selected,
};
use crate::overlays::MenuItem;
use crate::state::{Cut, Prefs, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow, HeaderDecor, LineDecor, LineSelect};
use crate::ui::illustration::Illustration;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, checkbox, empty_state, h_flex, icon, icon_button, shortcut,
    small_icon, toggle_group, v_flex,
};

/// The diff, drawn by a view of its own over the space [`Detail`] leaves for it. A scrolled
/// list notifies the view that drew it, and GPUI draws everything inside a notified view again,
/// so as part of `Detail` every scroll frame laid out the file list too.
pub struct DiffPane {
    detail: WeakEntity<Detail>,
    /// The files of the diff drawn last. Another diff is drawn a frame after the rest of the
    /// detail, so the new file list and the new diff don't share one frame's budget.
    shown: Option<Arc<Vec<FileDiff>>>,
    /// Waiting for that frame: until it comes, what changes is drawn then.
    deferred: bool,
    /// Where the diff was scrolled to when drawn last. gpui settles a scroll asked for past
    /// the end only as it lays the list out, after the sticky header was worked out, so
    /// a change draws once more.
    top: (usize, Pixels),
    _observe: Subscription,
}

impl DiffPane {
    pub fn new(detail: &Entity<Detail>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            detail: detail.downgrade(),
            shown: None,
            deferred: false,
            top: (0, px(0.)),
            // Whatever changes the detail can change the diff; scrolling only notifies this.
            _observe: cx.observe_in(detail, window, |this, detail, window, cx| {
                if this.deferred {
                    return;
                }
                let files = (detail.read(cx).diff.as_ref()).map(|diff| diff.files.clone());
                if this.shown.as_ref().map(Arc::as_ptr) == files.as_ref().map(Arc::as_ptr) {
                    return cx.notify();
                }
                this.shown = files;
                this.deferred = true;
                // GPUI runs these as a frame starts, before drawing it: the outer one runs
                // as the detail's frame starts and puts the diff in the frame after.
                let pane = cx.entity().downgrade();
                window.on_next_frame(move |window, _| {
                    window.on_next_frame(move |_, cx| {
                        (pane.update(cx, |this, cx| {
                            this.deferred = false;
                            cx.notify()
                        }))
                        .ok();
                    })
                });
            }),
        }
    }
}

impl Render for DiffPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::get(cx).clone();
        let drawn = self.detail.update(cx, |detail, cx| {
            detail.follow_scroll(cx);
            let top = detail.list.logical_scroll_top();
            (
                (top.item_ix, top.offset_in_item),
                detail.focus.clone(),
                detail.render_diff_tools(&theme, cx),
                detail.render_diff(&theme, cx),
                detail.render_find(window, &theme, cx),
            )
        });
        let Ok((top, focus, tools, diff, find)) = drawn else {
            return v_flex();
        };
        if top != self.top {
            self.top = top;
            window.request_animation_frame();
        }
        // A press in the diff, as on its scrollbar, hands the cursor back to its scroll;
        // what was pressed may then place it again.
        let unpin = self.detail.clone();
        v_flex()
            .size_full()
            .capture_any_mouse_down(move |_, _, cx| {
                (unpin.update(cx, |detail, _| detail.pinned = false)).ok();
            })
            // Keys go to the detail, as when the diff was drawn inside it.
            .on_mouse_down(MouseButton::Left, move |_, window, cx| {
                window.focus(&focus, cx)
            })
            .children(tools)
            .child(
                h_flex()
                    .flex_1()
                    .min_h(px(0.))
                    .relative()
                    .child(diff)
                    .children(find),
            )
    }
}

impl Detail {
    /// The diff line at `row`, if that row is one.
    fn line_at(&self, row: usize) -> Option<&DiffLine> {
        let diff = self.diff.as_ref()?;
        match *diff.rows.get(row)? {
            DiffRow::Line {
                file, hunk, line, ..
            } => diff.files.get(file)?.hunks().get(hunk)?.lines.get(line),
            // An unchanged line of a file shown whole, from its new side.
            DiffRow::Context { file, new, .. } => {
                let (_, lines, _) = self.whole.get(&diff.files.get(file)?.file.path)?.as_ref()?;
                lines.get((new as usize).checked_sub(1)?)
            }
            _ => None,
        }
    }

    /// How `file`'s rows lay out with `prefs`. Its numbers fit its last hunk's, or with the
    /// file shown whole, its length on either side. Its lines scroll sideways on each side,
    /// or unified, both sides as one.
    fn layout(&self, file: &FileDiff, prefs: &Prefs) -> diff::Layout {
        let mut layout = diff::Layout {
            wrap: prefs.wrap,
            plain: prefs.plain_lines,
            split: prefs.split,
            ..Default::default()
        };
        let DiffBody::Hunks {
            hunks,
            added,
            removed,
            widest,
            ..
        } = &file.body
        else {
            return layout;
        };
        let last = hunks.last().map_or(0, |hunk| {
            let side = |number: fn(&DiffLine) -> Option<u32>| {
                hunk.lines.iter().rev().find_map(number).unwrap_or(0)
            };
            side(|line| line.old).max(side(|line| line.new))
        });
        let (length, whole_columns) = match self.whole.get(&file.file.path) {
            Some(Some((_, lines, columns))) => {
                let new = lines.len() as u32;
                (new.max((new + removed).saturating_sub(*added)), *columns)
            }
            _ => (0, 0),
        };
        layout.digits = last.max(length).to_string().len();
        let columns = widest.map(|side| side.max(whole_columns));
        let scroll = self
            .sideways
            .get(&file.file.path)
            .cloned()
            .unwrap_or_default();
        layout.sideways = match prefs.split {
            true => [0, 1].map(|side| diff::Sideways {
                scroll: scroll[side].clone(),
                columns: columns[side],
            }),
            false => {
                let both = diff::Sideways {
                    scroll: scroll[1].clone(),
                    columns: columns[0].max(columns[1]),
                };
                [both.clone(), both]
            }
        };
        layout
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
            self.place_cursor(Cursor {
                file,
                hunk: Some(hunk),
            });
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
        let ground = || div().flex_1().min_w(px(0.)).h_full().bg(theme.bg_1);
        if let Some(err) = &self.diff_error {
            let message = empty_state(
                Some(Illustration::Papers),
                "Couldn't load the diff",
                err.clone(),
                theme,
                cx,
            );
            return ground().child(message).into_any_element();
        }
        let Some(diff) = self.shown_diff() else {
            let message = match (self.loading.is_some(), self.nothing_committed(cx)) {
                (true, _) => empty_state(None, "Loading diff…", "", theme, cx),
                (false, Some(line)) => empty_state(Some(Illustration::Cactus), "", line, theme, cx),
                (false, None) => empty_state(None, "", "", theme, cx),
            };
            return ground().child(message).into_any_element();
        };
        if diff.files.is_empty() {
            // A clean worktree is good news, as but.dev greets it; a commit with nothing in
            // it is a list with nothing in it.
            let (drawing, title, detail): (_, _, SharedString) = match diff.key.source {
                DiffSource::Worktree | DiffSource::Linked(_) => (
                    Illustration::Waving,
                    "No uncommitted changes",
                    "Edits show up here as soon as they're saved".into(),
                ),
                _ => (Illustration::Cactus, "", self.changes_nothing(cx)),
            };
            return ground()
                .child(empty_state(Some(drawing), title, detail, theme, cx))
                .into_any_element();
        }
        if diff.rows.is_empty() && self.filtering {
            let filter = self.file_filter.read(cx).text();
            let title = format!("No files match \"{}\"", filter.trim());
            return ground()
                .child(empty_state(
                    Some(Illustration::Papers),
                    title,
                    "Escape shows them all again",
                    theme,
                    cx,
                ))
                .into_any_element();
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
                        .bg(theme.bg_1)
                        .block_mouse_except_scroll()
                        .child(self.render_diff_row(ix, entity.clone(), cx)),
                )
        });
        let scrollbar = scrollbar("diff-scrollbar", Scroll::List(self.list.clone()), theme);
        ground()
            // With the bar above it, as far down as but.dev starts its first card.
            .pt(px(7.))
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
                            (detail.read(cx)).render_diff_row(ix, detail.downgrade(), cx)
                        })
                        .size_full(),
                    )
                    .children(sticky)
                    .children(scrollbar),
            )
            .into_any_element()
    }

    /// For a selected branch or stack with no commits, what but.dev's branch page says.
    fn nothing_committed(&self, cx: &App) -> Option<SharedString> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        let workspace = store.workspace(selection.project())?;
        let target = workspace
            .target
            .as_ref()
            .map_or("the target", |t| t.name.as_str());
        let branches: Vec<&Branch> = match selection {
            Selection::Branch(_, ref_name) => vec![workspace.find_branch(ref_name)?],
            Selection::Stack(_, bottom) => workspace.lane(bottom)?.iter().collect(),
            _ => return None,
        };
        let what = match selection {
            Selection::Stack(..) => "these branches",
            _ => "this branch",
        };
        (branches.iter().all(|branch| branch.commits.is_empty()))
            .then(|| format!("No commits above {target} on {what}").into())
    }

    /// What a diff of commits with no changes in it says, as but.dev's branch page does: a
    /// branch the target already has changes nothing against it.
    fn changes_nothing(&self, cx: &App) -> SharedString {
        let store = self.store.read(cx);
        let Some(selection) = store.selection.as_ref() else {
            return "No changes".into();
        };
        let workspace = store.workspace(selection.project());
        let target =
            (workspace.and_then(|ws| ws.target.as_ref())).map_or("the target", |t| t.name.as_str());
        match selection {
            Selection::Commit(..) => "This commit changes no files".into(),
            Selection::Branch(..) | Selection::Stack(..)
                if store
                    .branch_for(selection)
                    .is_some_and(|b| b.push == PushState::Integrated) =>
            {
                format!(
                    "{target} already has these commits, so the branch changes nothing against it"
                )
                .into()
            }
            Selection::Branch(..) | Selection::Stack(..) => {
                format!("Nothing changed against {target}").into()
            }
            _ => "No changes".into(),
        }
    }

    /// The bar over the diff, as but.dev's: the file list's toggle, which counts the files and
    /// their lines while the list is hidden, then how long lines show and where the sides go.
    pub(super) fn render_diff_tools(&self, theme: &Theme, cx: &App) -> Option<AnyElement> {
        let diff = self.shown_diff()?;
        let prefs = &self.store.read(cx).prefs;
        let (added, removed) = (diff.files.iter().map(FileDiff::stats))
            .fold((0, 0), |(a, r), (added, removed)| (a + added, r + removed));
        // To the detail's own handlers, wherever focus is.
        let act = |action: &'static dyn Action| {
            let focus = self.focus.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                focus.dispatch_action(action, window, cx)
            }
        };
        let hidden = self.files_hidden;
        let files_key = shortcut(&ToggleFiles, cx);
        let files = h_flex()
            .id("diff-files")
            .flex_none()
            .h(px(28.))
            .px(px(6.))
            .gap(px(6.))
            .rounded(px(4.))
            .hover(|el| el.bg(theme.hover()))
            .child(
                icon(if hidden {
                    Icon::SidebarNarrow
                } else {
                    Icon::FilesSidebar
                })
                .text_color(theme.text_2),
            )
            .when(hidden, |el| {
                el.child(
                    div()
                        .px(px(6.))
                        .rounded_full()
                        .bg(theme.fill_gray.opacity(0.1))
                        .text_size(px(11.))
                        .text_color(theme.text_2)
                        .child(diff.files.len().to_string()),
                )
                .child(diff::stats(added, removed, theme))
            })
            .tooltip(move |_, cx| {
                let tip = if hidden {
                    "Show the files"
                } else {
                    "Hide the files"
                };
                Tooltip::with_key(tip.into(), files_key.clone(), cx)
            })
            .on_click(act(&ToggleFiles));
        let group = || toggle_group(theme);
        // A choice of the group, which `action` makes when `on` isn't already.
        let choice = |id: &'static str,
                      label: &'static str,
                      tip: &'static str,
                      on: bool,
                      action: &'static dyn Action| {
            let button = (button(id, label).small().selected(on))
                .tooltip_with_key(tip, shortcut(action, cx));
            if on {
                button
            } else {
                button.on_click(act(action))
            }
        };
        let (wrap, split, plain) = (prefs.wrap, prefs.split, prefs.plain_lines);
        Some(
            h_flex()
                .flex_none()
                .h(px(40.))
                .px(px(12.))
                .gap(px(8.))
                .bg(theme.bg_1)
                .child(files)
                .child(div().flex_1())
                .child(
                    group()
                        .child(
                            (icon_button("diff-wrap", Icon::TextWrap)
                                .small()
                                .selected(wrap))
                            .tooltip_with_key("Wrap long lines", shortcut(&ToggleWrap, cx))
                            .on_click(act(&ToggleWrap)),
                        )
                        .child(
                            (icon_button("diff-backgrounds", Icon::TextBlock).small())
                                .selected(!plain)
                                .tooltip("Toggle diff backgrounds")
                                .on_click(act(&TogglePlainLines)),
                        ),
                )
                .child(
                    group()
                        .child(choice(
                            "diff-split",
                            "Split",
                            "Side by side",
                            split,
                            &ToggleSplit,
                        ))
                        .child(choice(
                            "diff-unified",
                            "Unified",
                            "One above the other",
                            !split,
                            &ToggleSplit,
                        )),
                )
                .into_any_element(),
        )
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
        // A folded file's header ends its card too.
        let last = diff::ends_card(&diff.rows, ix);
        let worktree = self.worktree_diff().is_some();
        let store = self.store.read(cx);
        let menu = |menu: Option<Cut>, line: Option<u32>, entity: WeakEntity<Detail>| {
            move |event: &gpui::MouseDownEvent, _: &mut Window, cx: &mut App| {
                if let Some(menu) = menu.clone() {
                    entity
                        .update(cx, |detail, cx| {
                            detail.changes_menu(menu, Some(line), event.position, cx)
                        })
                        .ok();
                }
            }
        };
        // The margin between the cards and the sides of the pane.
        let padded = div().w_full().px(px(12.));
        match row {
            DiffRow::File(file) => {
                let whole = self.whole_button(ix, file, &diff.files[file], entity.clone());
                let drawn = self.drawing_button(ix, file, &diff.files[file], entity.clone());
                let toggle = entity.clone();
                // What right-clicking the header offers, behind a ⋯ as well.
                let header_menu = self.diff_cut(file, None, cx);
                let more = header_menu.clone().map(|menu| {
                    let entity = entity.clone();
                    icon_button(("file-more", ix), Icon::Kebab)
                        .small()
                        .on_click(move |event, _, cx| {
                            let (menu, position) = (menu.clone(), event.position());
                            (entity.update(cx, |detail, cx| {
                                detail.changes_menu(menu, Some(None), position, cx)
                            }))
                            .ok();
                        })
                });
                let trailing = h_flex()
                    .flex_none()
                    .gap(px(4.))
                    // The toggle last but one: showing text adds the whole-file button, which
                    // mustn't move it from under the pointer.
                    .children(whole)
                    .children(drawn)
                    .children(more);
                let header = diff::file_header(
                    file,
                    &diff.files[file],
                    theme,
                    HeaderDecor {
                        trailing: Some(trailing.into_any_element()),
                        current: self.cursor.file == file,
                        folded: last,
                        ..Default::default()
                    },
                    cx,
                );
                padded
                    .id(("file-header", ix))
                    .child(header)
                    .on_mouse_down(MouseButton::Left, press_off(entity.clone()))
                    .when_some(header_menu, |el, header_menu| {
                        el.on_mouse_down(
                            MouseButton::Right,
                            menu(Some(header_menu), None, toggle.clone()),
                        )
                    })
                    .on_click(move |_, _, cx| {
                        toggle
                            .update(cx, |detail, cx| detail.toggle_fold(file, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Hunk { file, hunk } => {
                let hunks = diff.files[file].hunks();
                let Some(key) = hunks.get(hunk).map(|h| h.key) else {
                    return padded.into_any_element();
                };
                let path = &diff.files[file].file.path;
                let current = self.cursor
                    == Cursor {
                        file,
                        hunk: Some(hunk),
                    };
                let composing = self.composing && worktree;
                let leading = composing.then(|| {
                    let check = self.checks.hunk_check(path, key);
                    let (path, entity) = (path.clone(), entity.clone());
                    checkbox(("hunk-check", ix), check, theme)
                        .on_click(move |_, _, cx| {
                            // Not a click on the hunk, which shows its file whole.
                            cx.stop_propagation();
                            entity
                                .update(cx, |detail, cx| detail.toggle_hunk(&path, key, cx))
                                .ok();
                        })
                        .into_any_element()
                });
                // What discarding this hunk means, fixed now: by the time a click lands, a
                // reload may have moved other hunks into this row's indices.
                // A conflicted file goes whole or not at all; see `backend::discard`.
                let discard =
                    (diff.key.source.live() && !diff.files[file].file.conflicted).then(|| {
                        let change = diff.files[file].part(Some(vec![key]));
                        let (project, linked) = (diff.key.project.clone(), diff.key.linked());
                        discard_button(ix, project, linked, change, current, self.app.clone(), cx)
                    });
                // The lines before the hunk, unless the file shown whole has them above.
                let shown = matches!(
                    ix.checked_sub(1).map(|row| diff.rows[row]),
                    Some(DiffRow::Context { .. })
                );
                let unmodified = if shown {
                    0
                } else {
                    diff::unmodified_before(hunks, hunk)
                };
                let header = diff::hunk_header(
                    &hunks[hunk],
                    unmodified,
                    theme,
                    HeaderDecor {
                        leading,
                        trailing: discard,
                        excluded: composing && !self.checks.hunk_checked(path, key),
                        current,
                        ..Default::default()
                    },
                );
                let line = key.0.new_start;
                let carried = self.diff_cut(file, Some(vec![key]), cx);
                let (whole, dragged) = (entity.clone(), entity.clone());
                padded
                    .id(("hunk", ix))
                    .group("hunk-row")
                    .child(header)
                    .on_mouse_down(
                        MouseButton::Right,
                        menu(carried.clone(), Some(line), entity),
                    )
                    .when(unmodified > 0, |el| {
                        el.tooltip(Tooltip::text("Show the whole file")).on_click(
                            move |_, _, cx| {
                                whole
                                    .update(cx, |detail, cx| detail.toggle_whole(file, cx))
                                    .ok();
                            },
                        )
                    })
                    .on_mouse_down(MouseButton::Left, press_off(dragged.clone()))
                    .when_some(carried, |el, carried| {
                        let lines: Vec<_> =
                            (0..hunks[hunk].lines.len()).map(|l| (hunk, l)).collect();
                        let label = self.lines_label(file, &lines);
                        el.on_drag(carried, self.drag_preview(label, false, dragged))
                    })
                    .into_any_element()
            }
            DiffRow::Line { file, hunk, line } => {
                let Some(text) = self.line_at(ix) else {
                    return padded.into_any_element();
                };
                let menu = line_menu(ix, entity.clone());
                let place = (file, hunk, line);
                let (excluded, toggle) = self.line_check(diff, place, &entity);
                let decor = LineDecor {
                    excluded,
                    toggle,
                    gutter: self.line_gutter(place, &entity, cx),
                    select: Some(line_select(ix, entity)),
                    selected: self.selected_in_row(ix),
                    found: self.found_on(place),
                };
                let layout = self.layout(&diff.files[file], &store.prefs);
                let element = diff::line_row(text, place, last, theme, &layout, decor);
                padded
                    .child(element)
                    .on_mouse_down(MouseButton::Right, menu)
                    .into_any_element()
            }
            DiffRow::Pair {
                file,
                hunk,
                left,
                right,
            } => {
                let Some(lines) = diff.files[file].hunks().get(hunk).map(|h| &h.lines) else {
                    return padded.into_any_element();
                };
                let half = |line: Option<usize>| {
                    let Some(line) = line else {
                        return diff::Half::default();
                    };
                    let (excluded, toggle) = self.line_check(diff, (file, hunk, line), &entity);
                    diff::Half {
                        excluded,
                        toggle,
                        found: self.found_on((file, hunk, line)),
                        gutter: self.line_gutter((file, hunk, line), &entity, cx),
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
                let layout = self.layout(&diff.files[file], &store.prefs);
                let pair = diff::pair_row(sides, halves, place, last, theme, &layout);
                // A click puts the cursor on the hunk, so its keys act on what was clicked.
                let entity = entity.clone();
                padded
                    .child(pair)
                    .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                        entity
                            .update(cx, |detail, cx| {
                                detail.press_off_picks(cx);
                                detail.place_cursor(Cursor {
                                    file,
                                    hunk: Some(hunk),
                                });
                                cx.notify();
                            })
                            .ok();
                    })
                    .into_any_element()
            }
            DiffRow::Context { file, old, new } => {
                let Some(line) = self.line_at(ix) else {
                    return padded.into_any_element();
                };
                let menu = line_menu(ix, entity.clone());
                let decor = LineDecor {
                    select: Some(line_select(ix, entity)),
                    selected: self.selected_in_row(ix),
                    ..Default::default()
                };
                let layout = self.layout(&diff.files[file], &store.prefs);
                let element =
                    diff::context_row(line, file, (old, new), last, theme, &layout, decor);
                padded
                    .child(element)
                    .on_mouse_down(MouseButton::Right, menu)
                    .into_any_element()
            }
            DiffRow::Conflict {
                file,
                conflict,
                part,
            } => {
                let Some(found) = diff.files[file].conflicts().get(conflict) else {
                    return padded.into_any_element();
                };
                let layout = self.layout(&diff.files[file], &store.prefs);
                let element = self.conflict_row(found, part, &layout, last, entity, cx);
                padded.child(element).into_any_element()
            }
            DiffRow::Note(file) => padded
                .child(match self.drawing_of(&diff.files[file]) {
                    Some(drawn) => diff::drawing(drawn.before.clone(), drawn.after.clone(), theme),
                    None => diff::note(&diff.files[file], theme),
                })
                .into_any_element(),
            DiffRow::Gap => padded.h(px(10.)).into_any_element(),
        }
    }
}

impl Detail {
    /// On a changed file's header: show the unchanged lines around its hunks, or hide them.
    fn whole_button(
        &self,
        ix: usize,
        file: usize,
        file_diff: &FileDiff,
        entity: WeakEntity<Detail>,
    ) -> Option<AnyElement> {
        let has_context = matches!(file_diff.body, DiffBody::Hunks { .. })
            && self.drawing_of(file_diff).is_none()
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

    /// On an SVG's header: show its lines rather than its drawing, or the drawing again.
    fn drawing_button(
        &self,
        ix: usize,
        file: usize,
        file_diff: &FileDiff,
        entity: WeakEntity<Detail>,
    ) -> Option<AnyElement> {
        file_diff.drawing.as_ref()?;
        let text = self.svg_text.contains(&file_diff.file.path);
        Some(
            icon_button(("drawing", ix), Icon::TextCode)
                .small()
                .selected(text)
                .tooltip(if text {
                    "Show the drawing"
                } else {
                    "Show as text"
                })
                .on_click(move |_, _, cx| {
                    (entity.update(cx, |detail, cx| detail.toggle_drawing(file, cx))).ok();
                })
                .into_any_element(),
        )
    }

    /// The drawing `file` shows in place of its lines: an SVG's, unless it's shown as text.
    pub(super) fn drawing_of<'a>(&self, file: &'a FileDiff) -> Option<&'a Drawing> {
        (file.drawing.as_ref()).filter(|_| !self.svg_text.contains(&file.file.path))
    }

    /// Show `file`'s lines rather than its drawing, or the drawing again.
    fn toggle_drawing(&mut self, file: usize, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file) else {
            return;
        };
        if !self.svg_text.remove(&path) {
            self.svg_text.insert(path);
        }
        self.rebuild_rows(cx);
    }

    /// Show every line of `file`, or only its hunks again.
    pub(super) fn toggle_whole(&mut self, file: usize, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file) else {
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
        let worktree = diff.key.linked();
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
                let now = (this.diff.as_ref()).and_then(|diff| {
                    (diff.files.iter().enumerate())
                        .find(|(_, f)| f.file.path == path)
                        .map(|(ix, f)| (ix, f.fingerprint))
                });
                match now {
                    Some((_, now)) if now == fingerprint => {}
                    // The file changed while it was read: read it again as it is now.
                    Some((ix, _)) => return this.load_whole(ix, cx),
                    None => return,
                }
                match lines {
                    Ok(Some((lines, widest))) => {
                        this.whole
                            .insert(path, Some((fingerprint, Arc::new(lines), widest)));
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
        let Some(hunk) = diff.files[file].hunks().get(hunk) else {
            return (false, None);
        };
        if !self.composing || self.worktree_diff().is_none() {
            return (false, None);
        }
        let (path, key) = (diff.files[file].file.path.clone(), hunk.key);
        if hunk.lines[line].kind == LineKind::Context {
            return (!self.checks.hunk_checked(&path, key), None);
        }
        let excluded = !self.checks.line_checked(&path, key, line);
        let entity = entity.clone();
        let toggle = Rc::new(move |_: &mut Window, cx: &mut App| {
            entity
                .update(cx, |detail, cx| detail.toggle_line(&path, key, line, cx))
                .ok();
        });
        (excluded, Some(toggle))
    }
}

impl Detail {
    /// What line `place` (its file, hunk and index in the hunk) offers in its numbers outside
    /// the composer: picking it, and a grip that drags what it takes. None for an unchanged
    /// line, and in a diff whose changes can't move, like a branch's.
    fn line_gutter(
        &self,
        place: (usize, usize, usize),
        entity: &WeakEntity<Detail>,
        cx: &App,
    ) -> Option<diff::Gutter> {
        let (file, hunk, line) = place;
        let diff_file = self.diff.as_ref()?.files.get(file)?;
        let key = diff_file.hunks().get(hunk)?.key;
        let changed = (diff_file.hunks()[hunk].lines.get(line))
            .is_some_and(|line| line.kind != LineKind::Context);
        // The composer's checks own the numbers while it's open.
        if !changed || (self.composing && self.worktree_diff().is_some()) {
            return None;
        }
        // Lines of a diff that can't be cut from have no grip, nor numbers to pick.
        self.owned_cut(Vec::new(), cx)?;
        let picked = self.line_picked(&diff_file.file.path, key, line);
        // What a grip takes is worked out for the one under the pointer only, afresh as lines
        // get picked or the diff changes: it can be a whole file's lines, too many to work out
        // for every line drawn.
        let over = (self.grip_over.as_ref()).filter(|(at, _)| at.0 == file);
        let taken =
            |line: usize| over.is_some_and(|(_, lines)| lines.binary_search(&(hunk, line)).is_ok());
        let outline = taken(line).then(|| (line == 0 || !taken(line - 1), !taken(line + 1)));
        let drag = (over.is_some_and(|(at, _)| *at == place))
            .then(|| self.grip_takes(place))
            .and_then(|takes| {
                let cut = self.lines_cut(file, takes.iter().copied(), cx)?;
                Some((cut, self.lines_label(file, &takes)))
            });
        let picking = entity.clone();
        let pick = Rc::new(
            move |modifiers, dragging, window: &mut Window, cx: &mut App| {
                (picking.update(cx, |detail, cx| match dragging {
                    // Dragging on down the numbers from a press on one runs the pick along.
                    true if detail.picking_lines && detail.line_picks.is_some() => {
                        detail.pick_line(place, Pick::Run, cx)
                    }
                    true => {}
                    // With the diff's keys, so ⌘X cuts what's picked.
                    false => {
                        detail.picking_lines = true;
                        window.focus(&detail.focus, cx);
                        detail.pick_line(place, Pick::from(modifiers), cx)
                    }
                }))
                .ok();
            },
        );
        let hover = entity.clone();
        // As tall as the line and a little wider than its glyph, to land on easily.
        let grip = div()
            .id(("line-grip", (file << 40) | (hunk << 20) | line))
            .h_full()
            .px(px(2.))
            .flex()
            .items_center()
            .cursor(CursorStyle::OpenHand)
            .child(small_icon(Icon::DragVertical).text_color(Theme::get(cx).text_2))
            // Not a press on the number, which would pick the line, nor the start of
            // dragging over the numbers to pick more.
            .on_mouse_down(MouseButton::Left, {
                let pressed = entity.clone();
                move |_, _, cx| {
                    cx.stop_propagation();
                    (pressed.update(cx, |detail, _| detail.picking_lines = false)).ok();
                }
            })
            .on_hover(move |hovered, _, cx| {
                (hover.update(cx, |detail, cx| {
                    let ours = detail
                        .grip_over
                        .as_ref()
                        .is_some_and(|(at, _)| *at == place);
                    match (*hovered, ours) {
                        (true, false) => detail.grip_over = Some((place, detail.grip_takes(place))),
                        (false, true) => detail.grip_over = None,
                        _ => return,
                    }
                    cx.notify();
                }))
                .ok();
            })
            .when_some(drag, |el, (cut, label)| {
                el.on_drag(cut, self.drag_preview(label, picked, entity.clone()))
            })
            .into_any_element();
        Some(diff::Gutter {
            picked,
            pick,
            grip,
            outline,
        })
    }
}

/// A press on a row off its line numbers: see [`Detail::press_off_picks`].
fn press_off(entity: WeakEntity<Detail>) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) {
    move |_, _, cx| {
        (entity.update(cx, |detail, cx| detail.press_off_picks(cx))).ok();
    }
}

/// Right-clicking the line at row `ix`: see [`Detail::line_menu`].
fn line_menu(
    ix: usize,
    entity: WeakEntity<Detail>,
) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    move |event, window, cx| {
        (entity.update(cx, |detail, cx| {
            detail.line_menu(ix, event.position, window, cx)
        }))
        .ok();
    }
}

impl Detail {
    /// What right-clicking the line at `row` offers. Across selected text: copying it,
    /// finding it when it's on one line, and opening the file at the line. Otherwise what
    /// right-clicking its hunk's header does, or its file's for an unchanged line.
    fn line_menu(
        &mut self,
        row: usize,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(diff) = &self.diff else {
            return;
        };
        let (file, hunk) = match diff.rows.get(row) {
            Some(DiffRow::Line { file, hunk, .. }) => (*file, Some(*hunk)),
            Some(DiffRow::Context { file, .. }) => (*file, None),
            _ => return,
        };
        let line = self.line_at(row).and_then(|line| line.new.or(line.old));
        let selected = (self.text_range())
            .filter(|(start, end)| (start.0..=end.0).contains(&row))
            .and_then(|_| self.selected_text());
        let Some(text) = selected else {
            // A changed line's run, or the picked lines when it's one of them, as its grip
            // takes; an unchanged one's hunk.
            let changed = match diff.rows.get(row) {
                Some(DiffRow::Line { file, hunk, line }) => {
                    let lines = diff.files[*file].hunks().get(*hunk).map(|h| &h.lines);
                    (lines.and_then(|lines| lines.get(*line)))
                        .is_some_and(|line| line.kind != LineKind::Context)
                        .then_some((*file, *hunk, *line))
                }
                _ => None,
            };
            let key = hunk.and_then(|hunk| Some(diff.files[file].hunks().get(hunk)?.key));
            let menu = match changed {
                Some(place) => self.lines_cut(file, self.grip_takes(place), cx),
                None => self.diff_cut(file, key.map(|key| vec![key]), cx),
            };
            self.clear_text_selection(cx);
            if let Some(menu) = menu {
                self.changes_menu(menu, Some(line), position, cx);
            }
            return;
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.linked());
        let path = diff.files[file].file.path.clone();
        // Copy acts where the focus is: on the diff's selection.
        window.focus(&self.focus, cx);
        let mut items = vec![MenuItem::action("Copy", &crate::app::CopySelection, cx)];
        let query = text.trim();
        if !query.is_empty() && !query.contains('\n') {
            let label = format!("Find {}", crate::ui::quoted(query));
            let (query, entity) = (query.to_owned(), cx.entity().downgrade());
            items.push(MenuItem::new(label, move |_, window, cx| {
                (entity.update(cx, |detail, cx| detail.find_text(&query, window, cx))).ok();
            }));
        }
        let label = crate::open_in::editor_label(self.store.read(cx));
        items.push(
            MenuItem::new(label, move |this, _, cx| {
                let (file, worktree) = (Some(path.clone()), worktree.clone());
                this.open_in_editor(&project, worktree, file, line, position, cx)
            })
            .separated(),
        );
        (self.app)
            .update(cx, |app, cx| app.open_menu(position, items, cx))
            .ok();
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
                detail.press_off_picks(cx);
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

/// Discard for an uncommitted hunk, of the main checkout or of the `linked` worktree, shown
/// on the hunk under the cursor and on the one under the pointer.
fn discard_button(
    ix: usize,
    project: ProjectId,
    linked: Option<Arc<str>>,
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
                    let (changes, linked) = (vec![change.clone()], linked.clone());
                    app.update(cx, |app, cx| {
                        app.discard(project.clone(), linked, changes, window, cx)
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
