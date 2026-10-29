//! The frames the window drew in the last second, in green in its top right corner, for seeing
//! how fast it draws, and beside them in amber, as `60 · 30`, the pace of a frame of the last
//! few seconds that dipped well below that. Before them, a graph of how long each frame of
//! those seconds took, with the dips in amber, shows when they were. Settings' Developer page,
//! in a build of its own, turns it on.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use gpui::{Bounds, Context, Window, canvas, div, fill, point, prelude::*, size};

use crate::theme::Theme;
use crate::ui::{WINDOW_CONTROLS_WIDTH, dp, fixed, h_flex};

/// How long the slowest frame stays shown, so a dip of one frame lasts long enough to read.
const HOLD: Duration = Duration::from_secs(3);

/// A longer gap is the window not drawing, as while it's hidden or the counter is off, not a
/// slow frame.
const PAUSE: Duration = Duration::from_secs(1);

/// A frame whose pace is under this share of the frames a second is a dip. A share rather than
/// a fixed rate, so it holds at 60Hz and 120Hz, and a frame a little late, as the display's
/// timing jitters, isn't one.
const DIP: f32 = 0.8;

/// The graph's columns across `HOLD`, each the slowest frame drawn in its share of it, as at
/// 120Hz a frame is a fraction of a column wide.
const COLUMNS: usize = 75;

/// The frame time the graph's top stands for, 20 frames a second. A slower frame reaches it.
const TOP: Duration = Duration::from_millis(50);

/// When each frame of the last `HOLD` was drawn.
#[derive(Default)]
pub struct Fps {
    frames: VecDeque<Instant>,
}

impl Fps {
    /// Count a frame drawn at `now`: the frames of the last second, and the slowest frame of
    /// the last `HOLD` as the frames a second its pace would draw, when it's a dip.
    fn frame(&mut self, now: Instant) -> (usize, Option<usize>) {
        // The frames before a pause weren't drawn slowly; the window just wasn't drawing.
        if (self.frames.back()).is_some_and(|last| now.duration_since(*last) > PAUSE) {
            self.frames.clear();
        }
        while (self.frames.front()).is_some_and(|at| now.duration_since(*at) >= HOLD) {
            self.frames.pop_front();
        }
        self.frames.push_back(now);
        let fps = (self.frames.iter())
            .filter(|at| now.duration_since(**at) < Duration::from_secs(1))
            .count();
        let dip = (self.frames.iter().zip(self.frames.iter().skip(1)))
            .map(|(before, after)| after.duration_since(*before))
            .max()
            .filter(|gap| !gap.is_zero())
            .map(|gap| (1. / gap.as_secs_f32()).round() as usize)
            .filter(|low| (*low as f32) < fps as f32 * DIP);
        (fps, dip)
    }

    /// The slowest frame in each of `COLUMNS` shares of the `HOLD` before `now`, oldest first,
    /// or zero where none was drawn. A frame counts in every share it spans, so a long one is
    /// as wide as it took.
    fn columns(&self, now: Instant) -> [Duration; COLUMNS] {
        let share = (HOLD / COLUMNS as u32).as_nanos();
        let column = |at: &Instant| {
            let ago = (now.duration_since(*at).as_nanos() / share) as usize;
            COLUMNS - 1 - ago.min(COLUMNS - 1)
        };
        let mut columns = [Duration::ZERO; COLUMNS];
        for (before, after) in self.frames.iter().zip(self.frames.iter().skip(1)) {
            let took = after.duration_since(*before);
            let spanned = columns.get_mut(column(before)..=column(after));
            for slowest in spanned.unwrap_or_default() {
                *slowest = took.max(*slowest);
            }
        }
        columns
    }
}

