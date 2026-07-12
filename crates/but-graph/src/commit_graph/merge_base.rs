//! The merge-base algorithm: a port of Git's `paint_down_to_common`, kept isolated
//! because it is the trickiest algorithm in the graph.

use gix::hashtable::HashSet;

use super::CommitGraph;

impl CommitGraph {
    /// The nearest commit common to `target` and EVERY parent of `merge_commit`. BFS from
    /// `merge_commit` over all parents, so the nearest such commit wins.
    pub(crate) fn lowest_common_base(
        &self,
        merge_commit: gix::ObjectId,
        target: gix::ObjectId,
    ) -> Option<gix::ObjectId> {
        let mut common = self.ancestor_set(target);
        for parent in self.all_parent_ids(merge_commit) {
            let parent_ancestors = self.ancestor_set(parent);
            common.retain(|c| parent_ancestors.contains(c));
        }
        let mut seen = HashSet::default();
        let mut queue = std::collections::VecDeque::from([merge_commit]);
        while let Some(c) = queue.pop_front() {
            if common.contains(&c) {
                return Some(c);
            }
            if seen.insert(c) {
                queue.extend(self.all_parent_ids(c));
            }
        }
        None
    }

    /// The best common ancestor of `a` and `b` in this graph, or `None` if they share no
    /// history the walk captured. Criss-cross merges yield several candidate bases; like
    /// Git, redundant candidates (reachable from another candidate) are removed and the
    /// first surviving base is returned.
    pub fn merge_base(&self, a: gix::ObjectId, b: gix::ObjectId) -> Option<gix::ObjectId> {
        if a == b {
            return self.node(a).map(|_| a);
        }
        let (ia, ib) = (self.index_of(a)?, self.index_of(b)?);
        let mut flags = vec![0u8; self.nodes.len()];
        let bases = self.paint_down_to_common(ia, ib, &mut flags);
        if bases.is_empty() {
            return None;
        }
        let result = self.remove_redundant(&bases, &mut flags);
        result.first().map(|&idx| self.nodes[idx].id)
    }

    /// The root-distance ordering [`Self::paint_down_to_common`] wants: this graph's
    /// generations are tip-high, the paint wants root-high, so invert.
    fn rootward_generation(&self, idx: usize) -> usize {
        (u32::MAX - self.generations[idx]) as usize
    }

    /// Paint the ancestry of both nodes downward until every candidate common ancestor is
    /// found, returning `(node_idx, rootward_generation)` candidates. The algorithm and its
    /// flag discipline mirror Git's `paint_down_to_common`.
    fn paint_down_to_common(
        &self,
        first: usize,
        second: usize,
        flags: &mut [u8],
    ) -> Vec<(usize, usize)> {
        const SIDE1: u8 = 1 << 0;
        const SIDE2: u8 = 1 << 1;
        const RESULT: u8 = 1 << 2;
        const STALE: u8 = 1 << 3;

        // Priority queue ordered by rootward generation (higher = closer to root = lower
        // priority). Reverse makes the max-heap pop tip-closest nodes first. The bool is
        // whether the entry counted as non-stale when pushed.
        let mut queue: std::collections::BinaryHeap<(std::cmp::Reverse<usize>, usize, bool)> =
            std::collections::BinaryHeap::new();

        flags[first] |= SIDE1;
        queue.push((
            std::cmp::Reverse(self.rootward_generation(first)),
            first,
            true,
        ));
        flags[second] |= SIDE2;
        queue.push((
            std::cmp::Reverse(self.rootward_generation(second)),
            second,
            true,
        ));
        let mut non_stale = 2usize;

        let mut out = Vec::new();

        // Keep processing while there are potentially useful entries. Stale entries still
        // need to propagate their stale marker; once everything left is stale, no better
        // merge-base can be found. The counter over-approximates (an entry pushed non-stale
        // may go stale while queued), which only extends the loop past the last useful pop —
        // from there every queued node is stale, and a stale node can never become a
        // candidate, so the result cannot change. This avoids re-scanning the whole queue
        // before every pop.
        while non_stale > 0 {
            let Some((std::cmp::Reverse(generation), idx, counted)) = queue.pop() else {
                break;
            };
            if counted {
                non_stale -= 1;
            }
            let node_flags = flags[idx];
            let mut flags_without_result = node_flags & (SIDE1 | SIDE2 | STALE);

            // Reachable from both sides: a merge-base candidate.
            if flags_without_result == (SIDE1 | SIDE2) {
                if node_flags & RESULT == 0 {
                    flags[idx] |= RESULT;
                    out.push((idx, generation));
                }
                flags_without_result |= STALE;
            }

            for pidx in self.present_parent_indices(idx) {
                if (flags[pidx] & flags_without_result) != flags_without_result {
                    flags[pidx] |= flags_without_result;
                    let counted = flags[pidx] & STALE == 0;
                    if counted {
                        non_stale += 1;
                    }
                    queue.push((
                        std::cmp::Reverse(self.rootward_generation(pidx)),
                        pidx,
                        counted,
                    ));
                }
            }
        }

        out
    }

