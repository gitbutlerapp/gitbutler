use std::collections::VecDeque;

use crate::CommitFlags;

#[derive(Debug, Copy, Clone)]
pub struct Limit {
    inner: Option<usize>,
    /// The commit we want to see to be able to assume normal limits. Until then there is no limit.
    /// Each tracked commit is represented by bitflag, one for each goal, allowing commits to know
    /// if they can be reached by the tracked commit.
    /// The flag is empty if no goal is set.
    goal: CommitFlags,
}

/// Lifecycle and builders
impl Limit {
    pub fn new(value: Option<usize>) -> Self {
        Limit {
            inner: value,
            goal: CommitFlags::empty(),
        }
    }

    /// Keep queueing without limit until `goal` is seen in a commit that has **it ahead of itself**.
    /// Then stop searching for that goal.
    /// `goals` are used to keep track of existing bitflags.
    ///
    /// ### Note
    ///
    /// No goal will be set if we can't track more goals, effectively causing traversal to stop earlier,
    /// leaving potential isles in the graph.
    /// This can happen if we have to track a lot of remotes, but since these are queued later, they are also
    /// secondary and may just work for the typical remote.
    pub fn with_indirect_goal(mut self, goal: gix::ObjectId, goals: &mut Goals) -> Self {
        self.goal = goals.flag_for(goal).unwrap_or_default();
        self
    }

    /// Set two or more goals, by setting `goal` directly as previously obtained by [Goals::flag_for()].
    pub fn additional_goal(mut self, goal: CommitFlags) -> Self {
        self.goal |= goal;
        self
    }

    /// It's important to try to split the limit evenly so we don't create too
    /// much extra gas here. We do, however, make sure that we see each segment of a parent
    /// with one commit so we know exactly where it stops.
    /// The problem with this is that we never get back the split limit when segments re-unite,
    /// so effectively we loose gas here.
    pub fn per_parent(&self, num_parents: usize) -> Self {
        Limit {
            inner: self
                .inner
                .map(|l| if l == 0 { 0 } else { (l / num_parents).max(1) }),
            goal: self.goal,
        }
    }

