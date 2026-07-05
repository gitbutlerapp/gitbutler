//! Step graph traversal helpers.

use std::collections::HashSet;

use anyhow::Result;
use but_core::RefMetadata;

use crate::graph_rebase::{Editor, Selector, StepGraph, StepGraphIndex, ToSelector};

/// How far `a` is ahead of and behind `b`, counted in commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AheadBehind {
    /// Commits reachable from `a` but not `b` (the rev-set `a ^b`).
    pub ahead: usize,
    /// Commits reachable from `b` but not `a` (the rev-set `b ^a`).
    pub behind: usize,
}

/// Count the `Pick` steps (i.e. commits) among `steps`.
fn count_picks(graph: &StepGraph, steps: impl Iterator<Item = StepGraphIndex>) -> usize {
    steps.filter(|ix| graph.is_pick(*ix)).count()
}

struct Traversal<'graph> {
    graph: &'graph StepGraph,
    excluded: HashSet<StepGraphIndex>,
    seen: HashSet<StepGraphIndex>,
    tips: Vec<StepGraphIndex>,
}

impl<'graph> Traversal<'graph> {
    fn new(
        graph: &'graph StepGraph,
        start: StepGraphIndex,
        excluded: HashSet<StepGraphIndex>,
    ) -> Self {
        Self {
            graph,
            excluded,
            seen: HashSet::new(),
            tips: vec![start],
        }
    }
}

impl Iterator for Traversal<'_> {
    type Item = StepGraphIndex;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.tips.pop() {
            if self.excluded.contains(&n) || !self.seen.insert(n) {
                continue;
            }
            self.tips.extend(self.graph.parents(n));
            return Some(n);
        }
        None
    }
}

/// Every step reachable from `start` following parent edges (`Outgoing`).
pub(crate) fn reachable_from(
    graph: &StepGraph,
    start: StepGraphIndex,
) -> impl Iterator<Item = StepGraphIndex> + '_ {
    Traversal::new(graph, start, HashSet::new())
}

/// The rev-set `start ^excluded`: steps reachable from `start` but not
/// `excluded`.
pub(crate) fn a_not_b(
    graph: &StepGraph,
    start: StepGraphIndex,
    excluded: StepGraphIndex,
) -> impl Iterator<Item = StepGraphIndex> + '_ {
    let excluded = reachable_from(graph, excluded).collect();
    Traversal::new(graph, start, excluded)
}

/// All steps in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`.
pub(crate) fn all_until_optional_limit(
    graph: &StepGraph,
    start: StepGraphIndex,
    limit: Option<StepGraphIndex>,
) -> impl Iterator<Item = StepGraphIndex> + '_ {
    let excluded = limit
        .map(|limit| reachable_from(graph, limit).collect())
        .unwrap_or_default();
    Traversal::new(graph, start, excluded)
}

impl<M: RefMetadata> Editor<'_, '_, M> {
    /// Every selector reachable from `start` following parent edges.
    pub fn reachable_from(
        &self,
        start: impl ToSelector,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = start.to_selector(self)?.id;
        Ok(self
            .reachable_ids(start)
            .into_iter()
            .map(|id| self.new_selector(id)))
    }

    /// Node-era reachability over the positioned graph: picks and tombstones by edges (a
    /// reference start descends from its anchor), plus every reference chain the walk
    /// entered.
    fn reachable_ids(&self, start: StepGraphIndex) -> Vec<StepGraphIndex> {
        let seed = crate::graph_rebase::positions::resolve_to_pick(&self.graph, start);
        let picks: std::collections::HashSet<StepGraphIndex> = match seed {
            Some(seed) => reachable_from(&self.graph, seed).collect(),
            None => Default::default(),
        };
        let mut all: Vec<StepGraphIndex> = picks.iter().copied().collect();
        all.extend(crate::graph_rebase::positions::refs_reachable_with(
            &self.graph,
            start,
            &picks,
        ));
        all
    }

    /// The rev-set `start ^excluded`, yielding selectors.
    pub fn a_not_b(
        &self,
        start: impl ToSelector,
        excluded: impl ToSelector,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = start.to_selector(self)?.id;
        let excluded = excluded.to_selector(self)?.id;
        let excluded: std::collections::HashSet<StepGraphIndex> =
            self.reachable_ids(excluded).into_iter().collect();
        let result: Vec<StepGraphIndex> = self
            .reachable_ids(start)
            .into_iter()
            .filter(|id| !excluded.contains(id))
            .collect();
        Ok(result.into_iter().map(|id| self.new_selector(id)))
    }

