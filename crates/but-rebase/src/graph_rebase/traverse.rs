//! Commit graph traversal helpers.

use std::collections::HashSet;

use anyhow::Result;
use but_core::RefMetadata;

use crate::graph_rebase::graph_editor::PickIndex;
use crate::graph_rebase::mutate::node_entry;
use crate::graph_rebase::{Editor, EditorIndex, GraphEditor, Selector, ToSelector, positions};

/// How far `a` is ahead of and behind `b`, counted in commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AheadBehind {
    /// Commits reachable from `a` but not `b` (the rev-set `a ^b`).
    pub ahead: usize,
    /// Commits reachable from `b` but not `a` (the rev-set `b ^a`).
    pub behind: usize,
}

/// Count the `Pick` steps (i.e. commits) among `steps`.
fn count_picks(graph: &GraphEditor, steps: impl Iterator<Item = EditorIndex>) -> usize {
    steps.filter(|ix| graph.is_pick(*ix)).count()
}

struct Traversal<'graph> {
    graph: &'graph GraphEditor,
    excluded: HashSet<EditorIndex>,
    seen: HashSet<EditorIndex>,
    tips: Vec<EditorIndex>,
}

impl<'graph> Traversal<'graph> {
    fn new(graph: &'graph GraphEditor, start: EditorIndex, excluded: HashSet<EditorIndex>) -> Self {
        Self {
            graph,
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
                .extend(self.graph.parents(n).into_iter().map(EditorIndex::from));
            return Some(n);
        }
        None
    }
}

/// Every step reachable from `start` following parent edges (`Outgoing`).
pub(crate) fn reachable_from(
    graph: &GraphEditor,
    start: EditorIndex,
) -> impl Iterator<Item = EditorIndex> + '_ {
    Traversal::new(graph, start, HashSet::new())
}

/// The rev-set `start ^excluded`: steps reachable from `start` but not
/// `excluded`.
pub(crate) fn a_not_b(
    graph: &GraphEditor,
    start: EditorIndex,
    excluded: EditorIndex,
) -> impl Iterator<Item = EditorIndex> + '_ {
    all_until_optional_limit(graph, start, Some(excluded))
}

/// All steps in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`.
pub(crate) fn all_until_optional_limit(
    graph: &GraphEditor,
    start: EditorIndex,
    limit: Option<EditorIndex>,
) -> impl Iterator<Item = EditorIndex> + '_ {
    let excluded = limit
        .map(|limit| reachable_from(graph, limit).collect())
        .unwrap_or_default();
    Traversal::new(graph, start, excluded)
}

