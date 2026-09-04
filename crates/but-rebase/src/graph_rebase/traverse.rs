//! Commit graph traversal helpers.

use crate::graph_rebase::commits::ParentEntry;
use std::collections::HashSet;

use anyhow::Result;
use but_core::RefMetadata;

use crate::graph_rebase::anchor::Anchor;
use crate::graph_rebase::commits::CommitIndex;
use crate::graph_rebase::mutate::commit_entry;
use crate::graph_rebase::{Editor, EditorIndex, EditorStore, positions};

/// How far `a` is ahead of and behind `b`, counted in commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AheadBehind {
    /// Commits reachable from `a` but not `b` (the rev-set `a ^b`).
    pub ahead: usize,
    /// Commits reachable from `b` but not `a` (the rev-set `b ^a`).
    pub behind: usize,
}

/// Count the `CommitSpec` steps (i.e. commits) among `steps`.
fn count_commits(store: &EditorStore, steps: impl Iterator<Item = EditorIndex>) -> usize {
    steps.filter(|ix| store.is_commit(*ix)).count()
}

struct Traversal<'store> {
    store: &'store EditorStore,
    excluded: HashSet<EditorIndex>,
    seen: HashSet<EditorIndex>,
    tips: Vec<EditorIndex>,
}

impl<'store> Traversal<'store> {
    fn new(store: &'store EditorStore, start: EditorIndex, excluded: HashSet<EditorIndex>) -> Self {
        Self {
            store,
            excluded,
            seen: HashSet::new(),
            tips: vec![start],
        }
    }
}

impl Iterator for Traversal<'_> {
    type Item = EditorIndex;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.tips.pop() {
            if self.excluded.contains(&n) || !self.seen.insert(n) {
                continue;
            }
            self.tips
                .extend(self.store.parents(n).into_iter().map(EditorIndex::from));
            return Some(n);
        }
        None
    }
}

/// Every step reachable from `start` following parent entries.
pub(crate) fn reachable_from(
    store: &EditorStore,
    start: EditorIndex,
) -> impl Iterator<Item = EditorIndex> + '_ {
    Traversal::new(store, start, HashSet::new())
}

/// The rev-set `start ^excluded`: steps reachable from `start` but not
/// `excluded`.
pub(crate) fn a_not_b(
    store: &EditorStore,
    start: EditorIndex,
    excluded: EditorIndex,
) -> impl Iterator<Item = EditorIndex> + '_ {
    all_until_optional_limit(store, start, Some(excluded))
}

