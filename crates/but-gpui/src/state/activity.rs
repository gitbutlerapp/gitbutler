//! What the file watcher saw written in each checkout lately, for a worktree's overview: how
//! busy it's been a quarter of an hour at a time over the last week, which a restart keeps.

use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

/// How long one bar of activity covers, in seconds.
pub const QUARTER: i64 = 15 * 60;
/// How far back activity is kept, in seconds: a week, at most 672 quarters a checkout.
pub(super) const KEPT_FOR: i64 = 7 * 24 * 3600;

/// One checkout's writes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Activity {
    /// How many files were written in each quarter of an hour, by when it began in seconds
    /// since the epoch, oldest first; quarters without any are left out.
    pub quarters: VecDeque<(i64, u32)>,
    /// When the last file was written, in seconds since the epoch.
    pub last: Option<i64>,
}

impl Activity {
    /// Count `n` files written at `now`, and forget what's older than is kept.
    pub fn record(&mut self, now: i64, n: u32) {
        let quarter = now - now.rem_euclid(QUARTER);
        if n == 0 {
            return;
        }
        match self.quarters.back_mut() {
            Some((at, count)) if *at == quarter => *count += n,
            _ => self.quarters.push_back((quarter, n)),
        }
        self.last = Some(now);
        while (self.quarters.front()).is_some_and(|(at, _)| *at <= now - KEPT_FOR) {
            self.quarters.pop_front();
        }
    }

    /// How many files were written in each of the `n` quarters up to the one `now` is in,
    /// oldest first.
    pub fn bars(&self, now: i64, n: usize) -> Vec<u32> {
        let last = now - now.rem_euclid(QUARTER);
        let first = last - QUARTER * (n as i64 - 1);
        let mut bars = vec![0; n];
        for (at, count) in self.quarters.iter().filter(|(at, _)| *at >= first) {
            if let Some(bar) = bars.get_mut(((at - first) / QUARTER) as usize) {
                *bar += count;
            }
        }
        bars
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, KEPT_FOR, QUARTER};

    #[test]
    fn writes_count_by_quarter_for_a_week() {
        let mut activity = Activity::default();
        let start = 100 * QUARTER;
        activity.record(start, 2);
        activity.record(start + 60, 1);
        activity.record(start + QUARTER, 1);
        assert_eq!(
            activity.bars(start + QUARTER, 3),
            [0, 3, 1],
            "a quarter's writes, oldest first, up to the quarter now is in"
        );
        let later = start + KEPT_FOR + 1;
        activity.record(later, 1);
        assert_eq!(
            activity.bars(later, 1 + KEPT_FOR as usize / QUARTER as usize)[..2],
            [0, 1],
            "the first quarter's forgotten after a week, the next one kept"
        );
        assert_eq!(activity.last, Some(later));
    }
}