impl<M: RefMetadata> Editor<'_, M> {
    /// Returns all direct children of `target` together with their edge order.
    ///
    /// Children are represented as incoming edges into `target` in the editor graph.
    pub fn direct_children(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = target.to_selector(self)?;
        // A reference's children are the edges entering through its position.
        if self.graph.is_positioned(target.id) {
            return Ok(positions::edges_through(&self.graph, target.id)
                .into_iter()
                .map(|(child, parent_number)| (self.new_selector(child), parent_number))
                .collect());
        }
        Ok(self
            .graph
            .incoming_edges(target.id)
            .iter()
            .map(|&(child, parent_number)| (self.new_selector(child), parent_number))
            .collect())
    }

    /// Returns all direct parents of `target` together with their edge order.
    ///
    /// Parents are represented as outgoing edges from `target` in the editor graph.
    pub fn direct_parents(&self, target: impl ToSelector) -> Result<Vec<(Selector, usize)>> {
        let target = target.to_selector(self)?;
        // A reference's one downward link is its pick.
        if let Some(on) = self.graph.positioned_on(target.id) {
            let pick = self.resolved_pick(on)?;
            return Ok(vec![(self.new_selector(pick), 0)]);
        }
        Ok(self
            .graph
            .parents(target.id)
            .iter()
            .copied()
            .enumerate()
            .map(|(parent_number, parent)| (self.new_selector(parent), parent_number))
            .collect())
    }

    /// The interposed parent view of `target`: reference groups sit between a commit and
    /// its parent, so the view shows the branch names a walk would pass through. For a pick,
    /// each parent edge resolves to the top of the group it carries (falling back to the
    /// pick); for a reference, the next group member below, then the pick. Useful for
    /// renderers that interleave references with commits.
    pub fn position_parents(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;
        if let Some(on) = self.graph.positioned_on(target.id) {
            let pick = self.resolved_pick(on)?;
            // The physical member directly below is stored adjacency; the pick when at
            // the bottom of the stack.
            return Ok(vec![match self.graph.below_of(target.id) {
                Some(below) => self.new_selector(below),
                None => self.new_selector(pick),
            }]);
        }
        Ok(self
            .graph
            .parents(target.id)
            .iter()
            .copied()
            .enumerate()
            .map(|(parent_number, pick)| {
                let carried_top = self
                    .graph
                    .positioned_refs()
                    .filter(|&entry| {
                        let Some(node) = target.id.as_pick() else {
                            return false;
                        };
                        positions::edges_through(&self.graph, entry)
                            .contains(&(node, parent_number))
                            && positions::resolve_to_pick(&self.graph, entry) == Some(pick)
                    })
                    .max_by_key(|&entry| (positions::ref_depth(&self.graph, entry), entry));
                match carried_top {
                    Some(top) => self.new_selector(top),
                    None => self.new_selector(pick),
                }
            })
            .collect())
    }

    /// The interposed child view of `target` — the inverse of [`Self::position_parents`].
    ///
    /// For a pick, its children are the bottom members of the groups sitting on it plus the
    /// plain edges into it; for a reference, the next group member above, else its edges.
    pub fn position_children(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;
        if self.graph.is_positioned(target.id) {
            let pick = positions::resolve_to_pick(&self.graph, target.id);
            // Members sitting directly on it (group-mates and root siblings stacked above),
            // plus — when this is the top of its group — the edges that enter it.
            let mut out: Vec<Selector> = self
                .graph
                .positioned_refs()
                .filter(|&entry| {
                    EditorIndex::from(entry) != target.id
                        && self.graph.below_of(entry).map(EditorIndex::from) == Some(target.id)
                })
                .map(|entry| self.new_selector(entry))
                .collect();
            let target_edges = positions::edges_through(&self.graph, target.id);
            let target_depth = positions::ref_depth(&self.graph, target.id);
            let is_group_top = !self.graph.positioned_refs().any(|entry| {
                EditorIndex::from(entry) != target.id
                    && positions::edges_through(&self.graph, entry) == target_edges
                    && positions::ref_depth(&self.graph, entry) > target_depth
                    && positions::resolve_to_pick(&self.graph, entry) == pick
            });
            if is_group_top {
                out.extend(
                    target_edges
                        .iter()
                        .map(|(edge, _)| self.new_selector(*edge)),
                );
            }
            out.sort_by_key(|s| s.id);
            out.dedup_by_key(|s| s.id);
            return Ok(out);
        }
        // Bottom members sit directly on the pick; other edges are plain.
        let mut out: Vec<Selector> = self
            .graph
            .positioned_refs()
            .filter(|&entry| {
                self.graph.below_of(entry).is_none()
                    && positions::resolve_to_pick(&self.graph, entry).map(EditorIndex::from)
                        == Some(target.id)
            })
            .map(|entry| self.new_selector(entry))
            .collect();
        for &(child, parent_number) in self.graph.incoming_edges(target.id) {
            let carrying = self.graph.positioned_refs().any(|entry| {
                positions::edges_through(&self.graph, entry).contains(&(child, parent_number))
                    && positions::resolve_to_pick(&self.graph, entry).map(EditorIndex::from)
                        == Some(target.id)
            });
            if !carrying {
                out.push(self.new_selector(child));
            }
        }
        out.sort_by_key(|s| s.id);
        out.dedup_by_key(|s| s.id);
        Ok(out)
    }

    /// For a given step, find all the references that point to it.
    ///
    /// The reference selectors are provided in no particular order.
    pub fn step_references(&self, target: impl ToSelector) -> Result<Vec<Selector>> {
        let target = target.to_selector(self)?;

        let node = node_entry(target.id)?;
        Ok(
            crate::graph_rebase::positions::refs_resolving_to(&self.graph, node)
                .into_iter()
                .map(|entry| self.new_selector(entry))
                .collect(),
        )
    }

    /// Every selector reachable from `start` following parent edges.
    pub(crate) fn reachable_from(
        &self,
        start: impl ToSelector,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = start.to_selector(self)?.id;
        Ok(self
            .reachable_ids(start)
            .into_iter()
            .map(|id| self.new_selector(id)))
    }

    /// Reachability including references: picks and tombstones by edges (a reference start
    /// descends from its pick), plus every reference group the walk entered.
    fn reachable_ids(&self, start: EditorIndex) -> Vec<EditorIndex> {
        let seed = crate::graph_rebase::positions::resolve_to_pick(&self.graph, start);
        let picks: std::collections::HashSet<PickIndex> = match seed {
            Some(seed) => reachable_from(&self.graph, seed.into())
                .filter_map(|entry| entry.as_pick())
                .collect(),
            None => Default::default(),
        };
        let mut all: Vec<EditorIndex> = picks.iter().copied().map(EditorIndex::from).collect();
        all.extend(
            crate::graph_rebase::positions::refs_reachable_with(&self.graph, start, &picks)
                .into_iter()
                .map(EditorIndex::from),
        );
        all
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
    pub(crate) fn ahead_behind(
        &self,
        a: impl ToSelector,
        b: impl ToSelector,
    ) -> Result<AheadBehind> {
        let a = a.to_selector(self)?.id;
        let b = b.to_selector(self)?.id;
        // Only picks count, so reference endpoints stand for their picks.
        let a = crate::graph_rebase::positions::resolve_to_pick(&self.graph, a);
        let b = crate::graph_rebase::positions::resolve_to_pick(&self.graph, b);
        let (Some(a), Some(b)) = (a, b) else {
            return Ok(AheadBehind {
                ahead: 0,
                behind: 0,
            });
        };
        Ok(AheadBehind {
            ahead: count_picks(&self.graph, a_not_b(&self.graph, a.into(), b.into())),
            behind: count_picks(&self.graph, a_not_b(&self.graph, b.into(), a.into())),
        })
    }

    /// All selectors in `start ^limit`, or everything reachable from `start`
    /// when there is no `limit`.
    pub(crate) fn all_until_optional_limit(
        &self,
        start: impl ToSelector,
        limit: Option<Selector>,
    ) -> Result<impl Iterator<Item = Selector> + '_> {
        let start = start.to_selector(self)?.id;
        let limit = limit.map(|limit| limit.id);
        let excluded: std::collections::HashSet<EditorIndex> = limit
            .map(|limit| self.reachable_ids(limit).into_iter().collect())
            .unwrap_or_default();
        let result: Vec<EditorIndex> = self
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
    use crate::graph_rebase::graph_editor::PickIndex;
    use crate::graph_rebase::{GraphEditor, Pick};

    fn pick(graph: &mut GraphEditor) -> PickIndex {
        let id = gix::ObjectId::from_str("1000000000000000000000000000000000000000").unwrap();
        graph.add_node(Some(Pick::new_pick(id)))
    }

    /// `a -> b -> base` and `c -> base` (edges point child -> parent).
    /// `a ^c` must drop `base` (shared with `c`) but keep `a`, `b`.
    #[test]
    fn a_not_b_excludes_shared_ancestry() {
        let mut g = GraphEditor::default();
        let a = pick(&mut g);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.push_parent(a, b);
        g.push_parent(b, base);
        g.push_parent(c, base);

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

    /// `count_picks` over `a_not_b` ignores non-pick steps — these are exactly
    /// the two rev-sets `ahead_behind` maps to `ahead`/`behind`. `a -> none -> b
    /// -> base` and `c -> base`: `a ^c` reaches a `None` step plus picks `a`,
    /// `b`; only the two picks count. `c ^a` reaches `c`.
    #[test]
    fn count_picks_ignores_non_pick_steps() {
        let mut g = GraphEditor::default();
        let a = pick(&mut g);
        let none = g.add_node(None);
        let b = pick(&mut g);
        let base = pick(&mut g);
        let c = pick(&mut g);
        g.push_parent(a, none);
        g.push_parent(none, b);
        g.push_parent(b, base);
        g.push_parent(c, base);

        assert_eq!(count_picks(&g, a_not_b(&g, a.into(), c.into())), 2);
        assert_eq!(count_picks(&g, a_not_b(&g, c.into(), a.into())), 1);
    }
}