    /// Assure this limit won't perform any traversal after reaching its goals.
    pub fn without_allowance(mut self) -> Self {
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
    pub fn is_exhausted_or_decrement(&mut self, flags: CommitFlags, next: &Queue) -> bool {
        // Keep going if the goal wasn't seen yet, unlimited gas.
        if let Some(maybe_goal) = self.goal_reachable(flags)
            && (maybe_goal.is_empty() || self.set_single_goal_reached_keep_searching(maybe_goal))
        {
            return false;
        }
        // Do not let *any* non-goal tip consume gas as long as there is still anything with a goal in the queue
        // that need to meet their local branches.
        // This is effectively only affecting the entrypoint tips, which isn't setup with a goal.
        // TODO(perf): could we remember that we are a tip and look for our specific counterpart by matching the goal?
        //             That way unrelated tips wouldn't cause us to keep traversing.
        if self.goal_unset() && next.iter().any(|(_, _, _, limit)| !limit.goal_reached()) {
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
    /// Out-of-band way to use commit-flags differently - they never set the earlier flags, so we
    /// can use them.
    /// Return `true` if all goals are reached now.
    pub fn set_single_goal_reached_keep_searching(&mut self, goal: CommitFlags) -> bool {
        self.goal.remove(goal);
        if self.goal.is_empty() {
            self.goal.insert(CommitFlags::Integrated);
            false
        } else {
            true
        }
    }

    /// If `other` has a higher limit as ourselves, apply the higher limit to us.
    /// Nothing else is affected.
    pub fn adjust_limit_if_bigger(&mut self, other: Limit) {
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

    pub fn goal_reached(&self) -> bool {
        self.goal_unset() || self.goal.contains(CommitFlags::Integrated)
    }

    fn goal_unset(&self) -> bool {
        self.goal.is_empty()
    }
    /// Return `None` if this limit has no goal set, otherwise return `!CommitFlags::empty()` if `flags` contains it,
    /// meaning it was reached through the commit the flags belong to.
    /// This is useful to determine if a commit that is ahead was seen during traversal.
    #[inline]
    pub fn goal_reachable(&self, flags: CommitFlags) -> Option<CommitFlags> {
        if self.goal_reached() {
            None
        } else {
            Some(flags.intersection(self.goal_flags()))
        }
    }

    /// Return the goal flags, which may be empty.
    pub fn goal_flags(&self) -> CommitFlags {
        // Should only be one, at a time
        let all_goals = self.goal.bits() & !CommitFlags::all().bits();
        CommitFlags::from_bits_retain(all_goals)
    }

    /// Set our limit from `other`, but do not alter our goal.
    pub fn set_but_keep_goal(&mut self, other: Limit) {
        self.inner = other.inner;
    }
}

/// Lifecycle
impl Queue {
    pub fn new_with_limit(limit: Option<usize>) -> Self {
        Queue {
            inner: Default::default(),
            count: 0,
            max: limit,
            hard_limit_hit: false,
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
    /// Whether no more items should be queued for reasons other than the hard limit.
    exhausted: bool,
    /// Whether new items must maintain `inner` in traversal order.
    sorted: bool,
}

/// Counted queuing
impl Queue {
    /// Sort the queue items so that young commits come first. This way, the traversal goes
    /// back in time continuously, which helps to avoid having too many graph traversals
    /// in disjoint regions happen at the same time.
    /// Note that traversals sorted like this are much less prone to run into the `propagate_flags_downward`
    /// bottleneck. While they may (depending on the graph) create their own bottleneck if they end up missing
    /// their point of interest and overshoot to the beginning of time, this is still preferable over the flag
    /// propagation bottleneck. This is true Particularly if a commit-graph exists which typically is the case
    /// where this starts to matter, as it speeds up traversal by factor 8 easily.
    pub fn sort(&mut self) {
        if !self.sorted {
            self.inner
                .make_contiguous()
                .sort_by(|a, b| a.0.gen_then_time.cmp(&b.0.gen_then_time));
            self.sorted = true;
        }
    }
    #[must_use]
    pub fn push_back_exhausted(&mut self, item: QueueItem) -> bool {
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
    pub fn push_front_exhausted(&mut self, item: QueueItem) -> bool {
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
            .partition_point(|existing| existing.0.gen_then_time <= item.0.gen_then_time);
        self.inner.insert(index, item);
    }

    fn is_exhausted_after_increment(&mut self) -> bool {
        self.count += 1;
        self.exhausted || self.record_hard_limit_if_exhausted()
    }

    pub fn is_exhausted(&self) -> bool {
        self.exhausted || self.is_hard_limit_exhausted()
    }

    pub(crate) fn is_hard_limit_exhausted(&self) -> bool {
        self.max.is_some_and(|l| self.count >= l)
    }

    pub(crate) fn hard_limit_hit(&self) -> bool {
        self.hard_limit_hit
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
    pub fn add_goal_to(&mut self, id: gix::ObjectId, goal: CommitFlags) {
        let limit = self
            .inner
            .iter_mut()
            .find_map(|(info, _, _, limit)| (info.id == id).then_some(limit))
            .unwrap_or_else(|| panic!("BUG: {id} is queued"));
        *limit = limit.additional_goal(goal);
    }
}

/// Various other - good to know what we need though.
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
    pub fn flag_for(&mut self, goal: gix::ObjectId) -> Option<CommitFlags> {
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

/// What to do with a popped commit in the commit-first traversal.
#[derive(Debug, Copy, Clone)]
pub enum Step {
    /// The seeded tip of segment `into` (an initial tip or a discovered remote-tracking branch).
    SeedTip { into: usize },
    /// The first-parent continuation of `child`'s run.
    Continue { child: gix::ObjectId },
    /// The start of a new run, as parent number `parent_order` of the merge commit `child`.
    NewRunBelow {
        child: gix::ObjectId,
        parent_order: u32,
    },
}

impl Step {
    /// The segment this step seeds, if it is a [`Step::SeedTip`].
    pub fn seed_segment(&self) -> Option<usize> {
        match self {
            Step::SeedTip { into } => Some(*into),
            Step::Continue { .. } | Step::NewRunBelow { .. } => None,
        }
    }
}

pub type QueueItem = (super::walk::TraverseInfo, CommitFlags, Step, Limit);

#[cfg(test)]
mod tests {
    use super::Queue;

    #[test]
    fn explicit_exhaustion_does_not_count_as_hard_limit_hit() {
        let mut queue = Queue::new_with_limit(Some(1));

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
        let mut queue = Queue::new_with_limit(Some(0));

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
