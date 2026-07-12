//! The traversal's pacing machinery: [`Limit`] meters how far a tip may walk,
//! [`Goals`] hands out flag bits for commits that must still be found, [`Queue`]
//! orders the expansion, and [`Instruction`] tells a queued commit how to proceed.

use std::collections::VecDeque;

use crate::CommitFlags;

#[derive(Debug, Copy, Clone)]
pub struct Limit {
    inner: Option<usize>,
    /// What this tip still seeks; while it seeks, the commit-count limit is ignored.
    goal: Goal,
}

/// A tip's convergence state, spelled out — no marker bits, no punning.
#[derive(Debug, Copy, Clone)]
enum Goal {
    /// This tip never had a counterpart to find.
    None,
    /// The goal bits of counterparts still unfound (one bit per goal, handed out by
    /// [`Goals`]). Never empty, and never contains regular commit flags.
    Seeking(CommitFlags),
    /// Every counterpart was found; the budget rules again.
    Reached,
}

impl Goal {
    /// Seeking `bits`, with regular commit flags masked away; empty means no goal.
    fn seeking(bits: CommitFlags) -> Self {
        let bits = goal_bits(bits);
        if bits.is_empty() {
            Goal::None
        } else {
            Goal::Seeking(bits)
        }
    }
}

/// The subset of `flags` that are goal bits — the bits above the named commit flags.
pub(crate) fn goal_bits(flags: CommitFlags) -> CommitFlags {
    CommitFlags::from_bits_retain(flags.bits() & !CommitFlags::all().bits())
}

/// Lifecycle and builders
impl Limit {
    pub fn new(value: Option<usize>) -> Self {
        Limit {
            inner: value,
            goal: Goal::None,
        }
    }

    /// Walk without a limit until this tip reaches a commit carrying `goal`'s flag — i.e. a
    /// commit that has the goal commit above it. The goal counts as found then and normal
    /// limits apply.
    /// `goals` are used to keep track of existing bitflags.
    ///
    /// ### Note
    ///
    /// No goal will be set if we can't track more goals, effectively causing traversal to stop earlier,
    /// leaving potential isles in the graph.
    /// This can happen if we have to track a lot of remotes, but since these are queued later, they are also
    /// secondary and may just work for the typical remote.
    pub(crate) fn with_indirect_goal(mut self, goal: gix::ObjectId, goals: &mut Goals) -> Self {
        self.goal = Goal::seeking(goals.flag_for(goal).unwrap_or_default());
        self
    }

    /// Add goals, as previously obtained by [`Goals::flag_for()`]. A [`Reached`](Goal::Reached)
    /// tip is NOT re-armed: recruitment (re-encounter goal inheritance) may hand a finished
    /// tip new goals, and honoring them re-grants unlimited gas broadly — measured +22k
    /// commits on the merge-heavy stress repo. A finished hunt stays finished.
    pub(crate) fn additional_goal(mut self, goal: CommitFlags) -> Self {
        self.goal = match self.goal {
            Goal::Seeking(bits) => Goal::seeking(bits | goal),
            Goal::None => Goal::seeking(goal),
            Goal::Reached => Goal::Reached,
        };
        self
    }

    /// Split the remaining limit evenly across the parents, but give each at least 1 so every
    /// parent line shows one commit — enough to see where it stops. Split budgets are never
    /// merged back when lines re-unite, so a split loses gas overall — and that is the
    /// point, A/B-tested: without the split every merge parent walks the full remainder,
    /// budget × branching (measured +22k commits on the merge-heavy stress repo, invisible
    /// to the small fixtures).
    pub(crate) fn per_parent(&self, num_parents: usize) -> Self {
        Limit {
            inner: self
                .inner
                .map(|l| if l == 0 { 0 } else { (l / num_parents).max(1) }),
            goal: self.goal,
        }
    }

    /// Assure this limit won't perform any traversal after reaching its goals.
    pub(crate) fn without_allowance(mut self) -> Self {
        self.set_but_keep_goal(Limit::new(Some(0)));
        self
    }
}

