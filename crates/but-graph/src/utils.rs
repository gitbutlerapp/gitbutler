use std::collections::VecDeque;

use petgraph::{Direction, visit::NodeIndexable};

use crate::{Graph, Segment, SegmentIndex};

impl Graph {
    pub(crate) fn seen_table(&self) -> SeenTable {
        SeenTable::new(self.inner.node_bound())
    }
}

/// Fixed-size storage for tracking visited segments during graph walks.
///
/// Unlike [`SegmentTable<bool>`], this type only represents one concept:
/// whether a segment was seen in the current walk. It is intentionally static:
/// create it after the graph shape is known, and don't use it for segments
/// inserted after construction.
pub(crate) struct SeenTable {
    values: Vec<bool>,
}

impl SeenTable {
    /// Create a table with space for `num_segments` segment indices.
    fn new(num_segments: usize) -> Self {
        SeenTable {
            values: vec![false; num_segments],
        }
    }

    /// Insert `sidx` into the seen set if it wasn't present yet.
    ///
    /// Returns `true` if `sidx` was unseen before this call.
    pub(crate) fn insert_unseen(&mut self, sidx: SegmentIndex) -> bool {
        let value = &mut self.values[sidx.index()];
        if *value {
            return false;
        }
        *value = true;
        true
    }
}

/// Scratch storage keyed directly by [`SegmentIndex`].
///
/// Segment indices are dense `petgraph::NodeIndex` values while a graph is alive,
/// so a vector indexed by `SegmentIndex::index()` is a perfect lookup table and
/// avoids hash-map overhead in hot graph algorithms. This table is fixed-size:
/// create it after the graph shape is known and use it only while no newly
/// inserted segment can be addressed through it.
///
/// Instead of clearing the whole vector between phases, it remembers which slots
/// changed away from `empty` and resets only those slots. This keeps reuse cheap
/// for walks that touch a small part of a large graph.
pub(crate) struct SegmentTable<T> {
    values: Vec<T>,
    touched: Vec<SegmentIndex>,
    empty: T,
}

impl<T: Copy + PartialEq> SegmentTable<T> {
    /// Create a fixed-size table with one slot for every segment index currently
    /// representable by `node_bound`.
    pub(crate) fn new(node_bound: usize, empty: T) -> Self {
        SegmentTable {
            values: vec![empty; node_bound],
            touched: Vec::new(),
            empty,
        }
    }

    /// Reset all slots touched since the last clear back to the `empty` value.
    pub(crate) fn clear(&mut self) {
        for sidx in self.touched.drain(..) {
            self.values[sidx.index()] = self.empty;
        }
    }

    /// Return the value stored for `sidx`.
    pub(crate) fn get(&self, sidx: SegmentIndex) -> T {
        self.values[sidx.index()]
    }

    /// Return a mutable slot for `sidx`, marking it for later clearing if it is
    /// currently `empty`.
    pub(crate) fn get_mut(&mut self, sidx: SegmentIndex) -> &mut T {
        let index = sidx.index();
        if self.values[index] == self.empty {
            self.touched.push(sidx);
        }
        &mut self.values[index]
    }

    /// Set `sidx` to `value`.
    ///
    /// Use this when the caller already knows the slot is empty or doesn't need
    /// to know whether it changed.
    pub(crate) fn set(&mut self, sidx: SegmentIndex, value: T) {
        let index = sidx.index();
        if self.values[index] == self.empty {
            self.touched.push(sidx);
        }
        self.values[index] = value;
    }

    /// Set `sidx` to `value` only if it is still `empty`.
    ///
    /// Returns `true` if the slot changed. This is useful for visited sets.
    pub(crate) fn set_if_empty(&mut self, sidx: SegmentIndex, value: T) -> bool {
        let index = sidx.index();
        if self.values[index] != self.empty {
            return false;
        }
        self.touched.push(sidx);
        self.values[index] = value;
        true
    }
}

/// A [`SegmentTable`] that can accommodate segments inserted after construction.
///
/// Most algorithms should prefer [`SegmentTable`] because fixed-size direct
/// indexing makes invalid usage obvious. This wrapper is for longer-lived
/// scratch state in post-processing, where the graph may grow while the scratch
/// table is still reused. It preserves the same touched-slot clearing behavior,
/// but grows before accessing an out-of-range segment index.
pub(crate) struct GrowingSegmentTable<T> {
    inner: SegmentTable<T>,
}