    /// How far `a` is ahead of and behind `b`, counted in commits.
    ///
    /// `ahead` is the number of `Pick` steps in the rev-set `a ^b`; `behind` is
    /// the number in `b ^a`. Non-commit steps (references, placeholders) are not
    /// counted.
    ///
    /// This uses all-parents reachability — matching Git's fast-forward rule (a
    /// push fast-forwards iff the remote tip is reachable from the local tip) —
    /// rather than the first-parent-only branch-line reasoning of
    /// `but_workspace`'s `derive_push_status_from_graph`.
    pub fn ahead_behind(&self, a: impl ToSelector, b: impl ToSelector) -> Result<AheadBehind> {
        let a = a.to_selector(self)?.id;
        let b = b.to_selector(self)?.id;
        // Only picks count, so reference endpoints stand for their anchors.
        let a = crate::graph_rebase::positions::resolve_to_pick(&self.graph, a);
        let b = crate::graph_rebase::positions::resolve_to_pick(&self.graph, b);
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(AheadBehind {
                ahead: 0,
                behind: 0,
            });
        };
        Ok(AheadBehind {
            ahead: count_picks(&self.graph, a_not_b(&self.graph, a, b)),
            behind: count_picks(&self.graph, a_not_b(&self.graph, b, a)),
        })
    }

    /// All selectors in `start ^limit`, or everything reachable from `start`
    /// when there is no `limit`.
    pub fn all_until_optional_limit(
        &self,
        start: impl ToSelector,
        limit: Option<Selector>,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = start.to_selector(self)?.id;
        let limit = limit.map(|limit| limit.id);
        let excluded: std::collections::HashSet<StepGraphIndex> = limit
            .map(|limit| self.reachable_ids(limit).into_iter().collect())
            .unwrap_or_default();
        let result: Vec<StepGraphIndex> = self
            .reachable_ids(start)
            .into_iter()
            .filter(|id| !excluded.contains(id))
            .collect();
        Ok(result.into_iter().map(|id| self.new_selector(id)))
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, str::FromStr as _};

    use super::{a_not_b, all_until_optional_limit, count_picks, reachable_from};
    use crate::graph_rebase::{Step, StepGraph, StepGraphIndex};

    fn pick(graph: &mut StepGraph) -> StepGraphIndex {
        let id = gix::ObjectId::from_str("1000000000000000000000000000000000000000").unwrap();
        graph.add_node(Step::new_pick(id))
    }

    /// `a -> b -> base` and `c -> base` (edges point child -> parent).
    /// `a ^c` must drop `base` (shared with `c`) but keep `a`, `b`.
    #[test]
    fn a_not_b_excludes_shared_ancestry() {
        let mut g = StepGraph::default();
        let a = pick(&mut g);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.push_parent(a, b);
        g.push_parent(b, base);
        g.push_parent(c, base);

        assert_eq!(
            a_not_b(&g, a, c).collect::<HashSet<_>>(),
            [a, b].into_iter().collect()
        );
        assert_eq!(
            reachable_from(&g, a).collect::<HashSet<_>>(),
            [a, b, base].into_iter().collect()
        );
        assert_eq!(
            all_until_optional_limit(&g, a, Some(c)).collect::<HashSet<_>>(),
            [a, b].into_iter().collect()
        );
    }

    /// `count_picks` over `a_not_b` ignores non-pick steps — these are exactly
    /// the two rev-sets `ahead_behind` maps to `ahead`/`behind`. `a -> none -> b
    /// -> base` and `c -> base`: `a ^c` reaches a `None` step plus picks `a`,
    /// `b`; only the two picks count. `c ^a` reaches `c`.
    #[test]
    fn count_picks_ignores_non_pick_steps() {
        let mut g = StepGraph::default();
        let a = pick(&mut g);
        let none = g.add_node(Step::None);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.push_parent(a, none);
        g.push_parent(none, b);
        g.push_parent(b, base);
        g.push_parent(c, base);

        assert_eq!(count_picks(&g, a_not_b(&g, a, c)), 2);
        assert_eq!(count_picks(&g, a_not_b(&g, c, a)), 1);
    }
}
