//! The diff column: every changed file as a card, built row by row so only what's on
//! screen is laid out. Uncommitted hunks carry a Discard button while under the cursor or
//! the pointer, as in Zed's agent review, a linked worktree's too.

use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    Action, AnyElement, App, Bounds, ClickEvent, Context, CursorStyle, DispatchPhase, Div, Entity,
    MouseButton, MouseDownEvent, MouseMoveEvent, Pixels, Point, ScrollHandle, SharedString,
    Stateful, Subscription, WeakEntity, Window, anchored, canvas, deferred, div, list, point,
    prelude::*,
};

use super::blame::Blamed;
use super::file_list::At;
use super::find::Spot;
use super::lines::Pick;
use super::{
    Cursor, Detail, DiscardHunk, FILES_POPOVER_DELAY, FileLines, LoadedDiff, TextPoint,
    ToggleFiles, TogglePlainLines, ToggleSplit, ToggleWrap,
};
use crate::app::AppHandle;
use crate::backend::{
    self, Branch, ChangeKind, Checkout, DiffBody, DiffLine, DiffSource, Drawing, FileDiff, HunkKey,
    LineKind, PushState, Selected, Workspace,
};
use crate::overlays::MenuItem;
use crate::state::{Cut, Prefs, ProjectId, Selection};
use crate::theme::Theme;
use crate::ui::diff::{self, DiffRow, HeaderDecor, LineDecor, LineSelect};
use crate::ui::illustration::Illustration;
use crate::ui::scrollbar::{Scroll, scrollbar};
use crate::ui::{
    ButtonStyle, Icon, Tooltip, button, checkbox, commit_title, count_badge, dp, empty_state,
    fixed, h_flex, icon, icon_button, icon_toggle, segmented, shortcut, small_icon, toggle_group,
    v_flex,
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
            top: (0, Pixels::ZERO),
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
            detail.follow_drag(window, cx);
            let top = detail.list.logical_scroll_top();
            (
                (top.item_ix, top.offset_in_item),
                detail.focus.clone(),
                detail.render_diff_tools(window, &theme, cx),
                detail.render_diff(window, &theme, cx),
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
                    .min_h_0()
                    .relative()
                    .child(diff)
                    .children(find),
            )
    }
}

impl Detail {
    /// `file`'s lines, while it's shown whole and they were read for its patch as it is now.
    /// Until they're read again after an edit, the file shows only its hunks.
    pub(super) fn whole_lines(&self, file: &FileDiff) -> Option<&FileLines> {
        match self.whole.get(&file.file.path) {
            Some(Some(lines)) if lines.0 == file.fingerprint => Some(lines),
            _ => None,
        }
    }