/// All steps in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`.
pub(crate) fn all_until_optional_limit(
    store: &EditorStore,
    start: EditorIndex,
    limit: Option<EditorIndex>,
) -> impl Iterator<Item = EditorIndex> + '_ {
    let excluded = limit
        .map(|limit| reachable_from(store, limit).collect())
        .unwrap_or_default();
    Traversal::new(store, start, excluded)
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Returns all direct children of `target` together with their parent entry order.
    ///
    /// Children are represented as incoming parent entries into `target` in the editor graph.
    pub fn direct_children(&self, target: impl Into<Anchor>) -> Result<Vec<(EditorIndex, usize)>> {
        let target = self.resolve_anchor(target)?;
        // A reference's children are the parent entries entering through its position.
        if self.store.is_positioned(target) {
            return Ok(positions::entering(&self.store, target)
                .into_iter()
                .map(|ParentEntry { child, number }| (EditorIndex::from(child), number))
                .collect());
        }
        Ok(self
            .store
            .children_of(target)
            .iter()
            .map(|&ParentEntry { child, number }| (EditorIndex::from(child), number))
            .collect())
    }

    /// Returns all direct parents of `target` together with their parent entry order.
    ///
    /// Parents are represented as outgoing parent entries from `target` in the editor graph.
    pub fn direct_parents(&self, target: impl Into<Anchor>) -> Result<Vec<(EditorIndex, usize)>> {
        let target = self.resolve_anchor(target)?;
        // A reference's one downward link is its commit.
        if let Some(on) = self.store.positioned_on(target) {
            let commit = self.resolved_commit(on)?;
            return Ok(vec![(EditorIndex::from(commit), 0)]);
        }
        Ok(self
            .store
            .parents(target)
            .iter()
            .copied()
            .enumerate()
            .map(|(parent_number, parent)| (EditorIndex::from(parent), parent_number))
            .collect())
    }

    /// Everything a physical walk from `tip` passes through, `tip` included: the closure
    /// of [`Self::position_parents`]. Group members above a reference start stay outside —
    /// this answers "what lies on the paths below", not "which references decorate the
    /// reached commits" (id-equivalent reachability is a different, internal question).
    pub fn position_reachable(
        &self,
        tip: impl Into<Anchor>,
    ) -> Result<std::collections::HashSet<EditorIndex>> {
        let tip = self.resolve_anchor(tip)?;
        let mut seen = std::collections::HashSet::from([tip]);
        let mut tips = vec![tip];
        while let Some(tip) = tips.pop() {
            for parent in self.position_parents(tip)? {
                if seen.insert(parent) {
                    tips.push(parent);
                }
            }
        }
        Ok(seen)
    }

    /// The position-aware parent view of `target`: reference groups sit between a commit and
    /// its parent, so the view shows the branch names a walk would pass through. For a commit,
    /// each parent entry resolves to the top of the group it carries (falling back to the
    /// commit); for a reference, the next group member below, then the commit. Useful for
    /// renderers that interleave references with commits.
    pub fn position_parents(&self, target: impl Into<Anchor>) -> Result<Vec<EditorIndex>> {
        let target = self.resolve_anchor(target)?;
        if let Some(on) = self.store.positioned_on(target) {
            let commit = self.resolved_commit(on)?;
            // The physical member directly below is stored adjacency; the commit when at
            // the bottom of the stack.
            return Ok(vec![match self.store.below_of(target) {
                Some(below) => EditorIndex::from(below),
                None => EditorIndex::from(commit),
            }]);
        }
        Ok(self
            .store
            .parents(target)
            .iter()
            .copied()
            .enumerate()
            .map(|(parent_number, commit)| {
                let carried_top = self
                    .store
                    .positioned_refs()
                    .filter(|&entry| {
                        let Some(node) = target.as_commit() else {
                            return false;
                        };
                        positions::entering(&self.store, entry).contains(&ParentEntry {
                            child: node,
                            number: parent_number,
                        }) && self.store.resolve_to_commit(entry) == Some(commit)
                    })
                    .max_by_key(|&entry| (positions::ref_depth(&self.store, entry), entry));
                match carried_top {
                    Some(top) => EditorIndex::from(top),
                    None => EditorIndex::from(commit),
                }
            })
            .collect())
    }

    /// The position-aware child view of `target` — the inverse of [`Self::position_parents`].
    ///
    /// For a commit, its children are the bottom members of the groups sitting on it plus the
    /// plain parent entries into it; for a reference, the next group member above, else its parent entries.
    pub fn position_children(&self, target: impl Into<Anchor>) -> Result<Vec<EditorIndex>> {
        let target = self.resolve_anchor(target)?;
        if self.store.is_positioned(target) {
            let commit = self.store.resolve_to_commit(target);
            // Members sitting directly on it (group-mates and root siblings stacked above),
            // plus — when this is the top of its group — the parent entries that enter it.
            let mut out: Vec<EditorIndex> = self
                .store
                .positioned_refs()
                .filter(|&entry| {
                    EditorIndex::from(entry) != target
                        && self.store.below_of(entry).map(EditorIndex::from) == Some(target)
                })
                .map(EditorIndex::from)
                .collect();
            let target_entries = positions::entering(&self.store, target);
            let target_depth = positions::ref_depth(&self.store, target);
            let is_group_top = !self.store.positioned_refs().any(|entry| {
                EditorIndex::from(entry) != target
                    && positions::entering(&self.store, entry) == target_entries
                    && positions::ref_depth(&self.store, entry) > target_depth
                    && self.store.resolve_to_commit(entry) == commit
            });
            if is_group_top {
                out.extend(
                    target_entries
                        .iter()
                        .map(|entry| EditorIndex::from(entry.child)),
                );
            }
            out.sort();
            out.dedup();
            return Ok(out);
        }
        // Bottom members sit directly on the commit; other parent entries are plain.
        let mut out: Vec<EditorIndex> = self
            .store
            .positioned_refs()
            .filter(|&entry| {
                self.store.below_of(entry).is_none()
                    && self.store.resolve_to_commit(entry).map(EditorIndex::from) == Some(target)
            })
            .map(EditorIndex::from)
            .collect();
        for &ParentEntry {
            child,
            number: parent_number,
        } in self.store.children_of(target)
        {
            let carrying = self.store.positioned_refs().any(|entry| {
                positions::entering(&self.store, entry).contains(&ParentEntry {
                    child,
                    number: parent_number,
                }) && self.store.resolve_to_commit(entry).map(EditorIndex::from) == Some(target)
            });
            if !carrying {
                out.push(EditorIndex::from(child));
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    /// For a given step, find all the references that point to it.
    ///
    /// The reference indices are provided in no particular order.
    pub fn references_of(&self, target: impl Into<Anchor>) -> Result<Vec<EditorIndex>> {
        let target = self.resolve_anchor(target)?;

        let commit = commit_entry(target)?;
        Ok(
            crate::graph_rebase::positions::refs_resolving_to(&self.store, commit)
                .into_iter()
                .map(EditorIndex::from)
                .collect(),
        )
    }

    /// Every entry reachable from `start` following parent entries.
    pub(crate) fn reachable_from(
        &self,
        start: impl Into<Anchor>,
    ) -> Result<impl Iterator<Item = EditorIndex> + '_> {
        let start = self.resolve_anchor(start)?;
        Ok(self.reachable_ids(start).into_iter())
    }

    /// Reachability including references: commits and tombstones by parent entries (a reference start
    /// descends from its commit), plus every reference group the walk entered.
    fn reachable_ids(&self, start: EditorIndex) -> Vec<EditorIndex> {
        let seed = self.store.resolve_to_commit(start);
        let commits: std::collections::HashSet<CommitIndex> = match seed {
            Some(seed) => reachable_from(&self.store, seed.into())
                .filter_map(|entry| entry.as_commit())
                .collect(),
            None => Default::default(),
        };
        let mut all: Vec<EditorIndex> = commits.iter().copied().map(EditorIndex::from).collect();
        all.extend(
            crate::graph_rebase::positions::refs_reachable_with(&self.store, start, &commits)
                .into_iter()
                .map(EditorIndex::from),
        );
        all
    }

    /// How far `a` is ahead of and behind `b`, counted in commits.
    ///
    /// `ahead` is the number of `CommitSpec` steps in the rev-set `a ^b`; `behind` is
    /// the number in `b ^a`. Non-commit steps (references, placeholders) are not
    /// counted.
    ///
    /// This uses all-parents reachability — matching Git's fast-forward rule (a
    /// push fast-forwards iff the remote tip is reachable from the local tip) —
    /// rather than the first-parent-only branch-line reasoning of
    /// `but_workspace`'s `derive_push_status`.
    pub(crate) fn ahead_behind(
        &self,
        a: impl Into<Anchor>,
        b: impl Into<Anchor>,
    ) -> Result<AheadBehind> {
        let a = self.resolve_anchor(a)?;
        let b = self.resolve_anchor(b)?;
        // Only commits count, so reference endpoints stand for their commits.
        let a = self.store.resolve_to_commit(a);
        let b = self.store.resolve_to_commit(b);
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(AheadBehind {
                ahead: 0,
                behind: 0,
            });
        };
        Ok(AheadBehind {
            ahead: count_commits(&self.store, a_not_b(&self.store, a.into(), b.into())),
            behind: count_commits(&self.store, a_not_b(&self.store, b.into(), a.into())),
        })
    }

    /// All entries in `start ^limit`, or everything reachable from `start`
    /// when there is no `limit`.
    pub(crate) fn all_until_optional_limit(
        &self,
        start: impl Into<Anchor>,
        limit: Option<EditorIndex>,
    ) -> Result<impl Iterator<Item = EditorIndex> + '_> {
        let start = self.resolve_anchor(start)?;
        let excluded: std::collections::HashSet<EditorIndex> = limit
            .map(|limit| self.reachable_ids(limit).into_iter().collect())
            .unwrap_or_default();
        let result: Vec<EditorIndex> = self
            .reachable_ids(start)
            .into_iter()
            .filter(|id| !excluded.contains(id))
            .collect();
        Ok(result.into_iter())
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, str::FromStr as _};

    use super::{a_not_b, all_until_optional_limit, count_commits, reachable_from};
    use crate::graph_rebase::commits::CommitIndex;
    use crate::graph_rebase::{CommitSpec, EditorStore};

    fn commit(store: &mut EditorStore) -> CommitIndex {
        let id = gix::ObjectId::from_str("1000000000000000000000000000000000000000").unwrap();
        store.commits.add_commit(CommitSpec::new(id))
    }

    /// `a -> b -> base` and `c -> base` (parent entries point child -> parent).
    /// `a ^c` must drop `base` (shared with `c`) but keep `a`, `b`.
    #[test]
    fn a_not_b_excludes_shared_ancestry() {
        let mut g = EditorStore::default();
        let a = commit(&mut g);
        let b = commit(&mut g);
        let base = commit(&mut g);
        let c = commit(&mut g);
        g.commits.push_parent(a, b);
        g.commits.push_parent(b, base);
        g.commits.push_parent(c, base);

        assert_eq!(
            a_not_b(&g, a.into(), c.into()).collect::<HashSet<_>>(),
            [a.into(), b.into()].into_iter().collect()
        );
        assert_eq!(
            reachable_from(&g, a.into()).collect::<HashSet<_>>(),
            [a.into(), b.into(), base.into()].into_iter().collect()
        );
        assert_eq!(
            all_until_optional_limit(&g, a.into(), Some(c.into())).collect::<HashSet<_>>(),
            [a.into(), b.into()].into_iter().collect()
        );
    }

    /// `count_commits` over `a_not_b` ignores non-commit steps — these are exactly
    /// the two rev-sets `ahead_behind` maps to `ahead`/`behind`. `a -> none -> b
    /// -> base` and `c -> base`: `a ^c` reaches a `None` step plus commits `a`,
    /// `b`; only the two commits count. `c ^a` reaches `c`.
    #[test]
    fn count_picks_ignores_non_pick_steps() {
        let mut g = EditorStore::default();
        let a = commit(&mut g);
        let none = g.commits.add_tombstone();
        let b = commit(&mut g);
        let base = commit(&mut g);
        let c = commit(&mut g);
        g.commits.push_parent(a, none);
        g.commits.push_parent(none, b);
        g.commits.push_parent(b, base);
        g.commits.push_parent(c, base);

        assert_eq!(count_commits(&g, a_not_b(&g, a.into(), c.into())), 2);
        assert_eq!(count_commits(&g, a_not_b(&g, c.into(), a.into())), 1);
    }
}
