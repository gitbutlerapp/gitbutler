//! What the file watcher saw written in each checkout lately, for a worktree's overview: how
//! busy it's been a quarter of an hour at a time over the last week, which a restart keeps,
//! and the files written in the last 12 hours, which it doesn't.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// How long one bar of activity covers, in seconds.
pub const QUARTER: i64 = 15 * 60;
/// How far back activity is kept, in seconds: a week, at most 672 quarters a checkout.
const KEPT_FOR: i64 = 7 * 24 * 3600;
/// How far back the files written are kept, in seconds, as the overview's activity shows.
const FILES_FOR: i64 = 12 * 3600;
/// How many of the files written are kept at most, for an agent that writes all day.
const FILES_KEPT: usize = 20_000;

/// One checkout's writes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Activity {
    /// How many files were written in each quarter of an hour, by when it began in seconds
    /// since the epoch, oldest first; quarters without any are left out.
    pub quarters: VecDeque<(i64, u32)>,
    /// When the last file was written, in seconds since the epoch.
    pub last: Option<i64>,
    /// The files written in the last 12 hours, by their path in the checkout, oldest first.
    #[serde(skip)]
    files: VecDeque<(i64, Arc<str>)>,
}

impl Activity {
    /// Count `paths`, written at `now`, and forget what's older than is kept.
    pub fn record(&mut self, now: i64, paths: impl IntoIterator<Item = Arc<str>>) {
        let quarter = now - now.rem_euclid(QUARTER);
        let before = self.files.len();
        self.files.extend(paths.into_iter().map(|path| (now, path)));
        let n = (self.files.len() - before) as u32;
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
        while (self.files.front()).is_some_and(|(at, _)| *at <= now - FILES_FOR)
            || self.files.len() > FILES_KEPT
        {
            self.files.pop_front();
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

    /// The files written when `keep` says, by when, most often first, and how often.
    pub fn hot_files(&self, keep: impl Fn(i64) -> bool) -> Vec<(Arc<str>, usize)> {
        let mut counts: HashMap<&Arc<str>, usize> = HashMap::new();
        for (_, path) in self.files.iter().filter(|(at, _)| keep(*at)) {
            *counts.entry(path).or_default() += 1;
        }
        let mut hot: Vec<_> = (counts.into_iter())
            .map(|(path, n)| (path.clone(), n))
            .collect();
        hot.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        hot
    }
}

#[cfg(test)]
mod tests {
    use super::{Activity, QUARTER};

    #[test]
    fn writes_count_by_quarter_and_files_by_when() {
        let mut activity = Activity::default();
        let start = 100 * QUARTER;
        activity.record(start, ["a".into(), "b".into()]);
        activity.record(start + 60, ["a".into()]);
        activity.record(start + QUARTER, ["a".into()]);
        assert_eq!(
            activity.bars(start + QUARTER, 3),
            [0, 3, 1],
            "a quarter's writes, oldest first, up to the quarter now is in"
        );
        assert_eq!(
            activity.hot_files(|_| true),
            [("a".into(), 3), ("b".into(), 1)],
            "most often first"
        );
        assert_eq!(
            activity.hot_files(|at| at >= start + 60),
            [("a".into(), 2)],
            "only those written when asked"
        );
        let later = start + 12 * 3600 + 1;
        activity.record(later, ["c".into()]);
        assert_eq!(
            activity.hot_files(|_| true),
            [("a".into(), 2), ("c".into(), 1)],
            "the last 12 hours' kept"
        );
        assert_eq!(activity.last, Some(later));
    }
}