    /// What hunk `hunk` of `file`'s box says: how many lines before it are left out, unless
    /// the file shown whole has them above, and the scopes it names.
    fn hunk_title<'a>(&self, file: &'a FileDiff, hunk: usize) -> (u32, &'a [String]) {
        let (hunks, whole) = (file.hunks(), self.whole_lines(file).is_some());
        let unmodified = if whole {
            0
        } else {
            diff::unmodified_before(hunks, hunk)
        };
        // As `diff::rows` leaves a scope's lines out among conflicts.
        let around = whole || file.conflicts().is_empty();
        let named = diff::named_scopes(hunks, hunk, unmodified, around);
        (unmodified, named)
    }

    /// Whether hunk `hunk` of `file` goes without a box, as when it runs on from the one
    /// above it in a scope shown whole: nothing before it is left out and the lines show
    /// every scope it's in. Not in the composer, where the box holds its checkbox.
    fn boxless(&self, file: &FileDiff, hunk: usize) -> bool {
        let named = (file.hunks().get(hunk)).is_some_and(|hunk| !hunk.scope.is_empty());
        named && !self.composes() && matches!(self.hunk_title(file, hunk), (0, []))
    }

    /// Whether the cursor is on hunk `hunk` of `file` with no box to show it, so its lines
    /// carry the cursor's tab.
    fn marks_lines(&self, file_diff: &FileDiff, file: usize, hunk: usize) -> bool {
        self.cursor
            == (Cursor {
                file,
                hunk: Some(hunk),
            })
            && self.boxless(file_diff, hunk)
    }

    /// The diff line at `row`, if that row is one.
    fn line_at(&self, row: usize) -> Option<&DiffLine> {
        self.line_of(*self.diff.as_ref()?.rows.get(row)?)
    }

    /// The diff line `row` shows, if it shows one.
    pub(super) fn line_of(&self, row: DiffRow) -> Option<&DiffLine> {
        let diff = self.diff.as_ref()?;
        match row {
            DiffRow::Line {
                file, hunk, line, ..
            } => diff.files.get(file)?.hunks().get(hunk)?.lines.get(line),
            // An unchanged line, from its new side: of the file shown whole, or else of the
            // scope a hunk shows around it.
            DiffRow::Context { file, new, .. } => {
                let file = diff.files.get(file)?;
                match self.whole_lines(file) {
                    Some((_, lines, _)) => lines.get((new as usize).checked_sub(1)?),
                    None => (file.hunks().iter())
                        .flat_map(|hunk| hunk.lead.iter().chain(&hunk.tail))
                        .find(|line| line.new == Some(new)),
                }
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
        // The highest line number shown, from the last hunk's header and the scope's lines
        // after it, not a scan of its lines on every row drawn.
        let last = hunks.last().map_or(0, |hunk| {
            let h = hunk.key.0;
            (h.old_start + h.old_lines)
                .max(h.new_start + h.new_lines)
                .saturating_sub(1)
                + hunk.tail.len() as u32
        });
        let (length, whole_columns) = match self.whole_lines(file) {
            Some((_, lines, columns)) => {
                let new = lines.len() as u32;
                (new.max((new + removed).saturating_sub(*added)), *columns)
            }
            None => (0, 0),
        };
        layout.digits = last.max(length).to_string().len();
        let columns = widest.map(|side| side.max(whole_columns));
        let [old, new] = self
            .sideways
            .get(&file.file.path)
            .cloned()
            .unwrap_or_default();
        let sideways = |scroll, columns| diff::Sideways { scroll, columns };
        layout.sideways = match prefs.split {
            true => [sideways(old, columns[0]), sideways(new, columns[1])],
            false => {
                let both = sideways(new, columns[0].max(columns[1]));
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
                text.get(self.selected_in_row(row).unwrap_or(0..0))
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
        // The keys then act on what was pressed: its hunk, or an unchanged line's file.
        let cursor = match (self.diff.as_ref()).and_then(|diff| diff.rows.get(row)) {
            Some(&DiffRow::Line { file, hunk, .. }) => Some(Cursor {
                file,
                hunk: Some(hunk),
            }),
            Some(&DiffRow::Context { file, .. }) => Some(Cursor { file, hunk: None }),
            _ => None,
        };
        if let Some(cursor) = cursor {
            self.place_cursor(cursor);
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

    /// Where a drag selects text without scrolling: the diff's height, and the width the
    /// lines of the selection end's file show their text in, with the scroll that moves that
    /// text sideways unless it wraps.
    fn drag_area(&self, cx: &App) -> Option<(Bounds<Pixels>, Option<ScrollHandle>)> {
        let (_, (row, _)) = self.text_selection?;
        let diff = self.diff.as_ref()?;
        let path = &diff.files.get(diff.rows.get(row)?.file()?)?.file.path;
        let mut area = self.list.viewport_bounds();
        let sideways = (self.sideways.get(path).map(|[_, new]| new.clone()))
            .filter(|_| !self.store.read(cx).prefs.wrap);
        if let Some(text) = sideways.as_ref().map(ScrollHandle::bounds) {
            (area.origin.x, area.size.width) = (text.left(), text.size.width);
        }
        Some((area, sideways))
    }

    /// While a drag selects text past the edge of the diff, scroll toward the pointer, the
    /// faster the further past it is, and again each frame, so holding still keeps it going.
    /// The line level with the pointer then takes the selection's end.
    pub(super) fn follow_drag(&mut self, window: &mut Window, cx: &App) {
        let pointer = window.mouse_position();
        let area = self.selecting.then(|| self.drag_area(cx)).flatten();
        self.follow =
            (area.filter(|(area, _)| !area.contains(&pointer))).and_then(|(area, sideways)| {
                let step = |at: Pixels, start: Pixels, end: Pixels| {
                    let past = if at < start {
                        at - start
                    } else {
                        (at - end).max(Pixels::ZERO)
                    };
                    (past / 4.).clamp(-fixed(40.), fixed(40.))
                };
                let to = |at: Pixels, max: Pixels, by: Pixels| (at + by).clamp(Pixels::ZERO, max);
                let y = -self.list.scroll_px_offset_for_scrollbar().y;
                let max = self.list.max_offset_for_scrollbar().y;
                let by = to(y, max, step(pointer.y, area.top(), area.bottom())) - y;
                self.list.scroll_by(by);
                let mut scrolled = by != Pixels::ZERO;
                if let Some(handle) = sideways {
                    let (offset, max) = (handle.offset(), handle.max_offset().x);
                    let x = to(-offset.x, max, step(pointer.x, area.left(), area.right()));
                    scrolled |= x != -offset.x;
                    handle.set_offset(point(-x, offset.y));
                }
                if scrolled {
                    window.request_animation_frame();
                }
                let inside = area.bottom_right() - point(fixed(1.), fixed(1.));
                let at = pointer.clamp(&area.origin, &inside);
                // The edge can fall on a header, or below the last line, which take no
                // selection, so the line drawn nearest it does.
                let rows = self.diff.as_ref()?.rows.iter().enumerate();
                let top = self.list.logical_scroll_top().item_ix;
                let off =
                    |bounds: Bounds<Pixels>| (bounds.top() - at.y).max(at.y - bounds.bottom());
                let line = (rows.skip(top))
                    .map_while(|(ix, row)| Some((row, self.list.bounds_for_item(ix)?)))
                    .take_while(|(_, bounds)| bounds.top() < area.bottom())
                    .filter(|(row, _)| {
                        matches!(row, DiffRow::Line { .. } | DiffRow::Context { .. })
                    })
                    .map(|(_, bounds)| bounds)
                    .min_by_key(|bounds| off(*bounds))?;
                let y = if off(line) <= Pixels::ZERO {
                    at.y
                } else {
                    line.center().y
                };
                Some(point(at.x, y))
            });
    }

    /// Drop the text selection, saying whether there was one.
    pub fn clear_text_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let had = self.text_range().is_some();
        self.text_selection = None;
        cx.notify();
        had
    }

    fn render_diff(&self, window: &Window, theme: &Theme, cx: &Context<Self>) -> AnyElement {
        let ground = || div().flex_1().min_w_0().h_full().bg(theme.bg_1);
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
            // Another machine's branch, until it's here.
            if let Some(Selection::Mesh(project, id, pick)) = &self.store.read(cx).selection
                && !self.loading.is_some()
            {
                let message = self.machine_placeholder(project, id, pick, theme, cx);
                return ground().child(message).into_any_element();
            }
            // Loading is said once the wait is long, as `shown_diff` decides, so a quick read
            // doesn't flash it. Not with `loading_state`: its fade redraws this pane every
            // frame, which would draw the new diff in the same frame as the detail.
            let message = match (self.loading.is_some(), self.nothing_committed(cx)) {
                (true, _) if self.slow_to_load() => {
                    empty_state(None, "Loading diff…", "", theme, cx)
                }
                (false, Some(line)) => empty_state(Some(Illustration::Cactus), "", line, theme, cx),
                _ => empty_state(None, "", "", theme, cx),
            };
            return ground().child(message).into_any_element();
        };
        if diff.files.is_empty() {
            // A clean worktree is good news, as but.dev greets it; a commit with nothing in
            // it is a list with nothing in it.
            let (drawing, title, detail): (_, _, SharedString) = match diff.key.source {
                DiffSource::Worktree | DiffSource::Linked(_) => (
                    Some(Illustration::Waving),
                    "No uncommitted changes",
                    "Edits show up here as soon as they're saved".into(),
                ),
                // The header above says what it did instead. History compares what's
                // committed, so a discard's files, which restoring brings back, aren't listed.
                DiffSource::Snapshot { .. } => (None, "", "It changed no committed files".into()),
                _ => (Some(Illustration::Cactus), "", self.changes_nothing(cx)),
            };
            return ground()
                .child(empty_state(drawing, title, detail, theme, cx))
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
        let sticky = self.sticky_header(diff, theme).map(|(ix, lift)| {
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .h(dp(diff::FILE_HEADER_HEIGHT))
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
                        .child(self.render_diff_row(ix, entity.clone(), window, cx)),
                )
        });
        let scrollbar = scrollbar("diff-scrollbar", Scroll::List(self.list.clone()), theme);
        // A drag that selects past the edge, out of the window too, draws the pane again,
        // which scrolls toward the pointer.
        let past_edge = (self.selecting.then(|| self.drag_area(cx)).flatten()).map(|(area, _)| {
            canvas(
                |_, _, _| {},
                move |_, _, window, _| {
                    let pane = window.current_view();
                    window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                        if phase == DispatchPhase::Bubble
                            && event.dragging()
                            && !area.contains(&event.position)
                        {
                            cx.notify(pane);
                        }
                    });
                },
            )
            .absolute()
        });
        ground()
            // With the bar above it, as far down as but.dev starts its first card.
            .pt(dp(7.))
            // A press anywhere ends the last drag; one on a line starts a new one after this.
            .capture_any_mouse_down(cx.listener(|this, _, _, _| this.selecting = false))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.let_go(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.let_go(cx)),
            )
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(
                        list(self.list.clone(), move |ix, window, cx| {
                            let Some(detail) = entity.upgrade() else {
                                return div().into_any_element();
                            };
                            (detail.read(cx)).render_diff_row(ix, detail.downgrade(), window, cx)
                        })
                        .size_full(),
                    )
                    .children(sticky)
                    .children(scrollbar),
            )
            .children(past_edge)
            .into_any_element()
    }

    /// For a selected branch or stack with no commits, what but.dev's branch page says.
    fn nothing_committed(&self, cx: &App) -> Option<SharedString> {
        let store = self.store.read(cx);
        let selection = store.selection.as_ref()?;
        nothing_committed_in(store.workspace(selection.project())?, selection)
    }

    /// What a diff of commits with no changes in it says, as but.dev's branch page does: a
    /// branch the target already has changes nothing against it.
    fn changes_nothing(&self, cx: &App) -> SharedString {
        let store = self.store.read(cx);
        let Some(selection) = store.selection.as_ref() else {
            return "No changes".into();
        };
        let workspace = store.workspace(selection.project());
        let branches: Vec<_> = match selection {
            // As when a merge's rename detection conflicts with no file to show for it.
            Selection::Commit(_, id)
                if (store.find_commit(selection.project(), *id))
                    .is_some_and(|(_, commit)| commit.conflicted) =>
            {
                return "GitButler marked this commit conflicted, but Git names no file to \
                        resolve. Operations history can restore to before what conflicted it"
                    .into();
            }
            Selection::Commit(..) => return "This commit changes no files".into(),
            // A stack's branches all, as it has no branch of its own.
            Selection::Stack(_, bottom) => (workspace.and_then(|ws| ws.lane(bottom.as_ref())))
                .map_or_else(Vec::new, |lane| lane.branches.iter().collect()),
            Selection::Branch(..) => store.branch_for(selection).into_iter().collect(),
            _ => return "No changes".into(),
        };
        let target =
            (workspace.and_then(|ws| ws.target.as_ref())).map_or("the target", |t| t.name.as_str());
        let what = match selection {
            Selection::Stack(..) => "stack",
            _ => "branch",
        };
        // A conflicted commit's tree leaves out what it changes until it's resolved.
        if (branches.iter()).any(|b| b.commits.iter().any(|commit| commit.conflicted)) {
            return format!(
                "This {what}'s conflicted commits show their changes once they're resolved"
            )
            .into();
        }
        match !branches.is_empty() && branches.iter().all(|b| b.push == PushState::Integrated) {
            true => format!(
                "{target} already has these commits, so the {what} changes nothing against it"
            ),
            false => format!("Nothing changed against {target}"),
        }
        .into()
    }

    /// The button came up: what a press began ends. A grip held without being dragged lets
    /// go of its outline too, which hovering it again brings back.
    pub(super) fn let_go(&mut self, cx: &mut Context<Self>) {
        self.selecting = false;
        if std::mem::take(&mut self.grip_held) {
            self.grip_over = None;
            cx.notify();
        }
    }

    /// The pointer came onto the files button, or left it. The popover of its files opens
    /// once it rests there a moment, so passing over the button on the way elsewhere doesn't;
    /// back on the button from the popover, it stays.
    fn hover_files_button(&mut self, hovered: bool, cx: &mut Context<Self>) {
        self.files_hover.0 = hovered.then(|| match self.files_popover_shown.get() {
            true => Instant::now()
                .checked_sub(FILES_POPOVER_DELAY)
                .unwrap_or_else(Instant::now),
            false => Instant::now(),
        });
        if hovered {
            self._files_popover_delay = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(FILES_POPOVER_DELAY).await;
                this.update(cx, |_, cx| cx.notify()).ok();
            });
        }
        cx.notify();
    }

    /// The bar over the diff, as but.dev's: the file list's toggle, which counts the files and
    /// their lines while the list isn't beside the diff and lists them on hover, then how
    /// long lines show and where the sides go. A page whose column is its own list toggles the
    /// files beside it, when it has any; else it has only the count and the list on hover.
    fn render_diff_tools(
        &self,
        window: &Window,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let diff = self.shown_diff()?;
        let prefs = &self.store.read(cx).prefs;
        let (added, removed) = diff::total_stats(diff.files.iter());
        // To the detail's own handlers, wherever focus is.
        let act = |action: &'static dyn Action| {
            let focus = self.focus.clone();
            move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
                focus.dispatch_action(action, window, cx)
            }
        };
        let list_page = self.list_page(cx);
        let toggles = !list_page || self.has_files_beside(cx);
        // The files aren't beside the diff: hidden, left out for room, or a list page has none.
        let hidden = match list_page {
            true => self.beside_offset(cx) == 0.,
            false => !self.files_shown(window, cx),
        };
        let files_key = shortcut(&ToggleFiles, cx);
        let glyph = match (toggles, hidden) {
            (false, _) => Icon::FileDiff,
            (true, true) => Icon::SidebarNarrow,
            (true, false) => Icon::FilesSidebar,
        };
        let counts = hidden.then_some((diff.files.len(), added, removed));
        let toggle = files_toggle("diff-files", glyph, counts, theme)
            .on_hover(
                cx.listener(|this, hovered: &bool, _, cx| this.hover_files_button(*hovered, cx)),
            )
            // Hidden, the list opens on hover, which says more than a tooltip would.
            .when(toggles && !hidden, |el| {
                el.tooltip(Tooltip::keyed("Hide the files", files_key))
            })
            .when(toggles, |el| el.on_click(act(&ToggleFiles)));
        // Under the button, with a strip of its own above it that keeps it open on the way
        // from the button to it.
        let rested = (self.files_hover.0).is_some_and(|at| at.elapsed() >= FILES_POPOVER_DELAY);
        let open = hidden && (rested || self.files_hover.1);
        self.files_popover_shown.set(open);
        let list = open.then(|| {
            div().absolute().top(dp(28.)).left_0().child(
                deferred(
                    anchored().snap_to_window_with_margin(fixed(8.)).child(
                        div()
                            .id("files-popover")
                            .pt(dp(4.))
                            .occlude()
                            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                                this.files_hover.1 = *hovered;
                                cx.notify();
                            }))
                            .child(self.render_files_popover(theme, cx)),
                    ),
                )
                .with_priority(1),
            )
        });
        let files = div().relative().child(toggle).children(list);
        let flag = |id: &'static str, glyph, tip, on, action: &'static dyn Action| {
            icon_toggle(id, glyph, tip, shortcut(action, cx), on, theme).on_click(act(action))
        };
        // A way to lay the diff out, which `action` switches to when it isn't already.
        let layout = |id: &'static str,
                      label: &'static str,
                      tip: &'static str,
                      on: bool,
                      action: &'static dyn Action| {
            let segment = (div().id(id).child(label))
                .tooltip(Tooltip::keyed(tip, shortcut(action, cx)))
                .when(!on, |el| el.on_click(act(action)));
            (segment, on)
        };
        let (wrap, split, plain) = (prefs.wrap, prefs.split, prefs.plain_lines);
        Some(
            h_flex()
                .flex_none()
                .h(dp(40.))
                .px(dp(12.))
                .gap(dp(8.))
                .bg(theme.bg_1)
                .child(files)
                .child(div().flex_1())
                .child(toggle_group(
                    theme,
                    [
                        flag(
                            "diff-wrap",
                            Icon::TextWrap,
                            "Wrap long lines",
                            wrap,
                            &ToggleWrap,
                        ),
                        flag(
                            "diff-backgrounds",
                            Icon::TextBlock,
                            "Toggle diff backgrounds",
                            !plain,
                            &TogglePlainLines,
                        ),
                    ],
                ))
                .child(segmented(
                    false,
                    theme,
                    [
                        layout("diff-split", "Split", "Side by side", split, &ToggleSplit),
                        layout(
                            "diff-unified",
                            "Unified",
                            "One above the other",
                            !split,
                            &ToggleSplit,
                        ),
                    ],
                ))
                .into_any_element(),
        )
    }

    /// The header to keep at the top of the diff while its file scrolls under it, by row, and
    /// how far the file's end has pushed it up. Nothing while a header is at the top itself.
    fn sticky_header(&self, diff: &LoadedDiff, theme: &Theme) -> Option<(usize, Pixels)> {
        let top = self.list.logical_scroll_top();
        let row = *diff.rows.get(top.item_ix)?;
        let file = row.file()?;
        if row == DiffRow::File(file) && top.offset_in_item <= Pixels::ZERO {
            return None;
        }
        let header =
            (diff.rows.get(..=top.item_ix)?.iter()).rposition(|row| *row == DiffRow::File(file))?;
        // The file's last row pushes it, so that row and the card's end below it show whole
        // rather than a sliver of them under the header.
        let same_file =
            (diff.rows.iter().skip(top.item_ix)).take_while(|row| row.file() == Some(file));
        let last = top.item_ix + same_file.count() - 1;
        let lift = self
            .list
            .bounds_for_item(last)
            .map_or(Pixels::ZERO, |last| {
                let room = last.top() - self.list.viewport_bounds().top();
                (room - theme.scaled(diff::FILE_HEADER_HEIGHT)).min(Pixels::ZERO)
            });
        Some((header, lift))
    }

    fn render_diff_row(
        &self,
        ix: usize,
        entity: WeakEntity<Detail>,
        window: &Window,
        cx: &App,
    ) -> AnyElement {
        let theme = Theme::get(cx);
        let Some(diff) = &self.diff else {
            return div().into_any_element();
        };
        let Some(&row) = diff.rows.get(ix) else {
            return div().into_any_element();
        };
        let file_diff = row.file().and_then(|file| diff.files.get(file));
        // A folded file's header ends its card too.
        let last = diff::ends_card(&diff.rows, ix);
        // The file list's and the message's keys don't act on the diff's cursor.
        let solid = self.focus.is_focused(window);
        let store = self.store.read(cx);
        // Every file has its path to copy, even where nothing can change it.
        let menu = |menu: Option<Cut>,
                    file: &FileDiff,
                    hunk: Option<HunkKey>,
                    entity: WeakEntity<Detail>| {
            let path = file.file.path.clone();
            let line = hunk.map(|key| key.0.new_start);
            move |position: Point<Pixels>, cx: &mut App| {
                let at = At::File(path.clone(), line);
                (entity.update(cx, |detail, cx| {
                    detail.changes_menu(menu.clone(), at, hunk.map(Blamed::Hunk), position, cx)
                }))
                .ok();
            }
        };
        // The margin between the cards and the sides of the pane.
        let padded = div().w_full().px(dp(12.));
        match (row, file_diff) {
            (DiffRow::File(file), Some(file_diff)) => {
                let whole = self.whole_button(ix, file, file_diff, entity.clone());
                let drawn = self.drawing_button(ix, file, file_diff, entity.clone());
                let toggle = entity.clone();
                // What right-clicking the header offers, behind a ⋯ as well.
                let cut = self.diff_cut(file, None, cx);
                let open = menu(cut, file_diff, None, entity.clone());
                let more = icon_button(("file-more", ix), Icon::Kebab)
                    .small()
                    .on_click({
                        let open = open.clone();
                        move |event, _, cx| open(event.position(), cx)
                    });
                // Of commits shown one after another, which one it's from.
                let from = (file_diff.commit).and_then(|id| {
                    let store = self.store.read(cx);
                    let (_, commit) = store.workspace(&diff.key.project)?.find_commit(id)?;
                    Some(
                        div()
                            .max_w(dp(220.))
                            .truncate()
                            .text_size(dp(11.))
                            .text_color(theme.text_3)
                            .child(commit_title(&commit.title)),
                    )
                });
                let trailing = h_flex()
                    .flex_none()
                    .gap(dp(4.))
                    .children(from)
                    // The toggle last but one: showing text adds the whole-file button, which
                    // mustn't move it from under the pointer.
                    .children(whole)
                    .children(drawn)
                    .child(more);
                let header = diff::file_header(
                    file,
                    file_diff,
                    theme,
                    HeaderDecor {
                        trailing: Some(trailing.into_any_element()),
                        // Not while one of its hunks is, so one mark says what the keys act on.
                        current: self.cursor == Cursor { file, hunk: None },
                        solid,
                        folded: last,
                        ..Default::default()
                    },
                    cx,
                );
                padded
                    .id(("file-header", ix))
                    .child(header)
                    .on_mouse_down(MouseButton::Left, press_off(entity, None))
                    .on_mouse_down(MouseButton::Right, move |event, _, cx| {
                        open(event.position, cx)
                    })
                    .on_click(move |_, _, cx| {
                        toggle
                            .update(cx, |detail, cx| detail.toggle_fold(file, cx))
                            .ok();
                    })
                    .into_any_element()
            }
            (DiffRow::Hunk { file, hunk }, Some(file_diff)) => {
                let hunks = file_diff.hunks();
                let Some(found) = hunks.get(hunk) else {
                    return padded.into_any_element();
                };
                // The lines run on from the hunk above, with nothing between to say.
                if self.boxless(file_diff, hunk) {
                    return padded.into_any_element();
                }
                let (key, path) = (found.key, &file_diff.file.path);
                let at = Cursor {
                    file,
                    hunk: Some(hunk),
                };
                let current = self.cursor == at;
                let composing = self.composes();
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
                // Of a hunk with lines picked, just those, as ⌥⌘Z takes.
                let discard = (diff.key.source.live() && !file_diff.file.conflicted).then(|| {
                    let picked: std::collections::BTreeSet<usize> = (0..found.lines.len())
                        .filter(|line| self.line_picked(path, key, *line))
                        .collect();
                    let (keys, tip) = match picked.is_empty() {
                        true => (vec![key], "Throw this hunk away"),
                        false => (
                            self.part_keys(found, &picked),
                            "Throw the picked lines away",
                        ),
                    };
                    let change = file_diff.part(Some(keys));
                    let checkout = (diff.key.project.clone(), diff.key.files_checkout());
                    let app = self.app.clone();
                    discard_button(ix, checkout, change, current && solid, tip, app, cx)
                });
                let (unmodified, named) = self.hunk_title(file_diff, hunk);
                let header = diff::hunk_header(
                    found,
                    unmodified,
                    named,
                    theme,
                    HeaderDecor {
                        leading,
                        trailing: discard,
                        excluded: composing && !self.checks.hunk_checked(path, key),
                        current,
                        solid,
                        ..Default::default()
                    },
                );
                let carried = self.diff_cut(file, Some(vec![key]), cx);
                let (whole, dragged) = (entity.clone(), entity.clone());
                padded
                    .id(("hunk", ix))
                    .group("hunk-row")
                    .child(header)
                    .on_mouse_down(MouseButton::Right, {
                        let open = menu(carried.clone(), file_diff, Some(key), entity);
                        move |event, _, cx| open(event.position, cx)
                    })
                    .when(unmodified > 0, |el| {
                        el.tooltip(Tooltip::text("Show the whole file")).on_click(
                            move |_, _, cx| {
                                whole
                                    .update(cx, |detail, cx| {
                                        detail.toggle_whole(file, Some(hunk), cx)
                                    })
                                    .ok();
                            },
                        )
                    })
                    .on_mouse_down(MouseButton::Left, press_off(dragged.clone(), Some(at)))
                    .when_some(carried, |el, carried| {
                        let lines: Vec<_> = (0..found.lines.len()).map(|l| (hunk, l)).collect();
                        let label = self.lines_label(file, &lines);
                        el.on_drag(carried, self.drag_preview(label, false, dragged))
                    })
                    .into_any_element()
            }
            (DiffRow::Line { file, hunk, line }, Some(file_diff)) => {
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
                    select: Some(line_select(ix, entity, self.follow)),
                    selected: self.selected_in_row(ix),
                    found: self.found_on(Spot::Line(place)),
                };
                let layout = self.layout(file_diff, &store.prefs);
                let element = diff::line_row(text, place, last, theme, &layout, decor);
                let marked = self.marks_lines(file_diff, file, hunk);
                padded
                    .relative()
                    .child(element)
                    .when(marked, |el| el.child(line_tab(solid, theme)))
                    .on_mouse_down(MouseButton::Right, menu)
                    .into_any_element()
            }
            (
                DiffRow::Pair {
                    file,
                    hunk,
                    left,
                    right,
                },
                Some(file_diff),
            ) => {
                let Some(lines) = file_diff.hunks().get(hunk).map(|h| &h.lines) else {
                    return padded.into_any_element();
                };
                let half = |line: Option<usize>| {
                    let Some(line) = line else {
                        return diff::LineDecor::default();
                    };
                    let (excluded, toggle) = self.line_check(diff, (file, hunk, line), &entity);
                    diff::LineDecor {
                        excluded,
                        toggle,
                        found: self.found_on(Spot::Line((file, hunk, line))),
                        gutter: self.line_gutter((file, hunk, line), &entity, cx),
                        ..Default::default()
                    }
                };
                let sides = [
                    (left.and_then(|l| lines.get(l))).map(|line| (line, line.old)),
                    (right.and_then(|r| lines.get(r))).map(|line| (line, line.new)),
                ];
                // Each line is on one row only, so the right one, or the left where there's
                // none, tells rows apart.
                let place = (file, hunk, right.or(left).unwrap_or_default());
                let halves = [half(left), half(right)];
                let layout = self.layout(file_diff, &store.prefs);
                let pair = diff::pair_row(sides, halves, place, last, theme, &layout);
                let menu = line_menu(ix, entity.clone());
                let cursor = Some(Cursor {
                    file,
                    hunk: Some(hunk),
                });
                let marked = self.marks_lines(file_diff, file, hunk);
                padded
                    .relative()
                    .child(pair)
                    .when(marked, |el| el.child(line_tab(solid, theme)))
                    .on_mouse_down(MouseButton::Right, menu)
                    .on_mouse_down(MouseButton::Left, press_off(entity, cursor))
                    .into_any_element()
            }
            (DiffRow::Context { file, old, new }, Some(file_diff)) => {
                let Some(line) = self.line_at(ix) else {
                    return padded.into_any_element();
                };
                let menu = line_menu(ix, entity.clone());
                let decor = LineDecor {
                    select: Some(line_select(ix, entity, self.follow)),
                    selected: self.selected_in_row(ix),
                    found: self.found_on(Spot::Unchanged(file, new)),
                    ..Default::default()
                };
                let layout = self.layout(file_diff, &store.prefs);
                let element =
                    diff::context_row(line, file, (old, new), last, theme, &layout, decor);
                padded
                    .child(element)
                    .on_mouse_down(MouseButton::Right, menu)
                    .into_any_element()
            }
            (DiffRow::Conflict { conflict, part, .. }, Some(file_diff)) => {
                let Some(found) = file_diff.conflicts().get(conflict) else {
                    return padded.into_any_element();
                };
                let layout = self.layout(file_diff, &store.prefs);
                let path = &file_diff.file.path;
                let element =
                    self.conflict_row(path, found, part, &layout, last, entity, window, cx);
                // Its own id, as every conflict's bar names its buttons alike, and GPUI takes
                // a click by the id: unnamed, a click on the second conflict's went nowhere.
                padded
                    .id(("conflict-row", ix))
                    .child(element)
                    .into_any_element()
            }
            (DiffRow::Note(_), Some(file_diff)) => padded
                .child(match self.drawing_of(file_diff) {
                    Some(drawn) => diff::drawing(drawn.before.clone(), drawn.after.clone(), theme),
                    None => diff::note(file_diff, theme),
                })
                .into_any_element(),
            (DiffRow::Gap, _) => padded.h(dp(10.)).into_any_element(),
            // A row whose file is gone draws nothing.
            (_, None) => padded.into_any_element(),
        }
    }

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
                        .update(cx, |detail, cx| detail.toggle_whole(file, None, cx))
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

    /// Show every line of `file`, or only its hunks again. Asked by the header of its `hunk`,
    /// that hunk stays where it is on screen, the lines above it coming in above.
    fn toggle_whole(&mut self, file: usize, hunk: Option<usize>, cx: &mut Context<Self>) {
        let Some(path) = self.diff_path(file) else {
            return;
        };
        self.whole_anchor = None;
        if self.whole.remove(&path).is_none() {
            let top = self.list.logical_scroll_top().item_ix;
            self.whole_anchor = (hunk.zip(self.diff.as_ref())).and_then(|(hunk, diff)| {
                let at = (diff.rows.iter()).position(|r| *r == DiffRow::Hunk { file, hunk })?;
                Some((path.clone(), hunk, at.checked_sub(top)?))
            });
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
        let worktree = diff.key.files_checkout();
        let path = change.path.clone();
        // One read at a time: the one under way reads again when it lands stale.
        if !self.whole_reading.insert(path.clone()) {
            return;
        }
        cx.spawn(async move |this, cx| {
            let lines =
                backend::run(move || backend::file_lines(&root, worktree.linked(), &change)).await;
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
                let file = match now {
                    Some((ix, now)) if now == fingerprint => ix,
                    // The file changed while it was read: read it again as it is now.
                    Some((ix, _)) => return this.load_whole(ix, cx),
                    None => return,
                };
                let anchor = this.whole_anchor.take_if(|(asked, ..)| *asked == path);
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
                            store.failed("Couldn't read the file", &err, cx)
                        });
                    }
                }
                this.rebuild_rows(cx);
                if let Some((_, hunk, down)) = anchor
                    && let Some(at) = (this.diff.as_ref()).and_then(|diff| {
                        (diff.rows.iter()).position(|r| *r == DiffRow::Hunk { file, hunk })
                    })
                {
                    let top = this.list.logical_scroll_top();
                    (this.list).scroll_to(gpui::ListOffset {
                        item_ix: at.saturating_sub(down),
                        ..top
                    });
                }
            })
            .ok();
        })
        .detach();
    }

    /// Whether a hunk's line is left out of the next commit, and while composing, for a
    /// changed line, what clicking its numbers does.
    fn line_check(
        &self,
        diff: &LoadedDiff,
        (file, hunk, line): (usize, usize, usize),
        entity: &WeakEntity<Detail>,
    ) -> (bool, Option<diff::OnToggle>) {
        let cursor = Cursor {
            file,
            hunk: Some(hunk),
        };
        let Some(file_diff) = diff.files.get(file) else {
            return (false, None);
        };
        let Some(hunk) = file_diff.hunks().get(hunk) else {
            return (false, None);
        };
        let Some(kind) = hunk.lines.get(line).map(|line| line.kind) else {
            return (false, None);
        };
        if !self.composes() {
            return (false, None);
        }
        let (path, key) = (file_diff.file.path.clone(), hunk.key);
        if kind == LineKind::Context {
            return (!self.checks.hunk_checked(&path, key), None);
        }
        let excluded = !self.checks.line_checked(&path, key, line);
        let entity = entity.clone();
        // With the diff's keys, as a press on the line's text: they then act on its hunk,
        // rather than typing into the message.
        let toggle = Rc::new(move |window: &mut Window, cx: &mut App| {
            (entity.update(cx, |detail, cx| {
                detail.place_cursor(cursor);
                window.focus(&detail.focus, cx);
                detail.toggle_line(&path, key, line, cx)
            }))
            .ok();
        });
        (excluded, Some(toggle))
    }

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
        let (key, lines) = diff_file.hunks().get(hunk).map(|h| (h.key, &h.lines))?;
        let changed = (lines.get(line)).is_some_and(|line| line.kind != LineKind::Context);
        // The composer's checks own the numbers while it's open.
        if !changed || self.composes() {
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
            .px(dp(2.))
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
                    (pressed.update(cx, |detail, _| {
                        detail.picking_lines = false;
                        detail.grip_held = true;
                    }))
                    .ok();
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
                        // GPUI calls a pressed element unhovered as soon as the pointer moves,
                        // before it's moved far enough to start a drag. Letting go of the grip
                        // then would take its drag away, so a held one stays.
                        (false, true) if !detail.grip_held => detail.grip_over = None,
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

/// A press on a row off its line numbers: see [`Detail::press_off_picks`]. With `cursor`, the
/// cursor goes there, so the keys act on what was pressed.
fn press_off(
    entity: WeakEntity<Detail>,
    cursor: Option<Cursor>,
) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) {
    move |_, _, cx| {
        (entity.update(cx, |detail, cx| {
            detail.press_off_picks(cx);
            if let Some(cursor) = cursor {
                detail.place_cursor(cursor);
                cx.notify();
            }
        }))
        .ok();
    }
}