impl<T: Copy + PartialEq> GrowingSegmentTable<T> {
    /// Create a growable table initially sized for the current graph `node_bound`.
    pub(crate) fn new(node_bound: usize, empty: T) -> Self {
        GrowingSegmentTable {
            inner: SegmentTable::new(node_bound, empty),
        }
    }

    /// Reset all touched slots back to the `empty` value.
    pub(crate) fn clear(&mut self) {
        self.inner.clear();
    }

    /// Set `sidx` to `value`, growing first if necessary.
    pub(crate) fn set(&mut self, sidx: SegmentIndex, value: T) {
        self.ensure_index(sidx);
        self.inner.set(sidx, value);
    }

    /// Set `sidx` to `value` only if it is still `empty`, growing first if
    /// necessary.
    pub(crate) fn set_if_empty(&mut self, sidx: SegmentIndex, value: T) -> bool {
        self.ensure_index(sidx);
        self.inner.set_if_empty(sidx, value)
    }

    /// Ensure `sidx` is addressable by the wrapped table.
    fn ensure_index(&mut self, sidx: SegmentIndex) {
        let index = sidx.index();
        if index >= self.inner.values.len() {
            self.inner.values.resize(index + 1, self.inner.empty);
        }
    }
}

/// Reusable scratch state for repeated segment graph walks during post-processing.
///
/// Workspace post-processing may run many short traversals while also inserting
/// new segments into the graph. Reusing the queue and visited table avoids
/// allocating a fresh visited set for every walk, and the growable table keeps
/// the scratch state valid if post-processing creates segment indices that did
/// not exist when the scratch state was created.
pub(crate) struct SegmentVisitScratch {
    seen: GrowingSegmentTable<bool>,
    next: VecDeque<SegmentIndex>,
}

impl SegmentVisitScratch {
    /// Create scratch storage initially sized for the current graph.
    pub(crate) fn new(graph: &Graph) -> Self {
        SegmentVisitScratch {
            seen: GrowingSegmentTable::new(graph.inner.node_bound(), false),
            next: VecDeque::new(),
        }
    }

    /// Visit `start` and all reachable segments in `graph` until `visit_and_prune` returns
    /// `true` for a segment.
    ///
    /// `visit_and_prune` receives each visited segment. Returning `true` stops
    /// traversal through that segment, pruning its parents or children depending
    /// on `direction`; returning `false` continues traversal through its
    /// neighbors.
    pub(crate) fn visit_including_start_until(
        &mut self,
        graph: &Graph,
        start: SegmentIndex,
        direction: Direction,
        mut visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        self.visit_until(graph, start, direction, true, &mut visit_and_prune);
    }

    /// Visit all reachable segments in `graph` below or above `start`, excluding `start`
    /// from the callback.
    ///
    /// `visit_and_prune` receives each visited segment except `start`.
    /// Returning `true` stops traversal through that segment, pruning its
    /// parents or children depending on `direction`; returning `false`
    /// continues traversal through its neighbors.
    ///
    /// `start` is still marked as seen so it cannot be requeued through a cycle
    /// or cross-edge.
    pub(crate) fn visit_excluding_start_until(
        &mut self,
        graph: &Graph,
        start: SegmentIndex,
        direction: Direction,
        mut visit_and_prune: impl FnMut(&Segment) -> bool,
    ) {
        self.visit_until(graph, start, direction, false, &mut visit_and_prune);
    }

    /// Shared breadth-first traversal implementation.
    fn visit_until(
        &mut self,
        graph: &Graph,
        start: SegmentIndex,
        direction: Direction,
        include_start: bool,
        visit_and_prune: &mut impl FnMut(&Segment) -> bool,
    ) {
        self.reset();
        self.mark_known_unseen(start);
        self.next.push_back(start);

        while let Some(next_sidx) = self.next.pop_front() {
            if (!include_start && start == next_sidx) || !visit_and_prune(&graph[next_sidx]) {
                for neighbor in graph.inner.neighbors_directed(next_sidx, direction) {
                    if self.mark(neighbor) {
                        self.next.push_back(neighbor);
                    }
                }
            }
        }
    }

    /// Clear only the state touched by the previous walk.
    fn reset(&mut self) {
        self.next.clear();
        self.seen.clear();
    }

    /// Mark `sidx` if it hasn't been seen in the current walk.
    fn mark(&mut self, sidx: SegmentIndex) -> bool {
        self.seen.set_if_empty(sidx, true)
    }

    /// Mark `sidx` when the caller already knows it hasn't been seen.
    fn mark_known_unseen(&mut self, sidx: SegmentIndex) {
        self.seen.set(sidx, true);
    }
}
