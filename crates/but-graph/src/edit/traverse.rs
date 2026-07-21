//! Graph traversal helpers over node vectors.

use std::collections::HashSet;

use anyhow::Result;

use crate::{
    Node, NodeIndex,
    edit::{MutableNodeGraph, ToSelector},
    node::is_commit_like,
};

/// How far `a` is ahead of and behind `b`, counted in commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AheadBehind {
    /// Commits reachable from `a` but not `b` (the rev-set `a ^b`).
    pub ahead: usize,
    /// Commits reachable from `b` but not `a` (the rev-set `b ^a`).
    pub behind: usize,
}

/// Count the commit-like nodes among `indexes`.
pub(crate) fn count_picks(nodes: &[Node], indexes: impl Iterator<Item = NodeIndex>) -> usize {
    indexes.filter(|ix| is_commit_like(nodes, *ix)).count()
}

struct Traversal<'graph> {
    nodes: &'graph [Node],
    excluded: HashSet<NodeIndex>,
    seen: HashSet<NodeIndex>,
    tips: Vec<NodeIndex>,
}

impl<'graph> Traversal<'graph> {
    fn new(nodes: &'graph [Node], start: NodeIndex, excluded: HashSet<NodeIndex>) -> Self {
        Self {
            nodes,
            excluded,
            seen: HashSet::new(),
            tips: vec![start],
        }
    }
}

impl Iterator for Traversal<'_> {
    type Item = NodeIndex;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(n) = self.tips.pop() {
            if self.excluded.contains(&n) || !self.seen.insert(n) {
                continue;
            }
            self.tips.extend(self.nodes[n].parents().iter().copied());
            return Some(n);
        }
        None
    }
}

/// Every node reachable from `start` following parent edges.
pub fn reachable_from(nodes: &[Node], start: NodeIndex) -> impl Iterator<Item = NodeIndex> + '_ {
    Traversal::new(nodes, start, HashSet::new())
}

/// The rev-set `start ^excluded`: nodes reachable from `start` but not
/// `excluded`.
pub fn a_not_b(
    nodes: &[Node],
    start: NodeIndex,
    excluded: NodeIndex,
) -> impl Iterator<Item = NodeIndex> + '_ {
    let excluded = reachable_from(nodes, excluded).collect();
    Traversal::new(nodes, start, excluded)
}

/// All nodes in `start ^limit`, or everything reachable from `start` when there
/// is no `limit`.
pub fn all_until_optional_limit(
    nodes: &[Node],
    start: NodeIndex,
    limit: Option<NodeIndex>,
) -> impl Iterator<Item = NodeIndex> + '_ {
    let excluded = limit
        .map(|limit| reachable_from(nodes, limit).collect())
        .unwrap_or_default();
    Traversal::new(nodes, start, excluded)
}

impl MutableNodeGraph {
    /// Every node index reachable from `start` following parent edges.
    pub fn reachable_from(
        &self,
        start: impl ToSelector,
    ) -> Result<impl Iterator<Item = NodeIndex> + '_> {
        let start = start.to_selector(self)?;
        Ok(reachable_from(&self.nodes, start))
    }

    /// The rev-set `start ^excluded`, yielding node indexes.
    pub fn a_not_b(
        &self,
        start: impl ToSelector,
        excluded: impl ToSelector,
    ) -> Result<impl Iterator<Item = NodeIndex> + '_> {
        let start = start.to_selector(self)?;
        let excluded = excluded.to_selector(self)?;
        Ok(a_not_b(&self.nodes, start, excluded))
    }

    /// How far `a` is ahead of and behind `b`, counted in commits.
    ///
    /// `ahead` is the number of commit-like nodes in the rev-set `a ^b`;
    /// `behind` is the number in `b ^a`. Non-commit nodes (references,
    /// placeholders) are not counted.
    ///
    /// This uses all-parents reachability — matching Git's fast-forward rule (a
    /// push fast-forwards iff the remote tip is reachable from the local tip) —
    /// rather than the first-parent-only branch-line reasoning of
    /// `but_workspace`'s `derive_push_status_from_graph`.
    pub fn ahead_behind(&self, a: impl ToSelector, b: impl ToSelector) -> Result<AheadBehind> {
        let a = a.to_selector(self)?;
        let b = b.to_selector(self)?;
        Ok(AheadBehind {
            ahead: count_picks(&self.nodes, a_not_b(&self.nodes, a, b)),
            behind: count_picks(&self.nodes, a_not_b(&self.nodes, b, a)),
        })
    }

    /// All node indexes in `start ^limit`, or everything reachable from `start`
    /// when there is no `limit`.
    pub fn all_until_optional_limit(
        &self,
        start: impl ToSelector,
        limit: Option<NodeIndex>,
    ) -> Result<impl Iterator<Item = NodeIndex> + '_> {
        let start = start.to_selector(self)?;
        Ok(all_until_optional_limit(&self.nodes, start, limit))
    }
}