/// The cursor's tab on a line of a hunk with no header to carry it, at the card's edge past
/// the row's padding, as a header's is. The hunk's lines' tabs meet in one.
fn line_tab(solid: bool, theme: &Theme) -> gpui::Div {
    (diff::cursor_tab(solid, theme))
        .left(dp(12.))
        .top_0()
        .bottom_0()
        .rounded_none()
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
        // Side by side, a row holds a line of each side: the one picked, else the new one.
        let paired = match diff.rows.get(row) {
            Some(&DiffRow::Pair {
                file,
                hunk,
                left,
                right,
            }) => {
                let Some(file_diff) = diff.files.get(file) else {
                    return;
                };
                let key = file_diff.hunks().get(hunk).map(|h| h.key);
                let path = &file_diff.file.path;
                let picked = (key.into_iter())
                    .flat_map(|key| [left, right].into_iter().flatten().map(move |l| (key, l)))
                    .find(|(key, l)| self.line_picked(path, *key, *l))
                    .map(|(_, l)| l);
                picked.or(right).or(left).map(|line| (file, hunk, line))
            }
            _ => None,
        };
        let (file, hunk) = match (diff.rows.get(row), paired) {
            (_, Some((file, hunk, _))) => (file, Some(hunk)),
            (Some(DiffRow::Line { file, hunk, .. }), _) => (*file, Some(*hunk)),
            (Some(DiffRow::Context { file, .. }), _) => (*file, None),
            _ => return,
        };
        let Some(file_diff) = diff.files.get(file) else {
            return;
        };
        let line_of = |(file, hunk, line): (usize, usize, usize)| {
            diff.files.get(file)?.hunks().get(hunk)?.lines.get(line)
        };
        let line =
            (paired.and_then(line_of).or(self.line_at(row))).and_then(|line| line.new.or(line.old));
        let selected = (self.text_range())
            .filter(|(start, end)| (start.0..=end.0).contains(&row))
            .and_then(|_| self.selected_text());
        let Some(text) = selected else {
            // A changed line's run, or the picked lines when it's one of them, as its grip
            // takes; an unchanged one's hunk.
            let place = match diff.rows.get(row) {
                Some(&DiffRow::Line { file, hunk, line }) => Some((file, hunk, line)),
                _ => paired,
            };
            let changed = place
                .filter(|place| line_of(*place).is_some_and(|line| line.kind != LineKind::Context));
            let key = hunk.and_then(|hunk| Some(file_diff.hunks().get(hunk)?.key));
            let menu = match changed {
                Some(place) => self.lines_cut(file, self.grip_takes(place), cx),
                None => self.diff_cut(file, key.map(|key| vec![key]), cx),
            };
            let added = (changed.and_then(line_of))
                .filter(|line| line.kind == LineKind::Added)
                .and_then(|line| line.new);
            let blamed = added.map(Blamed::Line).or(key.map(Blamed::Hunk));
            let at = At::File(file_diff.file.path.clone(), line);
            self.clear_text_selection(cx);
            self.changes_menu(menu, at, blamed, position, cx);
            return;
        };
        let (project, worktree) = (diff.key.project.clone(), diff.key.files_checkout());
        let path = file_diff.file.path.clone();
        // Copy acts where the focus is: on the diff's selection.
        window.focus(&self.focus, cx);
        let query = text.trim().to_owned();
        let items = move |this: &mut Self, cx: &mut Context<Self>| {
            let mut items = vec![MenuItem::action("Copy", &crate::app::CopySelection, cx)];
            if !query.is_empty() && !query.contains('\n') {
                let label = format!("Find {}", crate::ui::quoted(&query));
                let entity = cx.entity().downgrade();
                items.push(MenuItem::new(label, move |_, window, cx| {
                    (entity.update(cx, |detail, cx| detail.find_text(&query, window, cx))).ok();
                }));
            }
            let label = crate::open_in::editor_label(this.store.read(cx));
            items.push(
                MenuItem::keyed(
                    label,
                    &crate::open_in::OpenInEditor,
                    cx,
                    move |this, _, cx| {
                        let (file, worktree) = (Some(path.clone()), worktree.clone());
                        this.open_in_editor(&project, worktree, file, line, position, cx)
                    },
                )
                .separated(),
            );
            items
        };
        self.open_menu(position, items, cx);
    }
}

