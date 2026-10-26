//! Several items of one list picked together, the way Finder picks them: ⌘-click (ctrl-click
//! off macOS) picks an item or drops it, and ⇧-click and ⇧-arrows pick the run from where the
//! last other click left off, growing or shrinking it as they go.

use std::collections::HashSet;
use std::hash::Hash;
use std::ops::Deref;

/// What's picked in one list, in the order picked. Empty unless two or more are; the list's
/// cursor, which its owner keeps, is one of them then.
#[derive(Debug, Clone)]
pub struct Picks<T> {
    picked: Vec<T>,
    /// What was picked before the current run began, which the run adds to.
    kept: Vec<T>,
    /// Where a run starts: the item last ⌘-clicked, or the cursor when the run began.
    anchor: Option<T>,
}

impl<T> Default for Picks<T> {
    fn default() -> Self {
        Self {
            picked: Vec::new(),
            kept: Vec::new(),
            anchor: None,
        }
    }
}

impl<T> Deref for Picks<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        &self.picked
    }
}

impl<T: Clone + Eq + Hash> Picks<T> {
    /// Pick `item` along with the `cursor`, or drop it when it's picked. Returns where the
    /// cursor goes: to `item` when it's now picked, else to the last pick.
    pub fn toggle(&mut self, cursor: &T, item: &T) -> T {
        if self.picked.is_empty() {
            self.picked.push(cursor.clone());
        }
        match self.picked.iter().position(|picked| picked == item) {
            Some(ix) => {
                self.picked.remove(ix);
            }
            None => self.picked.push(item.clone()),
        }
        let next = match self.picked.contains(item) {
            true => item.clone(),
            false => self.picked.last().unwrap_or(cursor).clone(),
        };
        self.kept = self.picked.clone();
        self.anchor = Some(next.clone());
        self.settle();
        next
    }

    /// Pick the run from the anchor, or from the `cursor` when there's none, to `item` in
    /// `order`, the list as shown, along with what was picked before the run began. `None`,
    /// changing nothing, when `item` isn't in `order`.
    pub fn run(&mut self, order: &[T], cursor: &T, item: &T) -> Option<T> {
        let at = |wanted: &T| order.iter().position(|it| it == wanted);
        let anchor = (self.anchor.as_ref())
            .filter(|anchor| at(anchor).is_some())
            .unwrap_or(cursor)
            .clone();
        let (from, to) = (at(&anchor)?, at(item)?);
        let mut picked = self.kept.clone();
        let mut seen: HashSet<T> = picked.iter().cloned().collect();
        for it in order.get(from.min(to)..=from.max(to))? {
            if seen.insert(it.clone()) {
                picked.push(it.clone());
            }
        }
        self.picked = picked;
        self.anchor = Some(anchor);
        self.settle();
        Some(item.clone())
    }

    /// Pick everything in `order`.
    pub fn all(&mut self, order: &[T]) {
        self.picked = order.to_vec();
        self.kept = self.picked.clone();
        self.anchor = None;
        self.settle();
    }

    /// Keep each pick as `follow` has it now, as items are rewritten, and drop those it
    /// doesn't find.
    pub fn follow(&mut self, follow: impl Fn(&T) -> Option<T>) {
        self.picked = self.picked.iter().filter_map(&follow).collect();
        self.kept = self.kept.iter().filter_map(&follow).collect();
        self.anchor = self.anchor.as_ref().and_then(&follow);
        self.settle();
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// One pick alone is just the cursor.
    fn settle(&mut self) {
        if self.picked.len() < 2 {
            self.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Picks;

    const LIST: [u8; 6] = [0, 1, 2, 3, 4, 5];

    #[test]
    fn a_run_grows_and_shrinks_from_its_anchor_keeping_earlier_picks() {
        let mut picks = Picks::default();
        assert_eq!(
            picks.toggle(&0, &2),
            2,
            "the cursor goes to what was picked"
        );
        assert_eq!(&*picks, [0, 2]);
        picks.run(&LIST, &2, &4);
        assert_eq!(
            &*picks,
            [0, 2, 3, 4],
            "the run starts at the ⌘-clicked item"
        );
        picks.run(&LIST, &4, &3);
        assert_eq!(
            &*picks,
            [0, 2, 3],
            "stepping back shrinks the run, not what came before"
        );
        picks.run(&LIST, &3, &2);
        assert_eq!(
            &*picks,
            [0, 2],
            "back at the anchor only the earlier picks stay"
        );
    }

    #[test]
    fn one_pick_alone_is_none() {
        let mut picks = Picks::default();
        picks.toggle(&1, &3);
        assert_eq!(
            picks.toggle(&3, &3),
            1,
            "the cursor falls back to the last pick"
        );
        assert!(picks.is_empty(), "a single item left is just the cursor");
        picks.run(&LIST, &1, &1);
        assert!(picks.is_empty(), "a run of one picks nothing");
    }

    #[test]
    fn following_drops_what_is_gone() {
        let mut picks = Picks::default();
        picks.run(&LIST, &1, &3);
        picks.follow(|it| (*it != 2).then_some(it * 10));
        assert_eq!(
            &*picks,
            [10, 30],
            "picks follow their items, and the gone one goes"
        );
        picks.follow(|it| (*it == 10).then_some(*it));
        assert!(picks.is_empty(), "one left over is no pick");
    }
}
