//! Step graph traversal helpers.

use std::collections::HashSet;

use anyhow::Result;
use but_core::RefMetadata;
use petgraph::{Direction, visit::EdgeRef as _};

use crate::graph_rebase::{Editor, Selector, StepGraph, StepGraphIndex, ToSelector};

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

    use super::{a_not_b, all_until_optional_limit, reachable_from};
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
}
