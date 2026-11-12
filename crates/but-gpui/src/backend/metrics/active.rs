//! What the user looked at, for `app_active` and `app_closed`: how often each view was opened
//! and how long each was on screen while the window was in front, up to [`IDLE`] after the last
//! key or click. The time is passed in.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use super::Props;

/// How long after the last key or click the window in front still counts as looked at.
const IDLE: Duration = Duration::from_secs(10 * 60);

pub(super) struct Active {
    /// When `app_active` last went out.
    pub(super) last: Option<Instant>,
    /// Each view since then: how many times it was opened, and how long it was looked at.
    views: BTreeMap<&'static str, (u64, Duration)>,
    shown: Option<&'static str>,
    /// While the window is in front: up to when time is counted, and the last key or click.
    front: Option<(Instant, Instant)>,
}

impl Active {
    pub(super) const fn new() -> Self {
        Active {
            last: None,
            views: BTreeMap::new(),
            shown: None,
            front: None,
        }
    }

    /// The view on screen, which what starts now is started from.
    pub(super) fn shown(&self) -> Option<&'static str> {
        self.shown
    }

    /// `view` came on screen at `now`, opened by the user or by the app, or with `None` nothing
    /// is on screen.
    pub(super) fn show(&mut self, view: Option<&'static str>, now: Instant) {
        self.count_time(now);
        self.shown = view;
    }

    /// The user opened `view`.
    pub(super) fn opened(&mut self, view: &'static str) {
        let (count, _) = self.views.entry(view).or_default();
        *count = count.saturating_add(1);
    }

    /// A key or click at `now`, in any of the app's windows. Back after more than [`IDLE`], the
    /// time away isn't counted. One while the window is behind, as in the graph's window, says
    /// nothing about this one.
    pub(super) fn pressed(&mut self, now: Instant) {
        match &mut self.front {
            Some((_, pressed)) if now.saturating_duration_since(*pressed) <= IDLE => *pressed = now,
            Some(_) => self.in_front(true, now),
            None => {}
        }
    }

    /// The window came in front at `now`, which counts as a press, or went behind.
    pub(super) fn in_front(&mut self, front: bool, now: Instant) {
        self.count_time(now);
        self.front = front.then_some((now, now));
    }

    /// What was looked at since this was last taken, counted up to `now`, as `<view>Views` and
    /// `<view>Seconds`.
    pub(super) fn take(&mut self, now: Instant) -> Props {
        self.count_time(now);
        let mut props = Props::new();
        for (view, (count, took)) in std::mem::take(&mut self.views) {
            if count > 0 {
                props.insert(format!("{view}Views"), count.into());
            }
            if took.as_secs() > 0 {
                props.insert(format!("{view}Seconds"), took.as_secs().into());
            }
        }
        props
    }

    /// Give the view on screen the time up to `now`, and count on from `now`.
    fn count_time(&mut self, now: Instant) {
        let Some((since, pressed)) = self.front else {
            return;
        };
        self.front = Some((now, pressed));
        let Some(view) = self.shown else {
            return;
        };
        let until = pressed.checked_add(IDLE).map_or(now, |idle| idle.min(now));
        let (_, time) = self.views.entry(view).or_default();
        *time = time.saturating_add(until.saturating_duration_since(since));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[test]
    fn views_are_timed_only_in_front_and_until_idle() {
        let start = Instant::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        let mut active = Active::new();
        active.in_front(true, at(0));
        active.show(Some("branch"), at(0));
        active.opened("branch");
        active.pressed(at(30));
        active.show(Some("commit"), at(30));
        active.opened("commit");
        active.in_front(false, at(50));
        // A key in the graph's window, with this one behind.
        active.pressed(at(60));
        // Two hours behind other apps, then in front with a click.
        active.in_front(true, at(7200));
        active.pressed(at(7210));
        // Left in front, untouched, while an agent's rebase has the app show the commit again.
        active.show(Some("commit"), at(9000));
        active.pressed(at(10_800));
        let looked = Value::Object(active.take(at(10_805)));
        assert_eq!(
            looked,
            json!({
                "branchViews": 1,
                "commitViews": 1,
                "branchSeconds": 30,
                "commitSeconds": 20 + 10 + 600 + 5,
            }),
            "time behind other apps and time past IDLE without a press isn't counted"
        );
        active.show(None, at(10_805));
        assert_eq!(
            Value::Object(active.take(at(10_900))),
            json!({}),
            "with nothing selected, no view gets the time"
        );
    }
}
