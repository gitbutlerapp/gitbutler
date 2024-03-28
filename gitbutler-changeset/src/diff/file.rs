use crate::{diff::range_intersection::RangeIntersection, Conflict, LineSpan, RawHunk};
use std::{
    collections::HashMap,
    ops::{
        Bound::{self, Excluded, Included, Unbounded},
        Range, RangeBounds,
    },
};

use unbounded_interval_tree::interval_tree::IntervalTree;
/// Represents a singular file with multiple hunk changes.
#[derive(Debug, Clone)]
pub struct File<H: RawHunk> {
    raw_hunks: Vec<H>,
    hunk_removals: IntervalTree<TaggedBound>,
    hunk_additions: IntervalTree<TaggedBound>,
}

impl<H: RawHunk> Default for File<H> {
    fn default() -> Self {
        Self {
            raw_hunks: Vec::new(),
            hunk_removals: IntervalTree::default(),
            hunk_additions: IntervalTree::default(),
        }
    }
}

impl<H: RawHunk> File<H> {
    /// Returns a vector of conflicting hunks based on the removals of both
    /// the hunks in this file and the span of removals in the given hunk.
    ///
    /// The returned vector contains the line spans of the conflicts,
    /// as well as a reference to the hunk that caused the conflict,
    /// **sorted by min(removal, addition) start lines**.
    #[inline]
    #[must_use]
    pub fn hunk_conflicts<'a, 'b>(&'a self, hunk: &'b H) -> Vec<Conflict<'a, 'b, H>> {
        let mut conflict_map = HashMap::new();

        for (span, hunk_index) in self.hunk_removals.calculate_conflicts(hunk) {
            conflict_map.insert(hunk_index, (Some(span), None));
        }

        for (span, hunk_index) in self.hunk_additions.calculate_conflicts(hunk) {
            conflict_map
                .entry(hunk_index)
                .and_modify(|e| e.1 = Some(span))
                .or_insert_with(|| (None, Some(span)));
        }

        let mut conflicts = conflict_map
            .into_iter()
            .map(|(hunk_index, (removals, additions))| {
                let conflicting_hunk = &self.raw_hunks[hunk_index];
                Conflict {
                    removals: removals.map(|span| (span, conflicting_hunk, hunk)),
                    additions: additions.map(|span| (span, conflicting_hunk, hunk)),
                }
            })
            .collect::<Vec<_>>();

        conflicts.sort_by_key(|conflict| {
            let removal_start = conflict
                .removals
                .as_ref()
                .map(|(span, _, _)| span.start())
                .unwrap_or(usize::MAX);
            let addition_start = conflict
                .additions
                .as_ref()
                .map(|(span, _, _)| span.start())
                .unwrap_or(usize::MAX);

            removal_start.min(addition_start)
        });

        conflicts
    }

    /// Finds all conflicts between the hunks in this file.
    ///
    /// The returned vector contains the line spans of the conflicts,
    /// as well as a reference to the hunks that caused the conflict,
    /// **sorted by min(removal, addition) start lines**.
    #[inline]
    #[must_use]
    pub fn conflicts(&self) -> Vec<Conflict<H>> {
        let mut conflict_map = HashMap::new();

        for (our_hunk_index, our_hunk) in self.raw_hunks.iter().enumerate() {
            for (their_span, their_hunk_index) in self.hunk_removals.calculate_conflicts(our_hunk) {
                conflict_map.insert((our_hunk_index, their_hunk_index), (Some(their_span), None));
            }

            for (their_span, their_hunk_index) in self.hunk_additions.calculate_conflicts(our_hunk)
            {
                conflict_map
                    .entry((our_hunk_index, their_hunk_index))
                    .and_modify(|e| e.1 = Some(their_span))
                    .or_insert_with(|| (None, Some(their_span)));
            }
        }

        let mut conflicts = conflict_map
            .into_iter()
            .map(
                |((our_hunk_index, their_hunk_index), (removals, additions))| {
                    let our_hunk = &self.raw_hunks[our_hunk_index];
                    let their_hunk = &self.raw_hunks[their_hunk_index];
                    Conflict {
                        removals: removals.map(|span| (span, our_hunk, their_hunk)),
                        additions: additions.map(|span| (span, our_hunk, their_hunk)),
                    }
                },
            )
            .collect::<Vec<_>>();

        conflicts.sort_by_key(|conflict| {
            let removal_start = conflict
                .removals
                .as_ref()
                .map(|(span, _, _)| span.start())
                .unwrap_or(usize::MAX);
            let addition_start = conflict
                .additions
                .as_ref()
                .map(|(span, _, _)| span.start())
                .unwrap_or(usize::MAX);

            removal_start.min(addition_start)
        });

        conflicts
    }