/// What the pointer does to the text of the line at row `ix`: select from where it's
/// pressed, and extend as it drags, to `follow` too while a drag scrolls past the edge.
fn line_select(ix: usize, entity: WeakEntity<Detail>, follow: Option<Point<Pixels>>) -> LineSelect {
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
        follow,
    }
}

/// Discard for an uncommitted hunk, of a project's main checkout or linked worktree, shown
/// on the hunk under the cursor and on the one under the pointer: of what `change` has,
/// which `tip` says.
fn discard_button(
    ix: usize,
    (project, checkout): (ProjectId, Checkout),
    change: Selected,
    current: bool,
    tip: &'static str,
    app: AppHandle,
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
                .tooltip_for(tip, &DiscardHunk)
                .on_click(move |_, window, cx| {
                    let (project, checkout) = (project.clone(), checkout.clone());
                    let changes = vec![change.clone()];
                    app.update_later(window, cx, move |app, window, cx| {
                        app.discard(project, checkout, changes, window, cx)
                    });
                }),
        )
        .into_any_element()
}

/// The word around byte `ix` of `text`, or an empty range when `ix` isn't in one.
fn word_at(text: &str, ix: usize) -> Range<usize> {
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let Some((before, after)) = text.split_at_checked(ix) else {
        return ix..ix;
    };
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| is_word(*c))
        .last()
        .map_or(ix, |(at, _)| at);
    let end = after
        .char_indices()
        .find(|(_, c)| !is_word(*c))
        .map_or(text.len(), |(at, _)| ix + at);
    start..end
}