    /// Remove candidates that are in the history of another candidate, keeping only the
    /// topologically most recent ones. Mirrors the segment-level algorithm this replaced.
    fn remove_redundant(&self, candidates: &[(usize, usize)], flags: &mut [u8]) -> Vec<usize> {
        const RESULT: u8 = 1 << 2;
        const STALE: u8 = 1 << 3;
        if candidates.is_empty() {
            return Vec::new();
        }
        flags.fill(0);

        let sorted_candidates = {
            let mut v = candidates.to_vec();
            // Rootward generation ascending: closer to tips first.
            v.sort_by_key(|&(_, generation)| generation);
            v
        };
        let mut min_gen_pos = 0;
        let mut min_gen = sorted_candidates[min_gen_pos].1;

        let mut walk_start: Vec<(usize, usize)> = Vec::with_capacity(candidates.len());
        for &(idx, _) in candidates {
            flags[idx] |= RESULT;
            for pidx in self.present_parent_indices(idx) {
                if flags[pidx] & STALE == 0 {
                    flags[pidx] |= STALE;
                    walk_start.push((pidx, self.rootward_generation(pidx)));
                }
            }
        }
        walk_start.sort_by_key(|&(idx, _)| idx);
        for &(idx, _) in &walk_start {
            flags[idx] &= !STALE;
        }

        let mut count_still_independent = candidates.len();
        let mut stack: Vec<(usize, usize)> = Vec::new();
        while let Some((idx, generation)) = walk_start.pop() {
            if count_still_independent <= 1 {
                break;
            }
            stack.clear();
            flags[idx] |= STALE;
            stack.push((idx, generation));

            while let Some((current, current_gen)) = stack.last().copied() {
                if flags[current] & RESULT != 0 {
                    flags[current] &= !RESULT;
                    count_still_independent -= 1;
                    if count_still_independent <= 1 {
                        break;
                    }
                    if current == sorted_candidates[min_gen_pos].0 {
                        while min_gen_pos < candidates.len() - 1
                            && flags[sorted_candidates[min_gen_pos].0] & STALE != 0
                        {
                            min_gen_pos += 1;
                        }
                        min_gen = sorted_candidates[min_gen_pos].1;
                    }
                }
                if current_gen > min_gen {
                    stack.pop();
                    continue;
                }
                let previous_len = stack.len();
                for pidx in self.present_parent_indices(current) {
                    if flags[pidx] & STALE == 0 {
                        flags[pidx] |= STALE;
                        stack.push((pidx, self.rootward_generation(pidx)));
                    }
                }
                if previous_len == stack.len() {
                    stack.pop();
                }
            }
        }

        candidates
            .iter()
            .filter_map(|&(idx, _)| (flags[idx] & STALE == 0).then_some(idx))
            .collect()
    }

    /// [`Self::merge_base`] folded over any number of commits — the octopus variant.
    pub fn merge_base_octopus(
        &self,
        commits: impl IntoIterator<Item = gix::ObjectId>,
    ) -> Option<gix::ObjectId> {
        let mut commits = commits.into_iter();
        let mut base = commits.next()?;
        for commit in commits {
            base = self.merge_base(base, commit)?;
        }
        Some(base)
    }
}