    /// Adds a hunk to the file.
    pub fn add_hunk(&mut self, hunk: H) {
        // Calculate the removal and addition spans for the hunk.
        let (removal_span, addition_span) = hunk.spans();
        let removal_span: Range<usize> = removal_span.into();
        let addition_span: Range<usize> = addition_span.into();

        // Add the hunk to the pool and return its index.
        let hunk_index = self.raw_hunks.len();
        self.raw_hunks.push(hunk);

        // Map in the removals and additions for the hunk.
        self.hunk_removals
            .insert(removal_span.as_tagged(hunk_index));
        self.hunk_additions
            .insert(addition_span.as_tagged(hunk_index));
    }
}

#[derive(Debug, Clone, Copy, Eq)]
struct TaggedBound {
    pub tag: usize,
    pub value: usize,
}

impl PartialEq for TaggedBound {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl PartialOrd for TaggedBound {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TaggedBound {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value.cmp(&other.value)
    }
}

trait Tag: RangeBounds<usize> {
    fn as_tagged(&self, tag: usize) -> (Bound<TaggedBound>, Bound<TaggedBound>) {
        let start = match self.start_bound() {
            Included(start) => Included(TaggedBound { tag, value: *start }),
            Excluded(start) => Excluded(TaggedBound { tag, value: *start }),
            Unbounded => Unbounded,
        };

        let end = match self.end_bound() {
            Included(end) => Included(TaggedBound { tag, value: *end }),
            Excluded(end) => Excluded(TaggedBound { tag, value: *end }),
            Unbounded => Unbounded,
        };

        (start, end)
    }
}

impl<T> Tag for T where T: RangeBounds<usize> {}

trait Untag: RangeBounds<TaggedBound> {
    fn untag(&self) -> (Bound<usize>, Bound<usize>) {
        let start = match self.start_bound() {
            Included(start) => Included(start.value),
            Excluded(start) => Excluded(start.value),
            Unbounded => Unbounded,
        };

        let end = match self.end_bound() {
            Included(end) => Included(end.value),
            Excluded(end) => Excluded(end.value),
            Unbounded => Unbounded,
        };

        (start, end)
    }
}

impl<T> Untag for T where T: RangeBounds<TaggedBound> {}

trait BoundTags {
    /// # Safety
    /// Do not call this method when the bound might be `Unbounded`.
    fn tag(&self) -> usize;
}

impl BoundTags for Bound<&TaggedBound> {
    fn tag(&self) -> usize {
        match self {
            Included(bound) => bound.tag,
            Excluded(bound) => bound.tag,
            Unbounded => unreachable!(),
        }
    }
}

trait TaggedIntervalTree {
    fn calculate_conflicts<'a, 'b, H: RawHunk>(
        &'a self,
        hunk: &'b H,
    ) -> impl Iterator<Item = (LineSpan, usize)> + 'a
    where
        'b: 'a;
}

impl TaggedIntervalTree for IntervalTree<TaggedBound> {
    fn calculate_conflicts<'a, 'b, H: RawHunk>(
        &'a self,
        hunk: &'b H,
    ) -> impl Iterator<Item = (LineSpan, usize)> + 'a
    where
        'b: 'a,
    {
        // We tag the source range as usize::MAX since the tag
        // is never used in lookups.
        let source_range: Range<_> = hunk.spans().0.into();
        let tagged_source_range = source_range.as_tagged(usize::MAX);

        self
            // Get a list of all intervals that overlap with the source range
            // (the range of additions/removals in the hunk).
            .get_interval_overlaps(&tagged_source_range)
            .into_iter()
            // Look up the hunk index for the conflicting interval (which is a full
            // interval of the conflicting range, not an intersection).
            // Then, reduce each of those intervals to lines that actually conflict.
            // Return the intersecting range and hunk index.
            .map(move |conflict_span| {
                debug_assert!(conflict_span.start_bound().tag() == conflict_span.end_bound().tag());

                let hunk_index = conflict_span.start_bound().tag();
                let source_span: Range<_> = hunk.spans().0.into();
                let conflicting_range = conflict_span.untag().intersection(&source_span).unwrap();
                (conflicting_range.into(), hunk_index)
            })
    }
}
