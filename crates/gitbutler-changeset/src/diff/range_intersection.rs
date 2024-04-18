//! Utility trait for calculating the intersection of two ranges.

use std::ops::{
    Bound::{Excluded, Included, Unbounded},
    Range, RangeBounds,
};

/// An ancillary trait for calculating the intersection of two ranges.
///
/// Blanket implementation for all types that implement `RangeBounds<T>`.
pub trait RangeIntersection<T>: RangeBounds<T>
where
    T: Ord + Copy,
{
    /// Calculate the intersection of two ranges.
    ///
    /// For example, given two ranges `a` and `b`, the intersection
    /// of `a` and `b` is the range `c` such that `c.start` is the
    /// maximum of `a.start` and `b.start`, and `c.end` is the minimum
    /// of `a.end` and `b.end`.
    ///
    /// Returns `None` if the ranges do not intersect.
    //
    // NOTE(qix-): These match statements kept intentionally 'verbose'
    // NOTE(qix-): as the alternative makes it almost entirely unreadable.
    #[allow(clippy::unnested_or_patterns, clippy::match_same_arms)]
    fn intersection<U: RangeBounds<T>>(&self, other: &U) -> Option<Range<T>> {
        let start = match (self.start_bound(), other.start_bound()) {
            (Included(a), Included(b)) => a.max(b),
            (Included(a), Excluded(b)) => a.max(b),
            (Excluded(a), Included(b)) => a.max(b),
            (Excluded(a), Excluded(b)) => a.max(b),
            (Included(a), Unbounded) => a,
            (Excluded(a), Unbounded) => a,
            (Unbounded, Included(b)) => b,
            (Unbounded, Excluded(b)) => b,
            (Unbounded, Unbounded) => return None,
        };

        let end = match (self.end_bound(), other.end_bound()) {
            (Included(a), Included(b)) => a.min(b),
            (Included(a), Excluded(b)) => a.min(b),
            (Excluded(a), Included(b)) => a.min(b),
            (Excluded(a), Excluded(b)) => a.min(b),
            (Included(a), Unbounded) => a,
            (Excluded(a), Unbounded) => a,
            (Unbounded, Included(b)) => b,
            (Unbounded, Excluded(b)) => b,
            (Unbounded, Unbounded) => return None,
        };

        if start < end {
            Some(*start..*end)
        } else {
            None
        }
    }
}

impl<T, U> RangeIntersection<T> for U
where
    T: Ord + Copy,
    U: RangeBounds<T>,
{
}
