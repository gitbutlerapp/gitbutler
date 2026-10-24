//! A checkout's activity, on its overview: how many edits and commits there were and how long
//! it was busy, then the files saved each quarter of an hour over the last 12 rising from a
//! line and the commits made then hanging under it, and what a quarter holds on hover.
//! Quarters picked off it, as the project's page picks days, narrow the overview to them.

use std::collections::{BTreeSet, HashSet};

use gpui::{Context, Div, div, prelude::*, relative};

use crate::backend::CommitState;
use crate::detail::Detail;
use crate::detail::panes::{clear_button, grouped, keyed_tiles, overview_split, overview_title};
use crate::detail::slots::{SlotGesture, SlotPick, ends_drag, picks_slot};
use crate::state::{Activity, QUARTER};
use crate::theme::Theme;
use crate::ui::{Icon, Tooltip, dp, faint, h_flex, local_clock, plural, small_icon, v_flex};

use super::BARS as QUARTERS;
use super::Made;
use super::{span_said, window_start};

/// How tall the tallest bar of edits is drawn.
const BARS_HEIGHT: f32 = 44.;
/// How far under the line the most commits of a quarter hang.
const COMMITS_HEIGHT: f32 = 14.;

/// The quarters picked, said: their span, "14:15–15:00", or each run of them, up to "now"
/// for one running into `now`.
fn quarters_said(quarters: &BTreeSet<i64>, now: i64) -> Option<String> {
    let mut runs: Vec<(i64, i64)> = Vec::new();
    for &quarter in quarters {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == quarter => *end = quarter,
            _ => runs.push((quarter, quarter)),
        }
    }
    let said = runs.iter().map(|(first, last)| {
        let end = (*last + 1) * QUARTER;
        let end = match end > now {
            true => "now".to_owned(),
            false => local_clock(end),
        };
        format!("{}–{end}", local_clock(first * QUARTER))
    });
    (!runs.is_empty()).then(|| said.collect::<Vec<_>>().join(", "))
}

/// `commits` by how far they've got, as a bar, glyphed as the navigator's commits are: landed
/// on the target, pushed, and only here; it reads as how far along. Commits only here fill in
/// grey, darker than pushed ones, so the page adds no colour beside the edits' teal. With none, an
/// empty bar holds its place, so picking a quiet quarter moves nothing under it.
fn commits_split(commits: &[Made], theme: &Theme) -> gpui::AnyElement {
    let count = |state: CommitState| commits.iter().filter(|c| c.state() == state).count();
    let split = overview_split(
        [
            (
                Icon::CommitFill,
                theme.purple,
                theme.purple,
                "landed".to_owned(),
                count(CommitState::Integrated),
            ),
            (
                Icon::CommitFill,
                theme.text_2,
                theme.border_1,
                "pushed".to_owned(),
                count(CommitState::Pushed),
            ),
            (
                Icon::Commit,
                theme.blue,
                theme.text_2,
                "not pushed".to_owned(),
                count(CommitState::Local),
            ),
        ],
        theme,
    );
    split.unwrap_or_else(|| {
        v_flex()
            .px(dp(6.))
            .pt(dp(8.))
            .gap(dp(6.))
            .child(div().h(dp(6.)).rounded(dp(2.)).bg(theme.border_2))
            .child(
                h_flex()
                    .gap(dp(4.))
                    .child(small_icon(Icon::Commit).text_color(theme.text_3))
                    .child(faint("No commits", theme)),
            )
            .into_any_element()
    })
}

/// The hour mark under the quarter starting at `start`, every third hour.
fn hour_mark(start: i64) -> Option<String> {
    let clock = local_clock(start);
    let hour: u32 = clock.get(..2)?.parse().ok()?;
    (clock.ends_with(":00") && hour.is_multiple_of(3)).then_some(clock)
}