/// Limit-check
impl Limit {
    /// Return `true` if this limit is depleted, or decrement it by one otherwise.
    ///
    /// `flags` are used to selectively decrement this limit.
    /// Thanks to flag-propagation there can be no runaways.
    pub(crate) fn is_exhausted_or_decrement(&mut self, flags: CommitFlags, next: &Queue) -> bool {
        // Keep going if the goal wasn't seen yet, unlimited gas.
        if let Some(maybe_goal) = self.goal_reachable(flags)
            && (maybe_goal.is_empty() || self.set_single_goal_reached_keep_searching(maybe_goal))
        {
            return false;
        }
        // THE FREE-RIDE, and it is load-bearing (A/B-tested): while any queued item still
        // seeks a goal, goalless tips walk free — a seeker can only find its counterpart
        // where that counterpart's lane has walked and planted flags, so starving the
        // goalless side starves the seeker too. Removing this fails the long/short
        // partition-connection fixtures. Refinement left open: ride only for tips whose
        // flags a seeker actually hunts, so unrelated seekers don't extend everyone.
        if self.goal_unset() && next.iter().any(|item| !item.limit.goal_reached()) {
            return false;
        }
        if self.inner.is_some_and(|l| l == 0) {
            return true;
        }
        self.inner = self.inner.map(|l| l - 1);
        false
    }
}

/// Other access and mutation
impl Limit {
    /// Mark `goal` as found. Returns `true` while other goals are still outstanding (keep
    /// searching); once the last goal is found, the limit is [`Reached`](Goal::Reached)
    /// and this returns `false`.
    pub(crate) fn set_single_goal_reached_keep_searching(&mut self, goal: CommitFlags) -> bool {
        let Goal::Seeking(mut bits) = self.goal else {
            return false;
        };
        bits.remove(goal);
        if bits.is_empty() {
            self.goal = Goal::Reached;
            false
        } else {
            self.goal = Goal::Seeking(bits);
            true
        }
    }

    /// Retire the intersection of this limit's outstanding goals with `reached` — goals a
    /// SIBLING item of the same seeker already found. A goal's purpose is the SEED's
    /// connection, not the item's: once any of the seed's items connected, the others walk
    /// on their budget again (a merge side-lineage that never meets the counterpart would
    /// otherwise walk to the root on unlimited gas).
    pub(crate) fn retire_goals(&mut self, reached: CommitFlags) {
        let hit = self.goal_flags().intersection(reached);
        if !hit.is_empty() {
            self.set_single_goal_reached_keep_searching(hit);
        }
    }

    /// If `other`'s limit is higher than ours, adopt it. Nothing else changes.
    pub(crate) fn adjust_limit_if_bigger(&mut self, other: Limit) {
        match (&mut self.inner, other.inner) {
            (inner @ Some(_), None) => *inner = None,
            (Some(x), Some(y)) => {
                if *x < y {
                    *x = y;
                }
            }
            (None, None) | (None, Some(_)) => {}
        }
    }

    pub(crate) fn goal_reached(&self) -> bool {
        matches!(self.goal, Goal::None | Goal::Reached)
    }

    fn goal_unset(&self) -> bool {
        matches!(self.goal, Goal::None)
    }
    /// Returns `None` when no goal is outstanding (none set, or already reached). Otherwise the
    /// subset of this limit's goal flags present in `flags` — non-empty means the goal commit
    /// lies above this commit, so it was reached through it.
    #[inline]
    pub(crate) fn goal_reachable(&self, flags: CommitFlags) -> Option<CommitFlags> {
        if self.goal_reached() {
            None
        } else {
            Some(flags.intersection(self.goal_flags()))
        }
    }

    /// The goal bits still sought, empty when none are.
    pub(crate) fn goal_flags(&self) -> CommitFlags {
        match self.goal {
            Goal::Seeking(bits) => bits,
            Goal::None | Goal::Reached => CommitFlags::empty(),
        }
    }

    /// Set our limit from `other`, but do not alter our goal.
    pub(crate) fn set_but_keep_goal(&mut self, other: Limit) {
        self.inner = other.inner;
    }
}