/// The file list's toggle over a diff, or over a checkout's overview where a diff's would be:
/// `glyph`, then while the list's hidden its files' `counts`, how many and their lines.
pub(super) fn files_toggle(
    id: &'static str,
    glyph: Icon,
    counts: Option<(usize, u32, u32)>,
    theme: &Theme,
) -> Stateful<Div> {
    h_flex()
        .id(id)
        .flex_none()
        .h(dp(28.))
        .px(dp(6.))
        .gap(dp(6.))
        .rounded(dp(4.))
        .hover(|el| el.bg(theme.hover()))
        .child(icon(glyph).text_color(theme.text_2))
        .when_some(counts, |el, (n, added, removed)| {
            el.child(count_badge(n, theme))
                .child(diff::stats(added, removed, theme))
        })
}

fn nothing_committed_in(workspace: &Workspace, selection: &Selection) -> Option<SharedString> {
    let target = workspace
        .target
        .as_ref()
        .map_or("the target", |t| t.name.as_str());
    let (branches, what, below): (Vec<&Branch>, _, _) = match selection {
        Selection::Branch(_, ref_name) => {
            // A branch stacked on another starts where that one ends, and a lane's bottom
            // branch where what the lane rests on does.
            let below = (workspace.lane_of(ref_name.as_ref())).and_then(|(lane, at)| {
                workspace
                    .chain_from(lane, at + 1)
                    .find_map(|link| link.top())
            });
            (
                vec![workspace.find_branch(ref_name.as_ref())?],
                "this branch",
                below,
            )
        }
        Selection::Stack(_, bottom) => {
            let lane = workspace.lane(bottom.as_ref())?;
            let below = workspace.beneath(lane).find_map(|link| link.top());
            (lane.branches.iter().collect(), "these branches", below)
        }
        _ => return None,
    };
    let base = below.map_or_else(
        || target.to_owned(),
        |(branch, from)| match from {
            Some(id) => format!("{} at {}", branch.name, id.short()),
            None => branch.name.clone(),
        },
    );
    (branches.iter().all(|branch| branch.commits.is_empty()))
        .then(|| format!("No commits above {base} on {what}").into())
}

