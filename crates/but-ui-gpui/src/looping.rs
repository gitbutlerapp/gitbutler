//! Motion that goes on for as long as something is under way, as a spinner's dots or a pulse,
//! drawn a step at a time on a timer. A repeating GPUI animation asks for a frame on every frame,
//! so the whole window would draw at the display's rate, 120 times a second on a ProMotion
//! screen, for as long as it showed: clippy rejects `Animation::repeat` for that.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use gpui::{canvas, prelude::*};

/// How long each step of a [`Looping`] motion shows. At 10 a second dots read as stepping on
/// purpose, and the window sleeps between steps.
pub const STEP: Duration = Duration::from_millis(100);

/// Where a motion that goes round every `steps` [`STEP`]s has got to now. Every one counts from
/// the same moment, so two on screen keep in step and their timers wake the window together.
#[derive(Debug, Clone, Copy)]
pub struct Looping {
    /// The step it's at, from 0 to `steps - 1`.
    pub step: usize,
    steps: usize,
    /// How long until the next step.
    due: Duration,
}

impl Looping {
    pub fn now(steps: usize) -> Looping {
        static START: LazyLock<Instant> = LazyLock::new(crate::clock::now);
        let since = crate::clock::now().saturating_duration_since(*START);
        let taken = since.as_nanos() / STEP.as_nanos();
        let into = since.as_nanos() % STEP.as_nanos();
        Looping {
            step: (taken.checked_rem(steps as u128).unwrap_or(0)) as usize,
            steps,
            due: STEP.saturating_sub(Duration::from_nanos(into as u64)),
        }
    }

    /// How far round it is, from 0 up to 1.
    pub fn progress(self) -> f32 {
        self.step as f32 / self.steps.max(1) as f32
    }

    /// What draws the next step once it's due, by drawing the view it's in again: put it in what
    /// the motion draws, where it takes no room. It asks for the next step as this one is drawn,
    /// so a motion that's gone, or one in a window that isn't drawn, asks for no more.
    pub fn next(self) -> impl IntoElement {
        canvas(
            |_, _, _| {},
            move |_, _, window, cx| {
                let view = window.current_view();
                (cx.spawn(async move |cx| {
                    cx.background_executor().timer(self.due).await;
                    cx.update(|cx| cx.notify(view));
                }))
                .detach();
            },
        )
        .absolute()
    }
}