/// Lifecycle
impl Queue {
    pub(crate) fn new_with_limit(limit: Option<usize>) -> Self {
        Queue {
            inner: Default::default(),
            count: 0,
            max: limit,
            hard_limit_hit: false,
            limit_hint_hit: false,
            exhausted: false,
            sorted: false,
        }
    }
}

/// A queue to keep track of tips, which additionally counts how much was queued over time.
#[derive(Debug)]
pub struct Queue {
    pub inner: VecDeque<QueueItem>,
    /// The current number of queued items.
    count: usize,
    /// The maximum number of queuing operations, each representing one commit.
    max: Option<usize>,
    /// Whether the hard limit stopped further queuing at least once.
    hard_limit_hit: bool,
    /// Whether a tip's commit budget (`Options::commits_limit_hint`) ran out while it still
    /// had parents to walk. Unlike [`Self::hard_limit_hit`] this is not about queuing capacity:
    /// it says an extent ENDS BECAUSE THE BUDGET DID, not because history did.
    limit_hint_hit: bool,
    /// Whether no more items should be queued for reasons other than the hard limit.
    exhausted: bool,
    /// Whether new items must maintain `inner` in traversal order.
    sorted: bool,
}

/// Counted queuing
impl Queue {
    /// Sort the queue so younger commits are processed first: the traversal then moves back in
    /// time as one front instead of racing ahead in disjoint regions, which avoids the
    /// `propagate_flags_downward` bottleneck. The trade-off — a tip that misses its goal can
    /// overshoot toward the beginning of history — is still cheaper, especially when a
    /// commit-graph file exists (an easy 8x on lookups), which is exactly when this matters.
    pub fn sort(&mut self) {
        if !self.sorted {
            self.inner
                .make_contiguous()
                .sort_by(|a, b| a.info.gen_then_time.cmp(&b.info.gen_then_time));
            self.sorted = true;
        }
    }
    #[must_use]
    pub(crate) fn push_back_exhausted(&mut self, item: QueueItem) -> bool {
        if self.exhausted || self.record_hard_limit_if_exhausted() {
            return true;
        }
        self.push_back_even_if_exhausted(item)
    }

    pub(crate) fn push_back_even_if_exhausted(&mut self, item: QueueItem) -> bool {
        if self.sorted {
            self.insert_sorted(item);
        } else {
            self.inner.push_back(item);
        }
        self.is_exhausted_after_increment()
    }
    #[must_use]
    pub(crate) fn push_front_exhausted(&mut self, item: QueueItem) -> bool {
        if self.exhausted || self.record_hard_limit_if_exhausted() {
            return true;
        }
        if self.sorted {
            self.insert_sorted(item);
        } else {
            self.inner.push_front(item);
        }
        self.is_exhausted_after_increment()
    }

    fn insert_sorted(&mut self, item: QueueItem) {
        let index = self
            .inner
            .partition_point(|existing| existing.info.gen_then_time <= item.info.gen_then_time);
        self.inner.insert(index, item);
    }

    fn is_exhausted_after_increment(&mut self) -> bool {
        self.count += 1;
        self.exhausted || self.record_hard_limit_if_exhausted()
    }

    pub(crate) fn is_exhausted(&self) -> bool {
        self.exhausted || self.is_hard_limit_exhausted()
    }

    pub(crate) fn is_hard_limit_exhausted(&self) -> bool {
        self.max.is_some_and(|l| self.count >= l)
    }

    pub(crate) fn hard_limit_hit(&self) -> bool {
        self.hard_limit_hit
    }

    pub(crate) fn limit_hint_hit(&self) -> bool {
        self.limit_hint_hit
    }

    /// Record that a tip stopped with parents left unwalked because its budget ran out.
    /// Callers must have established that there IS history below — a parentless commit
    /// stops for its own reason and is not a cut.
    pub(crate) fn record_limit_hint_cut(&mut self) {
        self.limit_hint_hit = true;
    }

    fn record_hard_limit_if_exhausted(&mut self) -> bool {
        let hard_limit_exhausted = self.is_hard_limit_exhausted();
        self.hard_limit_hit |= hard_limit_exhausted;
        hard_limit_exhausted
    }