#[cfg(test)]
mod tests {
    use super::nothing_committed_in;
    use crate::backend::FullName;
    use crate::backend::model::build::{branch, full, workspace, worktree};
    use crate::state::{ProjectId, Selection};

    #[test]
    #[expect(
        clippy::disallowed_methods,
        reason = "a test, and snapbox looks for the file its snapshot is in"
    )]
    fn an_empty_lane_bottom_starts_on_what_its_lane_rests_on() {
        let ws = workspace(
            vec![
                vec![branch("top", &[1]), branch("middle", &[2, 3])],
                vec![branch("upper", &[]), branch("lower", &[])],
            ],
            vec![
                worktree("wt", Some(3), vec![branch("wt", &[])]),
                worktree("outside", None, vec![branch("outside", &[])]),
                worktree("on-top", Some(1), vec![branch("on-top", &[])]),
            ],
        );
        let said = |kind: &str,
                    of: fn(ProjectId, Option<FullName>) -> Selection,
                    names: &[&str]| {
            (names.iter())
                .map(|name| {
                    let selection = of("project".into(), Some(full(&format!("refs/heads/{name}"))));
                    let said = nothing_committed_in(&ws, &selection);
                    format!("{kind}({name}): {said:?}\n")
                })
                .collect::<String>()
        };
        let said = said(
            "Branch",
            Selection::Branch,
            &["upper", "wt", "outside", "on-top"],
        ) + &said("Stack", Selection::Stack, &["lower", "wt"]);
        snapbox::assert_data_eq!(
            said,
            snapbox::str![[r#"
Branch(upper): Some("No commits above lower on this branch")
Branch(wt): Some("No commits above middle at 0303030 on this branch")
Branch(outside): Some("No commits above the target on this branch")
Branch(on-top): Some("No commits above top on this branch")
Stack(lower): Some("No commits above the target on these branches")
Stack(wt): Some("No commits above middle at 0303030 on these branches")

"#]]
        );
    }
}
