//! Diagnostics over the [`CommitGraph`]: aggregate counts, invariant checks, and
//! renderings for debug tooling. These replace the segment-graph diagnostics as the
//! workspace's debug surface — the arena and layout ARE the graph.

use std::fmt::Write as _;

use crate::CommitGraph;

/// Aggregate counts over a [`CommitGraph`], for debug tooling.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CommitGraphStatistics {
    /// The number of live commits.
    pub commits: usize,
    /// The number of parent edges connected within the graph.
    pub edges_connected: usize,
    /// The number of parent edges pointing outside the graph — where this subgraph roots.
    pub edges_cut: usize,
    /// The number of references attached to commits.
    pub refs: usize,
    /// Commits without children — the graph's tips.
    pub commits_at_tip: usize,
    /// Commits whose parents are all outside the graph — where this subgraph roots.
    pub commits_at_bottom: usize,
    /// Commits flagged as reachable from the workspace tip.
    pub commits_in_workspace: usize,
    /// Commits flagged as reachable from the target.
    pub commits_integrated: usize,
    /// Commits never owned only by a remote.
    pub commits_not_in_remote: usize,
    /// The number of positioned references in the ref layout, when stored.
    pub layout_refs: Option<usize>,
    /// Whether traversal stopped early on the hard limit.
    pub hard_limit_hit: bool,
    /// The entrypoint commit, if set.
    pub entrypoint: Option<gix::ObjectId>,
}

impl CommitGraph {
    /// Aggregate counts for debug tooling.
    pub fn statistics(&self) -> CommitGraphStatistics {
        let mut out = CommitGraphStatistics {
            hard_limit_hit: self.hard_limit_hit,
            entrypoint: self.entrypoint(),
            layout_refs: self.layout.as_ref().map(|l| l.facts.len()),
            ..Default::default()
        };
        for id in self.commit_ids() {
            let Some(node) = self.node(id) else { continue };
            out.commits += 1;
            out.refs += node.refs.len();
            if self
                .index_of(id)
                .is_none_or(|idx| self.children_at(idx).is_empty())
            {
                out.commits_at_tip += 1;
            }
            let mut connected_parents = 0;
            for parent in self.all_parent_ids(id) {
                if self.node(parent).is_some() {
                    out.edges_connected += 1;
                    connected_parents += 1;
                } else {
                    out.edges_cut += 1;
                }
            }
            if connected_parents == 0 {
                out.commits_at_bottom += 1;
            }
            let flags = node.flags;
            if flags.contains(crate::CommitFlags::InWorkspace) {
                out.commits_in_workspace += 1;
            }
            if flags.contains(crate::CommitFlags::Integrated) {
                out.commits_integrated += 1;
            }
            if flags.contains(crate::CommitFlags::NotInRemote) {
                out.commits_not_in_remote += 1;
            }
        }
        out
    }

    /// Check index and edge coherence, returning every violation. An empty result means
    /// the arena's lookups and adjacency agree with its payloads.
    pub fn validation_errors(&self) -> Vec<anyhow::Error> {
        let mut out = Vec::new();
        for id in self.commit_ids() {
            let Some(node) = self.node(id) else {
                out.push(anyhow::anyhow!("commit {id} listed but not resolvable"));
                continue;
            };
            if node.id != id {
                out.push(anyhow::anyhow!(
                    "commit {id} resolves to node payload {}",
                    node.id
                ));
            }
            for parent in self.all_parent_ids(id) {
                if let Some((pgen, cgen)) = self.generation_of(parent).zip(self.generation_of(id))
                    && pgen >= cgen
                {
                    out.push(anyhow::anyhow!(
                        "parent {parent} of {id} has generation {pgen} >= child generation {cgen}"
                    ));
                }
            }
        }
        if let Some(ep) = self.entrypoint()
            && self.node(ep).is_none()
        {
            out.push(anyhow::anyhow!("entrypoint {ep} is not in the graph"));
        }
        out
    }

    /// One line per commit in generation order (newest first), rendered like the walk's
    /// commit debug output.
    pub fn debug_string(&self) -> String {
        let mut ids: Vec<_> = self
            .commit_ids()
            .filter_map(|id| self.generation_of(id).map(|generation| (generation, id)))
            .collect();
        ids.sort_unstable_by(|a, b| b.cmp(a));
        let mut out = String::new();
        let entrypoint = self.entrypoint();
        for (_, id) in ids {
            let Some(node) = self.node(id) else { continue };
            writeln!(
                &mut out,
                "{}",
                crate::debug::commit_debug_string_inner(
                    node,
                    entrypoint == Some(id),
                    None,
                    self.hard_limit_hit,
                    false,
                )
            )
            .ok();
        }
        out
    }

    /// The commit graph as a Graphviz `dot` document. Very large graphs are pruned from
    /// the bottom so `dot` won't hang.
    pub fn dot_graph(&self) -> String {
        const LIMIT: usize = 5000;
        let mut ids: Vec<_> = self
            .commit_ids()
            .filter_map(|id| self.generation_of(id).map(|generation| (generation, id)))
            .collect();
        ids.sort_unstable_by(|a, b| b.cmp(a));
        if ids.len() > LIMIT {
            tracing::warn!(
                "Pruning {} commits from the bottom to assure 'dot' won't hang",
                ids.len() - LIMIT
            );
            ids.truncate(LIMIT);
        }
        let included: std::collections::HashSet<_> = ids.iter().map(|(_, id)| *id).collect();
        let entrypoint = self.entrypoint();
        let mut dot = String::from("digraph {\n    node [shape=box, fontsize=10]\n");
        for &(_, id) in &ids {
            let Some(node) = self.node(id) else { continue };
            let label = crate::debug::commit_debug_string_inner(
                node,
                entrypoint == Some(id),
                None,
                self.hard_limit_hit,
                false,
            )
            .replace('"', "\\\"");
            writeln!(&mut dot, "    \"{id}\" [label=\"{label}\"]").ok();
            for parent in self.all_parent_ids(id) {
                if included.contains(&parent) {
                    writeln!(&mut dot, "    \"{id}\" -> \"{parent}\"").ok();
                }
            }
        }
        dot.push_str("}\n");
        dot
    }
}