    /// Stop accepting new items while leaving already queued items to drain.
    pub(crate) fn exhaust(&mut self) {
        self.exhausted = true;
    }

    /// Add `goal` as additional goal to `id` or panic if `id` was not found.
    pub(crate) fn add_goal_to(&mut self, id: gix::ObjectId, goal: CommitFlags) {
        let limit = self
            .inner
            .iter_mut()
            .find_map(|item| (item.info.id == id).then_some(&mut item.limit))
            .unwrap_or_else(|| panic!("BUG: {id} is queued"));
        *limit = limit.additional_goal(goal);
    }
}

/// Plain access to the queued items.
impl Queue {
    pub fn pop_front(&mut self) -> Option<QueueItem> {
        self.inner.pop_front()
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut QueueItem> {
        self.inner.iter_mut()
    }
    pub fn iter(&self) -> impl Iterator<Item = &QueueItem> {
        self.inner.iter()
    }
}
/// A set of commits to keep track of in bitflags.
#[derive(Default)]
pub struct Goals(Vec<gix::ObjectId>);

impl Goals {
    /// Return the bitflag for `goal`, or `None` if we can't track any more goals.
    pub(crate) fn flag_for(&mut self, goal: gix::ObjectId) -> Option<CommitFlags> {
        let existing_flags = CommitFlags::all().iter().count();
        let max_goals = size_of::<CommitFlags>() * 8 - existing_flags;

        let goals = &mut self.0;
        let goal_index = match goals.iter().position(|existing| existing == &goal) {
            None => {
                let idx = goals.len();
                goals.push(goal);
                idx
            }
            Some(idx) => idx,
        };
        if goal_index >= max_goals {
            tracing::warn!("Goals limit reached, cannot track {goal}");
            None
        } else {
            Some(CommitFlags::from_bits_retain(
                1 << (existing_flags + goal_index),
            ))
        }
    }
}

/// The traversal queue's per-item payload: which stored commit queued this one (edges are
/// recorded at the PARENT's dequeue, so pruned items leave no edge), which seed-table entry it
/// belongs to (initial tips and remote tips; used for the initial sort, workspace-ownership
/// shuffles, and remote dedupe), and whether it starts a new segment.
#[derive(Debug, Clone, Copy)]
pub struct Instruction {
    pub queued_by: Option<gix::ObjectId>,
    pub seed: Option<usize>,
    /// The seed-table entry whose DESCENT this item belongs to, inherited down the walk
    /// (unlike [`Self::seed`], which only initial tips carry — it decides segment
    /// landing, while this decides goal retirement: a goal found by one of a seeker's
    /// items is found for all of them).
    pub seeker: Option<usize>,
}

/// One queued traversal step: the commit to visit, the flags it propagates, how it was
/// queued, and the budget it walks under.
#[derive(Debug, Clone)]
pub struct QueueItem {
    pub info: super::utils::TraverseInfo,
    pub flags: CommitFlags,
    pub instr: Instruction,
    pub limit: Limit,
}

#[cfg(test)]
mod tests {
    use super::Queue;

    #[test]
    fn explicit_exhaustion_does_not_count_as_hard_limit_hit() {
        let mut queue: Queue = Queue::new_with_limit(Some(1));

        queue.exhaust();

        assert!(queue.is_exhausted(), "explicit exhaustion stops queueing");
        assert!(
            !queue.record_hard_limit_if_exhausted(),
            "hard-limit state stays separate from explicit exhaustion"
        );
        assert!(
            !queue.hard_limit_hit(),
            "explicit exhaustion must not mark the hard limit as hit"
        );
    }

    #[test]
    fn hard_limit_exhaustion_records_hard_limit_hit() {
        let mut queue: Queue = Queue::new_with_limit(Some(0));

        assert!(
            queue.record_hard_limit_if_exhausted(),
            "a depleted hard limit stops queueing"
        );
        assert!(
            queue.hard_limit_hit(),
            "the queue records when the hard limit stopped queueing"
        );
    }
}