impl Render for Fps {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let (fps, dip) = self.frame(now);
        let columns = self.columns(now);
        // Drawn again on the next frame, so every frame counts, which keeps the window drawing
        // as fast as it can while this shows.
        window.request_animation_frame();
        let theme = Theme::get(cx);
        // The middle dot, not the dot operator, which Geist Mono doesn't have.
        let dip = dip.map(|dip| div().text_color(theme.warn).child(format!("· {dip}")));
        let (safe, warn) = (theme.safe, theme.warn);
        let graph = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let width = bounds.size.width / COLUMNS as f32;
                for (ix, took) in columns.into_iter().enumerate() {
                    let height = bounds.size.height * took.div_duration_f32(TOP).min(1.);
                    let corner = point(bounds.left() + width * ix as f32, bounds.bottom() - height);
                    // The same dip as the amber count's, unrounded.
                    let color = match took.as_secs_f32() * fps as f32 * DIP > 1. {
                        true => warn,
                        false => safe,
                    };
                    window.paint_quad(fill(Bounds::new(corner, size(width, height)), color));
                }
            },
        )
        .w(dp(COLUMNS as f32))
        .h(dp(12.));
        h_flex()
            .absolute()
            .top(dp(4.))
            // Clear of the window's buttons where the app draws them.
            .map(|el| match crate::ui::draws_window_controls(window) {
                true => el.right(fixed(WINDOW_CONTROLS_WIDTH + 8.)),
                false => el.right(dp(8.)),
            })
            .gap(dp(6.))
            .font_family(theme.mono_font.clone())
            .text_size(dp(11.))
            .text_color(theme.safe)
            .child(graph)
            .child(fps.to_string())
            .children(dip)
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{COLUMNS, Fps};

    /// Draw `frames` frames `gap` apart after `now`, which moves to the last, and what it counted.
    fn draw(
        fps: &mut Fps,
        now: &mut Instant,
        gap: Duration,
        frames: u32,
    ) -> (usize, Option<usize>) {
        let mut counted = (0, None);
        for _ in 0..frames {
            *now += gap;
            counted = fps.frame(*now);
        }
        counted
    }

    #[test]
    fn a_hitch_is_a_dip_while_held() {
        let (mut fps, mut now, ms) = (Fps::default(), Instant::now(), Duration::from_millis);
        let steady = draw(&mut fps, &mut now, ms(10), 200);
        assert_eq!(
            steady,
            (100, None),
            "a frame every 10ms is 100 a second, with no dip"
        );
        let late = draw(&mut fps, &mut now, ms(12), 1);
        assert_eq!(
            late,
            (100, None),
            "a frame at 83 a second is within 80% of 100, so not a dip"
        );
        let hitch = draw(&mut fps, &mut now, ms(50), 1);
        assert_eq!(
            hitch,
            (96, Some(20)),
            "one 50ms frame barely moves the count, but dips to its pace"
        );
        let held = draw(&mut fps, &mut now, ms(10), 290);
        assert_eq!(held.1, Some(20), "the hitch is still shown 2.9s on");
        let after = draw(&mut fps, &mut now, ms(10), 10);
        assert_eq!(after, (100, None), "and gone once its frames are 3s old");
    }

    #[test]
    fn a_pause_starts_over() {
        let (mut fps, mut now, ms) = (Fps::default(), Instant::now(), Duration::from_millis);
        draw(&mut fps, &mut now, ms(10), 200);
        let back = draw(&mut fps, &mut now, Duration::from_secs(2), 1);
        assert_eq!(back, (1, None), "2s without drawing isn't a slow frame");
        let next = draw(&mut fps, &mut now, ms(10), 200);
        assert_eq!(next, (100, None), "the frames after it count as before");
    }

    #[test]
    fn a_hitch_in_the_graph_is_as_wide_as_it_took() {
        let (mut fps, mut now, ms) = (Fps::default(), Instant::now(), Duration::from_millis);
        draw(&mut fps, &mut now, ms(10), 300);
        draw(&mut fps, &mut now, ms(120), 1);
        draw(&mut fps, &mut now, ms(10), 50);
        let hitch = 59..=62;
        let expected: Vec<_> = (0..COLUMNS)
            .map(|ix| if hitch.contains(&ix) { ms(120) } else { ms(10) })
            .collect();
        assert_eq!(
            fps.columns(now).to_vec(),
            expected,
            "the 120ms frame ended 0.5s ago and spans the four 40ms columns from 0.62s ago, \
             and every other column is a steady 10ms frame, back to 3s ago"
        );
    }
}