impl Detail {
    /// What happened over the last 12 hours up to `now`, or the quarters picked: its counts
    /// first, keys to the chart under them, then a column for each quarter of an hour, the
    /// files `activity` saw saved then rising from the line and the `commits` made then
    /// hanging under it, each to its own scale, as edits run to hundreds and commits to a few;
    /// each quarter's counts on hover. Pressing, ⌘- or shift-pressing or dragging across them
    /// picks quarters, which Clear by the title lets go of. `scoped` are the commits of the
    /// quarters shown.
    pub(super) fn now_activity(
        &self,
        (activity, now): (Option<&Activity>, i64),
        commits: &[Made],
        scoped: &[Made],
        status: Div,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let span = &self.now.span;
        let bars = activity.map_or_else(|| vec![0; QUARTERS], |a| a.bars(now, QUARTERS));
        let first = window_start(now) / QUARTER;
        let mut committed = vec![0usize; QUARTERS];
        for commit in commits {
            let at = commit.time().div_euclid(QUARTER) - first;
            if let Some(n) = usize::try_from(at)
                .ok()
                .and_then(|at| committed.get_mut(at))
            {
                *n += 1;
            }
        }
        let most = bars.iter().copied().max().unwrap_or(0).max(1);
        let most_commits = committed.iter().copied().max().unwrap_or(0).max(1);
        let picked = !span.slots.is_empty();
        let quarters = || (bars.iter().copied().zip(committed.iter().copied())).enumerate();
        let in_view = |ix: usize| span.keeps(first + ix as i64);
        let edits: u32 = quarters()
            .filter(|(ix, _)| in_view(*ix))
            .map(|(_, (n, _))| n)
            .sum();
        let busy = quarters()
            .filter(|(ix, (n, landed))| in_view(*ix) && (*n > 0 || *landed > 0))
            .count();
        // Once every commit's files are read, so the count doesn't climb as they come in.
        let changed: Option<HashSet<&String>> = (scoped.iter())
            .map(|commit| self.now.paths.get(&commit.id()))
            .collect::<Option<Vec<_>>>()
            .map(|paths| paths.into_iter().flatten().collect());
        let tiles = [
            (
                "Edits",
                grouped(edits as usize),
                "Files saved here, as the file watcher saw them while the app ran: a file saved \
                 twice counts twice",
                Some(theme.pop),
            ),
            (
                "Commits",
                grouped(scoped.len()),
                "Commits made here then",
                Some(theme.text_2),
            ),
            (
                "Files changed",
                changed.map_or("–".to_owned(), |files| grouped(files.len())),
                "Files those commits changed",
                None,
            ),
            (
                "Busy for",
                match busy {
                    0 => "–".to_owned(),
                    n => span_said(n as i64 * QUARTER),
                },
                "Quarters of an hour with an edit or a commit",
                None,
            ),
        ];
        let range = quarters_said(&span.slots, now).unwrap_or("last 12 hours".to_owned());
        let clear = picked.then(|| {
            clear_button("now-quarters-clear").on_click(cx.listener(|this, _, _, cx| {
                this.now.span = SlotPick::default();
                cx.notify()
            }))
        });
        let columns = quarters().map(|(ix, (n, landed))| {
            let quarter = first + ix as i64;
            let start = quarter * QUARTER;
            let inside = span.keeps(quarter);
            let mut tip = format!("{}–{}", local_clock(start), local_clock(start + QUARTER));
            match (n, landed) {
                (0, 0) => tip.push_str(" · quiet"),
                (0, _) => {}
                (n, _) => tip.push_str(&format!(" · {}", plural(n as usize, "edit"))),
            }
            if landed > 0 {
                tip.push_str(&format!(" · {}", plural(landed, "commit")));
            }
            let (edit, commit) = match inside {
                true => (theme.pop, theme.text_2),
                false => (theme.pop.opacity(0.2), theme.text_2.opacity(0.25)),
            };
            let column = v_flex()
                .id(("now-quarter", ix))
                .flex_1()
                .tooltip(Tooltip::text(tip));
            picks_slot(
                column,
                quarter,
                &span.slots,
                theme,
                Detail::quarter_gesture,
                cx,
            )
            .child(
                div()
                    .h(dp(BARS_HEIGHT))
                    .px(dp(1.))
                    .flex()
                    .items_end()
                    .child(
                        div()
                            .w_full()
                            // A quiet quarter a stub on the line, as the project's
                            // activity draws a quiet day.
                            .h(dp((n as f32 / most as f32 * BARS_HEIGHT).max(2.)))
                            .rounded_t(dp(1.))
                            .bg(match n {
                                0 => theme.border_2,
                                _ => edit,
                            }),
                    ),
            )
            // The commits under the line, apart from the edits over it.
            .child(
                div()
                    .h(dp(COMMITS_HEIGHT))
                    .pt(dp(2.))
                    .px(dp(1.))
                    .when(landed > 0, |el| {
                        let share = landed as f32 / most_commits as f32;
                        el.child(
                            div()
                                .w_full()
                                .h(dp((share * (COMMITS_HEIGHT - 2.)).max(4.)))
                                .rounded_b(dp(1.))
                                .bg(commit),
                        )
                    }),
            )
        });
        // Every third hour marked, centred where its quarter begins, clear of both ends and of
        // "now" at the right, which takes the last two hours' room in a narrow window.
        let marks = (2..QUARTERS - 8).filter_map(|ix| {
            let mark = hour_mark((first + ix as i64) * QUARTER)?;
            Some(
                h_flex()
                    .absolute()
                    .left(relative(ix as f32 / QUARTERS as f32))
                    .w_0()
                    .justify_center()
                    .child(faint(mark, theme).flex_none()),
            )
        });
        v_flex()
            .gap(dp(6.))
            // As tall without Clear as with it, a small button's height and the row's foot.
            .child(
                overview_title("Activity in this worktree", range, theme)
                    .min_h(dp(24.))
                    .children(clear),
            )
            .child(status)
            .child(keyed_tiles(tiles, theme))
            .child(commits_split(scoped, theme))
            .child(
                v_flex()
                    .px(dp(14.))
                    .pt(dp(10.))
                    .gap(dp(2.))
                    .child(ends_drag(
                        h_flex().children(columns),
                        Detail::quarter_gesture,
                        cx,
                    ))
                    .child(
                        div()
                            .relative()
                            .h(dp(14.))
                            .children(marks)
                            .child(faint("now", theme).absolute().right_0()),
                    ),
            )
    }

    fn quarter_gesture(&mut self, gesture: SlotGesture, cx: &mut Context<Self>) {
        if self.now.span.apply(gesture) {
            cx.notify();
        }
    }
}
