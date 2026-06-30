//! Step graph traversal helpers.

use std::collections::HashSet;

use anyhow::Result;
use but_core::RefMetadata;
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{Editor, Selector, Step, StepGraph, StepGraphIndex, ToSelector};

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
    steps
        .filter(|ix| matches!(graph[*ix], Step::Pick(_)))
        .count()
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
            self.tips.extend(
                self.graph
                    .edges_directed(n, Direction::Outgoing)
                    .map(|e| e.target()),
            );
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
        let start = self
            .history
            .normalize_selector(start.to_selector(self)?)?
            .id;
        Ok(reachable_from(&self.graph, start).map(|id| self.new_selector(id)))
    }

    /// The rev-set `start ^excluded`, yielding selectors.
    pub fn a_not_b(
        &self,
        start: impl ToSelector,
        excluded: impl ToSelector,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = self
            .history
            .normalize_selector(start.to_selector(self)?)?
            .id;
        let excluded = self
            .history
            .normalize_selector(excluded.to_selector(self)?)?
            .id;
        Ok(a_not_b(&self.graph, start, excluded).map(|id| self.new_selector(id)))
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
        let a = self.history.normalize_selector(a.to_selector(self)?)?.id;
        let b = self.history.normalize_selector(b.to_selector(self)?)?.id;
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
        let start = self
            .history
            .normalize_selector(start.to_selector(self)?)?
            .id;
        let limit = limit
            .map(|limit| {
                self.history
                    .normalize_selector(limit)
                    .map(|selector| selector.id)
            })
            .transpose()?;
        Ok(all_until_optional_limit(&self.graph, start, limit).map(|id| self.new_selector(id)))
    }
}

#[cfg(test)]
mod test {
    use std::{collections::HashSet, str::FromStr as _};

    use super::{a_not_b, all_until_optional_limit, count_picks, reachable_from};
    use crate::graph_rebase::{Edge, Step, StepGraph, StepGraphIndex};

    fn pick(graph: &mut StepGraph) -> StepGraphIndex {
        let id = gix::ObjectId::from_str("1000000000000000000000000000000000000000").unwrap();
        graph.add_node(Step::new_pick(id))
    }

    /// `a -> b -> base` and `c -> base` (edges point child -> parent).
    /// `a ^c` must drop `base` (shared with `c`) but keep `a`, `b`.
    #[test]
    fn a_not_b_excludes_shared_ancestry() {
        let mut g = StepGraph::new();
        let a = pick(&mut g);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.add_edge(a, b, Edge { order: 0 });
        g.add_edge(b, base, Edge { order: 0 });
        g.add_edge(c, base, Edge { order: 0 });

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
        let mut g = StepGraph::new();
        let a = pick(&mut g);
        let none = g.add_node(Step::None);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.add_edge(a, none, Edge { order: 0 });
        g.add_edge(none, b, Edge { order: 0 });
        g.add_edge(b, base, Edge { order: 0 });
        g.add_edge(c, base, Edge { order: 0 });

        assert_eq!(count_picks(&g, a_not_b(&g, a, c)), 2);
        assert_eq!(count_picks(&g, a_not_b(&g, c, a)), 1);
    }
}
