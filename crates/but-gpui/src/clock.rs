//! The time motion is drawn at: folds, slides and rows sliding to a new order read [`now`],
//! never `Instant::now()`. It's the real time, except in a scripted run whose `clock stop` stops
//! it, where it moves only as `tick` and `film` move it, so any moment of a motion can be drawn
//! and shot. Timers, timings and what waits for loads keep the real time, as does what GPUI
//! animates itself (`with_animation`, `with_spring`).

#[cfg(feature = "dev-script")]
use std::time::Duration;
use std::time::Instant;

#[cfg(feature = "dev-script")]
use parking_lot::Mutex;

/// Where a stopped clock stands, and how far behind the real time a running one is, since it
/// runs on from where it stopped.
#[cfg(feature = "dev-script")]
struct Clock {
    stopped: Option<Instant>,
    behind: Duration,
}

#[cfg(feature = "dev-script")]
static CLOCK: Mutex<Clock> = Mutex::new(Clock {
    stopped: None,
    behind: Duration::ZERO,
});

/// The time to draw motion at.
pub fn now() -> Instant {
    #[cfg(feature = "dev-script")]
    {
        let clock = CLOCK.lock();
        let now = Instant::now();
        (clock.stopped).unwrap_or_else(|| now.checked_sub(clock.behind).unwrap_or(now))
    }
    #[cfg(not(feature = "dev-script"))]
    {
        Instant::now()
    }
}

/// Stop the clock where it is.
#[cfg(feature = "dev-script")]
pub fn stop() {
    let at = now();
    CLOCK.lock().stopped.get_or_insert(at);
}

/// Run the clock again from where it stopped.
#[cfg(feature = "dev-script")]
pub fn run() {
    let mut clock = CLOCK.lock();
    if let Some(at) = clock.stopped.take() {
        clock.behind = Instant::now().saturating_duration_since(at);
    }
}

/// Whether the clock is stopped.
#[cfg(feature = "dev-script")]
pub fn stopped() -> bool {
    CLOCK.lock().stopped.is_some()
}

/// Move a stopped clock on `by`. False while it runs.
#[cfg(feature = "dev-script")]
pub fn tick(by: Duration) -> bool {
    match &mut CLOCK.lock().stopped {
        Some(at) => {
            *at += by;
            true
        }
        None => false,
    }
}
