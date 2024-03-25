use std::ops::{
    Bound::{Excluded, Included, Unbounded},
    Range, RangeBounds,
};

pub trait RangeIntersection<T>: RangeBounds<T>
where
    T: Ord + Copy,
{
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
