//! A checkout's activity, on its overview: the files written each quarter of an hour over the
//! last 12, a dot under each quarter something was committed in, and what a quarter holds on
//! hover. Quarters picked off it, as the project's page picks days, narrow the overview to
//! them.

use std::collections::BTreeSet;

use gpui::{Context, Div, MouseButton, MouseDownEvent, MouseMoveEvent, div, prelude::*, relative};

use crate::backend::Commit;
use crate::detail::Detail;
use crate::detail::panes::overview_heading;
use crate::detail::slots::{SlotGesture, SlotPick};
use crate::state::{Activity, QUARTER};
use crate::theme::Theme;
use crate::ui::{ButtonStyle, Tooltip, button, dp, faint, h_flex, local_clock, plural, v_flex};

use super::BARS as QUARTERS;

/// How tall the tallest bar is drawn.
const BARS_HEIGHT: f32 = 44.;

/// The quarters picked, said: their span, "14:15–15:00", or each run of them, up to "now"
/// for one running into `now`.
pub(super) fn quarters_said(quarters: &BTreeSet<i64>, now: i64) -> Option<String> {
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

/// The hour mark under the quarter starting at `start`, every third hour.
fn hour_mark(start: i64) -> Option<String> {
    let clock = local_clock(start);
    let hour: u32 = clock.get(..2)?.parse().ok()?;
    (clock.ends_with(":00") && hour.is_multiple_of(3)).then_some(clock)
}

impl Detail {
    /// The activity of the last 12 hours: a bar of the files `activity` saw written each
    /// quarter of an hour, a dot under those `commits` landed in, each quarter's counts on
    /// hover; pressing, ⌘- or shift-pressing or dragging across them picks quarters, which
    /// Clear lets go of.
    pub(super) fn now_activity(
        &self,
        activity: Option<&Activity>,
        commits: &[&Commit],
        now: i64,
        theme: &Theme,
        cx: &Context<Self>,
    ) -> Div {
        let span = &self.now.span;
        let bars = activity.map_or_else(|| vec![0; QUARTERS], |a| a.bars(now, QUARTERS));
        let first = (now - now.rem_euclid(QUARTER)) / QUARTER - (QUARTERS as i64 - 1);
        let mut committed = vec![0usize; QUARTERS];
        for commit in commits {
            let at = commit.time.div_euclid(QUARTER) - first;
            if let Some(n) = usize::try_from(at)
                .ok()
                .and_then(|at| committed.get_mut(at))
            {
                *n += 1;
            }
        }
        let most = bars.iter().copied().max().unwrap_or(0).max(1);
        let picked = !span.slots.is_empty();
        let in_view = |ix: &usize| span.keeps(first + *ix as i64);
        let written: u32 = (bars.iter().enumerate())
            .filter(|(ix, _)| in_view(ix))
            .map(|(_, n)| n)
            .sum();
        let landed: usize = (committed.iter().enumerate())
            .filter(|(ix, _)| in_view(ix))
            .map(|(_, n)| n)
            .sum();
        let counts = format!(
            "{} written · {}",
            plural(written as usize, "file"),
            plural(landed, "commit")
        );
        let detail = match quarters_said(&span.slots, now) {
            Some(said) => h_flex()
                .gap(dp(6.))
                .child(faint(format!("{said}: {counts}"), theme))
                .child(
                    button("now-quarters-clear", "Clear")
                        .small()
                        .style(ButtonStyle::Ghost)
                        .on_click(cx.listener(|this, _, _, cx| this.clear_quarters(cx))),
                ),
            // What the bars and the dots are, in their colours.
            None => h_flex()
                .gap(dp(6.))
                .child(
                    div()
                        .size(dp(7.))
                        .rounded(dp(1.5))
                        .bg(theme.pop.opacity(0.7)),
                )
                .child(faint(
                    format!("{} written", plural(written as usize, "file")),
                    theme,
                ))
                .child(div().ml(dp(4.)).size(dp(6.)).rounded_full().bg(theme.blue))
                .child(faint(
                    format!("{} · last 12 hours", plural(landed, "commit")),
                    theme,
                )),
        };
        let columns = (0..QUARTERS).map(|ix| {
            let quarter = first + ix as i64;
            let start = quarter * QUARTER;
            let (n, landed) = (bars[ix], committed[ix]);
            let inside = !picked || span.keeps(quarter);
            // A run of quarters picked reads as one band, rounded only at its ends.
            let joined = |next: i64| picked && inside && span.keeps(next);
            let mut tip = format!("{}–{}", local_clock(start), local_clock(start + QUARTER));
            match (n, landed) {
                (0, 0) => tip.push_str(" · quiet"),
                (0, _) => {}
                (n, _) => tip.push_str(&format!(" · {} written", plural(n as usize, "file"))),
            }
            if landed > 0 {
                tip.push_str(&format!(" · {}", plural(landed, "commit")));
            }
            v_flex()
                .id(("now-quarter", ix))
                .flex_1()
                .when(!joined(quarter - 1), |el| el.rounded_l(dp(3.)))
                .when(!joined(quarter + 1), |el| el.rounded_r(dp(3.)))
                .when(picked && inside, |el| el.bg(theme.selected_unfocused()))
                .hover(|el| el.bg(theme.hover()))
                .tooltip(Tooltip::text(tip))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        let (shift, toggle) = (event.modifiers.shift, event.modifiers.secondary());
                        this.quarter_gesture(
                            SlotGesture::Press {
                                slot: quarter,
                                shift,
                                toggle,
                            },
                            cx,
                        )
                    }),
                )
                .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                    if event.pressed_button == Some(MouseButton::Left) {
                        this.quarter_gesture(SlotGesture::Drag(quarter), cx)
                    }
                }))
                .child(
                    div()
                        .h(dp(BARS_HEIGHT))
                        .px(dp(1.))
                        .flex()
                        .items_end()
                        .child(
                            div()
                                .w_full()
                                .h(dp(match n {
                                    0 => 0.,
                                    n => (n as f32 / most as f32 * BARS_HEIGHT).max(2.),
                                }))
                                .rounded_t(dp(1.))
                                .bg(match inside {
                                    true => theme.pop.opacity(0.7),
                                    false => theme.pop.opacity(0.2),
                                }),
                        ),
                )
                .child(div().h(dp(1.)).bg(theme.border_3))
                .child(
                    div()
                        .h(dp(14.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(landed > 0, |el| {
                            el.child(div().size(dp(6.)).rounded_full().bg(match inside {
                                true => theme.blue,
                                false => theme.blue.opacity(0.3),
                            }))
                        }),
                )
        });
        // Every third hour marked, centred where its quarter begins, clear of both ends and of
        // "now" at the right.
        let marks = (2..QUARTERS - 6).filter_map(|ix| {
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
        let release = |this: &mut Detail,
                       _: &gpui::MouseUpEvent,
                       _: &mut gpui::Window,
                       cx: &mut Context<Detail>| {
            this.quarter_gesture(SlotGesture::Release, cx)
        };
        v_flex()
            .child(overview_heading("Activity", detail, theme).h(dp(28.)))
            .child(
                v_flex()
                    .px(dp(14.))
                    .pt(dp(4.))
                    .gap(dp(2.))
                    .child(
                        h_flex()
                            .on_mouse_up(MouseButton::Left, cx.listener(release))
                            .on_mouse_up_out(MouseButton::Left, cx.listener(release))
                            .children(columns),
                    )
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

    fn clear_quarters(&mut self, cx: &mut Context<Self>) {
        self.now.span = SlotPick::default();
        cx.notify();
    }
}
